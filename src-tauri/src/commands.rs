use crate::dialogs::{DialogPort, NativeDialogPort};
use crate::state::{AppState, AppStateError};
use mfa_contracts::{CapabilityId, ModuleId, ModuleManifest, ModuleType};
use mfa_db::{HealthCheck, ListQualityItems};
use mfa_ingestion::{HealthSnapshot, HealthState, IngestionCoordinator, RecoveryMode, now_request};
use mfa_module_host::{InstalledModule, PackageInstaller, UninstallTransaction};
use serde::Serialize;
use std::collections::BTreeMap;
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
    if let Err(error) = state.refresh_registry() {
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
