use crate::error::DatabaseError;
use crate::provenance::{
    DataQualityItem, ExtensionContractRegistration, ExtensionContractRegistrationResult,
    SnapshotCommitResult, ValidatedSnapshotBatch,
};
pub use crate::views::{QueryView, ViewResponse};
use mfa_contracts::{ModuleId, UtcInstant};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AssetRegistration {
    pub asset_id: Uuid,
    pub source_module_id: ModuleId,
    pub asset_type: String,
    pub original_filename: String,
    pub archive_path: String,
    pub byte_sha256: String,
    pub file_size: u64,
    pub received_at: UtcInstant,
}

#[derive(Debug, Clone)]
pub struct RegisterAsset {
    pub asset: AssetRegistration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterAssetResult {
    pub asset_id: Uuid,
    pub inserted: bool,
    pub needs_processing: bool,
}

#[derive(Debug, Clone)]
pub struct RegisterReceipt {
    pub receipt_id: Uuid,
    pub source_module_id: ModuleId,
    pub inbox_path: String,
    pub original_filename: String,
    pub discovered_at: UtcInstant,
    pub asset_id: Option<Uuid>,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterReceiptResult {
    pub receipt_id: Uuid,
    pub asset_id: Option<Uuid>,
    pub outcome: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HealthCheck;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub actor_thread_id: String,
    pub schema_version: u32,
}

#[derive(Debug, Clone)]
pub struct StartAttempt {
    pub attempt_id: Uuid,
    pub asset_id: Uuid,
    pub source_module_id: ModuleId,
    pub source_module_version: String,
    pub source_module_package_sha256: String,
    pub source_api_version: String,
    pub mapping_version: String,
    pub schema_fingerprint: String,
    pub logical_snapshot_key: String,
    pub started_at: UtcInstant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartAttemptResult {
    pub attempt_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct FailAttempt {
    pub attempt_id: Uuid,
    pub finished_at: UtcInstant,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub record_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailAttemptResult {
    pub attempt_id: Uuid,
}

#[derive(Debug, Clone, Default)]
pub struct MarkInterrupted;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkInterruptedResult {
    pub count: u64,
}

#[derive(Debug, Clone)]
pub struct ReconcileArchive {
    pub source_module_id: ModuleId,
    pub archive_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileArchiveResult {
    pub registered_assets: u64,
    pub missing_assets: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveAssetRecord {
    pub asset_id: Uuid,
    pub source_module_id: ModuleId,
    pub asset_type: String,
    pub original_filename: String,
    pub archive_path: String,
    pub byte_sha256: String,
    pub file_size: u64,
    pub received_at: UtcInstant,
}

#[derive(Debug, Clone)]
pub struct ReconcileArchiveInventory {
    pub source_module_id: ModuleId,
    pub assets: Vec<ArchiveAssetRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileArchiveInventoryResult {
    pub registered_assets: u64,
    pub missing_assets: u64,
    pub missing_asset_ids: Vec<Uuid>,
    pub assets_to_ingest: Vec<ArchiveAssetRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct ListQualityItems;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListQualityItemsResult {
    pub items: Vec<DataQualityItem>,
}

#[derive(Debug, Clone)]
pub struct CommitSnapshot(pub Arc<ValidatedSnapshotBatch>);

#[derive(Debug)]
pub enum DatabaseCommand {
    HealthCheck(oneshot::Sender<Result<HealthCheckResult, DatabaseError>>),
    RegisterReceipt(
        RegisterReceipt,
        oneshot::Sender<Result<RegisterReceiptResult, DatabaseError>>,
    ),
    RegisterAsset(
        RegisterAsset,
        oneshot::Sender<Result<RegisterAssetResult, DatabaseError>>,
    ),
    StartAttempt(
        StartAttempt,
        oneshot::Sender<Result<StartAttemptResult, DatabaseError>>,
    ),
    FailAttempt(
        FailAttempt,
        oneshot::Sender<Result<FailAttemptResult, DatabaseError>>,
    ),
    MarkInterrupted(oneshot::Sender<Result<MarkInterruptedResult, DatabaseError>>),
    ReconcileArchive(
        ReconcileArchive,
        oneshot::Sender<Result<ReconcileArchiveResult, DatabaseError>>,
    ),
    ReconcileArchiveInventory(
        ReconcileArchiveInventory,
        oneshot::Sender<Result<ReconcileArchiveInventoryResult, DatabaseError>>,
    ),
    ListQualityItems(oneshot::Sender<Result<ListQualityItemsResult, DatabaseError>>),
    QueryView(
        QueryView,
        oneshot::Sender<Result<ViewResponse, DatabaseError>>,
    ),
    CommitSnapshot(
        CommitSnapshot,
        oneshot::Sender<Result<SnapshotCommitResult, DatabaseError>>,
    ),
    RegisterExtensionContract(
        ExtensionContractRegistration,
        oneshot::Sender<Result<ExtensionContractRegistrationResult, DatabaseError>>,
    ),
    Shutdown(oneshot::Sender<Result<(), DatabaseError>>),
}

pub trait IntoDatabaseCommand<R>: Send + 'static {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<R, DatabaseError>>,
    ) -> DatabaseCommand;
}

impl IntoDatabaseCommand<HealthCheckResult> for HealthCheck {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<HealthCheckResult, DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::HealthCheck(response)
    }
}

impl IntoDatabaseCommand<RegisterAssetResult> for RegisterAsset {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<RegisterAssetResult, DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::RegisterAsset(self, response)
    }
}

impl IntoDatabaseCommand<RegisterReceiptResult> for RegisterReceipt {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<RegisterReceiptResult, DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::RegisterReceipt(self, response)
    }
}

impl IntoDatabaseCommand<StartAttemptResult> for StartAttempt {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<StartAttemptResult, DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::StartAttempt(self, response)
    }
}

impl IntoDatabaseCommand<FailAttemptResult> for FailAttempt {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<FailAttemptResult, DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::FailAttempt(self, response)
    }
}

impl IntoDatabaseCommand<MarkInterruptedResult> for MarkInterrupted {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<MarkInterruptedResult, DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::MarkInterrupted(response)
    }
}

impl IntoDatabaseCommand<ReconcileArchiveResult> for ReconcileArchive {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<ReconcileArchiveResult, DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::ReconcileArchive(self, response)
    }
}

impl IntoDatabaseCommand<ReconcileArchiveInventoryResult> for ReconcileArchiveInventory {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<ReconcileArchiveInventoryResult, DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::ReconcileArchiveInventory(self, response)
    }
}

impl IntoDatabaseCommand<ListQualityItemsResult> for ListQualityItems {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<ListQualityItemsResult, DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::ListQualityItems(response)
    }
}

impl IntoDatabaseCommand<ViewResponse> for QueryView {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<ViewResponse, DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::QueryView(self, response)
    }
}

impl IntoDatabaseCommand<SnapshotCommitResult> for CommitSnapshot {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<SnapshotCommitResult, DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::CommitSnapshot(self, response)
    }
}

impl IntoDatabaseCommand<ExtensionContractRegistrationResult> for ExtensionContractRegistration {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<ExtensionContractRegistrationResult, DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::RegisterExtensionContract(self, response)
    }
}

impl IntoDatabaseCommand<()> for Shutdown {
    fn into_database_command(
        self,
        response: oneshot::Sender<Result<(), DatabaseError>>,
    ) -> DatabaseCommand {
        DatabaseCommand::Shutdown(response)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Shutdown;
