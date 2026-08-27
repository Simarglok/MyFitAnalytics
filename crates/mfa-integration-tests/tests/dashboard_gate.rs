use mfa_archive::ArchiveReconciler;
use mfa_config::{AppSettings, SettingsStore, WorkspacePaths};
use mfa_contracts::{AvailabilityState, CapabilityId, DashboardRequirement, ModuleId};
use mfa_dashboard_host::{
    AvailabilityResolver, CoverageCatalog, Freshness, ModuleRegistryView, ResolvedCapabilities,
    ResolvedCapability,
};
use mfa_module_host::{CapabilityRegistry, ModuleRegistry, PackageInstaller};
use myfitanalytics::commands::{
    get_dashboard_inner, get_navigation_inner, save_phase_event_inner, select_provider_inner,
};
use myfitanalytics::dialogs::DialogPort;
use myfitanalytics::view_models::{DateRangeView, PhaseEventInput};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::{sleep, timeout};

const EXPECTED_JSON: &str = include_str!("../../../web/e2e/fixtures/expected-dashboard.json");

struct WorkspaceDialog(Mutex<Option<PathBuf>>);

impl DialogPort for WorkspaceDialog {
    fn pick_workspace_root(&self) -> Option<PathBuf> {
        self.0.lock().expect("dialog lock is not poisoned").take()
    }

    fn pick_module_package(&self) -> Option<PathBuf> {
        None
    }

    fn pick_source_inbox(&self, _module_id: &ModuleId) -> Option<PathBuf> {
        None
    }
}

fn package(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../dist/modules")
        .join(name)
}

fn fixture(path: &str) -> Vec<u8> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../modules")
        .join(path)
        .pipe_read()
}

trait ReadBytes {
    fn pipe_read(self) -> Vec<u8>;
}

impl ReadBytes for PathBuf {
    fn pipe_read(self) -> Vec<u8> {
        fs::read(self).expect("checked-in synthetic fixture is readable")
    }
}

fn expected() -> Value {
    serde_json::from_str(EXPECTED_JSON).expect("dashboard expectation JSON is valid")
}

fn state(root: &TempDir) -> myfitanalytics::AppState {
    let config_root = root.path().join("config");
    let module_root = root.path().join("modules");
    fs::create_dir_all(&config_root).unwrap();
    let installer = PackageInstaller::new(&module_root);
    for name in [
        "mynetdiary.mfasource",
        "hevy.mfasource",
        "base.mfadashboard",
    ] {
        installer.install(&package(name)).unwrap();
    }
    let modules = installer.list().unwrap();
    let mut settings = AppSettings::default();
    CapabilityRegistry::new()
        .apply_bundled_defaults(&modules, &mut settings)
        .unwrap();
    SettingsStore::new(config_root.join("settings.json"))
        .save(&settings)
        .unwrap();
    myfitanalytics::AppState::from_roots_with_core_catalog(
        &config_root,
        &module_root,
        br#"{"locale":"en","namespace":"core","messages":{"app.title":"MyFitAnalytics"}}"#,
    )
    .unwrap()
}

fn block_keys(document: &mfa_contracts::DashboardDocument) -> BTreeSet<String> {
    document
        .blocks
        .iter()
        .map(|block| match block {
            mfa_contracts::DashboardBlock::Card(value) => value.key.clone(),
            mfa_contracts::DashboardBlock::Table(value) => value.key.clone(),
            mfa_contracts::DashboardBlock::StatusPanel(value) => value.key.clone(),
            mfa_contracts::DashboardBlock::Chart(value) => value.key.clone(),
        })
        .collect()
}

fn state_name(state: &AvailabilityState) -> &'static str {
    match state {
        AvailabilityState::DisabledByUser => "disabled_by_user",
        AvailabilityState::MissingDependency => "missing_dependency",
        AvailabilityState::IncompatibleContract => "incompatible_contract",
        AvailabilityState::MissingCapability => "missing_capability",
        AvailabilityState::WaitingForData => "waiting_for_data",
        AvailabilityState::InsufficientCoverage => "insufficient_coverage",
        AvailabilityState::Ready => "ready",
    }
}

fn card_value<'a>(document: &'a mfa_contracts::DashboardDocument, key: &str) -> &'a Value {
    let card = document
        .blocks
        .iter()
        .find_map(|block| match block {
            mfa_contracts::DashboardBlock::Card(card) if card.key == key => Some(card),
            _ => None,
        })
        .unwrap_or_else(|| panic!("dashboard card {key} is missing"));
    card.value
        .get("value")
        .unwrap_or_else(|| panic!("dashboard card {key} is unavailable"))
}

fn point_for_date<'a>(points: &'a [Value], date: &str) -> &'a Value {
    points
        .iter()
        .find(|point| point["local_date"] == date)
        .unwrap_or_else(|| panic!("dashboard series is missing {date}"))
}

fn assert_golden_number(actual: f64, expected: &Value, label: &str) {
    let expected = expected
        .as_f64()
        .unwrap_or_else(|| panic!("golden {label} is not numeric"));
    assert!(
        (actual - expected).abs() < 1e-9,
        "{label}: actual={actual}, expected={expected}"
    );
}

fn assert_availability_matrix(expected: &Value) {
    let capability = CapabilityId::try_from("body.weight").unwrap();
    let requirement = DashboardRequirement {
        capability: capability.clone(),
        extension: None,
    };
    let provider = ModuleId::try_from("hevy").unwrap();
    let resolver = AvailabilityResolver;
    let mut observed = BTreeSet::new();
    let expected_states = expected["availabilityStates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|state| state.as_str().unwrap())
        .collect::<Vec<_>>();
    let mut modules = ModuleRegistryView::default();
    let ready_registry = || {
        ResolvedCapabilities::from([(
            capability.clone(),
            ResolvedCapability {
                provider: Some(provider.clone()),
                contract_compatible: true,
                has_successful_snapshot: true,
                freshness: Freshness::Fresh,
            },
        )])
    };
    let mut check = |registry: ResolvedCapabilities,
                     coverage: CoverageCatalog,
                     modules: &ModuleRegistryView,
                     expected_state: &str| {
        let availability = resolver.resolve(&requirement, &registry, &coverage, modules);
        assert_eq!(state_name(&availability.state), expected_state);
        observed.insert(state_name(&availability.state));
    };

    modules.disabled_by_user = true;
    modules.missing_dependency = true;
    modules.incompatible_contract = true;
    check(
        ResolvedCapabilities::default(),
        CoverageCatalog::default(),
        &modules,
        expected_states[0],
    );

    modules.disabled_by_user = false;
    check(
        ResolvedCapabilities::default(),
        CoverageCatalog::default(),
        &modules,
        expected_states[1],
    );

    modules.missing_dependency = false;
    check(
        ResolvedCapabilities::from([(
            capability.clone(),
            ResolvedCapability {
                provider: Some(provider.clone()),
                contract_compatible: false,
                has_successful_snapshot: true,
                freshness: Freshness::Fresh,
            },
        )]),
        CoverageCatalog::default(),
        &modules,
        expected_states[2],
    );

    modules.incompatible_contract = false;
    check(
        ResolvedCapabilities::default(),
        CoverageCatalog::default(),
        &modules,
        expected_states[3],
    );
    check(
        ResolvedCapabilities::from([(
            capability.clone(),
            ResolvedCapability {
                provider: Some(provider.clone()),
                contract_compatible: true,
                has_successful_snapshot: false,
                freshness: Freshness::Fresh,
            },
        )]),
        CoverageCatalog::default(),
        &modules,
        expected_states[4],
    );

    let mut insufficient = CoverageCatalog::default();
    insufficient.sufficient.insert(capability.clone(), false);
    check(ready_registry(), insufficient, &modules, expected_states[5]);
    check(
        ready_registry(),
        CoverageCatalog::from([(capability.clone(), true)]),
        &modules,
        expected_states[6],
    );

    assert_eq!(
        observed,
        expected_states.into_iter().collect::<BTreeSet<_>>()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_dashboard_gate_imports_fixtures_and_queries_every_base_page() {
    let root = TempDir::new().unwrap();
    let app = state(&root);
    let workspace_root = root.path().join("workspace");
    myfitanalytics::commands::choose_workspace_root_inner(
        &app,
        &WorkspaceDialog(Mutex::new(Some(workspace_root.clone()))),
    )
    .await
    .unwrap()
    .expect("workspace selection returns a view");

    let mynetdiary = ModuleId::try_from("mynetdiary").unwrap();
    let hevy = ModuleId::try_from("hevy").unwrap();
    fs::write(
        WorkspacePaths::new(workspace_root.clone())
            .source_inbox(&mynetdiary)
            .join("valid-full.xls"),
        fixture("sources/mynetdiary/tests/fixtures/valid-full.xls"),
    )
    .unwrap();
    for name in ["measurement_data.csv", "workout_data.csv"] {
        fs::write(
            WorkspacePaths::new(workspace_root.clone())
                .source_inbox(&hevy)
                .join(name),
            fixture(&format!("sources/hevy/tests/fixtures/{name}")),
        )
        .unwrap();
    }

    myfitanalytics::commands::refresh_now_inner(&app)
        .await
        .unwrap();
    myfitanalytics::commands::refresh_now_inner(&app)
        .await
        .unwrap();

    let ingestion = timeout(Duration::from_secs(20), async {
        loop {
            let navigation = get_navigation_inner(&app).await.unwrap();
            let _overview = get_dashboard_inner(
                "base".to_owned(),
                "overview".to_owned(),
                DateRangeView::synthetic_default(),
                &app,
            )
            .await
            .unwrap();
            let archived_mynetdiary = ArchiveReconciler::new(
                WorkspacePaths::new(workspace_root.clone()),
                mynetdiary.clone(),
            )
            .scan()
            .unwrap()
            .assets
            .len();
            let archived_hevy =
                ArchiveReconciler::new(WorkspacePaths::new(workspace_root.clone()), hevy.clone())
                    .scan()
                    .unwrap()
                    .assets
                    .len();
            if navigation.items.len() == 6
                && archived_mynetdiary == 1
                && archived_hevy == 2
                && !WorkspacePaths::new(workspace_root.clone())
                    .source_inbox(&mynetdiary)
                    .join("valid-full.xls")
                    .exists()
                && !WorkspacePaths::new(workspace_root.clone())
                    .source_inbox(&hevy)
                    .join("measurement_data.csv")
                    .exists()
                && !WorkspacePaths::new(workspace_root.clone())
                    .source_inbox(&hevy)
                    .join("workout_data.csv")
                    .exists()
            {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    if ingestion.is_err() {
        let status = myfitanalytics::commands::get_ingestion_status_inner(&app)
            .await
            .unwrap();
        let mynetdiary_archive_count = ArchiveReconciler::new(
            WorkspacePaths::new(workspace_root.clone()),
            mynetdiary.clone(),
        )
        .scan()
        .unwrap()
        .assets
        .len();
        let hevy_archive_count =
            ArchiveReconciler::new(WorkspacePaths::new(workspace_root.clone()), hevy.clone())
                .scan()
                .unwrap()
                .assets
                .len();
        panic!(
            "synthetic source imports did not commit: health={:?}, archives=({mynetdiary_archive_count},{hevy_archive_count}), inbox=({},{},{})",
            status,
            WorkspacePaths::new(workspace_root.clone())
                .source_inbox(&mynetdiary)
                .join("valid-full.xls")
                .exists(),
            WorkspacePaths::new(workspace_root.clone())
                .source_inbox(&hevy)
                .join("measurement_data.csv")
                .exists(),
            WorkspacePaths::new(workspace_root.clone())
                .source_inbox(&hevy)
                .join("workout_data.csv")
                .exists()
        );
    }

    assert!(
        !WorkspacePaths::new(workspace_root.clone())
            .source_inbox(&mynetdiary)
            .join("valid-full.xls")
            .exists()
    );
    assert!(
        !WorkspacePaths::new(workspace_root.clone())
            .source_inbox(&hevy)
            .join("measurement_data.csv")
            .exists()
    );
    assert!(
        !WorkspacePaths::new(workspace_root.clone())
            .source_inbox(&hevy)
            .join("workout_data.csv")
            .exists()
    );
    assert_eq!(
        ArchiveReconciler::new(WorkspacePaths::new(workspace_root.clone()), mynetdiary)
            .scan()
            .unwrap()
            .assets
            .len(),
        1
    );
    assert_eq!(
        ArchiveReconciler::new(WorkspacePaths::new(workspace_root.clone()), hevy)
            .scan()
            .unwrap()
            .assets
            .len(),
        2
    );

    let expected = expected();
    let expected_pages = expected["pages"].as_object().unwrap();
    let range = DateRangeView {
        start: expected["dateRange"]["start"].as_str().unwrap().to_owned(),
        end: expected["dateRange"]["end"].as_str().unwrap().to_owned(),
    };
    let navigation = get_navigation_inner(&app).await.unwrap();
    assert_eq!(
        navigation
            .items
            .iter()
            .map(|item| item.page_id.as_str())
            .collect::<Vec<_>>(),
        expected["navigationPageIds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|page| page.as_str().unwrap())
            .collect::<Vec<_>>()
    );
    for (page_id, expected_page) in expected_pages {
        let page = get_dashboard_inner("base".to_owned(), page_id.clone(), range.clone(), &app)
            .await
            .unwrap();
        assert_eq!(page.title_key, expected_page["titleKey"]);
        assert_eq!(
            state_name(&page.availability.state),
            expected_page["availabilityState"].as_str().unwrap()
        );
        assert_eq!(
            block_keys(&page.document),
            expected_page["requiredBlockKeys"]
                .as_array()
                .unwrap()
                .iter()
                .map(|key| key.as_str().unwrap().to_owned())
                .collect()
        );
        assert!(page.freshness.generated_at.contains('T'));
        assert!(page.coverage.expected_days > 0);
    }

    let expected_analytics = &expected["analytics"];
    let body_page = get_dashboard_inner("base".to_owned(), "body".to_owned(), range.clone(), &app)
        .await
        .unwrap();
    let body_value = card_value(&body_page.document, "body.raw_weight");
    let expected_weights = expected_analytics["body"]["weightsKg"].as_array().unwrap();
    let observations = body_value["observations"].as_array().unwrap();
    assert_eq!(observations.len(), expected_weights.len());
    for (observation, expected_weight) in observations.iter().zip(expected_weights) {
        assert_golden_number(
            observation["value_kg"].as_f64().unwrap(),
            expected_weight,
            "body.weightsKg",
        );
    }
    let daily_medians = body_value["dailyMedianKg"].as_array().unwrap();
    assert_eq!(
        daily_medians.len(),
        expected_analytics["body"]["dailyMediansKg"]
            .as_array()
            .unwrap()
            .len()
    );
    for (point, expected_weight) in daily_medians.iter().zip(
        expected_analytics["body"]["dailyMediansKg"]
            .as_array()
            .unwrap(),
    ) {
        assert_golden_number(
            point["value_kg"].as_f64().unwrap(),
            expected_weight,
            "body.dailyMediansKg",
        );
    }
    let last_weight = point_for_date(
        body_value["trailing7dMeanKg"].as_array().unwrap(),
        "2026-02-02",
    );
    assert_golden_number(
        last_weight["value"].as_f64().unwrap(),
        &expected_analytics["body"]["trailing7dMeanKgAtLastObservedDate"],
        "body.trailing7dMeanKgAtLastObservedDate",
    );
    assert_golden_number(
        body_value["slope28d"]["slope_per_day"].as_f64().unwrap(),
        &expected_analytics["body"]["slope28dPerDay"],
        "body.slope28dPerDay",
    );

    let nutrition_page = get_dashboard_inner(
        "base".to_owned(),
        "nutrition".to_owned(),
        range.clone(),
        &app,
    )
    .await
    .unwrap();
    let nutrition_value = card_value(&nutrition_page.document, "nutrition.calories");
    let nutrition_days = nutrition_value["days"].as_array().unwrap();
    assert_eq!(
        nutrition_days
            .iter()
            .map(|day| day["logged_item_count"].as_u64().unwrap())
            .sum::<u64>(),
        expected_analytics["nutrition"]["itemCount"]
            .as_u64()
            .unwrap()
    );
    assert_eq!(
        nutrition_days
            .iter()
            .filter(|day| day["quality"] == "complete")
            .count(),
        expected_analytics["nutrition"]["completeDays"]
            .as_u64()
            .unwrap() as usize
    );
    assert_eq!(
        nutrition_days
            .iter()
            .filter(|day| day["quality"] == "partial_fields")
            .count(),
        expected_analytics["nutrition"]["partialDays"]
            .as_u64()
            .unwrap() as usize
    );
    assert_eq!(
        nutrition_days
            .iter()
            .filter(|day| day["quality"] == "missing")
            .count(),
        expected_analytics["nutrition"]["missingDays"]
            .as_u64()
            .unwrap() as usize
    );
    let last_complete_date = nutrition_days
        .iter()
        .find(|day| day["quality"] == "complete")
        .unwrap()["local_date"]
        .as_str()
        .unwrap()
        .to_owned();
    let last_complete_mean = point_for_date(
        nutrition_value["trailing7dMeanCalories"]
            .as_array()
            .unwrap(),
        &last_complete_date,
    );
    assert_golden_number(
        last_complete_mean["value"].as_f64().unwrap(),
        &expected_analytics["nutrition"]["trailing7dMeanCaloriesAtLastCompleteDay"],
        "nutrition.trailing7dMeanCaloriesAtLastCompleteDay",
    );

    let activity_page = get_dashboard_inner(
        "base".to_owned(),
        "activity".to_owned(),
        range.clone(),
        &app,
    )
    .await
    .unwrap();
    let activity_value = card_value(&activity_page.document, "activity.steps");
    let activity_date = expected_analytics["activity"]["observedDate"]
        .as_str()
        .unwrap();
    assert_golden_number(
        point_for_date(activity_value["steps"].as_array().unwrap(), activity_date)["value"]
            .as_f64()
            .unwrap(),
        &expected_analytics["activity"]["steps"],
        "activity.steps",
    );
    let activity_summary =
        point_for_date(activity_value["events"].as_array().unwrap(), activity_date);
    assert_eq!(
        activity_summary["accepted_event_count"].as_u64().unwrap(),
        expected_analytics["activity"]["acceptedEventCount"]
            .as_u64()
            .unwrap()
    );
    assert_eq!(
        activity_summary["duration_seconds"].as_u64().unwrap(),
        expected_analytics["activity"]["durationSeconds"]
            .as_u64()
            .unwrap()
    );
    assert_golden_number(
        activity_summary["distance_km"].as_f64().unwrap(),
        &expected_analytics["activity"]["distanceKm"],
        "activity.distanceKm",
    );
    assert_golden_number(
        activity_summary["estimated_calories_kcal"]
            .as_f64()
            .unwrap(),
        &expected_analytics["activity"]["estimatedCaloriesKcal"],
        "activity.estimatedCaloriesKcal",
    );
    assert_eq!(
        activity_summary["unknown_event_count"].as_u64().unwrap(),
        expected_analytics["activity"]["unknownEventCount"]
            .as_u64()
            .unwrap()
    );
    assert_golden_number(
        point_for_date(
            activity_value["mean_steps_7d"].as_array().unwrap(),
            activity_date,
        )["value"]
            .as_f64()
            .unwrap(),
        &expected_analytics["activity"]["meanSteps7dAtLastObservedDate"],
        "activity.meanSteps7dAtLastObservedDate",
    );
    assert_golden_number(
        point_for_date(
            activity_value["mean_steps_28d"].as_array().unwrap(),
            activity_date,
        )["value"]
            .as_f64()
            .unwrap(),
        &expected_analytics["activity"]["meanSteps28dAtLastObservedDate"],
        "activity.meanSteps28dAtLastObservedDate",
    );
    assert_golden_number(
        point_for_date(
            activity_value["heart_rate"].as_array().unwrap(),
            activity_date,
        )["value"]
            .as_f64()
            .unwrap(),
        &expected_analytics["activity"]["heartRateBpm"],
        "activity.heartRate",
    );
    assert_golden_number(
        point_for_date(activity_value["water"].as_array().unwrap(), activity_date)["value"]
            .as_f64()
            .unwrap(),
        &expected_analytics["activity"]["waterMl"],
        "activity.water",
    );

    let strength_page = get_dashboard_inner(
        "base".to_owned(),
        "strength".to_owned(),
        range.clone(),
        &app,
    )
    .await
    .unwrap();
    let strength_value = card_value(&strength_page.document, "strength.sessions");
    for (field, golden) in [
        ("seven_day", "sessions7d"),
        ("fourteen_day", "sessions14d"),
        ("twenty_eight_day", "sessions28d"),
    ] {
        assert_eq!(
            strength_value["session_counts"][field].as_u64().unwrap(),
            expected_analytics["strength"][golden].as_u64().unwrap()
        );
    }
    assert_eq!(
        strength_value["working_sets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|set| set["count"].as_u64().unwrap())
            .sum::<u64>(),
        expected_analytics["strength"]["workingSets"]
            .as_u64()
            .unwrap()
    );
    assert_golden_number(
        strength_value["weekly_best_e1rm"][0]["value_kg"]
            .as_f64()
            .unwrap(),
        &expected_analytics["strength"]["weeklyBestE1rmKg"],
        "strength.weeklyBestE1rmKg",
    );
    let tdee = &nutrition_value["tdee"];
    assert_eq!(tdee["state"], expected_analytics["tdee"]["state"]);
    for (field, golden) in [
        ("complete_nutrition_days", "completeNutritionDays"),
        ("weight_days", "weightDays"),
        ("first_7d_weight_days", "first7dWeightDays"),
        ("last_7d_weight_days", "last7dWeightDays"),
    ] {
        assert_eq!(
            tdee["coverage"][field].as_u64().unwrap(),
            expected_analytics["tdee"][golden].as_u64().unwrap()
        );
    }
    assert_eq!(
        tdee["coverage"]["slope_available"],
        expected_analytics["tdee"]["slopeAvailable"]
    );
    assert_eq!(
        tdee["coverage"]["excluded_days"],
        expected_analytics["tdee"]["excludedDaysBeforePhase"].clone()
    );

    let provider_capability = expected["providerSelection"]["capability"]
        .as_str()
        .unwrap()
        .to_owned();
    let provider_module = expected["providerSelection"]["moduleId"]
        .as_str()
        .unwrap()
        .to_owned();
    let selected =
        select_provider_inner(provider_capability.clone(), provider_module.clone(), &app)
            .await
            .unwrap();
    assert_eq!(selected.capability, provider_capability);
    assert_eq!(selected.module_id, provider_module);
    assert_eq!(
        selected.active_providers[expected["providerSelection"]["capability"]
            .as_str()
            .unwrap()],
        expected["providerSelection"]["moduleId"].as_str().unwrap()
    );

    let event = save_phase_event_inner(
        PhaseEventInput {
            phase_event_id: None,
            event_type: expected["phaseEvent"]["eventType"]
                .as_str()
                .unwrap()
                .to_owned(),
            start_date: expected["phaseEvent"]["startDate"]
                .as_str()
                .unwrap()
                .to_owned(),
            end_date: expected["phaseEvent"]["endDate"]
                .as_str()
                .unwrap()
                .to_owned(),
            description: Some(
                expected["phaseEvent"]["description"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            ),
            exclude_from_tdee: expected["phaseEvent"]["excludeFromTdee"].as_bool().unwrap(),
        },
        &app,
    )
    .await
    .unwrap();
    assert_eq!(
        event.event_type,
        expected["phaseEvent"]["eventType"].as_str().unwrap()
    );
    assert_eq!(
        event.start_date,
        expected["phaseEvent"]["startDate"].as_str().unwrap()
    );
    assert_eq!(
        event.end_date,
        expected["phaseEvent"]["endDate"].as_str().unwrap()
    );
    assert_eq!(
        event.exclude_from_tdee,
        expected["phaseEvent"]["excludeFromTdee"].as_bool().unwrap()
    );
    assert!(event.phase_event_id.len() >= 32);

    let body_after_phase =
        get_dashboard_inner("base".to_owned(), "body".to_owned(), range.clone(), &app)
            .await
            .unwrap();
    let body_after_value = card_value(&body_after_phase.document, "body.raw_weight");
    let persisted_phases = body_after_value["phaseEvents"].as_array().unwrap();
    assert_eq!(persisted_phases.len(), 1);
    assert_eq!(
        persisted_phases[0]["event_type"].as_str().unwrap(),
        expected["phaseEvent"]["eventType"].as_str().unwrap()
    );

    let nutrition_after_phase = get_dashboard_inner(
        "base".to_owned(),
        "nutrition".to_owned(),
        range.clone(),
        &app,
    )
    .await
    .unwrap();
    let tdee_after_phase = card_value(&nutrition_after_phase.document, "nutrition.calories");
    assert_eq!(
        tdee_after_phase["tdee"]["coverage"]["excluded_days"],
        expected["analytics"]["tdee"]["excludedDaysAfterPhase"].clone()
    );

    assert_availability_matrix(&expected);

    app.shutdown_storage().await.unwrap();
}
