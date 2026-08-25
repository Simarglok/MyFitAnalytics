use crate::queue::BoxFuture;
use crate::recovery::{FailurePoint, FaultInjector, InjectedFailure, NoFaultInjector};
use mfa_archive::{ArchiveReconciler, ArchiveRecord, ReconciliationError};
use mfa_contracts::ModuleId;
use mfa_db::{DatabaseError, DatabaseService};
use std::fs::{self, File, OpenOptions};
use std::io::{self};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;

pub trait ArchiveAssetImporter: Send + Sync {
    fn supports(&self, _source_module_id: &ModuleId) -> bool {
        true
    }

    fn import<'a>(
        &'a self,
        database: DatabaseService,
        asset: ArchiveRecord,
    ) -> BoxFuture<'a, Result<(), RebuildError>>;

    fn validate<'a>(&'a self, database: DatabaseService)
    -> BoxFuture<'a, Result<(), RebuildError>>;
}

#[derive(Debug, Clone)]
pub struct RebuildPlan {
    pub assets: Vec<ArchiveRecord>,
    pub missing_source_packages: Vec<ModuleId>,
}

#[derive(Debug, Clone)]
pub struct ArchiveRebuildConfig {
    pub database_path: PathBuf,
    pub recovery_root: PathBuf,
    pub actor_capacity: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveRebuildConfirmation {
    pub acknowledge_data_replacement: bool,
}

impl ArchiveRebuildConfirmation {
    pub fn confirmed() -> Self {
        Self {
            acknowledge_data_replacement: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveRebuildResult {
    pub assets_rebuilt: u64,
    pub recovery_copy: PathBuf,
    pub database_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum RebuildError {
    #[error("rebuild requires explicit acknowledgement before replacing the active database")]
    ConfirmationRequired,
    #[error(transparent)]
    Archive(#[from] ReconciliationError),
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("rebuild filesystem operation {operation} failed: {detail}")]
    Io {
        operation: &'static str,
        detail: String,
    },
    #[error("archive importer failed: {detail}")]
    Importer { detail: String },
    #[error("rebuild source packages are unavailable: {modules:?}")]
    MissingSourcePackages { modules: Vec<ModuleId> },
    #[error("rebuild fault was injected at {point:?}")]
    FaultInjected { point: FailurePoint },
}

pub struct ArchiveRebuildService {
    database: Option<DatabaseService>,
    config: ArchiveRebuildConfig,
    reconciler: ArchiveReconciler,
    importer: Arc<dyn ArchiveAssetImporter>,
    fault_injector: Arc<dyn FaultInjector>,
}

pub type RebuildService = ArchiveRebuildService;
pub type RebuildConfig = ArchiveRebuildConfig;
pub type RebuildConfirmation = ArchiveRebuildConfirmation;
pub type RebuildResult = ArchiveRebuildResult;

impl ArchiveRebuildService {
    pub fn new(
        database: DatabaseService,
        config: ArchiveRebuildConfig,
        reconciler: ArchiveReconciler,
        importer: Arc<dyn ArchiveAssetImporter>,
    ) -> Self {
        Self::new_with_fault_injector(
            database,
            config,
            reconciler,
            importer,
            Arc::new(NoFaultInjector),
        )
    }

    pub fn new_with_fault_injector(
        database: DatabaseService,
        config: ArchiveRebuildConfig,
        reconciler: ArchiveReconciler,
        importer: Arc<dyn ArchiveAssetImporter>,
        fault_injector: Arc<dyn FaultInjector>,
    ) -> Self {
        Self {
            database: Some(database),
            config,
            reconciler,
            importer,
            fault_injector,
        }
    }

    pub fn database(&self) -> &DatabaseService {
        self.database
            .as_ref()
            .expect("archive rebuild service database is available until shutdown")
    }

    pub async fn shutdown(&mut self) -> Result<(), RebuildError> {
        if let Some(database) = self.database.take() {
            database.shutdown().await?;
        }
        Ok(())
    }

    pub async fn preflight(&self) -> Result<RebuildPlan, RebuildError> {
        let inventory = self.reconciler.scan()?;
        let missing_source_packages = if self.importer.supports(self.reconciler.source_module_id())
        {
            Vec::new()
        } else {
            vec![self.reconciler.source_module_id().clone()]
        };
        Ok(RebuildPlan {
            assets: inventory.assets,
            missing_source_packages,
        })
    }

    pub async fn rebuild(
        &mut self,
        confirmation: ArchiveRebuildConfirmation,
    ) -> Result<ArchiveRebuildResult, RebuildError> {
        if !confirmation.acknowledge_data_replacement {
            return Err(RebuildError::ConfirmationRequired);
        }
        let plan = self.preflight().await?;
        if !plan.missing_source_packages.is_empty() {
            return Err(RebuildError::MissingSourcePackages {
                modules: plan.missing_source_packages,
            });
        }
        if let Err(error) = self.fault_injector.check(FailurePoint::RecoveryCopy) {
            return Err(rebuild_fault(error));
        }
        let database = self.database.take().ok_or_else(|| RebuildError::Io {
            operation: "take_active_database",
            detail: "database service is already shut down".to_owned(),
        })?;
        database.shutdown().await?;

        let recovery_copy =
            match create_recovery_copy(&self.config.database_path, &self.config.recovery_root) {
                Ok(path) => path,
                Err(error) => {
                    self.database = Some(
                        DatabaseService::start(
                            &self.config.database_path,
                            self.config.actor_capacity,
                        )
                        .await?,
                    );
                    return Err(error);
                }
            };
        let temporary_path = self
            .config
            .database_path
            .with_extension("duckdb.rebuild-tmp");
        if temporary_path.exists() {
            fs::remove_file(&temporary_path).map_err(|error| RebuildError::Io {
                operation: "remove_stale_rebuild_database",
                detail: error.to_string(),
            })?;
        }

        let temporary = match DatabaseService::start(&temporary_path, self.config.actor_capacity)
            .await
        {
            Ok(database) => database,
            Err(error) => {
                self.database = Some(
                    DatabaseService::start(&self.config.database_path, self.config.actor_capacity)
                        .await?,
                );
                return Err(error.into());
            }
        };
        let mut rebuilt = 0_u64;
        let import_result = async {
            for asset in plan.assets {
                self.fault_injector
                    .check(FailurePoint::RebuildImport)
                    .map_err(rebuild_fault)?;
                self.importer.import(temporary.clone(), asset).await?;
                rebuilt += 1;
            }
            self.importer.validate(temporary.clone()).await
        }
        .await;
        let temporary_shutdown = temporary.shutdown().await;
        if let Err(error) = import_result {
            let _ = temporary_shutdown;
            let _ = fs::remove_file(&temporary_path);
            self.database = Some(
                DatabaseService::start(&self.config.database_path, self.config.actor_capacity)
                    .await?,
            );
            return Err(error);
        }
        temporary_shutdown?;

        if let Err(error) = self.fault_injector.check(FailurePoint::ActiveSwitch) {
            let _ = fs::remove_file(&temporary_path);
            self.database = Some(
                DatabaseService::start(&self.config.database_path, self.config.actor_capacity)
                    .await?,
            );
            return Err(rebuild_fault(error));
        }

        if let Err(error) = fs::rename(&temporary_path, &self.config.database_path) {
            let _ = fs::remove_file(&temporary_path);
            self.database = Some(
                DatabaseService::start(&self.config.database_path, self.config.actor_capacity)
                    .await?,
            );
            return Err(RebuildError::Io {
                operation: "atomically_replace_active_database",
                detail: error.to_string(),
            });
        }
        self.database = Some(
            DatabaseService::start(&self.config.database_path, self.config.actor_capacity).await?,
        );
        Ok(ArchiveRebuildResult {
            assets_rebuilt: rebuilt,
            recovery_copy,
            database_path: self.config.database_path.clone(),
        })
    }
}

fn rebuild_fault(error: InjectedFailure) -> RebuildError {
    RebuildError::FaultInjected { point: error.point }
}

fn create_recovery_copy(
    database_path: &Path,
    recovery_root: &Path,
) -> Result<PathBuf, RebuildError> {
    fs::create_dir_all(recovery_root).map_err(|error| RebuildError::Io {
        operation: "create_recovery_directory",
        detail: error.to_string(),
    })?;
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.fZ");
    let mut source = File::open(database_path).map_err(|error| RebuildError::Io {
        operation: "open_active_database_for_recovery",
        detail: error.to_string(),
    })?;
    for suffix in 0..10_000_u32 {
        let name = if suffix == 0 {
            format!("storage-recovery-{timestamp}.duckdb")
        } else {
            format!("storage-recovery-{timestamp}-{suffix}.duckdb")
        };
        let destination = recovery_root.join(name);
        let mut target = match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&destination)
        {
            Ok(target) => target,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(RebuildError::Io {
                    operation: "create_recovery_copy",
                    detail: error.to_string(),
                });
            }
        };
        let copy_result = (|| {
            io::copy(&mut source, &mut target).map_err(|error| RebuildError::Io {
                operation: "copy_active_database_for_recovery",
                detail: error.to_string(),
            })?;
            target.sync_all().map_err(|error| RebuildError::Io {
                operation: "sync_recovery_copy",
                detail: error.to_string(),
            })?;
            sync_directory(recovery_root).map_err(|error| RebuildError::Io {
                operation: "sync_recovery_directory",
                detail: error.to_string(),
            })?;
            Ok::<(), RebuildError>(())
        })();
        if let Err(error) = copy_result {
            let _ = fs::remove_file(&destination);
            return Err(error);
        }
        return Ok(destination);
    }
    Err(RebuildError::Io {
        operation: "create_recovery_copy",
        detail: "recovery filename space exhausted".to_owned(),
    })
}

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}
