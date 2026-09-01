use mfa_config::{AppSettings, SettingsStore};
use mfa_contracts::{AvailabilityState, DashboardBlock, LocalDate};
use mfa_dashboard_host::DashboardOutput;
use mfa_module_host::PackageInstaller;
use myfitanalytics::commands::{
    choose_workspace_root_inner, delete_phase_event, delete_phase_event_inner,
    get_bootstrap_state_inner, get_dashboard_inner, get_navigation_inner, list_phase_events,
    list_phase_events_inner, save_phase_event_inner, select_provider_inner,
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

fn state_with_bundled_sources() -> (myfitanalytics::AppState, TempDir) {
    let root = TempDir::new().unwrap();
    let config_root = root.path().join("config");
    let module_root = root.path().join("modules");
    fs::create_dir_all(&config_root).unwrap();
    let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist/modules");
    let installer = PackageInstaller::new(&module_root);
    for name in [
        "hevy.mfasource",
        "mynetdiary.mfasource",
        "base.mfadashboard",
    ] {
        installer.install(&package_root.join(name)).unwrap();
    }
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

fn fixture_range() -> DateRangeView {
    DateRangeView {
        start: "2026-01-01".to_owned(),
        end: "2026-01-31".to_owned(),
    }
}

#[test]
fn phase_event_commands_expose_typed_list_and_delete_entrypoints() {
    let _ = list_phase_events;
    let _ = delete_phase_event;
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
        fixture_range(),
        &state,
    )
    .await
    .unwrap();
    assert_eq!(dashboard.page_id, "overview");
    assert!(dashboard.coverage.observed_days <= dashboard.coverage.expected_days);
    let DashboardOutput::Document(document) = &dashboard.document else {
        panic!("expected a dashboard document");
    };
    assert!(
        document
            .blocks
            .iter()
            .any(|block| matches!(block, DashboardBlock::Chart(_)))
    );
    let value: Value = serde_json::to_value(&dashboard).unwrap();
    assert!(value.get("rawSnapshot").is_none());
    assert!(value.to_string().len() < 100_000);
}

#[tokio::test]
async fn missing_provider_selection_is_reported_in_navigation_and_dashboard() {
    let (state, _root) = state();
    let navigation = get_navigation_inner(&state).await.unwrap();
    assert!(!navigation.items.is_empty());
    for item in navigation.items {
        assert_eq!(
            item.availability.state,
            AvailabilityState::MissingCapability
        );
        assert_eq!(
            item.availability.action.as_deref(),
            Some("dashboard.action.configure_source")
        );
    }

    let dashboard = get_dashboard_inner(
        "base".to_owned(),
        "overview".to_owned(),
        fixture_range(),
        &state,
    )
    .await
    .unwrap();
    assert_eq!(
        dashboard.availability.state,
        AvailabilityState::MissingCapability
    );
    assert_eq!(
        dashboard.availability.action.as_deref(),
        Some("dashboard.action.configure_source")
    );
}

#[tokio::test]
async fn explicit_provider_selection_reports_waiting_for_data() {
    let (state, _root) = state_with_bundled_sources();
    let bootstrap_before = get_bootstrap_state_inner(&state).await.unwrap();
    assert!(bootstrap_before.active_providers.is_empty());

    let navigation_before = get_navigation_inner(&state).await.unwrap();
    for item in navigation_before.items {
        assert_eq!(
            item.availability.state,
            AvailabilityState::MissingCapability
        );
        assert_eq!(
            item.availability.action.as_deref(),
            Some("dashboard.action.configure_source")
        );
    }
    let dashboard_before = get_dashboard_inner(
        "base".to_owned(),
        "overview".to_owned(),
        fixture_range(),
        &state,
    )
    .await
    .unwrap();
    assert_eq!(
        dashboard_before.availability.state,
        AvailabilityState::MissingCapability
    );
    assert_eq!(
        dashboard_before.availability.action.as_deref(),
        Some("dashboard.action.configure_source")
    );
    let bootstrap_after_probe = get_bootstrap_state_inner(&state).await.unwrap();
    assert!(bootstrap_after_probe.active_providers.is_empty());

    for (capability, provider) in [
        ("activity.days", "mynetdiary"),
        ("activity.events", "mynetdiary"),
        ("heart_rate.observations", "mynetdiary"),
        ("body.fat_percentage", "hevy"),
        ("body.weight", "hevy"),
        ("nutrition.items", "mynetdiary"),
        ("strength.sessions", "hevy"),
        ("strength.sets", "hevy"),
    ] {
        select_provider_inner(capability.to_owned(), provider.to_owned(), &state)
            .await
            .unwrap();
    }

    let bootstrap_after_selection = get_bootstrap_state_inner(&state).await.unwrap();
    assert_eq!(bootstrap_after_selection.active_providers.len(), 8);
    assert_eq!(
        bootstrap_after_selection.active_providers["activity.days"],
        "mynetdiary"
    );
    assert_eq!(
        bootstrap_after_selection.active_providers["body.weight"],
        "hevy"
    );

    let navigation = get_navigation_inner(&state).await.unwrap();
    for item in navigation.items {
        assert_eq!(item.availability.state, AvailabilityState::WaitingForData);
        assert_eq!(
            item.availability.action.as_deref(),
            Some("dashboard.action.import_data")
        );
    }

    let dashboard = get_dashboard_inner(
        "base".to_owned(),
        "overview".to_owned(),
        fixture_range(),
        &state,
    )
    .await
    .unwrap();
    assert_eq!(
        dashboard.availability.state,
        AvailabilityState::WaitingForData
    );
    assert_eq!(
        dashboard.availability.action.as_deref(),
        Some("dashboard.action.import_data")
    );
}

#[tokio::test]
async fn navigation_uses_the_injected_local_date_when_no_observation_exists() {
    let (state, _root) = state();
    let navigation = myfitanalytics::commands::get_navigation_inner_at(
        &state,
        "2026-04-15".parse::<LocalDate>().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        navigation.initial_range,
        DateRangeView {
            start: "2026-03-16".to_owned(),
            end: "2026-04-15".to_owned(),
        }
    );
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

#[tokio::test]
async fn phase_event_commands_list_update_and_delete_typed_views() {
    let (state, root) = state();
    choose_workspace_root_inner(
        &state,
        &WorkspaceDialog(Mutex::new(Some(root.path().join("workspace")))),
    )
    .await
    .unwrap();

    let created = save_phase_event_inner(
        PhaseEventInput {
            phase_event_id: None,
            event_type: "bulk".to_owned(),
            start_date: "2026-02-01".to_owned(),
            end_date: "2026-02-03".to_owned(),
            description: Some("before edit".to_owned()),
            exclude_from_tdee: true,
        },
        &state,
    )
    .await
    .unwrap();
    assert_eq!(
        list_phase_events_inner(&state).await.unwrap(),
        vec![created.clone()]
    );

    let updated = save_phase_event_inner(
        PhaseEventInput {
            phase_event_id: Some(created.phase_event_id.clone()),
            event_type: "maintenance".to_owned(),
            start_date: "2026-02-02".to_owned(),
            end_date: "2026-02-04".to_owned(),
            description: Some("after edit".to_owned()),
            exclude_from_tdee: false,
        },
        &state,
    )
    .await
    .unwrap();
    assert_eq!(
        list_phase_events_inner(&state).await.unwrap(),
        vec![updated]
    );

    delete_phase_event_inner(created.phase_event_id, &state)
        .await
        .unwrap();
    assert!(list_phase_events_inner(&state).await.unwrap().is_empty());
}

#[test]
fn availability_state_serializes_as_a_primitive_snake_case_string() {
    let view = myfitanalytics::view_models::AvailabilityView {
        state: mfa_contracts::AvailabilityState::InsufficientCoverage,
        reason_key: "dashboard.insufficient_coverage".to_owned(),
        required_capabilities: vec!["body.weight".to_owned()],
        required_dependencies: Vec::new(),
        freshness: mfa_dashboard_host::Freshness::Fresh,
        action: Some("dashboard.action.import_data".to_owned()),
    };

    let serialized = serde_json::to_value(view).unwrap();
    assert_eq!(serialized["state"], "insufficient_coverage");
    assert!(serialized["state"].is_string());
    assert_eq!(serialized["action"], "dashboard.action.import_data");
}

#[tokio::test]
async fn unconfigured_dashboard_does_not_emit_ready_inner_status_or_capabilities() {
    let (state, _root) = state();
    let dashboard = get_dashboard_inner(
        "base".to_owned(),
        "overview".to_owned(),
        fixture_range(),
        &state,
    )
    .await
    .unwrap();

    assert_ne!(
        dashboard.availability.state,
        mfa_contracts::AvailabilityState::Ready
    );
    let document = match dashboard.document {
        DashboardOutput::Document(document) => document,
        other => panic!("expected a non-error dashboard document, got {other:?}"),
    };
    assert!(!document.blocks.iter().any(|block| matches!(
        block,
        DashboardBlock::StatusPanel(panel)
            if panel.state == mfa_contracts::AvailabilityState::Ready
    )));
    let body_weight = document
        .blocks
        .iter()
        .find_map(|block| match block {
            DashboardBlock::Card(card) if card.key == "overview.body_weight" => Some(&card.value),
            _ => None,
        })
        .expect("overview body weight card");
    assert_eq!(body_weight["available"], false);
}
