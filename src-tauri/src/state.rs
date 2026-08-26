use crate::events::{DataChangedSink, spawn_event_forwarders};
use mfa_archive::{ArchiveCoordinator, ArchiveReconciler};
use mfa_config::{
    AppPaths, AppSettings, PathPolicyError, SettingsError, SettingsStore, WorkspacePaths,
};
use mfa_contracts::ModuleId;
use mfa_db::{DatabaseError, DatabaseService};
use mfa_ingestion::{
    IngestionCoordinator, IngestionDependencies, IngestionError, RecoveryGate, ScanReason,
    now_request, recover_sources,
};
#[cfg(any(test, feature = "test-support"))]
use mfa_module_host::UninstallFinalizationFault;
use mfa_module_host::{
    CapabilityError, CapabilityRegistry, ComponentRuntime, InstalledModule, LocaleError,
    LocaleResolver, ModuleRegistry, PackageError, PackageInstaller, ProviderResolution,
    RuntimeLimits,
};
use semver::Version;
use std::collections::BTreeMap;
use std::fs;
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
    #[error(
        "storage initialization failed and restoration failed: {original}; restoration: {restoration}"
    )]
    StorageRestoration {
        original: String,
        restoration: String,
    },
}

impl AppStateError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Settings(error) => error.code(),
            Self::Modules(error) => error.code(),
            Self::Capabilities(error) => error.code(),
            Self::Locales(error) => error.code(),
            Self::Storage { .. } => "storage_initialization",
            Self::StorageRestoration { .. } => "storage_restoration_failed",
        }
    }
}

pub(crate) struct StorageRuntime {
    pub workspace: WorkspacePaths,
    pub app_paths: AppPaths,
    pub database: DatabaseService,
    pub coordinators: BTreeMap<ModuleId, IngestionCoordinator>,
    pub recovery_gate: RecoveryGate,
}

struct CandidateStorageRuntime {
    runtime: StorageRuntime,
    owns_database: bool,
}

pub struct AppState {
    pub(crate) settings: Mutex<AppSettings>,
    pub(crate) modules: Mutex<Vec<InstalledModule>>,
    pub(crate) providers: Mutex<ProviderResolution>,
    pub(crate) locale: Mutex<LocaleResolver>,
    config_root: PathBuf,
    module_root: PathBuf,
    core_catalog: Vec<u8>,
    app_version: Version,
    default_app_data_root: PathBuf,
    bundled_packages: Mutex<BTreeMap<ModuleId, PathBuf>>,
    storage: Mutex<Option<StorageRuntime>>,
    event_sink: Mutex<Option<Arc<dyn DataChangedSink>>>,
    #[cfg(any(test, feature = "test-support"))]
    uninstall_finalization_fault: Mutex<Option<UninstallFinalizationFault>>,
}

impl AppState {
    pub fn from_roots(
        config_root: impl AsRef<Path>,
        module_root: impl AsRef<Path>,
        core_locale_root: impl AsRef<Path>,
    ) -> Result<Self, AppStateError> {
        let config_root = config_root.as_ref().to_path_buf();
        let core_catalog =
            fs::read(core_locale_root.as_ref().join("messages.json")).map_err(|error| {
                AppStateError::Locales(LocaleError::Io {
                    detail: error.to_string(),
                })
            })?;
        let settings = SettingsStore::new(config_root.join("settings.json")).load()?;
        let app_version =
            Version::parse(env!("CARGO_PKG_VERSION")).expect("package version is valid semver");
        let installer =
            PackageInstaller::with_app_version(module_root.as_ref(), app_version.clone());
        let modules = installer.list()?;
        let providers = CapabilityRegistry::new().resolve_runtime(&modules, &settings)?;
        let locale = LocaleResolver::from_core_json(&core_catalog, modules.clone())?;
        Ok(Self {
            settings: Mutex::new(settings),
            modules: Mutex::new(modules),
            providers: Mutex::new(providers),
            locale: Mutex::new(locale),
            module_root: module_root.as_ref().to_path_buf(),
            core_catalog,
            app_version,
            default_app_data_root: config_root.join("app-data"),
            config_root,
            bundled_packages: Mutex::new(BTreeMap::new()),
            storage: Mutex::new(None),
            event_sink: Mutex::new(None),
            #[cfg(any(test, feature = "test-support"))]
            uninstall_finalization_fault: Mutex::new(None),
        })
    }

    pub fn from_roots_with_core_catalog(
        config_root: impl AsRef<Path>,
        module_root: impl AsRef<Path>,
        core_catalog: &[u8],
    ) -> Result<Self, AppStateError> {
        Self::from_roots_with_core_catalog_at_app_version(
            config_root,
            module_root,
            core_catalog,
            Version::parse(env!("CARGO_PKG_VERSION")).expect("package version is valid semver"),
        )
    }

    pub fn from_roots_with_core_catalog_at_app_version(
        config_root: impl AsRef<Path>,
        module_root: impl AsRef<Path>,
        core_catalog: &[u8],
        app_version: Version,
    ) -> Result<Self, AppStateError> {
        let config_root = config_root.as_ref().to_path_buf();
        let settings = SettingsStore::new(config_root.join("settings.json")).load()?;
        let installer =
            PackageInstaller::with_app_version(module_root.as_ref(), app_version.clone());
        let modules = installer.list()?;
        let providers = CapabilityRegistry::new().resolve_runtime(&modules, &settings)?;
        let locale = LocaleResolver::from_core_json(core_catalog, modules.clone())?;
        Ok(Self {
            settings: Mutex::new(settings),
            modules: Mutex::new(modules),
            providers: Mutex::new(providers),
            locale: Mutex::new(locale),
            module_root: module_root.as_ref().to_path_buf(),
            core_catalog: core_catalog.to_vec(),
            app_version,
            default_app_data_root: config_root.join("app-data"),
            config_root,
            bundled_packages: Mutex::new(BTreeMap::new()),
            storage: Mutex::new(None),
            event_sink: Mutex::new(None),
            #[cfg(any(test, feature = "test-support"))]
            uninstall_finalization_fault: Mutex::new(None),
        })
    }

    pub(crate) fn settings(&self) -> AppSettings {
        self.settings
            .lock()
            .expect("settings state lock is not poisoned")
            .clone()
    }

    pub(crate) fn save_settings(&self, settings: AppSettings) -> Result<(), AppStateError> {
        SettingsStore::new(self.config_root.join("settings.json")).save(&settings)?;
        *self.settings.lock().map_err(|_| AppStateError::Storage {
            detail: "settings state lock poisoned".to_owned(),
        })? = settings;
        Ok(())
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_uninstall_finalization_fault(&self, fault: Option<UninstallFinalizationFault>) {
        if let Ok(mut configured_fault) = self.uninstall_finalization_fault.lock() {
            *configured_fault = fault;
        }
    }

    pub(crate) fn package_installer(&self) -> PackageInstaller {
        PackageInstaller::with_app_version(&self.module_root, self.app_version.clone())
    }

    pub(crate) fn uninstall_package_installer(&self) -> PackageInstaller {
        let installer = self.package_installer();
        #[cfg(any(test, feature = "test-support"))]
        if let Some(fault) = self
            .uninstall_finalization_fault
            .lock()
            .ok()
            .and_then(|mut fault| fault.take())
        {
            return installer.with_uninstall_finalization_fault(fault);
        }
        installer
    }

    pub(crate) async fn install_package(
        &self,
        package: &Path,
    ) -> Result<InstalledModule, AppStateError> {
        let installer = self.package_installer();
        let previous = self.modules();
        let installed = installer.install(package)?;
        if let Err(error) = self.refresh_registry() {
            if !previous.iter().any(|module| {
                module.module_id == installed.module_id
                    && module.package_hash == installed.package_hash
            }) {
                let _ = installer.uninstall(&installed.module_id);
                if let Some(previous) = previous
                    .iter()
                    .find(|module| module.module_id == installed.module_id)
                {
                    let _ = installer.restore_active(previous);
                }
            }
            let _ = self.refresh_registry();
            return Err(error);
        }
        let reconfigure_result = if installed.module_type == mfa_contracts::ModuleType::Source {
            self.reconfigure_source(&installed.module_id).await
        } else {
            Ok(())
        };
        if let Err(error) = reconfigure_result {
            if !previous.iter().any(|module| {
                module.module_id == installed.module_id
                    && module.package_hash == installed.package_hash
            }) {
                let _ = installer.uninstall(&installed.module_id);
            }
            if let Some(old) = previous
                .iter()
                .find(|module| module.module_id == installed.module_id)
            {
                let _ = installer.restore_active(old);
            }
            let _ = self.refresh_registry();
            let _ = self.reconfigure_source(&installed.module_id).await;
            return Err(error);
        }
        Ok(installed)
    }

    pub(crate) fn select_provider(
        &self,
        capability: mfa_contracts::CapabilityId,
        module_id: ModuleId,
    ) -> Result<ProviderResolution, AppStateError> {
        let mut settings = self.settings();
        let resolution = CapabilityRegistry::new().select_provider(
            &self.modules(),
            &mut settings,
            &capability,
            &module_id,
        )?;
        self.save_settings(settings)?;
        *self.providers.lock().map_err(|_| AppStateError::Storage {
            detail: "provider state lock poisoned".to_owned(),
        })? = resolution.clone();
        Ok(resolution)
    }

    pub(crate) async fn reconfigure_source(
        &self,
        module_id: &ModuleId,
    ) -> Result<(), AppStateError> {
        let settings = self.settings();
        let event_sink = self
            .event_sink
            .lock()
            .map_err(|_| AppStateError::Storage {
                detail: "event sink state lock poisoned".to_owned(),
            })?
            .clone();
        let module = self
            .modules()
            .into_iter()
            .find(|module| &module.module_id == module_id);
        let (workspace_root, database, recovery_gate) = {
            let storage = self.storage.lock().map_err(|_| AppStateError::Storage {
                detail: "storage state lock poisoned".to_owned(),
            })?;
            let Some(storage) = storage.as_ref() else {
                return Ok(());
            };
            (
                storage.workspace.root.clone(),
                storage.database.clone(),
                storage.recovery_gate.clone(),
            )
        };
        let workspace =
            WorkspacePaths::with_source_inbox_roots(workspace_root, settings.source_inbox_roots);

        let Some(module) = module.filter(|module| {
            module.enabled && module.module_type == mfa_contracts::ModuleType::Source
        }) else {
            let coordinator = self
                .storage
                .lock()
                .map_err(|_| AppStateError::Storage {
                    detail: "storage state lock poisoned".to_owned(),
                })?
                .as_mut()
                .and_then(|storage| storage.coordinators.remove(module_id));
            if let Some(coordinator) = coordinator {
                coordinator
                    .shutdown()
                    .await
                    .map_err(|error| AppStateError::Storage {
                        detail: error.to_string(),
                    })?;
            }
            return Ok(());
        };

        workspace
            .enable_source(&module.module_id)
            .map_err(|error| AppStateError::Storage {
                detail: error.to_string(),
            })?;
        let coordinator = self
            .start_source_coordinator(workspace.clone(), database, recovery_gate, module)
            .await?;
        let previous = self
            .storage
            .lock()
            .map_err(|_| AppStateError::Storage {
                detail: "storage state lock poisoned".to_owned(),
            })?
            .as_mut()
            .and_then(|storage| {
                storage.workspace = workspace.clone();
                storage
                    .coordinators
                    .insert(module_id.clone(), coordinator.clone())
            });
        if let Some(previous) = previous {
            let _ = previous.shutdown().await;
        }
        if let Some(sink) = event_sink {
            spawn_event_forwarders(sink, vec![coordinator]);
        }
        Ok(())
    }

    async fn start_source_coordinator(
        &self,
        workspace: WorkspacePaths,
        database: DatabaseService,
        recovery_gate: RecoveryGate,
        module: InstalledModule,
    ) -> Result<IngestionCoordinator, AppStateError> {
        let archive = ArchiveCoordinator::new(workspace.clone(), module.module_id.clone());
        let runtime = Arc::new(ComponentRuntime::default());
        let dependencies = IngestionDependencies {
            workspace,
            source_module: module,
            archive,
            database,
            runtime,
            limits: RuntimeLimits::default(),
            queue_capacity: 32,
        };
        let coordinator =
            IngestionCoordinator::start(dependencies).map_err(|error| AppStateError::Storage {
                detail: error.to_string(),
            })?;
        coordinator.set_recovery_gate(recovery_gate);
        if let Err(error) = coordinator
            .request_scan(now_request(ScanReason::Startup))
            .await
        {
            let detail = error.to_string();
            let _ = coordinator.shutdown().await;
            return Err(AppStateError::Storage { detail });
        }
        Ok(coordinator)
    }

    pub(crate) fn modules(&self) -> Vec<InstalledModule> {
        self.modules
            .lock()
            .expect("module state lock is not poisoned")
            .clone()
    }

    pub(crate) fn providers(&self) -> ProviderResolution {
        self.providers
            .lock()
            .expect("provider state lock is not poisoned")
            .clone()
    }

    pub fn register_bundled_package(&self, module_id: ModuleId, path: PathBuf) {
        if let Ok(mut packages) = self.bundled_packages.lock() {
            packages.insert(module_id, path);
        }
    }

    pub(crate) fn bundled_packages(&self) -> BTreeMap<ModuleId, PathBuf> {
        self.bundled_packages
            .lock()
            .map(|packages| packages.clone())
            .unwrap_or_default()
    }

    pub(crate) fn refresh_registry(&self) -> Result<(), AppStateError> {
        let installer = self.package_installer();
        let modules = installer.list()?;
        self.refresh_registry_from_modules(modules)
    }

    pub(crate) fn refresh_registry_during_uninstall(
        &self,
        installer: &PackageInstaller,
    ) -> Result<(), AppStateError> {
        let modules = installer.list_without_recovery()?;
        self.refresh_registry_from_modules(modules)
    }

    fn refresh_registry_from_modules(
        &self,
        modules: Vec<InstalledModule>,
    ) -> Result<(), AppStateError> {
        let settings = self.settings();
        let providers = CapabilityRegistry::new().resolve_runtime(&modules, &settings)?;
        let locale = LocaleResolver::from_core_json(&self.core_catalog, modules.clone())?;
        *self.modules.lock().map_err(|_| AppStateError::Storage {
            detail: "module state lock poisoned".to_owned(),
        })? = modules;
        *self.providers.lock().map_err(|_| AppStateError::Storage {
            detail: "provider state lock poisoned".to_owned(),
        })? = providers;
        *self.locale.lock().map_err(|_| AppStateError::Storage {
            detail: "locale state lock poisoned".to_owned(),
        })? = locale;
        Ok(())
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
        let previous_runtime = self
            .storage
            .lock()
            .map_err(|_| AppStateError::Storage {
                detail: "storage state lock poisoned".to_owned(),
            })?
            .as_ref()
            .map(|storage| (storage.app_paths.clone(), storage.database.clone()));
        let candidate_settings = self.settings();
        let settings_before_swap = candidate_settings.clone();
        let candidate_app_paths = self.configured_app_paths();
        WorkspacePaths::validate(&workspace_root, &candidate_app_paths).map_err(|error| {
            AppStateError::Storage {
                detail: error.to_string(),
            }
        })?;
        candidate_app_paths
            .ensure_directories()
            .map_err(|error| AppStateError::Storage {
                detail: error.to_string(),
            })?;
        let prepared_workspace = WorkspacePaths::with_source_inbox_roots(
            workspace_root.clone(),
            candidate_settings.source_inbox_roots.clone(),
        );
        let source_modules: Vec<InstalledModule> = self
            .modules()
            .into_iter()
            .filter(|module| {
                module.enabled && module.module_type == mfa_contracts::ModuleType::Source
            })
            .collect();
        for module in &source_modules {
            prepared_workspace
                .enable_source(&module.module_id)
                .map_err(|error| AppStateError::Storage {
                    detail: error.to_string(),
                })?;
        }

        let reuse_database =
            previous_runtime
                .as_ref()
                .and_then(|(previous_app_paths, database)| {
                    (previous_app_paths.database == candidate_app_paths.database)
                        .then(|| database.clone())
                });
        let candidate = self
            .build_workspace_runtime(
                workspace_root.clone(),
                candidate_app_paths.clone(),
                candidate_settings.clone(),
                reuse_database,
            )
            .await?;
        let mut settings = candidate_settings;
        settings.workspace_root = Some(workspace_root);
        settings.app_data_root = Some(candidate_app_paths.app_data.clone());
        let event_sink = match self
            .event_sink
            .lock()
            .map(|event_sink| event_sink.clone())
            .map_err(|_| ())
        {
            Ok(event_sink) => event_sink,
            Err(()) => {
                shutdown_candidate(candidate).await;
                return Err(AppStateError::Storage {
                    detail: "event sink state lock poisoned".to_owned(),
                });
            }
        };
        let CandidateStorageRuntime {
            runtime: candidate_runtime,
            owns_database,
        } = candidate;
        let candidate_database_identity = candidate_runtime.database.identity();
        let candidate_workspace = candidate_runtime.workspace.clone();
        let mut candidate_runtime = Some(candidate_runtime);
        let forwarders = event_sink.as_ref().map(|sink| {
            (
                Arc::clone(sink),
                candidate_runtime
                    .as_ref()
                    .expect("candidate runtime is retained")
                    .coordinators
                    .values()
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        });
        if let Err(error) =
            SettingsStore::new(self.config_root.join("settings.json")).save(&settings)
        {
            shutdown_candidate(CandidateStorageRuntime {
                runtime: candidate_runtime
                    .take()
                    .expect("candidate runtime is retained"),
                owns_database,
            })
            .await;
            return Err(error.into());
        }
        let swap_result = {
            let mut settings_state = self.settings.lock().map_err(|_| AppStateError::Storage {
                detail: "settings state lock poisoned".to_owned(),
            });
            let mut storage = self.storage.lock().map_err(|_| AppStateError::Storage {
                detail: "storage state lock poisoned".to_owned(),
            });
            match (&mut settings_state, &mut storage) {
                (Ok(settings_state), Ok(storage)) => {
                    **settings_state = settings.clone();
                    Ok(storage.replace(
                        candidate_runtime
                            .take()
                            .expect("candidate runtime is retained"),
                    ))
                }
                (Err(error), _) => Err(AppStateError::Storage {
                    detail: error.to_string(),
                }),
                (_, Err(error)) => Err(AppStateError::Storage {
                    detail: error.to_string(),
                }),
            }
        };
        let old_runtime = match swap_result {
            Ok(old_runtime) => old_runtime,
            Err(error) => {
                let _ = SettingsStore::new(self.config_root.join("settings.json"))
                    .save(&settings_before_swap);
                shutdown_candidate(CandidateStorageRuntime {
                    runtime: candidate_runtime
                        .take()
                        .expect("candidate runtime is retained"),
                    owns_database,
                })
                .await;
                return Err(error);
            }
        };
        if let Some((sink, coordinators)) = forwarders {
            spawn_event_forwarders(sink, coordinators);
        }
        if let Some(old_runtime) = old_runtime {
            let shares_database = old_runtime.database.identity() == candidate_database_identity;
            shutdown_runtime(old_runtime, !shares_database).await?;
        }
        Ok(candidate_workspace)
    }

    async fn build_workspace_runtime(
        &self,
        workspace_root: PathBuf,
        app_paths: AppPaths,
        settings: AppSettings,
        reusable_database: Option<DatabaseService>,
    ) -> Result<CandidateStorageRuntime, AppStateError> {
        let workspace = WorkspacePaths::with_source_inbox_roots(
            workspace_root.clone(),
            settings.source_inbox_roots.clone(),
        );
        let source_modules: Vec<InstalledModule> = self
            .modules()
            .into_iter()
            .filter(|module| {
                module.enabled && module.module_type == mfa_contracts::ModuleType::Source
            })
            .collect();
        for module in &source_modules {
            workspace
                .enable_source(&module.module_id)
                .map_err(|error| AppStateError::Storage {
                    detail: error.to_string(),
                })?;
        }

        let owns_database = reusable_database.is_none();
        let database = match reusable_database {
            Some(database) => database,
            None => DatabaseService::start(&app_paths.database, 32)
                .await
                .map_err(|error| AppStateError::Storage {
                    detail: error.to_string(),
                })?,
        };
        let recovery_gate = RecoveryGate::new();
        let reconcilers = source_modules
            .iter()
            .map(|module| ArchiveReconciler::new(workspace.clone(), module.module_id.clone()))
            .collect();
        if let Err(error) =
            recover_sources(database.clone(), reconcilers, recovery_gate.clone()).await
        {
            if owns_database {
                let _ = database.clone().shutdown().await;
            }
            return Err(AppStateError::Storage {
                detail: error.to_string(),
            });
        }
        let mut coordinators = BTreeMap::new();
        for module in source_modules {
            let module_id = module.module_id.clone();
            let coordinator = match self
                .start_source_coordinator(
                    workspace.clone(),
                    database.clone(),
                    recovery_gate.clone(),
                    module,
                )
                .await
            {
                Ok(coordinator) => coordinator,
                Err(error) => {
                    shutdown_coordinators(coordinators.into_values()).await;
                    if owns_database {
                        let _ = database.clone().shutdown().await;
                    }
                    return Err(error);
                }
            };
            coordinators.insert(module_id, coordinator);
        }

        Ok(CandidateStorageRuntime {
            runtime: StorageRuntime {
                workspace: workspace.clone(),
                app_paths: app_paths.clone(),
                database,
                coordinators,
                recovery_gate,
            },
            owns_database,
        })
    }

    pub fn set_event_sink(&self, sink: Arc<dyn DataChangedSink>) {
        if let Ok(mut event_sink) = self.event_sink.lock() {
            *event_sink = Some(Arc::clone(&sink));
        }
        let coordinators = self
            .storage
            .lock()
            .ok()
            .and_then(|storage| {
                storage
                    .as_ref()
                    .map(|storage| storage.coordinators.values().cloned().collect::<Vec<_>>())
            })
            .unwrap_or_default();
        if !coordinators.is_empty() {
            spawn_event_forwarders(sink, coordinators);
        }
    }

    pub(crate) fn storage_lock(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<StorageRuntime>>, AppStateError> {
        self.storage.lock().map_err(|_| AppStateError::Storage {
            detail: "storage state lock poisoned".to_owned(),
        })
    }

    pub fn storage_database_identity(&self) -> Option<usize> {
        self.storage
            .lock()
            .ok()
            .and_then(|storage| storage.as_ref().map(|storage| storage.database.identity()))
    }

    pub fn storage_recovery_gate_identity(&self) -> Option<usize> {
        self.storage.lock().ok().and_then(|storage| {
            storage
                .as_ref()
                .map(|storage| storage.recovery_gate.identity())
        })
    }

    pub fn storage_coordinator_identity(&self, module_id: &ModuleId) -> Option<usize> {
        self.storage.lock().ok().and_then(|storage| {
            storage
                .as_ref()
                .and_then(|storage| storage.coordinators.get(module_id))
                .map(IngestionCoordinator::identity)
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
        shutdown_runtime(runtime, true).await
    }
}

async fn shutdown_candidate(candidate: CandidateStorageRuntime) {
    let _ = shutdown_runtime(candidate.runtime, candidate.owns_database).await;
}

async fn shutdown_runtime(
    runtime: StorageRuntime,
    shutdown_database: bool,
) -> Result<(), AppStateError> {
    for coordinator in runtime.coordinators.into_values() {
        coordinator
            .shutdown()
            .await
            .map_err(|error| AppStateError::Storage {
                detail: error.to_string(),
            })?;
    }
    if shutdown_database {
        runtime
            .database
            .shutdown()
            .await
            .map_err(|error| AppStateError::Storage {
                detail: error.to_string(),
            })?;
    }
    Ok(())
}

async fn shutdown_coordinators(coordinators: impl IntoIterator<Item = IngestionCoordinator>) {
    for coordinator in coordinators {
        let _ = coordinator.shutdown().await;
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

#[cfg(test)]
mod tests {
    use super::AppState;
    use mfa_config::{AppSettings, SettingsStore};
    use mfa_db::HealthCheck;
    use tempfile::TempDir;

    #[tokio::test]
    async fn failed_candidate_start_restores_the_previous_storage_runtime() {
        let root = TempDir::new().unwrap();
        let config_root = root.path().join("config");
        let module_root = root.path().join("modules");
        std::fs::create_dir_all(&config_root).unwrap();
        SettingsStore::new(config_root.join("settings.json"))
            .save(&AppSettings::default())
            .unwrap();
        let state = AppState::from_roots_with_core_catalog(
            &config_root,
            &module_root,
            br#"{"locale":"en","namespace":"core","messages":{}}"#,
        )
        .unwrap();
        let old_workspace = root.path().join("old-workspace");
        state
            .configure_workspace(old_workspace.clone())
            .await
            .unwrap();
        let old_database_identity = state.storage_database_identity().unwrap();

        let candidate_app_data = root.path().join("candidate-app-data");
        std::fs::create_dir_all(candidate_app_data.join("myfitanalytics.duckdb")).unwrap();
        let mut settings = state.settings();
        settings.app_data_root = Some(candidate_app_data);
        state.save_settings(settings).unwrap();

        let error = state
            .configure_workspace(root.path().join("new-workspace"))
            .await
            .unwrap_err();
        assert_eq!(error.code(), "storage_initialization");
        assert_eq!(
            state.storage_database_identity(),
            Some(old_database_identity)
        );
        let database = state
            .storage_lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .database
            .clone();
        database.execute(HealthCheck).await.unwrap();
        assert_eq!(state.settings().workspace_root, Some(old_workspace));
    }
}
