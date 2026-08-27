use mfa_config::{AppSettings, SettingsStore};
use mfa_contracts::DashboardBlock;
use mfa_module_host::PackageInstaller;
use myfitanalytics::commands::{
    choose_workspace_root_inner, get_dashboard_inner, get_navigation_inner, save_phase_event_inner,
};
use myfitanalytics::dialogs::DialogPort;
use myfitanalytics::view_models::{DateRangeView, PhaseEventInput};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tempfile::TempDir;

struct WorkspaceDialog(Mutex<Option<PathBuf>>);

impl DialogPort for WorkspaceDialog {
    fn pick_workspace_root(&self) -> Option<PathBuf> {
        self.0.lock().unwrap().take()
    }

    fn pick_module_package(&self) -> Option<PathBuf> {
        None
    }

    fn pick_source_inbox(&self, _module_id: &mfa_contracts::ModuleId) -> Option<PathBuf> {
        None
    }
}

fn state() -> (myfitanalytics::AppState, TempDir) {
    let root = TempDir::new().unwrap();
    let config_root = root.path().join("config");
    let module_root = root.path().join("modules");
    fs::create_dir_all(&config_root).unwrap();
    PackageInstaller::new(&module_root)
        .install(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist/modules/base.mfadashboard"),
        )
        .unwrap();
    SettingsStore::new(config_root.join("settings.json"))
        .save(&AppSettings::default())
        .unwrap();
    let state = myfitanalytics::AppState::from_roots_with_core_catalog(
        &config_root,
        &module_root,
        br#"{"locale":"en","namespace":"core","messages":{"app.title":"MyFitAnalytics"}}"#,
    )
    .unwrap();
    (state, root)
}

#[tokio::test]
async fn navigation_and_dashboard_commands_return_safe_typed_views() {
    let (state, _root) = state();
    let navigation = get_navigation_inner(&state).await.unwrap();
    assert_eq!(navigation.items.len(), 6);
    assert!(
        navigation
            .items
            .iter()
            .any(|item| item.page_id == "overview")
    );
    let serialized = serde_json::to_string(&navigation).unwrap();
    assert!(!serialized.contains("archive"));
    assert!(!serialized.contains("SELECT"));

    let dashboard = get_dashboard_inner(
        "base".to_owned(),
        "overview".to_owned(),
        DateRangeView::synthetic_default(),
        &state,
    )
    .await
    .unwrap();
    assert_eq!(dashboard.page_id, "overview");
    assert!(dashboard.coverage.observed_days <= dashboard.coverage.expected_days);
    assert!(
        dashboard
            .document
            .blocks
            .iter()
            .any(|block| matches!(block, DashboardBlock::Chart(_)))
    );
    let value: Value = serde_json::to_value(&dashboard).unwrap();
    assert!(value.get("rawSnapshot").is_none());
    assert!(value.to_string().len() < 100_000);
}

#[tokio::test]
async fn phase_event_command_round_trips_a_user_owned_event() {
    let (state, root) = state();
    choose_workspace_root_inner(
        &state,
        &WorkspaceDialog(Mutex::new(Some(root.path().join("workspace")))),
    )
    .await
    .unwrap();
    let event = save_phase_event_inner(
        PhaseEventInput {
            phase_event_id: None,
            event_type: "cut".to_owned(),
            start_date: "2026-01-01".to_owned(),
            end_date: "2026-01-07".to_owned(),
            description: Some("synthetic test phase".to_owned()),
            exclude_from_tdee: true,
        },
        &state,
    )
    .await
    .unwrap();
    assert_eq!(event.event_type, "cut");
    assert!(event.phase_event_id.len() > 10);
    assert!(
        !serde_json::to_string(&event)
            .unwrap()
            .contains(root.path().to_string_lossy().as_ref())
    );
}
