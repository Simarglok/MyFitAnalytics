use crate::error::IngestionError;
use crate::events::CoreEvent;
use crate::queue::{BoxFuture, ScanExecutor, ScanQueue, ScanReport};

use mfa_archive::{
    ArchiveCoordinator, ArchiveDisposition, ScanRequest, StableCandidate, StableScanner,
};
use mfa_config::WorkspacePaths;
use mfa_contracts::{
    AssetMetadata, AssetReadError, CanonicalObservation, ModuleId, ModuleManifest, ReadOnlyAsset,
    SourceBatch, UtcInstant,
};
use mfa_db::{
    AssetRegistration, AttemptIdentity, CommitSnapshot, DataQualityItem, DatabaseService,
    ExtensionContractRegistration, ExtensionRecord, FailAttempt, LineageLink, LogicalSnapshotKey,
    RegisterAsset, RegisterReceipt, SourceRecord, ValidatedSnapshotBatch, validation,
};
use mfa_module_host::{InstalledModule, RuntimeError, RuntimeLimits};
use serde_json::json;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use uuid::Uuid;

pub trait SourceInvoker: Send + Sync + 'static {
    fn invoke_source<'a>(
        &'a self,
        module: &'a InstalledModule,
        asset: Arc<dyn ReadOnlyAsset>,
        limits: RuntimeLimits,
    ) -> BoxFuture<'a, Result<SourceBatch, RuntimeError>>;
}

impl SourceInvoker for mfa_module_host::ComponentRuntime {
    fn invoke_source<'a>(
        &'a self,
        module: &'a InstalledModule,
        asset: Arc<dyn ReadOnlyAsset>,
        limits: RuntimeLimits,
    ) -> BoxFuture<'a, Result<SourceBatch, RuntimeError>> {
        Box::pin(mfa_module_host::ComponentRuntime::invoke_source(
            self, module, asset, limits,
        ))
    }
}

pub struct IngestionDependencies {
    pub workspace: WorkspacePaths,
    pub source_module: InstalledModule,
    pub archive: ArchiveCoordinator,
    pub database: DatabaseService,
    pub runtime: Arc<dyn SourceInvoker>,
    pub limits: RuntimeLimits,
    pub queue_capacity: usize,
}

impl Clone for IngestionDependencies {
    fn clone(&self) -> Self {
        Self {
            workspace: self.workspace.clone(),
            source_module: self.source_module.clone(),
            archive: self.archive.clone(),
            database: self.database.clone(),
            runtime: Arc::clone(&self.runtime),
            limits: self.limits,
            queue_capacity: self.queue_capacity,
        }
    }
}

#[derive(Clone)]
struct PipelineExecutor {
    state: Arc<Mutex<PipelineState>>,
}

struct PipelineState {
    dependencies: IngestionDependencies,
    scanner: StableScanner,
    events: broadcast::Sender<CoreEvent>,
    runtime_invocations: u64,
    duplicate_receipts: u64,
    completed_assets: u64,
    failed_before_archive_receipts: u64,
    fail_next_asset: bool,
}

pub struct IngestionCoordinator {
    queue: ScanQueue,
    state: Arc<Mutex<PipelineState>>,
    events: broadcast::Sender<CoreEvent>,
}

impl IngestionCoordinator {
    pub fn start(dependencies: IngestionDependencies) -> Result<Self, IngestionError> {
        if !dependencies.source_module.enabled {
            return Err(IngestionError::AssetFailure {
                code: "source_module_disabled".to_owned(),
                detail: dependencies.source_module.module_id.to_string(),
            });
        }
        if dependencies.queue_capacity == 0 {
            return Err(IngestionError::QueueClosed);
        }
        let (events, _) = broadcast::channel(128);
        let state = Arc::new(Mutex::new(PipelineState {
            dependencies,
            scanner: StableScanner::new(),
            events: events.clone(),
            runtime_invocations: 0,
            duplicate_receipts: 0,
            completed_assets: 0,
            failed_before_archive_receipts: 0,
            fail_next_asset: false,
        }));
        let executor = PipelineExecutor {
            state: Arc::clone(&state),
        };
        let capacity = state.lock().unwrap().dependencies.queue_capacity;
        let queue = ScanQueue::start(executor, capacity);
        Ok(Self {
            queue,
            state,
            events,
        })
    }

    pub async fn request_scan(
        &self,
        request: ScanRequest,
    ) -> Result<crate::queue::ScanTicket, IngestionError> {
        self.queue.request_scan(request).await
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CoreEvent> {
        self.events.subscribe()
    }

    pub async fn shutdown(self) -> Result<(), IngestionError> {
        self.queue.shutdown().await
    }

    pub fn runtime_invocations(&self) -> u64 {
        self.state.lock().unwrap().runtime_invocations
    }

    pub fn duplicate_receipts(&self) -> u64 {
        self.state.lock().unwrap().duplicate_receipts
    }

    pub fn completed_assets(&self) -> u64 {
        self.state.lock().unwrap().completed_assets
    }

    pub fn failed_before_archive_receipts(&self) -> u64 {
        self.state.lock().unwrap().failed_before_archive_receipts
    }

    pub fn set_runtime_failure_for_next_asset(&self) {
        self.state.lock().unwrap().fail_next_asset = true;
    }
}

impl ScanExecutor for PipelineExecutor {
    fn execute(&self, request: ScanRequest) -> BoxFuture<'_, Result<ScanReport, IngestionError>> {
        let state = Arc::clone(&self.state);
        Box::pin(async move { run_scan(state, request).await })
    }
}

async fn run_scan(
    state: Arc<Mutex<PipelineState>>,
    _request: ScanRequest,
) -> Result<ScanReport, IngestionError> {
    let candidates = {
        let mut state_guard = state.lock().map_err(|_| IngestionError::CriticalFailure {
            code: "pipeline_state_poisoned".to_owned(),
            detail: "pipeline state mutex poisoned".to_owned(),
        })?;
        let inbox = state_guard
            .dependencies
            .workspace
            .source_inbox(&state_guard.dependencies.source_module.module_id);
        state_guard
            .scanner
            .scan(&inbox)
            .map_err(|error| IngestionError::ScanFailed {
                detail: error.to_string(),
            })?
    };
    let mut report = ScanReport::default();
    for candidate in candidates {
        match process_candidate(Arc::clone(&state), candidate).await {
            Ok(AssetOutcome::Completed) => report.completed_assets += 1,
            Ok(AssetOutcome::Duplicate) => report.duplicate_assets += 1,
            Err(error) => {
                report.failed_assets += 1;
                let _ = error;
            }
        }
    }
    Ok(report)
}

enum AssetOutcome {
    Completed,
    Duplicate,
}

async fn process_candidate(
    state: Arc<Mutex<PipelineState>>,
    candidate: StableCandidate,
) -> Result<AssetOutcome, IngestionError> {
    emit(&state, CoreEvent::Stage("stable"));
    let source_path = candidate.path.clone();
    let inbox_filename = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unnamed-asset")
        .to_owned();
    let (dependencies, received_at, inject_failure) = {
        let mut guard = state.lock().map_err(|_| poisoned())?;
        let inject_failure = std::mem::take(&mut guard.fail_next_asset);
        (
            guard.dependencies.clone(),
            UtcInstant::from(chrono::Utc::now()),
            inject_failure,
        )
    };
    let archived = match dependencies.archive.archive(candidate, received_at.clone()) {
        Ok(archived) => archived,
        Err(error) => {
            let detail = error.to_string();
            let receipt = dependencies
                .database
                .execute(RegisterReceipt {
                    receipt_id: Uuid::new_v4(),
                    source_module_id: dependencies.source_module.module_id.clone(),
                    inbox_path: source_path.to_string_lossy().into_owned(),
                    original_filename: inbox_filename,
                    discovered_at: received_at,
                    asset_id: None,
                    outcome: "failed_before_archive".to_owned(),
                })
                .await;
            if receipt.is_ok() {
                let mut guard = state.lock().map_err(|_| poisoned())?;
                guard.failed_before_archive_receipts += 1;
            }
            return Err(IngestionError::TransientFailure {
                code: "archive_failed".to_owned(),
                detail,
            });
        }
    };
    emit(&state, CoreEvent::Stage("archive_verified"));
    fs::remove_file(&source_path).map_err(|error| IngestionError::TransientFailure {
        code: "inbox_delete_failed".to_owned(),
        detail: error.to_string(),
    })?;
    emit(&state, CoreEvent::Stage("inbox_removed"));

    let registered_asset = dependencies
        .database
        .execute(RegisterAsset {
            asset: AssetRegistration {
                asset_id: archived.asset_id,
                source_module_id: archived.source_module_id.clone(),
                asset_type: "source_export".to_owned(),
                original_filename: archived.original_filename.clone(),
                archive_path: archived.archive_path.to_string_lossy().into_owned(),
                byte_sha256: archived.byte_sha256.clone(),
                file_size: archived.file_size,
                received_at: archived.received_at.clone(),
            },
        })
        .await
        .map_err(database_error)?;
    let duplicate = archived.disposition == ArchiveDisposition::ExistingExactDuplicate
        || !registered_asset.inserted;
    dependencies
        .database
        .execute(RegisterReceipt {
            receipt_id: Uuid::new_v4(),
            source_module_id: archived.source_module_id.clone(),
            inbox_path: source_path.to_string_lossy().into_owned(),
            original_filename: inbox_filename,
            discovered_at: received_at.clone(),
            asset_id: Some(registered_asset.asset_id),
            outcome: if duplicate {
                "duplicate".to_owned()
            } else {
                "accepted".to_owned()
            },
        })
        .await
        .map_err(database_error)?;
    emit(&state, CoreEvent::Stage("receipt_registered"));
    if duplicate {
        let mut guard = state.lock().map_err(|_| poisoned())?;
        guard.duplicate_receipts += 1;
        return Ok(AssetOutcome::Duplicate);
    }

    let (module_version, source_api_version, mapping_version) =
        source_metadata(&dependencies.source_module)?;
    let logical_key = infer_logical_key(
        &dependencies.source_module.module_id,
        &dependencies.source_module,
        None,
    )?;
    let attempt = AttemptIdentity {
        attempt_id: Uuid::new_v4(),
        asset_id: registered_asset.asset_id,
        source_module_id: archived.source_module_id.clone(),
        source_module_version: module_version,
        source_module_package_sha256: dependencies.source_module.package_hash.clone(),
        source_api_version,
        mapping_version,
        schema_fingerprint: format!("module:{}", dependencies.source_module.package_hash),
        logical_snapshot_key: logical_key,
        started_at: received_at.clone(),
    };
    dependencies
        .database
        .execute(attempt.start_command())
        .await
        .map_err(database_error)?;
    emit(&state, CoreEvent::Stage("attempt_started"));

    if inject_failure {
        let _ = fail_attempt(
            &dependencies.database,
            attempt.attempt_id,
            "module_guest_error",
            "synthetic parse failure",
        )
        .await;
        emit(&state, CoreEvent::QualityChanged);
        return Err(IngestionError::AssetFailure {
            code: "module_guest_error".to_owned(),
            detail: "synthetic parse failure".to_owned(),
        });
    }

    let asset: Arc<dyn ReadOnlyAsset> = Arc::new(FileAsset::open(
        registered_asset.asset_id,
        archived.archive_path.clone(),
        archived.original_filename.clone(),
    )?);
    increment_runtime_invocations(&state)?;
    let batch = dependencies
        .runtime
        .invoke_source(&dependencies.source_module, asset, dependencies.limits)
        .await
        .map_err(|error| {
            let code = error.code().to_owned();
            let detail = error.detail().to_owned();
            (code, detail)
        });
    let batch = match batch {
        Ok(batch) => {
            emit(&state, CoreEvent::Stage("guest_parsed"));
            batch
        }
        Err((code, detail)) => {
            let _ = fail_attempt(&dependencies.database, attempt.attempt_id, &code, &detail).await;
            emit(&state, CoreEvent::QualityChanged);
            return Err(IngestionError::AssetFailure { code, detail });
        }
    };
    let validated = build_validated_batch(&attempt, &archived, &dependencies.source_module, batch)?;
    validation::validate_batch(&validated).map_err(|error| IngestionError::AssetFailure {
        code: error.code().to_owned(),
        detail: error.to_string(),
    })?;
    emit(&state, CoreEvent::Stage("host_validated"));
    for contract in extension_contracts(&validated, &dependencies.source_module)? {
        dependencies
            .database
            .execute(contract)
            .await
            .map_err(database_error)?;
    }
    let result = dependencies
        .database
        .execute(CommitSnapshot(Arc::new(validated)))
        .await;
    match result {
        Ok(result) => {
            emit(&state, CoreEvent::Stage("snapshot_committed"));
            let _ = state.lock().map(|mut guard| guard.completed_assets += 1);
            let _ = state.lock().map(|guard| {
                guard.events.send(CoreEvent::DataChanged {
                    capabilities: result.changed_capabilities,
                    dashboards: Vec::new(),
                })
            });
            Ok(AssetOutcome::Completed)
        }
        Err(error) => {
            let _ = fail_attempt(
                &dependencies.database,
                attempt.attempt_id,
                error.code(),
                &error.to_string(),
            )
            .await;
            Err(IngestionError::AssetFailure {
                code: error.code().to_owned(),
                detail: error.to_string(),
            })
        }
    }
}

fn source_metadata(module: &InstalledModule) -> Result<(String, String, String), IngestionError> {
    match &module.manifest {
        ModuleManifest::Source(manifest) => Ok((
            manifest.module_version.to_string(),
            manifest.source_api_version.to_string(),
            manifest.mapping_version.to_string(),
        )),
        _ => Err(IngestionError::AssetFailure {
            code: "source_module_type_mismatch".to_owned(),
            detail: "source module manifest required".to_owned(),
        }),
    }
}

fn infer_logical_key(
    module_id: &ModuleId,
    _module: &InstalledModule,
    year: Option<i32>,
) -> Result<LogicalSnapshotKey, IngestionError> {
    LogicalSnapshotKey::new(match year {
        Some(year) => format!("{module_id}:{year}"),
        None => format!("{module_id}:default"),
    })
    .map_err(|error| IngestionError::AssetFailure {
        code: error.code().to_owned(),
        detail: error.to_string(),
    })
}

fn build_validated_batch(
    attempt: &AttemptIdentity,
    archived: &mfa_archive::ArchivedAsset,
    module: &InstalledModule,
    batch: SourceBatch,
) -> Result<ValidatedSnapshotBatch, IngestionError> {
    let mapping_version = source_metadata(module)?.2;
    let mut source_records = Vec::with_capacity(batch.records.len().max(1));
    let mut observations = batch.records;
    let mut lineage = Vec::new();
    let mut used_ids = std::collections::HashSet::new();
    for (index, observation) in observations.iter_mut().enumerate() {
        let raw_id = observation_source_id(observation).unwrap_or_else(|| format!("row-{index}"));
        let mut source_record_id = format!("{}:{raw_id}", archived.asset_id);
        if !used_ids.insert(source_record_id.clone()) {
            source_record_id = format!("{source_record_id}:{index}");
            used_ids.insert(source_record_id.clone());
        }
        set_observation_source_id(observation, source_record_id.clone());
        source_records.push(SourceRecord {
            source_record_id: source_record_id.clone(),
            sheet_name: Some(archived.original_filename.clone()),
            source_row_number: (index + 1) as u32,
            source_record_key: format!(
                "{}:{}:{}",
                archived.asset_id,
                archived.original_filename,
                index + 1
            ),
            raw_payload: serde_json::to_value(&*observation).map_err(|error| {
                IngestionError::AssetFailure {
                    code: "source_record_serialization".to_owned(),
                    detail: error.to_string(),
                }
            })?,
        });
        let (entity_type, entity_id) = mfa_db::provenance::canonical_entity_key(observation);
        lineage.push(LineageLink {
            canonical_entity_type: entity_type,
            canonical_entity_id: entity_id,
            source_record_id,
            mapping_version: mapping_version.clone(),
        });
    }

    let issues = batch
        .issues
        .into_iter()
        .enumerate()
        .map(|(index, issue)| DataQualityItem {
            data_quality_item_id: format!("{}:issue:{index}", attempt.attempt_id),
            item_type: issue.code,
            source_asset_id: Some(archived.asset_id),
            source_record_id: None,
            severity: "warning".to_owned(),
            message: issue.message,
            status: "open".to_owned(),
            created_at: attempt.started_at.clone(),
            resolved_at: None,
        })
        .collect();
    let extensions = batch
        .extensions
        .into_iter()
        .enumerate()
        .map(|(index, extension)| ExtensionRecord {
            extension_record_id: format!("{}:extension:{index}", attempt.attempt_id),
            source_record_id: source_records
                .first()
                .map(|record| record.source_record_id.clone())
                .unwrap_or_else(|| format!("{}:extension-source:{index}", attempt.attempt_id)),
            source_module_id: module.module_id.clone(),
            contract_id: format!("{}@{}", extension.namespace, extension.contract_version),
            contract_version: extension.contract_version.to_string(),
            occurred_local_at: None,
            local_date: None,
            payload: extension.payload,
        })
        .collect();
    Ok(ValidatedSnapshotBatch {
        logical_key: attempt.logical_snapshot_key.clone(),
        attempt: attempt.clone(),
        source_records,
        observations,
        extensions,
        lineage,
        issues,
    })
}

fn extension_contracts(
    batch: &ValidatedSnapshotBatch,
    module: &InstalledModule,
) -> Result<Vec<ExtensionContractRegistration>, IngestionError> {
    Ok(batch
        .extensions
        .iter()
        .map(|extension| ExtensionContractRegistration {
            contract_id: extension.contract_id.clone(),
            source_module_id: module.module_id.clone(),
            namespace: extension
                .contract_id
                .split_once('@')
                .map(|(namespace, _)| namespace.to_owned())
                .unwrap_or_else(|| extension.contract_id.clone()),
            contract_version: extension.contract_version.clone(),
            payload_schema: json!({}),
        })
        .collect())
}

fn observation_source_id(observation: &CanonicalObservation) -> Option<String> {
    match observation {
        CanonicalObservation::NutritionItem(value) => value.source_record_id.clone(),
        CanonicalObservation::BodyMeasurement(value) => value.source_record_id.clone(),
        CanonicalObservation::ActivityEvent(value) => value.source_record_id.clone(),
        CanonicalObservation::HeartRate(value) => value.source_record_id.clone(),
        CanonicalObservation::ExerciseSet(value) => value.source_record_id.clone(),
        CanonicalObservation::ActivityDay(_)
        | CanonicalObservation::WorkoutSession(_)
        | CanonicalObservation::PhaseEvent(_) => None,
    }
}

fn set_observation_source_id(observation: &mut CanonicalObservation, source_record_id: String) {
    match observation {
        CanonicalObservation::NutritionItem(value) => {
            value.source_record_id = Some(source_record_id)
        }
        CanonicalObservation::BodyMeasurement(value) => {
            value.source_record_id = Some(source_record_id)
        }
        CanonicalObservation::ActivityEvent(value) => {
            value.source_record_id = Some(source_record_id)
        }
        CanonicalObservation::HeartRate(value) => value.source_record_id = Some(source_record_id),
        CanonicalObservation::ExerciseSet(value) => value.source_record_id = Some(source_record_id),
        CanonicalObservation::ActivityDay(_)
        | CanonicalObservation::WorkoutSession(_)
        | CanonicalObservation::PhaseEvent(_) => {}
    }
}

fn emit(state: &Arc<Mutex<PipelineState>>, event: CoreEvent) {
    if let Ok(guard) = state.lock() {
        let _ = guard.events.send(event);
    }
}

fn increment_runtime_invocations(state: &Arc<Mutex<PipelineState>>) -> Result<(), IngestionError> {
    state
        .lock()
        .map(|mut guard| guard.runtime_invocations += 1)
        .map_err(|_| poisoned())
}

async fn fail_attempt(
    database: &DatabaseService,
    attempt_id: Uuid,
    code: &str,
    detail: &str,
) -> Result<(), IngestionError> {
    database
        .execute(FailAttempt {
            attempt_id,
            finished_at: UtcInstant::from(chrono::Utc::now()),
            status: "failed".to_owned(),
            error_code: Some(code.to_owned()),
            error_message: Some(detail.to_owned()),
            record_count: 0,
        })
        .await
        .map(|_| ())
        .map_err(database_error)
}

fn database_error(error: mfa_db::DatabaseError) -> IngestionError {
    IngestionError::CriticalFailure {
        code: error.code().to_owned(),
        detail: error.to_string(),
    }
}

fn poisoned() -> IngestionError {
    IngestionError::CriticalFailure {
        code: "pipeline_state_poisoned".to_owned(),
        detail: "pipeline state mutex poisoned".to_owned(),
    }
}

struct FileAsset {
    asset_id: Uuid,
    path: PathBuf,
    file_name: String,
    byte_len: u64,
}

impl FileAsset {
    fn open(asset_id: Uuid, path: PathBuf, file_name: String) -> Result<Self, IngestionError> {
        let byte_len = fs::metadata(&path)
            .map_err(|error| IngestionError::AssetFailure {
                code: "archive_unavailable".to_owned(),
                detail: error.to_string(),
            })?
            .len();
        Ok(Self {
            asset_id,
            path,
            file_name,
            byte_len,
        })
    }
}

impl ReadOnlyAsset for FileAsset {
    fn metadata(&self) -> AssetMetadata {
        AssetMetadata {
            asset_id: self.asset_id,
            file_name: self.file_name.clone(),
            media_type: "application/octet-stream".to_owned(),
            byte_len: self.byte_len,
        }
    }

    fn read_at(&self, offset: u64, max_bytes: u32) -> Result<Vec<u8>, AssetReadError> {
        let mut file = File::open(&self.path).map_err(|error| AssetReadError::Unavailable {
            detail: error.to_string(),
        })?;
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| AssetReadError::Unavailable {
                detail: error.to_string(),
            })?;
        let mut bytes = vec![0; max_bytes as usize];
        let read = file
            .read(&mut bytes)
            .map_err(|error| AssetReadError::Unavailable {
                detail: error.to_string(),
            })?;
        bytes.truncate(read);
        Ok(bytes)
    }
}
