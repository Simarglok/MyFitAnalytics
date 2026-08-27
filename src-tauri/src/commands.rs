use crate::dialogs::{DialogPort, NativeDialogPort};
use crate::state::{AppState, AppStateError};
use crate::view_models::{
    AvailabilityView, CoverageView, DashboardPageView, DateRangeView, FreshnessView,
    NavigationItemView, NavigationView, PhaseEventInput, PhaseEventView, ProviderView,
};
use chrono::Utc;
use mfa_analytics::{DateRange, MetricContext, WeightObservation, weight_analytics};
use mfa_contracts::{
    AvailabilityState, CanonicalObservation, CapabilityId, DashboardInput, ModuleId,
    ModuleManifest, ModuleType, PhaseEvent,
};
use mfa_db::{
    CreatePhaseEvent, DeletePhaseEvent, HealthCheck, ListPhaseEvents, ListQualityItems,
    LogicalSnapshotKey, QuerySnapshot, UpdatePhaseEvent,
};
use mfa_ingestion::{HealthSnapshot, HealthState, IngestionCoordinator, RecoveryMode, now_request};
use mfa_module_host::{ComponentRuntime, InstalledModule, PackageInstaller, UninstallTransaction};
use serde::Serialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapState {
    pub product_name: String,
    pub locale: String,
    pub active_providers: BTreeMap<String, String>,
    pub modules: Vec<ModuleView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleView {
    pub id: String,
    pub module_type: String,
    pub version: String,
    pub enabled: bool,
    pub localization_namespace: String,
    pub display_name: String,
    pub provided_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleCatalogEntryView {
    pub module: ModuleView,
    pub origin: String,
    pub install_state: String,
    pub available_version: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub workspace_root: String,
    pub app_data_root: String,
    pub database_path: String,
    pub recovery_path: String,
    pub backup_path: String,
    pub archive_root: String,
    pub source_paths: Vec<SourcePathView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePathView {
    pub module_id: String,
    pub inbox_path: String,
    pub archive_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanTicketView {
    pub scan_id: String,
    pub coalesced_requests: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthView {
    pub state: String,
    pub working_jobs: u64,
    pub waiting_assets: u64,
    pub attention_items: u64,
    pub critical_items: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestionStatusView {
    pub health: HealthView,
    pub queue_capacity: usize,
    pub recovery_mode: String,
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityItemView {
    pub id: String,
    pub item_type: String,
    pub severity: String,
    pub message: String,
    pub status: String,
    pub asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptView {
    pub asset_id: String,
    pub attempt_id: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl From<AppStateError> for CommandError {
    fn from(error: AppStateError) -> Self {
        Self {
            code: error.code().to_owned(),
            message: error.to_string(),
        }
    }
}

pub async fn get_bootstrap_state_inner(state: &AppState) -> Result<BootstrapState, CommandError> {
    let settings = state.settings();
    Ok(BootstrapState {
        product_name: "MyFitAnalytics".to_owned(),
        locale: settings.locale,
        active_providers: state
            .providers()
            .active_providers
            .iter()
            .map(|(capability, module)| (capability.to_string(), module.to_string()))
            .collect(),
        modules: list_modules_inner(state).await?,
    })
}

pub async fn list_modules_inner(state: &AppState) -> Result<Vec<ModuleView>, CommandError> {
    Ok(state.modules().iter().map(module_view).collect())
}

fn module_view(module: &InstalledModule) -> ModuleView {
    let provided_capabilities = match &module.manifest {
        ModuleManifest::Source(manifest) => manifest
            .provided_capabilities
            .iter()
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    };
    ModuleView {
        id: module.module_id.to_string(),
        module_type: match module.module_type {
            ModuleType::Source => "source",
            ModuleType::Dashboard => "dashboard",
            ModuleType::Locale => "locale",
        }
        .to_owned(),
        version: module.module_version.to_string(),
        enabled: module.enabled,
        localization_namespace: localization_namespace(&module.manifest),
        display_name: display_name(&module.module_id),
        provided_capabilities,
    }
}

fn display_name(module_id: &ModuleId) -> String {
    match module_id.as_str() {
        "hevy" => "Hevy".to_owned(),
        "mynetdiary" => "MyNetDiary".to_owned(),
        other => other.to_owned(),
    }
}

pub async fn list_module_catalog_inner(
    state: &AppState,
) -> Result<Vec<ModuleCatalogEntryView>, CommandError> {
    let installer = state.package_installer();
    let mut latest_modules = BTreeMap::new();
    for module in state.modules() {
        latest_modules
            .entry(module.module_id.clone())
            .and_modify(|current: &mut InstalledModule| {
                if module.module_version > current.module_version {
                    *current = module.clone();
                }
            })
            .or_insert(module);
    }
    let modules: Vec<InstalledModule> = latest_modules.into_values().collect();
    let bundled = state.bundled_packages();
    let mut entries = Vec::new();
    let mut installed_ids = BTreeMap::new();

    for module in &modules {
        installed_ids.insert(module.module_id.clone(), ());
        let bundled_path = bundled.get(&module.module_id);
        let inspected = bundled_path.and_then(|path| installer.inspect(path).ok());
        let bundled_error = bundled_path
            .and_then(|path| installer.inspect(path).err())
            .map(|error| error.code().to_owned());
        let inspect_error = installer
            .installed_app_compatibility_error(module)
            .map(str::to_owned)
            .or(bundled_error);
        let available_version = inspected.as_ref().and_then(|package| {
            let version = manifest_version(&package.manifest);
            (*version > module.module_version).then(|| version.to_string())
        });
        let install_state = catalog_install_state(
            inspect_error.as_deref(),
            available_version.is_some(),
            module.enabled,
        );
        entries.push(ModuleCatalogEntryView {
            module: module_view(module),
            origin: if bundled_path.is_some() {
                "bundled".to_owned()
            } else {
                "installed".to_owned()
            },
            install_state: install_state.to_owned(),
            available_version,
            error_code: inspect_error,
        });
    }

    for (module_id, package_path) in bundled {
        if installed_ids.contains_key(&module_id) {
            continue;
        }
        match installer.inspect(&package_path) {
            Ok(package) => {
                let module = module_view_from_manifest(&package.manifest, false);
                entries.push(ModuleCatalogEntryView {
                    module,
                    origin: "bundled".to_owned(),
                    install_state: "available".to_owned(),
                    available_version: Some(manifest_version(&package.manifest).to_string()),
                    error_code: None,
                });
            }
            Err(error) => {
                let module = ModuleView {
                    id: module_id.to_string(),
                    module_type: "source".to_owned(),
                    version: "unknown".to_owned(),
                    enabled: false,
                    localization_namespace: format!("source.{}", module_id),
                    display_name: display_name(&module_id),
                    provided_capabilities: Vec::new(),
                };
                entries.push(ModuleCatalogEntryView {
                    module,
                    origin: "bundled".to_owned(),
                    install_state: catalog_install_state_for_inspect_error(error.code()).to_owned(),
                    available_version: None,
                    error_code: Some(error.code().to_owned()),
                });
            }
        }
    }
    Ok(entries)
}

pub async fn choose_workspace_root_inner(
    state: &AppState,
    dialogs: &dyn DialogPort,
) -> Result<Option<WorkspaceView>, CommandError> {
    let Some(path) = dialogs.pick_workspace_root() else {
        return Ok(None);
    };
    state
        .configure_workspace(path)
        .await
        .map_err(CommandError::from)?;
    workspace_view(state).map(Some)
}

pub async fn get_workspace_view_inner(state: &AppState) -> Result<WorkspaceView, CommandError> {
    workspace_view(state)
}

pub async fn choose_and_install_module_inner(
    state: &AppState,
    dialogs: &dyn DialogPort,
) -> Result<Option<ModuleView>, CommandError> {
    let Some(path) = dialogs.pick_module_package() else {
        return Ok(None);
    };
    state
        .install_package(&path)
        .await
        .map(|module| Some(module_view(&module)))
        .map_err(CommandError::from)
}

pub async fn choose_source_inbox_inner(
    state: &AppState,
    module_id: String,
    dialogs: &dyn DialogPort,
) -> Result<Option<WorkspaceView>, CommandError> {
    let module_id = parse_module_id(&module_id)?;
    if !state
        .modules()
        .iter()
        .any(|module| module.module_id == module_id)
    {
        return Err(command_error(
            "module_not_found",
            "source module is not installed",
        ));
    }
    let Some(path) = dialogs.pick_source_inbox(&module_id) else {
        return Ok(None);
    };
    std::fs::create_dir_all(&path).map_err(|error| {
        command_error(
            "inbox_unavailable",
            &format!("could not create inbox: {error}"),
        )
    })?;
    let previous = state.settings();
    let mut settings = previous.clone();
    settings.source_inbox_roots.insert(module_id.clone(), path);
    state.save_settings(settings).map_err(CommandError::from)?;
    if let Err(error) = state.reconfigure_source(&module_id).await {
        if let Err(rollback_error) = state.save_settings(previous) {
            return Err(command_error(
                "inbox_rollback_failed",
                &format!("{error}; restoring settings failed: {rollback_error}"),
            ));
        }
        if let Err(rollback_error) = state.reconfigure_source(&module_id).await {
            return Err(command_error(
                "inbox_rollback_failed",
                &format!("{error}; restoring runtime failed: {rollback_error}"),
            ));
        }
        return Err(error.into());
    }
    workspace_view(state).map(Some)
}

pub async fn set_module_enabled_inner(
    state: &AppState,
    module_id: String,
    enabled: bool,
) -> Result<ModuleView, CommandError> {
    let module_id = parse_module_id(&module_id)?;
    let previous = state
        .modules()
        .into_iter()
        .find(|module| module.module_id == module_id)
        .ok_or_else(|| command_error("module_not_found", "module is not installed"))?;
    if enabled && state.settings().workspace_root.is_none() {
        return Err(command_error(
            "workspace_required",
            "configure a workspace before enabling a source",
        ));
    }
    let installer = state.package_installer();
    installer
        .set_enabled(&module_id, enabled)
        .map_err(|error| command_error(error.code(), &error.to_string()))?;
    if let Err(error) = state.refresh_registry() {
        let _ = installer.set_enabled(&module_id, previous.enabled);
        let _ = state.refresh_registry();
        return Err(error.into());
    }
    let reconfigure_result = if previous.module_type == ModuleType::Source {
        state.reconfigure_source(&module_id).await
    } else {
        Ok(())
    };
    if let Err(error) = reconfigure_result {
        let _ = installer.set_enabled(&module_id, previous.enabled);
        let _ = state.refresh_registry();
        let _ = state.reconfigure_source(&module_id).await;
        return Err(error.into());
    }
    state
        .modules()
        .into_iter()
        .find(|module| module.module_id == module_id)
        .map(|module| module_view(&module))
        .ok_or_else(|| command_error("module_not_found", "module is not installed"))
}

pub async fn uninstall_module_inner(
    state: &AppState,
    module_id: String,
) -> Result<(), CommandError> {
    let module_id = parse_module_id(&module_id)?;
    let module = state
        .modules()
        .into_iter()
        .find(|module| module.module_id == module_id)
        .ok_or_else(|| command_error("module_not_found", "module is not installed"))?;
    if module.enabled {
        return Err(command_error(
            "module_must_be_disabled",
            "disable the module before uninstalling it",
        ));
    }
    let previous = state.settings();
    let mut settings = previous.clone();
    settings
        .active_providers
        .retain(|_, selected| selected != &module_id);
    let installer = state.uninstall_package_installer();
    let mut transaction = installer
        .stage_uninstall(&module_id)
        .map_err(|error| command_error(error.code(), &error.to_string()))?;
    if let Err(error) = state.save_settings(settings) {
        let rollback = rollback_uninstall(state, &installer, &mut transaction, &previous);
        return Err(rollback.unwrap_or_else(|| error.into()));
    }
    if let Err(error) = installer.apply_uninstall(&mut transaction) {
        let rollback = rollback_uninstall(state, &installer, &mut transaction, &previous);
        return Err(rollback.unwrap_or_else(|| command_error(error.code(), &error.to_string())));
    }
    if let Err(error) = state.refresh_registry_during_uninstall(&installer) {
        let rollback = rollback_uninstall(state, &installer, &mut transaction, &previous);
        return Err(rollback.unwrap_or_else(|| error.into()));
    }
    let reconfigure_result = if module.module_type == ModuleType::Source {
        state.reconfigure_source(&module_id).await
    } else {
        Ok(())
    };
    if let Err(error) = reconfigure_result {
        let rollback = rollback_uninstall(state, &installer, &mut transaction, &previous);
        let runtime_rollback = state.reconfigure_source(&module_id).await;
        if let Some(rollback) = rollback {
            return Err(rollback);
        }
        if let Err(runtime_error) = runtime_rollback {
            return Err(command_error(
                "uninstall_rollback_failed",
                &format!("{error}; restoring runtime failed: {runtime_error}"),
            ));
        }
        return Err(error.into());
    }
    if let Err(error) = installer.finalize_uninstall(&mut transaction) {
        let rollback = rollback_uninstall(state, &installer, &mut transaction, &previous);
        return Err(rollback.unwrap_or_else(|| command_error(error.code(), &error.to_string())));
    }
    Ok(())
}

pub async fn update_module_inner(
    state: &AppState,
    module_id: String,
) -> Result<ModuleView, CommandError> {
    let module_id = parse_module_id(&module_id)?;
    let package = state.bundled_packages().remove(&module_id).ok_or_else(|| {
        command_error(
            "module_update_unavailable",
            "no bundled update is available",
        )
    })?;
    state
        .install_package(&package)
        .await
        .map(|module| module_view(&module))
        .map_err(CommandError::from)
}

pub async fn select_module_provider_inner(
    state: &AppState,
    capability: String,
    module_id: String,
) -> Result<ProviderSelectionView, CommandError> {
    let capability = CapabilityId::try_from(capability)
        .map_err(|error| command_error("invalid_capability", &error.to_string()))?;
    let module_id = parse_module_id(&module_id)?;
    let resolution = state
        .select_provider(capability, module_id)
        .map_err(CommandError::from)?;
    Ok(ProviderSelectionView {
        active_providers: resolution
            .active_providers
            .into_iter()
            .map(|(capability, module)| (capability.to_string(), module.to_string()))
            .collect(),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSelectionView {
    pub active_providers: BTreeMap<String, String>,
}

const BASE_DASHBOARD_PAGES: [(&str, &str); 6] = [
    ("overview", "base.overview.title"),
    ("body", "base.body.title"),
    ("nutrition", "base.nutrition.title"),
    ("activity", "base.activity.title"),
    ("strength", "base.strength.title"),
    ("sources", "base.sources.title"),
];

pub async fn get_navigation_inner(state: &AppState) -> Result<NavigationView, CommandError> {
    let items = BASE_DASHBOARD_PAGES
        .into_iter()
        .map(|(page_id, title_key)| NavigationItemView {
            id: format!("base:{page_id}"),
            page_id: page_id.to_owned(),
            title_key: title_key.to_owned(),
            module_id: "base".to_owned(),
            availability: dashboard_availability(state, "base", 0, 1),
        })
        .collect();
    Ok(NavigationView { items })
}

pub async fn get_dashboard_inner(
    module_id: String,
    page_id: String,
    range: DateRangeView,
    state: &AppState,
) -> Result<DashboardPageView, CommandError> {
    let module_id = parse_module_id(&module_id)?;
    let (start, end) = range
        .parse()
        .map_err(|detail| command_error("invalid_date_range", &detail))?;
    if !BASE_DASHBOARD_PAGES
        .iter()
        .any(|(page, _)| *page == page_id)
    {
        return Err(command_error(
            "invalid_dashboard_page",
            "the dashboard page is not available",
        ));
    }
    let module = state
        .modules()
        .into_iter()
        .find(|module| module.module_id == module_id && module.module_type == ModuleType::Dashboard)
        .ok_or_else(|| {
            command_error(
                "dashboard_not_found",
                "the dashboard module is not installed",
            )
        })?;
    if !module.enabled {
        return Err(command_error(
            "dashboard_disabled",
            "the dashboard module is disabled",
        ));
    }
    let manifest = match &module.manifest {
        ModuleManifest::Dashboard(manifest) => manifest,
        _ => {
            return Err(command_error(
                "dashboard_not_found",
                "the dashboard module is not installed",
            ));
        }
    };
    let requested = DateRange { start, end };
    let (input, observed_days, latest_observation_date) = dashboard_input(
        state,
        &requested,
        page_id.as_str(),
        &manifest.required_capabilities,
    )
    .await?;
    let document = ComponentRuntime::default()
        .invoke_dashboard(
            &module,
            input.clone(),
            mfa_module_host::RuntimeLimits::default(),
        )
        .await
        .map_err(|error| {
            command_error(
                error.code(),
                "the dashboard module could not render this page",
            )
        })?;
    let localization_keys = base_localization_keys();
    mfa_dashboard_host::validate_document(&document, &input, &localization_keys)
        .map_err(|error| command_error(error.code(), "the dashboard document was rejected"))?;
    let expected_days = requested.len_days();
    Ok(DashboardPageView {
        module_id: module_id.to_string(),
        page_id,
        title_key: document.title_key.clone(),
        document,
        availability: dashboard_availability(state, "base", observed_days, expected_days),
        coverage: CoverageView {
            expected_days,
            observed_days,
            ratio: if expected_days == 0 {
                0.0
            } else {
                observed_days as f64 / expected_days as f64
            },
            sufficient: observed_days.saturating_mul(2) >= expected_days,
        },
        freshness: FreshnessView {
            latest_observation_date: latest_observation_date.map(|date| date.to_string()),
            generated_at: Utc::now().to_rfc3339(),
        },
    })
}

pub async fn select_provider_inner(
    capability: String,
    module_id: String,
    state: &AppState,
) -> Result<ProviderView, CommandError> {
    let capability_id = CapabilityId::try_from(capability.clone())
        .map_err(|error| command_error("invalid_capability", &error.to_string()))?;
    let module_id = parse_module_id(&module_id)?;
    let resolution = state
        .select_provider(capability_id, module_id.clone())
        .map_err(CommandError::from)?;
    Ok(ProviderView {
        capability,
        module_id: module_id.to_string(),
        active_providers: resolution
            .active_providers
            .into_iter()
            .map(|(capability, module)| (capability.to_string(), module.to_string()))
            .collect(),
    })
}

pub async fn save_phase_event_inner(
    input: PhaseEventInput,
    state: &AppState,
) -> Result<PhaseEventView, CommandError> {
    if input.event_type.trim().is_empty() {
        return Err(command_error(
            "invalid_phase_event",
            "phase event type is required",
        ));
    }
    let start_date: mfa_contracts::LocalDate = input
        .start_date
        .parse::<mfa_contracts::LocalDate>()
        .map_err(|error| command_error("invalid_phase_event", &error.to_string()))?;
    let end_date: mfa_contracts::LocalDate = input
        .end_date
        .parse::<mfa_contracts::LocalDate>()
        .map_err(|error| command_error("invalid_phase_event", &error.to_string()))?;
    if start_date > end_date {
        return Err(command_error(
            "invalid_phase_event",
            "phase event start must not be after end",
        ));
    }
    let phase_event_id = input
        .phase_event_id
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|error| command_error("invalid_phase_event", &error.to_string()))?
        .unwrap_or_else(Uuid::new_v4);
    let event = PhaseEvent {
        phase_event_id,
        event_type: input.event_type,
        start_date,
        end_date,
        description: input.description,
        exclude_from_tdee: input.exclude_from_tdee,
    };
    let database = state
        .storage_lock()
        .map_err(CommandError::from)?
        .as_ref()
        .map(|storage| storage.database.clone())
        .ok_or_else(|| {
            command_error(
                "workspace_required",
                "configure a workspace before editing phase events",
            )
        })?;
    let saved = if input.phase_event_id.is_some() {
        database
            .execute(UpdatePhaseEvent { phase_event: event })
            .await
    } else {
        database
            .execute(CreatePhaseEvent { phase_event: event })
            .await
    }
    .map_err(|error| command_error("phase_event_save_failed", &error.to_string()))?;
    Ok(saved.into())
}

pub async fn list_phase_events_inner(
    state: &AppState,
) -> Result<Vec<PhaseEventView>, CommandError> {
    let database = state
        .storage_lock()
        .map_err(CommandError::from)?
        .as_ref()
        .map(|storage| storage.database.clone())
        .ok_or_else(|| {
            command_error(
                "workspace_required",
                "configure a workspace before reading phase events",
            )
        })?;
    database
        .execute(ListPhaseEvents)
        .await
        .map(|events| events.into_iter().map(Into::into).collect())
        .map_err(|error| command_error("phase_event_query_failed", &error.to_string()))
}

pub async fn delete_phase_event_inner(
    phase_event_id: String,
    state: &AppState,
) -> Result<(), CommandError> {
    let phase_event_id = Uuid::parse_str(&phase_event_id)
        .map_err(|error| command_error("invalid_phase_event", &error.to_string()))?;
    let database = state
        .storage_lock()
        .map_err(CommandError::from)?
        .as_ref()
        .map(|storage| storage.database.clone())
        .ok_or_else(|| {
            command_error(
                "workspace_required",
                "configure a workspace before editing phase events",
            )
        })?;
    let deleted = database
        .execute(DeletePhaseEvent { phase_event_id })
        .await
        .map_err(|error| command_error("phase_event_delete_failed", &error.to_string()))?;
    if deleted {
        Ok(())
    } else {
        Err(command_error(
            "phase_event_not_found",
            "the phase event no longer exists",
        ))
    }
}

async fn dashboard_input(
    state: &AppState,
    requested: &DateRange,
    page_id: &str,
    requirements: &[mfa_contracts::DashboardRequirement],
) -> Result<(DashboardInput, u64, Option<mfa_contracts::LocalDate>), CommandError> {
    let database = state
        .storage_lock()
        .map_err(CommandError::from)?
        .as_ref()
        .map(|storage| storage.database.clone());
    let providers = state.providers();
    let mut snapshots = Vec::new();
    if let Some(database) = database {
        for module_id in providers.active_providers.values() {
            let key = LogicalSnapshotKey::new(module_id.to_string())
                .map_err(|error| command_error("snapshot_query_failed", &error.to_string()))?;
            snapshots.push(
                database
                    .execute(QuerySnapshot::active(key))
                    .await
                    .map_err(|error| command_error("snapshot_query_failed", &error.to_string()))?,
            );
        }
    }
    let mut weight_observations = Vec::new();
    let mut observed_dates = BTreeSet::new();
    let mut counts = BTreeMap::new();
    for snapshot in &snapshots {
        for record in &snapshot.canonical_records {
            let Ok(observation) = serde_json::from_value::<CanonicalObservation>(record.clone())
            else {
                continue;
            };
            match observation {
                CanonicalObservation::BodyMeasurement(value) => {
                    observed_dates.insert(value.local_date);
                    weight_observations.push(WeightObservation {
                        observation_id: value.body_measurement_id.to_string(),
                        local_date: value.local_date,
                        weight_kg: value.weight_kg,
                    });
                    *counts.entry("body.weight").or_insert(0u64) += 1;
                }
                CanonicalObservation::NutritionItem(value) => {
                    observed_dates.insert(value.local_date);
                    *counts.entry("nutrition.items").or_insert(0) += 1;
                }
                CanonicalObservation::ActivityDay(value) => {
                    observed_dates.insert(value.local_date);
                    *counts.entry("activity.days").or_insert(0) += 1;
                }
                CanonicalObservation::ActivityEvent(value) => {
                    observed_dates.insert(value.local_date);
                    *counts.entry("activity.events").or_insert(0) += 1;
                }
                CanonicalObservation::HeartRate(value) => {
                    observed_dates.insert(value.observed_local_at.0.date().into());
                    *counts.entry("heart_rate.observations").or_insert(0) += 1;
                }
                CanonicalObservation::WorkoutSession(value) => {
                    observed_dates.insert(value.started_local_at.0.date().into());
                    *counts.entry("workouts.sessions").or_insert(0) += 1;
                }
                CanonicalObservation::ExerciseSet(_value) => {
                    *counts.entry("workouts.sets").or_insert(0) += 1;
                }
                CanonicalObservation::PhaseEvent(value) => {
                    observed_dates.insert(value.start_date);
                }
            }
        }
    }
    let context = MetricContext {
        requested: *requested,
        as_of: requested.end,
        snapshot_refs: Vec::new(),
        algorithm_version: mfa_analytics::AlgorithmVersion::new("base-analytics-v1"),
    };
    let weight = weight_analytics(&context, &weight_observations);
    let weight_value = json!({
        "dailyMedianKg": weight.daily_median,
        "trailing7dMeanKg": weight.trailing_7d_mean,
        "slope28d": weight.slope_28d,
    });
    let observed_days = observed_dates
        .iter()
        .filter(|date| requested.contains(**date))
        .count() as u64;
    let mut capabilities = BTreeMap::new();
    for requirement in requirements {
        let name = requirement.capability.to_string();
        capabilities.insert(
            requirement.capability.clone(),
            if name == "body.weight" {
                weight_value.clone()
            } else {
                json!({
                    "recordCount": counts.get(name.as_str()).copied().unwrap_or_default(),
                    "observedDays": observed_days,
                })
            },
        );
    }
    capabilities.insert(
        CapabilityId::try_from("dashboard.page")
            .map_err(|error| command_error("invalid_dashboard_page", &error.to_string()))?,
        json!(page_id),
    );
    Ok((
        DashboardInput {
            capabilities,
            extensions: BTreeMap::new(),
        },
        observed_days,
        observed_dates.iter().next_back().copied(),
    ))
}

fn dashboard_availability(
    state: &AppState,
    module_id: &str,
    observed_days: u64,
    expected_days: u64,
) -> AvailabilityView {
    let Some(module) = state
        .modules()
        .into_iter()
        .find(|module| module.module_id.as_str() == module_id)
    else {
        return AvailabilityView {
            state: AvailabilityState::MissingDependency,
            reason_key: "dashboard.module_missing".to_owned(),
            required_capabilities: Vec::new(),
            required_dependencies: vec![module_id.to_owned()],
        };
    };
    let state_value = if !module.enabled {
        AvailabilityState::DisabledByUser
    } else if observed_days == 0 {
        AvailabilityState::WaitingForData
    } else if observed_days.saturating_mul(2) < expected_days {
        AvailabilityState::InsufficientCoverage
    } else {
        AvailabilityState::Ready
    };
    AvailabilityView {
        state: state_value,
        reason_key: "dashboard.availability".to_owned(),
        required_capabilities: Vec::new(),
        required_dependencies: Vec::new(),
    }
}

fn base_localization_keys() -> BTreeSet<String> {
    [
        "overview.title",
        "overview.body_weight",
        "overview.nutrition",
        "overview.quality",
        "overview.quality_ready",
        "overview.quality_missing",
        "overview.trend",
        "body.title",
        "body.raw_weight",
        "body.daily_median",
        "body.trailing_mean",
        "body.slope",
        "body.trend",
        "body.missing",
        "body.ready",
        "nutrition.title",
        "nutrition.calories",
        "nutrition.protein",
        "nutrition.fiber",
        "nutrition.quality",
        "nutrition.trend",
        "nutrition.missing",
        "nutrition.ready",
        "activity.title",
        "activity.steps",
        "activity.duration",
        "activity.distance",
        "activity.heart_rate",
        "activity.trend",
        "activity.missing",
        "activity.ready",
        "strength.title",
        "strength.sessions",
        "strength.sets",
        "strength.e1rm",
        "strength.duration",
        "strength.trend",
        "strength.missing",
        "strength.ready",
        "sources.title",
        "sources.providers",
        "sources.coverage",
        "sources.quality",
        "sources.ready",
        "sources.missing",
    ]
    .into_iter()
    .map(|key| format!("base.{key}"))
    .collect()
}

fn rollback_uninstall(
    state: &AppState,
    installer: &PackageInstaller,
    transaction: &mut UninstallTransaction,
    previous_settings: &mfa_config::AppSettings,
) -> Option<CommandError> {
    let mut failures = Vec::new();
    if let Err(error) = installer.rollback_uninstall(transaction) {
        failures.push(error.to_string());
    }
    if let Err(error) = state.save_settings(previous_settings.clone()) {
        failures.push(error.to_string());
    }
    if let Err(error) = state.refresh_registry() {
        failures.push(error.to_string());
    }
    (!failures.is_empty()).then(|| command_error("uninstall_rollback_failed", &failures.join("; ")))
}

fn parse_module_id(value: &str) -> Result<ModuleId, CommandError> {
    ModuleId::try_from(value.to_owned())
        .map_err(|error| command_error("invalid_module_id", &error.to_string()))
}

fn command_error(code: &str, detail: &str) -> CommandError {
    let message = match code {
        "workspace_required" => "Configure a workspace before enabling this source.".to_owned(),
        "module_must_be_disabled" => "Disable this module before uninstalling it.".to_owned(),
        "module_update_unavailable" => "No update is available for this module.".to_owned(),
        "module_not_found" => "This module is no longer installed.".to_owned(),
        "inbox_unavailable" => "The selected inbox is not available.".to_owned(),
        _ => detail.to_owned(),
    };
    CommandError {
        code: code.to_owned(),
        message,
    }
}

fn module_view_from_manifest(manifest: &ModuleManifest, enabled: bool) -> ModuleView {
    let (module_id, module_type, version, namespace, capabilities) = match manifest {
        ModuleManifest::Source(manifest) => (
            &manifest.module_id,
            manifest.module_type,
            &manifest.module_version,
            &manifest.localization_namespace,
            manifest
                .provided_capabilities
                .iter()
                .map(ToString::to_string)
                .collect(),
        ),
        ModuleManifest::Dashboard(manifest) => (
            &manifest.module_id,
            manifest.module_type,
            &manifest.module_version,
            &manifest.localization_namespace,
            Vec::new(),
        ),
        ModuleManifest::Locale(manifest) => (
            &manifest.module_id,
            manifest.module_type,
            &manifest.module_version,
            &manifest.localization_namespace,
            Vec::new(),
        ),
    };
    ModuleView {
        id: module_id.to_string(),
        module_type: match module_type {
            ModuleType::Source => "source",
            ModuleType::Dashboard => "dashboard",
            ModuleType::Locale => "locale",
        }
        .to_owned(),
        version: version.to_string(),
        enabled,
        localization_namespace: namespace.clone(),
        display_name: display_name(module_id),
        provided_capabilities: capabilities,
    }
}

fn manifest_version(manifest: &ModuleManifest) -> &mfa_contracts::ContractVersion {
    match manifest {
        ModuleManifest::Source(manifest) => &manifest.module_version,
        ModuleManifest::Dashboard(manifest) => &manifest.module_version,
        ModuleManifest::Locale(manifest) => &manifest.module_version,
    }
}

fn catalog_install_state(
    inspect_error: Option<&str>,
    has_available_version: bool,
    enabled: bool,
) -> &'static str {
    match inspect_error {
        Some(code) => catalog_install_state_for_inspect_error(code),
        None if has_available_version => "update",
        None if enabled => "enabled",
        None => "disabled",
    }
}

fn catalog_install_state_for_inspect_error(code: &str) -> &'static str {
    if code.starts_with("incompatible_") {
        "incompatible"
    } else {
        "error"
    }
}

#[cfg(test)]
mod tests {
    use super::{catalog_install_state, catalog_install_state_for_inspect_error};

    #[test]
    fn incompatible_package_errors_have_an_explicit_catalog_state() {
        assert_eq!(
            catalog_install_state(Some("incompatible_source_api"), false, true),
            "incompatible"
        );
        assert_eq!(
            catalog_install_state(Some("incompatible_package_format"), false, false),
            "incompatible"
        );
    }

    #[test]
    fn uninstalled_incompatible_inspect_errors_are_not_generic_errors() {
        assert_eq!(
            catalog_install_state_for_inspect_error("incompatible_source_api"),
            "incompatible"
        );
        assert_eq!(
            catalog_install_state_for_inspect_error("incompatible_app_version"),
            "incompatible"
        );
    }
}

pub async fn refresh_now_inner(state: &AppState) -> Result<ScanTicketView, CommandError> {
    let coordinators = storage_coordinators(state)?;
    let mut first_ticket = None;
    let mut coalesced_requests: u32 = 0;
    for coordinator in coordinators {
        let ticket = coordinator
            .request_scan(now_request(mfa_ingestion::ScanReason::Manual))
            .await
            .map_err(|error| CommandError {
                code: "refresh_failed".to_owned(),
                message: error.to_string(),
            })?;
        if first_ticket.is_none() {
            first_ticket = Some(ticket);
        } else {
            coalesced_requests = coalesced_requests.saturating_add(ticket.coalesced_requests);
        }
    }
    let ticket = first_ticket.ok_or_else(|| CommandError {
        code: "source_module_unavailable".to_owned(),
        message: "no enabled source module is configured".to_owned(),
    })?;
    Ok(ScanTicketView {
        scan_id: ticket.scan_id.to_string(),
        coalesced_requests: ticket.coalesced_requests.saturating_add(coalesced_requests),
    })
}

pub async fn get_ingestion_status_inner(
    state: &AppState,
) -> Result<IngestionStatusView, CommandError> {
    let (database, coordinators, gate, queue_capacity) = {
        let storage = state.storage_lock().map_err(CommandError::from)?;
        let Some(storage) = storage.as_ref() else {
            return Ok(IngestionStatusView {
                health: health_view(HealthSnapshot::from_counts(0, 0, 0, 0)),
                queue_capacity: 0,
                recovery_mode: "unconfigured".to_owned(),
                configured: false,
            });
        };
        (
            storage.database.clone(),
            storage.coordinators.values().cloned().collect::<Vec<_>>(),
            storage.recovery_gate.clone(),
            storage.database.queue_capacity(),
        )
    };
    database
        .execute(HealthCheck)
        .await
        .map_err(|error| CommandError {
            code: "database_unavailable".to_owned(),
            message: error.to_string(),
        })?;
    let mut health = HealthSnapshot::from_counts(0, 0, 0, 0);
    for coordinator in coordinators {
        let snapshot = coordinator.health_snapshot();
        health = HealthSnapshot::from_counts(
            health.working_jobs + snapshot.working_jobs,
            health.waiting_assets + snapshot.waiting_assets,
            health.attention_items + snapshot.attention_items,
            health.critical_items + snapshot.critical_items,
        );
    }
    Ok(IngestionStatusView {
        health: health_view(health),
        queue_capacity,
        recovery_mode: match gate.mode() {
            RecoveryMode::Recovery => "recovery",
            RecoveryMode::Normal => "normal",
        }
        .to_owned(),
        configured: true,
    })
}

pub async fn list_quality_items_inner(
    state: &AppState,
) -> Result<Vec<QualityItemView>, CommandError> {
    let database = {
        let storage = state.storage_lock().map_err(CommandError::from)?;
        storage.as_ref().map(|storage| storage.database.clone())
    };
    let Some(database) = database else {
        return Ok(Vec::new());
    };
    let response = database
        .execute(ListQualityItems)
        .await
        .map_err(|error| CommandError {
            code: "database_unavailable".to_owned(),
            message: error.to_string(),
        })?;
    Ok(response
        .items
        .into_iter()
        .map(|item| QualityItemView {
            id: item.data_quality_item_id,
            item_type: item.item_type,
            severity: item.severity,
            message: item.message,
            status: item.status,
            asset_id: item.source_asset_id.map(|asset_id| asset_id.to_string()),
        })
        .collect())
}

pub async fn retry_asset_inner(
    state: &AppState,
    asset_id: String,
) -> Result<AttemptView, CommandError> {
    let asset_id = Uuid::parse_str(&asset_id).map_err(|error| CommandError {
        code: "invalid_asset_id".to_owned(),
        message: error.to_string(),
    })?;
    let coordinators = storage_coordinators(state)?;
    let mut last_error = None;
    let mut retry = None;
    for coordinator in coordinators {
        match coordinator.retry_asset(asset_id).await {
            Ok(result) => {
                retry = Some(result);
                break;
            }
            Err(error) if is_archive_asset_not_found(&error) => last_error = Some(error),
            Err(error) => {
                return Err(CommandError {
                    code: "retry_failed".to_owned(),
                    message: error.to_string(),
                });
            }
        }
    }
    let retry = retry.ok_or_else(|| CommandError {
        code: "retry_failed".to_owned(),
        message: last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "no enabled source module is configured".to_owned()),
    })?;
    Ok(AttemptView {
        asset_id: retry.asset_id.to_string(),
        attempt_id: Some(retry.attempt_id.to_string()),
        status: "retry_queued".to_owned(),
        error_code: None,
    })
}

fn storage_coordinators(state: &AppState) -> Result<Vec<IngestionCoordinator>, CommandError> {
    let storage = state.storage_lock().map_err(CommandError::from)?;
    let coordinators = storage
        .as_ref()
        .map(|storage| storage.coordinators.values().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    if coordinators.is_empty() {
        Err(CommandError {
            code: "source_module_unavailable".to_owned(),
            message: "no enabled source module is configured".to_owned(),
        })
    } else {
        Ok(coordinators)
    }
}

fn is_archive_asset_not_found(error: &mfa_ingestion::IngestionError) -> bool {
    matches!(
        error,
        mfa_ingestion::IngestionError::AssetFailure { code, .. }
            if code == "archive_asset_not_found"
    )
}

fn workspace_view(state: &AppState) -> Result<WorkspaceView, CommandError> {
    let settings = state.settings();
    let app_paths = state.configured_app_paths();
    let storage = state.storage_lock().map_err(CommandError::from)?;
    let workspace_root = storage
        .as_ref()
        .map(|storage| storage.workspace.root.clone())
        .or(settings.workspace_root)
        .unwrap_or_default();
    let workspace = mfa_config::WorkspacePaths::with_source_inbox_roots(
        &workspace_root,
        settings.source_inbox_roots.clone(),
    );
    let source_paths = state
        .modules()
        .iter()
        .filter(|module| module.enabled && module.module_type == mfa_contracts::ModuleType::Source)
        .map(|module| SourcePathView {
            module_id: module.module_id.to_string(),
            inbox_path: workspace
                .source_inbox(&module.module_id)
                .to_string_lossy()
                .into_owned(),
            archive_path: workspace
                .source_archive(&module.module_id)
                .to_string_lossy()
                .into_owned(),
        })
        .collect();
    Ok(WorkspaceView {
        workspace_root: workspace_root.to_string_lossy().into_owned(),
        app_data_root: app_paths.app_data.to_string_lossy().into_owned(),
        database_path: app_paths.database.to_string_lossy().into_owned(),
        recovery_path: app_paths.recovery.to_string_lossy().into_owned(),
        backup_path: app_paths.recovery.to_string_lossy().into_owned(),
        archive_root: workspace
            .root
            .join("archive")
            .to_string_lossy()
            .into_owned(),
        source_paths,
    })
}

fn health_view(snapshot: HealthSnapshot) -> HealthView {
    HealthView {
        state: match snapshot.state {
            HealthState::Healthy => "healthy",
            HealthState::Working => "working",
            HealthState::Attention => "attention",
            HealthState::Blocked => "blocked",
        }
        .to_owned(),
        working_jobs: snapshot.working_jobs,
        waiting_assets: snapshot.waiting_assets,
        attention_items: snapshot.attention_items,
        critical_items: snapshot.critical_items,
    }
}

fn localization_namespace(manifest: &ModuleManifest) -> String {
    match manifest {
        ModuleManifest::Source(manifest) => manifest.localization_namespace.clone(),
        ModuleManifest::Dashboard(manifest) => manifest.localization_namespace.clone(),
        ModuleManifest::Locale(manifest) => manifest.localization_namespace.clone(),
    }
}

#[tauri::command]
pub async fn get_bootstrap_state(
    state: tauri::State<'_, AppState>,
) -> Result<BootstrapState, CommandError> {
    get_bootstrap_state_inner(&state).await
}

#[tauri::command]
pub async fn list_modules(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ModuleView>, CommandError> {
    list_modules_inner(&state).await
}

#[tauri::command]
pub async fn list_module_catalog(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ModuleCatalogEntryView>, CommandError> {
    list_module_catalog_inner(&state).await
}

#[tauri::command]
pub async fn choose_workspace_root(
    state: tauri::State<'_, AppState>,
) -> Result<Option<WorkspaceView>, CommandError> {
    choose_workspace_root_inner(&state, &NativeDialogPort).await
}

#[tauri::command]
pub async fn get_workspace_view(
    state: tauri::State<'_, AppState>,
) -> Result<WorkspaceView, CommandError> {
    get_workspace_view_inner(&state).await
}

#[tauri::command]
pub async fn choose_and_install_module(
    state: tauri::State<'_, AppState>,
) -> Result<Option<ModuleView>, CommandError> {
    choose_and_install_module_inner(&state, &NativeDialogPort).await
}

#[tauri::command]
pub async fn choose_source_inbox(
    module_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<WorkspaceView>, CommandError> {
    choose_source_inbox_inner(&state, module_id, &NativeDialogPort).await
}

#[tauri::command]
pub async fn set_module_enabled(
    module_id: String,
    enabled: bool,
    state: tauri::State<'_, AppState>,
) -> Result<ModuleView, CommandError> {
    set_module_enabled_inner(&state, module_id, enabled).await
}

#[tauri::command]
pub async fn update_module(
    module_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ModuleView, CommandError> {
    update_module_inner(&state, module_id).await
}

#[tauri::command]
pub async fn uninstall_module(
    module_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError> {
    uninstall_module_inner(&state, module_id).await
}

#[tauri::command]
pub async fn select_module_provider(
    capability: String,
    module_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ProviderSelectionView, CommandError> {
    select_module_provider_inner(&state, capability, module_id).await
}

#[tauri::command]
pub async fn get_navigation(
    state: tauri::State<'_, AppState>,
) -> Result<NavigationView, CommandError> {
    get_navigation_inner(&state).await
}

#[tauri::command]
pub async fn get_dashboard(
    module_id: String,
    page_id: String,
    range: DateRangeView,
    state: tauri::State<'_, AppState>,
) -> Result<DashboardPageView, CommandError> {
    get_dashboard_inner(module_id, page_id, range, &state).await
}

#[tauri::command]
pub async fn select_provider(
    capability: String,
    module_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<ProviderView, CommandError> {
    select_provider_inner(capability, module_id, &state).await
}

#[tauri::command]
pub async fn save_phase_event(
    input: PhaseEventInput,
    state: tauri::State<'_, AppState>,
) -> Result<PhaseEventView, CommandError> {
    save_phase_event_inner(input, &state).await
}

#[tauri::command]
pub async fn refresh_now(
    state: tauri::State<'_, AppState>,
) -> Result<ScanTicketView, CommandError> {
    refresh_now_inner(&state).await
}

#[tauri::command]
pub async fn get_ingestion_status(
    state: tauri::State<'_, AppState>,
) -> Result<IngestionStatusView, CommandError> {
    get_ingestion_status_inner(&state).await
}

#[tauri::command]
pub async fn list_quality_items(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<QualityItemView>, CommandError> {
    list_quality_items_inner(&state).await
}

#[tauri::command]
pub async fn retry_asset(
    asset_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<AttemptView, CommandError> {
    retry_asset_inner(&state, asset_id).await
}
