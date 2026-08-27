use mfa_config::{AppSettings, SettingsStore};
use mfa_contracts::ModuleId;
use myfitanalytics::dialogs::DialogPort;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::time::{Duration, timeout};

struct WorkspacePicker(Mutex<Option<PathBuf>>);

impl WorkspacePicker {
    fn new(path: PathBuf) -> Self {
        Self(Mutex::new(Some(path)))
    }
}

impl DialogPort for WorkspacePicker {
    fn pick_workspace_root(&self) -> Option<PathBuf> {
        self.0.lock().unwrap().take()
    }

    fn pick_module_package(&self) -> Option<PathBuf> {
        None
    }

    fn pick_source_inbox(&self, _module_id: &ModuleId) -> Option<PathBuf> {
        None
    }
}

async fn choose_workspace_root(
    state: &myfitanalytics::AppState,
    path: PathBuf,
) -> myfitanalytics::commands::WorkspaceView {
    myfitanalytics::commands::choose_workspace_root_inner(state, &WorkspacePicker::new(path))
        .await
        .unwrap()
        .unwrap()
}

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
          "artifact_signatures":["sha256:embedded"],
          "extension_contracts":[],
          "settings_schema":{},
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

fn two_source_state() -> (myfitanalytics::AppState, TempDir) {
    let root = TempDir::new().unwrap();
    let config_root = root.path().join("config");
    let module_root = root.path().join("modules");
    fs::create_dir_all(&config_root).unwrap();
    SettingsStore::new(config_root.join("settings.json"))
        .save(&AppSettings::default())
        .unwrap();
    for (module_id, capability, namespace) in [
        ("first-source", "body.weight", "source.first"),
        ("second-source", "body.height", "source.second"),
    ] {
        let embedded = module_root.join(format!("{module_id}/1.0.0/embedded"));
        fs::create_dir_all(&embedded).unwrap();
        fs::write(
            embedded.join("module.json"),
            format!(
                r#"{{
                  "module_type":"source",
                  "module_id":"{module_id}",
                  "module_version":"1.0.0",
                  "package_format_version":"1.0.0",
                  "source_api_version":"1.0.0",
                  "mapping_version":"1.0.0",
                  "compatible_app_versions":[">=0.1.0"],
                  "provided_capabilities":["{capability}"],
                  "accepted_file_patterns":["*.json"],
                  "artifact_signatures":["sha256:embedded"],
                  "extension_contracts":[],
                  "settings_schema":{{}},
                  "entrypoint_hash":"sha256:embedded",
                  "localization_namespace":"{namespace}"
                }}"#
            ),
        )
        .unwrap();
    }
    let state = myfitanalytics::AppState::from_roots_with_core_catalog(
        &config_root,
        &module_root,
        br#"{"locale":"en","namespace":"core","messages":{"app.title":"MyFitAnalytics"}}"#,
    )
    .unwrap();
    (state, root)
}

fn event_state() -> (myfitanalytics::AppState, TempDir) {
    let root = TempDir::new().unwrap();
    let config_root = root.path().join("config");
    let module_root = root.path().join("modules/guest-source/1.0.0/embedded");
    fs::create_dir_all(&module_root).unwrap();
    SettingsStore::new(config_root.join("settings.json"))
        .save(&AppSettings::default())
        .unwrap();
    fs::write(
        module_root.join("module.json"),
        br#"{
          "module_type":"source",
          "module_id":"guest-source",
          "module_version":"1.0.0",
          "package_format_version":"1.0.0",
          "source_api_version":"1.0.0",
          "mapping_version":"1.0.0",
          "compatible_app_versions":[">=0.1.0"],
          "provided_capabilities":["body.weight"],
          "accepted_file_patterns":["*.fixture"],
          "artifact_signatures":["sha256:aa2003b68f1ebbd7d8c22f0d41d93b6e6c6d4c71e9b8b2d46e8ed8dfa9db57c1"],
          "extension_contracts":[],
          "settings_schema":{},
          "entrypoint_hash":"sha256:aa2003b68f1ebbd7d8c22f0d41d93b6e6c6d4c71e9b8b2d46e8ed8dfa9db57c1",
          "localization_namespace":"source.guest"
        }"#,
    )
    .unwrap();
    fs::write(
        module_root.join("module.wasm"),
        include_bytes!("../../crates/mfa-module-host/tests/fixtures/guest-source.wasm"),
    )
    .unwrap();
    let state = myfitanalytics::AppState::from_roots_with_core_catalog(
        &config_root,
        root.path().join("modules"),
        br#"{"locale":"en","namespace":"core","messages":{"app.title":"MyFitAnalytics"}}"#,
    )
    .unwrap();
    (state, root)
}

#[tokio::test]
async fn workspace_command_persists_settings_and_exposes_non_icloud_paths() {
    let (state, root, _config_root) = state();
    let workspace_root = root.path().join("workspace");
    let view = choose_workspace_root(&state, workspace_root.clone()).await;
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
async fn failed_workspace_reconfiguration_preserves_the_working_session() {
    let (state, root, _config_root) = state();
    let workspace_root = root.path().join("workspace");
    choose_workspace_root(&state, workspace_root).await;

    let invalid_root = root.path().join("config/app-data");
    let error = myfitanalytics::commands::choose_workspace_root_inner(
        &state,
        &WorkspacePicker::new(invalid_root),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "storage_initialization");

    let status = myfitanalytics::commands::get_ingestion_status_inner(&state)
        .await
        .unwrap();
    assert!(status.configured);
    assert!(
        !myfitanalytics::commands::refresh_now_inner(&state)
            .await
            .unwrap()
            .scan_id
            .is_empty()
    );
    state.shutdown_storage().await.unwrap();
}

#[tokio::test]
async fn refresh_status_and_quality_commands_return_safe_typed_dtos() {
    let (state, root, _config_root) = state();
    let workspace_root = root.path().join("workspace");
    choose_workspace_root(&state, workspace_root.clone()).await;
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

#[tokio::test]
async fn refresh_scans_all_enabled_sources_with_independent_health() {
    let (state, root) = two_source_state();
    let workspace_root = root.path().join("workspace");
    let view = choose_workspace_root(&state, workspace_root.clone()).await;
    assert_eq!(view.source_paths.len(), 2);
    for source in &view.source_paths {
        fs::write(
            std::path::Path::new(&source.inbox_path).join("asset.json"),
            source.module_id.as_bytes(),
        )
        .unwrap();
    }

    myfitanalytics::commands::refresh_now_inner(&state)
        .await
        .unwrap();
    myfitanalytics::commands::refresh_now_inner(&state)
        .await
        .unwrap();
    let status = myfitanalytics::commands::get_ingestion_status_inner(&state)
        .await
        .unwrap();

    assert_eq!(status.health.attention_items, 2);
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

#[tokio::test]
async fn event_sink_attaches_when_workspace_is_set_after_app_start() {
    let (state, root) = event_state();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    state.set_event_sink(Arc::new(move |event| {
        let _ = sender.send(event);
    }));
    let workspace_root = root.path().join("workspace");
    let view = choose_workspace_root(&state, workspace_root.clone()).await;
    let inbox = std::path::Path::new(&view.source_paths[0].inbox_path);
    fs::write(inbox.join("event.fixture"), b"event bytes").unwrap();
    myfitanalytics::commands::refresh_now_inner(&state)
        .await
        .unwrap();
    myfitanalytics::commands::refresh_now_inner(&state)
        .await
        .unwrap();

    let event = timeout(Duration::from_secs(2), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.capabilities, vec!["body.composition", "body.weight"]);
    assert!(event.dashboards.is_empty());
    let value = serde_json::to_value(event).unwrap();
    assert!(value.get("rows").is_none());
    assert!(value.get("paths").is_none());
    state.shutdown_storage().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn thirty_two_concurrent_query_and_refresh_commands_share_one_actor() {
    let (state, root, _config_root) = state();
    let state = Arc::new(state);
    let workspace_root = root.path().join("workspace");
    choose_workspace_root(&state, workspace_root.clone()).await;

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
