use mfa_config::{AppSettings, SettingsStore};
use serde_json::json;
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

fn state() -> (myfitanalytics::AppState, TempDir, std::path::PathBuf) {
    let root = TempDir::new().unwrap();
    let config_root = root.path().join("config");
    let module_root = root.path().join("modules");
    fs::create_dir_all(&config_root).unwrap();
    fs::create_dir_all(module_root.join("bundled-source/1.0.0/embedded")).unwrap();
    SettingsStore::new(config_root.join("settings.json"))
        .save(&AppSettings::default())
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
    let state = myfitanalytics::AppState::from_roots_with_core_catalog(
        &config_root,
        &module_root,
        br#"{"locale":"en","namespace":"core","messages":{"app.title":"MyFitAnalytics"}}"#,
    )
    .unwrap();
    (state, root, config_root)
}

#[tokio::test]
async fn workspace_command_persists_settings_and_exposes_non_icloud_paths() {
    let (state, root, _config_root) = state();
    let workspace_root = root.path().join("workspace");
    let view = myfitanalytics::commands::set_workspace_root_inner(
        &state,
        workspace_root.to_string_lossy().into_owned(),
    )
    .await
    .unwrap();
    assert_eq!(view.workspace_root, workspace_root.to_string_lossy());
    assert!(view.app_data_root.contains("app-data"));
    assert!(view.database_path.ends_with("myfitanalytics.duckdb"));
    assert!(view.recovery_path.ends_with("recovery"));
    assert!(
        view.source_paths
            .iter()
            .any(|path| path.module_id == "bundled-source")
    );
    assert!(view.source_paths[0].inbox_path.contains("inbox"));
    assert!(view.source_paths[0].archive_path.contains("archive"));
    assert!(!view.workspace_root.contains("iCloud"));
    state.shutdown_storage().await.unwrap();
}

#[tokio::test]
async fn refresh_status_and_quality_commands_return_safe_typed_dtos() {
    let (state, root, _config_root) = state();
    let workspace_root = root.path().join("workspace");
    myfitanalytics::commands::set_workspace_root_inner(
        &state,
        workspace_root.to_string_lossy().into_owned(),
    )
    .await
    .unwrap();
    let ticket = myfitanalytics::commands::refresh_now_inner(&state)
        .await
        .unwrap();
    assert!(!ticket.scan_id.is_empty());
    let status = myfitanalytics::commands::get_ingestion_status_inner(&state)
        .await
        .unwrap();
    assert_eq!(status.health.state, "healthy");
    assert_eq!(status.queue_capacity, 32);
    let items = myfitanalytics::commands::list_quality_items_inner(&state)
        .await
        .unwrap();
    assert!(items.is_empty());
    let serialized = serde_json::to_value(&status).unwrap();
    assert_eq!(serialized["health"]["state"], json!("healthy"));
    state.shutdown_storage().await.unwrap();
}

#[test]
fn data_changed_event_contains_only_refresh_identifiers() {
    let event = myfitanalytics::events::DataChangedEvent {
        capabilities: vec!["body.weight".to_owned()],
        dashboards: vec!["dashboard.summary".to_owned()],
    };
    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["capabilities"], json!(["body.weight"]));
    assert_eq!(value["dashboards"], json!(["dashboard.summary"]));
    assert!(value.get("rows").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn thirty_two_concurrent_query_and_refresh_commands_share_one_actor() {
    let (state, root, _config_root) = state();
    let state = Arc::new(state);
    let workspace_root = root.path().join("workspace");
    myfitanalytics::commands::set_workspace_root_inner(
        &state,
        workspace_root.to_string_lossy().into_owned(),
    )
    .await
    .unwrap();

    let mut calls = Vec::with_capacity(32);
    for index in 0..32 {
        let state = Arc::clone(&state);
        calls.push(tokio::spawn(async move {
            if index % 2 == 0 {
                myfitanalytics::commands::refresh_now_inner(&state)
                    .await
                    .map(|ticket| !ticket.scan_id.is_empty())
            } else {
                myfitanalytics::commands::get_ingestion_status_inner(&state)
                    .await
                    .map(|status| status.configured)
            }
        }));
    }
    for call in calls {
        assert!(call.await.unwrap().unwrap());
    }
    let state = Arc::try_unwrap(state).ok().unwrap();
    state.shutdown_storage().await.unwrap();
}
