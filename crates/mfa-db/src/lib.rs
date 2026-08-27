pub mod actor;
pub mod command;
pub mod error;
pub mod fault;
pub mod migrations;
pub mod provenance;
pub mod validation;
pub mod views;

use crate::actor::run_actor;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tokio::sync::{mpsc, oneshot};

pub use command::{
    ArchiveAssetRecord, AssetRegistration, CommitSnapshot, DatabaseCommand, FailAttempt,
    FailAttemptResult, HealthCheck, HealthCheckResult, IntoDatabaseCommand, ListQualityItems,
    ListQualityItemsResult, MarkInterrupted, MarkInterruptedResult, QueryAttempt,
    QueryAttemptResult, QuerySnapshot, QueryView, ReconcileArchive, ReconcileArchiveInventory,
    ReconcileArchiveInventoryResult, ReconcileArchiveResult, RegisterAsset, RegisterAssetResult,
    RegisterReceipt, RegisterReceiptResult, Shutdown, SnapshotResponse, StartAttempt,
    StartAttemptResult,
};
pub use error::DatabaseError;
pub use fault::{
    DatabaseFailurePoint, DatabaseFault, DatabaseFaultInjector, NoDatabaseFaultInjector,
    TestDatabaseFaultInjector,
};
pub use migrations::CURRENT_SCHEMA_VERSION;
pub use provenance::{
    AttemptIdentity, DataQualityItem, ExtensionContractRegistration,
    ExtensionContractRegistrationResult, ExtensionRecord, LineageLink, LogicalSnapshotKey,
    RecordCounts, SnapshotCommitResult, SourceRecord, ValidatedSnapshotBatch,
};
pub use validation::ValidationError;
pub use views::{ViewRequest, ViewResponse};

#[derive(Clone)]
pub struct DatabaseService {
    sender: mpsc::Sender<DatabaseCommand>,
    capacity: usize,
    join_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl std::fmt::Debug for DatabaseService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DatabaseService")
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}

impl DatabaseService {
    pub async fn start(path: &Path, capacity: usize) -> Result<Self, DatabaseError> {
        Self::start_with_fault_injector(path, capacity, Arc::new(NoDatabaseFaultInjector)).await
    }

    pub async fn start_with_fault_injector(
        path: &Path,
        capacity: usize,
        fault_injector: Arc<dyn DatabaseFaultInjector>,
    ) -> Result<Self, DatabaseError> {
        if capacity == 0 {
            return Err(DatabaseError::InvalidPath {
                detail: "database command capacity must be positive".to_owned(),
            });
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| DatabaseError::InvalidPath {
                detail: error.to_string(),
            })?;
        }
        let path = path.to_path_buf();
        let (sender, receiver) = mpsc::channel(capacity);
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
        let join_handle = std::thread::Builder::new()
            .name("mfa-db-actor".to_owned())
            .spawn(move || run_actor(path, receiver, ready_sender, fault_injector))
            .map_err(|error| DatabaseError::Open {
                detail: error.to_string(),
            })?;
        let startup = tokio::task::spawn_blocking(move || ready_receiver.recv())
            .await
            .map_err(|error| DatabaseError::Open {
                detail: error.to_string(),
            })?
            .map_err(|error| DatabaseError::Open {
                detail: error.to_string(),
            })?;
        if let Err(error) = startup {
            let _ = join_handle.join();
            return Err(error);
        }
        Ok(Self {
            sender,
            capacity,
            join_handle: Arc::new(Mutex::new(Some(join_handle))),
        })
    }

    pub fn queue_capacity(&self) -> usize {
        self.capacity
    }

    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.join_handle) as usize
    }

    pub async fn execute<R, C>(&self, command: C) -> Result<R, DatabaseError>
    where
        C: IntoDatabaseCommand<R>,
    {
        let (response_sender, response_receiver) = oneshot::channel();
        self.sender
            .send(command.into_database_command(response_sender))
            .await
            .map_err(|_| DatabaseError::ChannelClosed)?;
        response_receiver
            .await
            .map_err(|_| DatabaseError::ActorStopped)?
    }

    pub async fn shutdown(self) -> Result<(), DatabaseError> {
        let result = self.execute(Shutdown).await;
        let join_handle = self
            .join_handle
            .lock()
            .map_err(|_| DatabaseError::Shutdown {
                detail: "actor join handle mutex is poisoned".to_owned(),
            })?
            .take();
        if let Some(join_handle) = join_handle {
            tokio::task::spawn_blocking(move || {
                join_handle.join().map_err(|_| DatabaseError::Shutdown {
                    detail: "database actor panicked".to_owned(),
                })
            })
            .await
            .map_err(|error| DatabaseError::Shutdown {
                detail: error.to_string(),
            })??;
        }
        result
    }
}
