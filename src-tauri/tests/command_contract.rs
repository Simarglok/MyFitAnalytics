use mfa_config::{AppSettings, SettingsStore};
use std::fs;
use tempfile::TempDir;

fn state() -> (myfitanalytics::AppState, TempDir) {
    let root = TempDir::new().unwrap();
    let config_root = root.path().join("config");
    let module_root = root.path().join("modules");
    let core = root.path().join("core");
    fs::create_dir_all(&config_root).unwrap();
    fs::create_dir_all(module_root.join("bundled-source/1.0.0/embedded")).unwrap();
    fs::create_dir_all(&core).unwrap();

    let settings = AppSettings {
        locale: "en-GB".to_owned(),
        ..AppSettings::default()
    };
    SettingsStore::new(config_root.join("settings.json"))
        .save(&settings)
        .unwrap();

    fs::write(
        module_root.join("bundled-source/1.0.0/embedded/module.json"),
        br#"{
          "module_type":"source",
          "module_id":"bundled-source",
          "module_version":"1.0.0",
          "package_format_version":"1.0.0",
          "source_api_version":"1.0.0",
          "mapping_version":"1.0.0",
          "compatible_app_versions":[">=0.1.0"],
          "provided_capabilities":["body.weight"],
          "accepted_file_patterns":["*.json"],
          "entrypoint_hash":"sha256:embedded",
          "localization_namespace":"source.bundled"
        }"#,
    )
    .unwrap();
    fs::write(
        core.join("messages.json"),
        br#"{"locale":"en","namespace":"core","messages":{"app.title":"MyFitAnalytics"}}"#,
    )
    .unwrap();

    let state = myfitanalytics::AppState::from_roots(&config_root, &module_root, &core).unwrap();
    (state, root)
}

#[tokio::test]
async fn bootstrap_command_returns_typed_safe_state() {
    let (state, _root) = state();
    let bootstrap = myfitanalytics::commands::get_bootstrap_state_inner(&state)
        .await
        .unwrap();
    assert_eq!(bootstrap.product_name, "MyFitAnalytics");
    assert_eq!(bootstrap.locale, "en-GB");
    assert!(bootstrap.active_providers.is_empty());
    assert_eq!(bootstrap.modules.len(), 1);
    assert_eq!(bootstrap.modules[0].id, "bundled-source");
    let serialized = serde_json::to_string(&bootstrap).unwrap();
    assert!(serialized.contains("bundled-source"));
    assert!(!serialized.contains("/tmp"));
    assert!(!serialized.contains("internal"));
}

#[tokio::test]
async fn list_modules_exposes_metadata_without_paths_or_internal_errors() {
    let (state, _root) = state();
    let modules = myfitanalytics::commands::list_modules_inner(&state)
        .await
        .unwrap();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].module_type, "source");
    let serialized = serde_json::to_string(&modules).unwrap();
    assert!(!serialized.contains("root"));
    assert!(!serialized.contains("sha256"));
}

#[tokio::test]
async fn embedded_core_catalog_does_not_require_a_repository_path() {
    let root = TempDir::new().unwrap();
    let config_root = root.path().join("config");
    let module_root = root.path().join("modules");
    fs::create_dir_all(&config_root).unwrap();
    SettingsStore::new(config_root.join("settings.json"))
        .save(&AppSettings::default())
        .unwrap();

    let state = myfitanalytics::AppState::from_roots_with_core_catalog(
        &config_root,
        &module_root,
        br#"{"locale":"en","namespace":"core","messages":{"app.title":"MyFitAnalytics"}}"#,
    )
    .unwrap();
    let bootstrap = myfitanalytics::commands::get_bootstrap_state_inner(&state)
        .await
        .unwrap();
    assert_eq!(bootstrap.product_name, "MyFitAnalytics");
    assert!(bootstrap.modules.is_empty());
}
