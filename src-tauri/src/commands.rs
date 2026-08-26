use crate::state::{AppState, AppStateError};
use mfa_contracts::ModuleManifest;
use mfa_db::{HealthCheck, ListQualityItems};
use mfa_ingestion::{HealthSnapshot, HealthState, IngestionCoordinator, RecoveryMode, now_request};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
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
    Ok(state
        .modules()
        .iter()
        .map(|module| ModuleView {
            id: module.module_id.to_string(),
            module_type: match module.module_type {
                mfa_contracts::ModuleType::Source => "source",
                mfa_contracts::ModuleType::Dashboard => "dashboard",
                mfa_contracts::ModuleType::Locale => "locale",
            }
            .to_owned(),
            version: module.module_version.to_string(),
            enabled: module.enabled,
            localization_namespace: localization_namespace(&module.manifest),
        })
        .collect())
}

pub async fn set_workspace_root_inner(
    state: &AppState,
    path: String,
) -> Result<WorkspaceView, CommandError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(CommandError {
            code: "invalid_workspace_root".to_owned(),
            message: "workspace root must not be empty".to_owned(),
        });
    }
    state
        .configure_workspace(PathBuf::from(trimmed))
        .await
        .map_err(CommandError::from)?;
    workspace_view(state)
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
            storage.coordinators.clone(),
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
        .map(|storage| storage.coordinators.clone())
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
    let workspace = mfa_config::WorkspacePaths::new(&workspace_root);
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
pub async fn set_workspace_root(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<WorkspaceView, CommandError> {
    set_workspace_root_inner(&state, path).await
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
