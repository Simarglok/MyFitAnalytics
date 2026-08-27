use mfa_config::{AppSettings, SettingsStore};
use mfa_module_host::PackageInstaller;
use myfitanalytics::commands::select_provider_inner;
use serde_json::Value;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn state_with_bundled_sources() -> (myfitanalytics::AppState, TempDir) {
    let root = TempDir::new().unwrap();
    let config_root = root.path().join("config");
    let module_root = root.path().join("modules");
    fs::create_dir_all(&config_root).unwrap();
    SettingsStore::new(config_root.join("settings.json"))
        .save(&AppSettings::default())
        .unwrap();
    let package_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist/modules");
    let installer = PackageInstaller::new(&module_root);
    for name in ["hevy.mfasource", "mynetdiary.mfasource"] {
        installer.install(&package_root.join(name)).unwrap();
    }
    let state = myfitanalytics::AppState::from_roots_with_core_catalog(
        &config_root,
        &module_root,
        br#"{"locale":"en","namespace":"core","messages":{"app.title":"MyFitAnalytics"}}"#,
    )
    .unwrap();
    (state, root)
}

#[tokio::test]
async fn provider_selection_rejects_unoffered_capabilities_and_returns_safe_state() {
    let (state, _root) = state_with_bundled_sources();
    let error = select_provider_inner("body.weight".to_owned(), "mynetdiary".to_owned(), &state)
        .await
        .unwrap_err();
    assert_eq!(error.code, "capability_not_offered");

    let selected = select_provider_inner("body.weight".to_owned(), "hevy".to_owned(), &state)
        .await
        .unwrap();
    let json: Value = serde_json::to_value(selected).unwrap();
    assert_eq!(json["activeProviders"]["body.weight"], "hevy");
    assert!(!json.to_string().contains("package_hash"));
}
