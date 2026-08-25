use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use thiserror::Error;

use mfa_archive::{ArchiveReconciler, ReconciliationError};
use mfa_db::{
    ArchiveAssetRecord, DatabaseError, DatabaseService, MarkInterrupted, ReconcileArchiveInventory,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FailurePoint {
    ArchiveCopy,
    ArchiveHashVerify,
    ArchiveRename,
    InboxDelete,
    AssetRegistration,
    GuestParse,
    BuildBatch,
    HostValidation,
    TransactionStart,
    CanonicalInsert,
    ActiveSwitch,
    EventEmission,
    DatabaseOpen,
    RecoveryCopy,
    RebuildImport,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("injected failure at {point:?}")]
pub struct InjectedFailure {
    pub point: FailurePoint,
}

pub trait FaultInjector: Send + Sync {
    fn check(&self, point: FailurePoint) -> Result<(), InjectedFailure>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoFaultInjector;

impl FaultInjector for NoFaultInjector {
    fn check(&self, _point: FailurePoint) -> Result<(), InjectedFailure> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct TestFaultInjector {
    remaining: Arc<Mutex<BTreeMap<FailurePoint, u32>>>,
}

impl TestFaultInjector {
    pub fn fail_once(&self, point: FailurePoint) {
        self.remaining.lock().unwrap().insert(point, 1);
    }

    pub fn fail_always(&self, point: FailurePoint) {
        self.remaining.lock().unwrap().insert(point, u32::MAX);
    }

    pub fn clear(&self, point: FailurePoint) {
        self.remaining.lock().unwrap().remove(&point);
    }
}

impl FaultInjector for TestFaultInjector {
    fn check(&self, point: FailurePoint) -> Result<(), InjectedFailure> {
        let mut remaining = self.remaining.lock().unwrap();
        let Some(budget) = remaining.get_mut(&point) else {
            return Ok(());
        };
        if *budget != u32::MAX {
            *budget = budget.saturating_sub(1);
            if *budget == 0 {
                remaining.remove(&point);
            }
        }
        Err(InjectedFailure { point })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryMode {
    Recovery,
    Normal,
}

#[derive(Debug, Clone)]
pub struct RecoveryGate {
    mode: Arc<AtomicU8>,
}

impl RecoveryGate {
    pub fn new() -> Self {
        Self {
            mode: Arc::new(AtomicU8::new(0)),
        }
    }

    pub fn ready() -> Self {
        let gate = Self::new();
        gate.complete();
        gate
    }

    pub fn mode(&self) -> RecoveryMode {
        if self.mode.load(Ordering::Acquire) == 1 {
            RecoveryMode::Normal
        } else {
            RecoveryMode::Recovery
        }
    }

    pub fn complete(&self) {
        self.mode.store(1, Ordering::Release);
    }

    pub fn ensure_ingestion_allowed(&self) -> Result<(), crate::IngestionError> {
        if self.mode() == RecoveryMode::Normal {
            Ok(())
        } else {
            Err(crate::IngestionError::CriticalFailure {
                code: "recovery_in_progress".to_owned(),
                detail: "archive and database reconciliation has not completed".to_owned(),
            })
        }
    }
}

impl Default for RecoveryGate {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryReport {
    pub interrupted_attempts: u64,
    pub registered_assets: u64,
    pub missing_assets: u64,
    pub assets_to_ingest: u64,
}

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Archive(#[from] ReconciliationError),
}

pub struct RecoveryService {
    database: DatabaseService,
    reconciler: ArchiveReconciler,
    gate: RecoveryGate,
}

impl RecoveryService {
    pub fn new(
        database: DatabaseService,
        reconciler: ArchiveReconciler,
        gate: RecoveryGate,
    ) -> Self {
        Self {
            database,
            reconciler,
            gate,
        }
    }

    pub fn gate(&self) -> RecoveryGate {
        self.gate.clone()
    }

    pub async fn startup(&self) -> Result<RecoveryReport, RecoveryError> {
        let interrupted = self.database.execute(MarkInterrupted).await?.count;
        let inventory = self.reconciler.scan()?;
        let assets = inventory
            .assets
            .into_iter()
            .map(|asset| ArchiveAssetRecord {
                asset_id: asset.asset_id,
                source_module_id: asset.source_module_id,
                asset_type: "source_export".to_owned(),
                original_filename: asset.original_filename,
                archive_path: asset.archive_path.to_string_lossy().into_owned(),
                byte_sha256: asset.byte_sha256,
                file_size: asset.file_size,
                received_at: asset.received_at,
            })
            .collect();
        let result = self
            .database
            .execute(ReconcileArchiveInventory {
                source_module_id: self.reconciler.source_module_id().clone(),
                assets,
            })
            .await?;
        self.gate.complete();
        Ok(RecoveryReport {
            interrupted_attempts: interrupted,
            registered_assets: result.registered_assets,
            missing_assets: result.missing_assets,
            assets_to_ingest: result.assets_to_ingest.len() as u64,
        })
    }
}
