use mfa_archive::{ArchiveCoordinator, ArchiveReconciler};
use mfa_config::{
    AppPaths, AppSettings, PathPolicyError, SettingsError, SettingsStore, WorkspacePaths,
};
use mfa_db::{DatabaseError, DatabaseService};
use mfa_ingestion::{
    IngestionCoordinator, IngestionDependencies, IngestionError, RecoveryGate, RecoveryService,
};
use mfa_module_host::{
    CapabilityError, CapabilityRegistry, ComponentRuntime, InstalledModule, LocaleError,
    LocaleResolver, ModuleRegistry, PackageError, PackageInstaller, ProviderResolution,
    RuntimeLimits,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppStateError {
    #[error("settings could not be loaded: {0}")]
    Settings(#[from] SettingsError),
    #[error("installed modules could not be loaded: {0}")]
    Modules(#[from] PackageError),
    #[error("capability providers could not be resolved: {0}")]
    Capabilities(#[from] CapabilityError),
    #[error("locale catalogs could not be loaded: {0}")]
    Locales(#[from] LocaleError),
    #[error("storage could not be initialized: {detail}")]
    Storage { detail: String },
}

impl AppStateError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Settings(error) => error.code(),
            Self::Modules(error) => error.code(),
            Self::Capabilities(error) => error.code(),
            Self::Locales(error) => error.code(),
            Self::Storage { .. } => "storage_initialization",
        }
    }
}

pub(crate) struct StorageRuntime {
    pub workspace: WorkspacePaths,
    pub database: DatabaseService,
    pub coordinator: Option<IngestionCoordinator>,
    pub recovery_gate: RecoveryGate,
}

pub struct AppState {
    pub(crate) settings: Mutex<AppSettings>,
    pub(crate) modules: Vec<InstalledModule>,
    pub(crate) providers: ProviderResolution,
    pub(crate) locale: LocaleResolver,
    config_root: PathBuf,
    default_app_data_root: PathBuf,
    storage: Mutex<Option<StorageRuntime>>,
}

impl AppState {
    pub fn from_roots(
        config_root: impl AsRef<Path>,
        module_root: impl AsRef<Path>,
        core_locale_root: impl AsRef<Path>,
    ) -> Result<Self, AppStateError> {
        let config_root = config_root.as_ref().to_path_buf();
        let settings = SettingsStore::new(config_root.join("settings.json")).load()?;
        let installer = PackageInstaller::new(module_root.as_ref());
        let modules = installer.list()?;
        let providers = CapabilityRegistry::new().resolve(&modules, &settings)?;
        let locale = LocaleResolver::new(core_locale_root, modules.clone())?;
        Ok(Self {
            settings: Mutex::new(settings),
            modules,
            providers,
            locale,
            default_app_data_root: config_root.join("app-data"),
            config_root,
            storage: Mutex::new(None),
        })
    }

    pub fn from_roots_with_core_catalog(
        config_root: impl AsRef<Path>,
        module_root: impl AsRef<Path>,
        core_catalog: &[u8],
    ) -> Result<Self, AppStateError> {
        let config_root = config_root.as_ref().to_path_buf();
        let settings = SettingsStore::new(config_root.join("settings.json")).load()?;
        let installer = PackageInstaller::new(module_root.as_ref());
        let modules = installer.list()?;
        let providers = CapabilityRegistry::new().resolve(&modules, &settings)?;
        let locale = LocaleResolver::from_core_json(core_catalog, modules.clone())?;
        Ok(Self {
            settings: Mutex::new(settings),
            modules,
            providers,
            locale,
            default_app_data_root: config_root.join("app-data"),
            config_root,
            storage: Mutex::new(None),
        })
    }

    pub(crate) fn settings(&self) -> AppSettings {
        self.settings
            .lock()
            .expect("settings state lock is not poisoned")
            .clone()
    }

    pub(crate) fn modules(&self) -> &[InstalledModule] {
        &self.modules
    }

    pub(crate) fn providers(&self) -> &ProviderResolution {
        &self.providers
    }

    #[allow(dead_code)]
    pub(crate) fn locale(&self) -> &LocaleResolver {
        &self.locale
    }

    pub(crate) fn configured_app_paths(&self) -> AppPaths {
        let configured = self.settings();
        AppPaths::new(
            configured
                .app_data_root
                .unwrap_or_else(|| self.default_app_data_root.clone()),
        )
    }

    pub(crate) async fn configure_workspace(
        &self,
        workspace_root: PathBuf,
    ) -> Result<WorkspacePaths, AppStateError> {
        self.shutdown_storage().await?;
        let app_paths = self.configured_app_paths();
        WorkspacePaths::validate(&workspace_root, &app_paths).map_err(|error| {
            AppStateError::Storage {
                detail: error.to_string(),
            }
        })?;
        app_paths
            .ensure_directories()
            .map_err(|error| AppStateError::Storage {
                detail: error.to_string(),
            })?;
        let workspace = WorkspacePaths::new(workspace_root.clone());
        let source_module = self
            .modules
            .iter()
            .find(|module| {
                module.enabled && module.module_type == mfa_contracts::ModuleType::Source
            })
            .cloned();
        if let Some(module) = &source_module {
            workspace
                .enable_source(&module.module_id)
                .map_err(|error| AppStateError::Storage {
                    detail: error.to_string(),
                })?;
        }

        let database = DatabaseService::start(&app_paths.database, 32)
            .await
            .map_err(|error| AppStateError::Storage {
                detail: error.to_string(),
            })?;
        let recovery_gate = RecoveryGate::new();
        let coordinator = if let Some(module) = source_module {
            let reconciler = ArchiveReconciler::new(workspace.clone(), module.module_id.clone());
            let recovery =
                RecoveryService::new(database.clone(), reconciler, recovery_gate.clone());
            if let Err(error) = recovery.startup().await {
                let _ = database.clone().shutdown().await;
                return Err(AppStateError::Storage {
                    detail: error.to_string(),
                });
            }
            let archive = ArchiveCoordinator::new(workspace.clone(), module.module_id.clone());
            let runtime = Arc::new(ComponentRuntime::default());
            let dependencies = IngestionDependencies {
                workspace: workspace.clone(),
                source_module: module,
                archive,
                database: database.clone(),
                runtime,
                limits: RuntimeLimits::default(),
                queue_capacity: 32,
            };
            let coordinator = match IngestionCoordinator::start(dependencies) {
                Ok(coordinator) => coordinator,
                Err(error) => {
                    let _ = database.clone().shutdown().await;
                    return Err(AppStateError::Storage {
                        detail: error.to_string(),
                    });
                }
            };
            coordinator.set_recovery_gate(recovery_gate.clone());
            Some(coordinator)
        } else {
            recovery_gate.complete();
            None
        };

        let settings_path = self.config_root.join("settings.json");
        let mut settings = self.settings();
        settings.workspace_root = Some(workspace_root);
        settings.app_data_root = Some(app_paths.app_data.clone());
        SettingsStore::new(settings_path).save(&settings)?;
        let mut storage = self.storage.lock().map_err(|_| AppStateError::Storage {
            detail: "storage state lock poisoned".to_owned(),
        })?;
        *storage = Some(StorageRuntime {
            workspace: workspace.clone(),
            database,
            coordinator,
            recovery_gate,
        });
        drop(storage);
        *self.settings.lock().map_err(|_| AppStateError::Storage {
            detail: "settings state lock poisoned".to_owned(),
        })? = settings;
        Ok(workspace)
    }

    pub(crate) fn storage_lock(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<StorageRuntime>>, AppStateError> {
        self.storage.lock().map_err(|_| AppStateError::Storage {
            detail: "storage state lock poisoned".to_owned(),
        })
    }

    pub async fn shutdown_storage(&self) -> Result<(), AppStateError> {
        let runtime = self
            .storage
            .lock()
            .map_err(|_| AppStateError::Storage {
                detail: "storage state lock poisoned".to_owned(),
            })?
            .take();
        let Some(runtime) = runtime else {
            return Ok(());
        };
        if let Some(coordinator) = runtime.coordinator {
            coordinator
                .shutdown()
                .await
                .map_err(|error| AppStateError::Storage {
                    detail: error.to_string(),
                })?;
        }
        runtime
            .database
            .shutdown()
            .await
            .map_err(|error| AppStateError::Storage {
                detail: error.to_string(),
            })
    }
}

impl From<PathPolicyError> for AppStateError {
    fn from(error: PathPolicyError) -> Self {
        Self::Storage {
            detail: error.to_string(),
        }
    }
}

impl From<DatabaseError> for AppStateError {
    fn from(error: DatabaseError) -> Self {
        Self::Storage {
            detail: error.to_string(),
        }
    }
}

impl From<IngestionError> for AppStateError {
    fn from(error: IngestionError) -> Self {
        Self::Storage {
            detail: error.to_string(),
        }
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        if let Ok(storage) = self.storage.get_mut() {
            let _ = storage.take();
        }
    }
}
