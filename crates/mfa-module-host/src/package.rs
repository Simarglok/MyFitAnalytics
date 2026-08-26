use crate::error::PackageError;
use crate::store::{
    ActivePackage, BundledCatalogEntry, ModuleState, UninstallJournal, UninstallPhase,
    clear_uninstall_journal, load_state, load_uninstall_journal, save_state,
    save_uninstall_journal, sync_directory,
};
use jsonschema::validator_for;
use mfa_contracts::{
    ContractVersion, DashboardManifest, LocaleManifest, ModuleId, ModuleManifest, ModuleType,
    SourceManifest,
};
use semver::{Version, VersionReq};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;
use zip::CompressionMethod;
use zip::ZipArchive;
use zip::write::SimpleFileOptions;

const DEFAULT_MAX_UNCOMPRESSED_BYTES: u64 = 67_108_864;
const ENTRYPOINT_NAME: &str = "module.wasm";
const HOST_API_RANGE: &str = ">=1.0.0, <2.0.0";

const SOURCE_SCHEMA: &str =
    include_str!("../../../modules/sdk/schemas/source-manifest.schema.json");
const DASHBOARD_SCHEMA: &str =
    include_str!("../../../modules/sdk/schemas/dashboard-manifest.schema.json");
const LOCALE_SCHEMA: &str =
    include_str!("../../../modules/sdk/schemas/locale-manifest.schema.json");

#[derive(Debug, Clone)]
pub struct InspectedEntry {
    pub path: String,
    pub bytes: Vec<u8>,
    pub is_dir: bool,
    pub unix_mode: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct InspectedPackage {
    pub manifest: ModuleManifest,
    pub package_hash: String,
    pub entries: Vec<InspectedEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledModule {
    pub module_id: ModuleId,
    pub module_type: ModuleType,
    pub module_version: ContractVersion,
    pub package_hash: String,
    pub root: PathBuf,
    pub enabled: bool,
    pub manifest: ModuleManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledPackageInfo {
    pub module_id: ModuleId,
    pub module_version: ContractVersion,
    pub package_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledModuleUpdate {
    pub available: BundledPackageInfo,
    pub installed: Option<BundledPackageInfo>,
}

pub struct UninstallTransaction {
    module_id: ModuleId,
    original_root: PathBuf,
    staged_root: PathBuf,
    backup_path: PathBuf,
    version_root: PathBuf,
    previous_state: ModuleState,
    phase: UninstallPhase,
    state_applied: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallFinalizationFault {
    BeforeDelete,
    AfterDeleteBeforeRead,
    AfterReadBeforeRemoveVersion,
    AfterRemoveBeforeSync,
    BeforeBackupDelete,
    AfterBackupDeleteBeforeSync,
    BeforeJournalClear,
    BeforeRegistryRefresh,
    BeforeRestoreMove,
    BeforeRestoreRead,
    BeforeStateSync,
    BeforeStageMove,
    AfterStageMoveBeforeSync,
    BeforeBackupRead,
}

impl UninstallTransaction {
    fn journal(&self) -> UninstallJournal {
        UninstallJournal {
            module_id: self.module_id.to_string(),
            original_root: self.original_root.clone(),
            staged_root: self.staged_root.clone(),
            backup_path: self.backup_path.clone(),
            version_root: self.version_root.clone(),
            previous_state: self.previous_state.clone(),
            phase: self.phase,
        }
    }

    fn from_journal(store_root: &Path, journal: &UninstallJournal) -> Result<Self, PackageError> {
        let module_id = ModuleId::try_from(journal.module_id.as_str()).map_err(|error| {
            PackageError::StateInvalid {
                detail: format!("uninstall journal contains an invalid module id: {error}"),
            }
        })?;
        validate_path_component(module_id.as_str(), "module id")?;
        let active_package = journal
            .previous_state
            .active_packages
            .get(module_id.as_str())
            .ok_or_else(|| PackageError::AtomicUninstall {
                detail: "uninstall journal has no active package snapshot".to_owned(),
            })?;
        let module_version = ContractVersion::try_from(active_package.module_version.clone())
            .map_err(|error| PackageError::AtomicUninstall {
                detail: format!("uninstall journal has an invalid module version: {error}"),
            })?;
        validate_package_hash(&active_package.package_hash)?;
        let staged_transaction_id = transaction_id_from_path(
            &journal.staged_root,
            ".uninstall-staging-",
            "uninstall staging path",
            "",
        )?;
        let backup_transaction_id = transaction_id_from_path(
            &journal.backup_path,
            ".uninstall-backup-",
            "uninstall backup path",
            ".zip",
        )?;
        if staged_transaction_id != backup_transaction_id {
            return Err(PackageError::AtomicUninstall {
                detail: "uninstall journal transaction paths do not share an id".to_owned(),
            });
        }
        let expected_version_root = store_root
            .join(module_id.as_str())
            .join(module_version.to_string());
        let expected_original_root = expected_version_root.join(&active_package.package_hash);
        let expected_staged_root =
            expected_version_root.join(format!(".uninstall-staging-{staged_transaction_id}"));
        let expected_backup_path =
            store_root.join(format!(".uninstall-backup-{staged_transaction_id}.zip"));
        let canonical_store_root =
            fs::canonicalize(store_root).map_err(|error| PackageError::AtomicUninstall {
                detail: format!("canonicalize uninstall store root failed: {error}"),
            })?;
        validate_transaction_path(
            store_root,
            &canonical_store_root,
            &journal.original_root,
            &expected_original_root,
            "original package path",
        )?;
        validate_transaction_path(
            store_root,
            &canonical_store_root,
            &journal.staged_root,
            &expected_staged_root,
            "staged package path",
        )?;
        validate_transaction_path(
            store_root,
            &canonical_store_root,
            &journal.backup_path,
            &expected_backup_path,
            "backup package path",
        )?;
        validate_transaction_path(
            store_root,
            &canonical_store_root,
            &journal.version_root,
            &expected_version_root,
            "version package path",
        )?;
        Ok(Self {
            module_id,
            original_root: expected_original_root,
            staged_root: expected_staged_root,
            backup_path: expected_backup_path,
            version_root: expected_version_root,
            previous_state: journal.previous_state.clone(),
            phase: journal.phase,
            state_applied: matches!(
                journal.phase,
                UninstallPhase::StateApplied
                    | UninstallPhase::PackageRemoved
                    | UninstallPhase::Committed
            ),
        })
    }
}

fn validate_path_component(value: &str, label: &str) -> Result<(), PackageError> {
    let mut components = Path::new(value).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(PackageError::AtomicUninstall {
            detail: format!("uninstall journal {label} is not a safe path component"),
        });
    }
    Ok(())
}

fn validate_package_hash(value: &str) -> Result<(), PackageError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PackageError::AtomicUninstall {
            detail: "uninstall journal has an invalid package hash".to_owned(),
        });
    }
    Ok(())
}

fn transaction_id_from_path(
    path: &Path,
    prefix: &str,
    label: &str,
    suffix: &str,
) -> Result<Uuid, PackageError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| PackageError::AtomicUninstall {
            detail: format!("uninstall journal {label} has no valid filename"),
        })?;
    let value = name
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
        .ok_or_else(|| PackageError::AtomicUninstall {
            detail: format!("uninstall journal {label} has an invalid filename"),
        })?;
    Uuid::parse_str(value).map_err(|error| PackageError::AtomicUninstall {
        detail: format!("uninstall journal {label} has an invalid transaction id: {error}"),
    })
}

fn validate_transaction_path(
    store_root: &Path,
    canonical_store_root: &Path,
    actual: &Path,
    expected: &Path,
    label: &str,
) -> Result<(), PackageError> {
    if actual != expected {
        return Err(PackageError::AtomicUninstall {
            detail: format!("uninstall journal {label} is outside the transaction layout"),
        });
    }
    ensure_no_symlink_components(store_root, actual, label)?;
    let canonical = canonicalize_existing_ancestor(actual, label)?;
    if !canonical.starts_with(canonical_store_root) {
        return Err(PackageError::AtomicUninstall {
            detail: format!("uninstall journal {label} resolves outside the store root"),
        });
    }
    Ok(())
}

fn ensure_no_symlink_components(
    store_root: &Path,
    path: &Path,
    label: &str,
) -> Result<(), PackageError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                current.pop();
            }
            Component::Normal(name) => current.push(name),
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                if current == store_root || !store_root.starts_with(&current) {
                    return Err(PackageError::AtomicUninstall {
                        detail: format!("uninstall journal {label} contains a symlink"),
                    });
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(PackageError::AtomicUninstall {
                    detail: format!("inspect uninstall journal {label} failed: {error}"),
                });
            }
        }
    }
    Ok(())
}

fn canonicalize_existing_ancestor(path: &Path, label: &str) -> Result<PathBuf, PackageError> {
    let mut current = path.to_path_buf();
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(&current) {
            Ok(_) => {
                let mut canonical =
                    fs::canonicalize(&current).map_err(|error| PackageError::AtomicUninstall {
                        detail: format!("canonicalize uninstall journal {label} failed: {error}"),
                    })?;
                for component in suffix.iter().rev() {
                    canonical.push(component);
                }
                return Ok(canonical);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = current
                    .file_name()
                    .ok_or_else(|| PackageError::AtomicUninstall {
                        detail: format!("uninstall journal {label} has no existing ancestor"),
                    })?
                    .to_os_string();
                suffix.push(name);
                if !current.pop() {
                    return Err(PackageError::AtomicUninstall {
                        detail: format!("uninstall journal {label} has no existing ancestor"),
                    });
                }
            }
            Err(error) => {
                return Err(PackageError::AtomicUninstall {
                    detail: format!("inspect uninstall journal {label} failed: {error}"),
                });
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PackageInstaller {
    pub(crate) store_root: PathBuf,
    max_uncompressed_bytes: u64,
    host_api_range: VersionReq,
    current_app_version: Version,
    uninstall_finalization_fault: Option<UninstallFinalizationFault>,
}

impl PackageInstaller {
    pub fn new(store_root: impl Into<PathBuf>) -> Self {
        Self::with_app_version(
            store_root,
            Version::parse(env!("CARGO_PKG_VERSION")).expect("package version is valid semver"),
        )
    }

    pub fn with_app_version(store_root: impl Into<PathBuf>, current_app_version: Version) -> Self {
        Self {
            store_root: store_root.into(),
            max_uncompressed_bytes: DEFAULT_MAX_UNCOMPRESSED_BYTES,
            host_api_range: VersionReq::parse(HOST_API_RANGE).expect("static host API range"),
            current_app_version,
            uninstall_finalization_fault: None,
        }
    }

    pub fn with_max_uncompressed_bytes(mut self, limit: u64) -> Self {
        self.max_uncompressed_bytes = limit;
        self
    }

    pub fn with_uninstall_finalization_fault(mut self, fault: UninstallFinalizationFault) -> Self {
        self.uninstall_finalization_fault = Some(fault);
        self
    }

    pub fn store_root(&self) -> &Path {
        &self.store_root
    }

    pub fn inspect(&self, package: &Path) -> Result<InspectedPackage, PackageError> {
        self.recover_pending_uninstall()?;
        let bytes = fs::read(package).map_err(PackageError::from)?;
        let package_hash = digest(&bytes);
        detect_duplicate_archive_entries(&bytes)?;
        let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(PackageError::from)?;
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        let mut total_uncompressed = 0u64;

        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).map_err(PackageError::from)?;
            let raw_path = entry.name().to_owned();
            let path = normalize_archive_path(&raw_path)?;
            if !seen.insert(path.clone()) {
                if path == "module.json" {
                    return Err(PackageError::DuplicateManifest);
                }
                return Err(PackageError::DuplicateEntry { path });
            }
            if entry.is_symlink() {
                return Err(PackageError::SymlinkEntry { path });
            }
            total_uncompressed = total_uncompressed
                .checked_add(entry.size())
                .ok_or(PackageError::UncompressedSizeLimit)?;
            if total_uncompressed > self.max_uncompressed_bytes {
                return Err(PackageError::UncompressedSizeLimit);
            }
            let is_dir = entry.is_dir();
            let unix_mode = entry.unix_mode();
            let mut content = Vec::new();
            if !is_dir {
                entry
                    .read_to_end(&mut content)
                    .map_err(PackageError::from)?;
            }
            entries.push(InspectedEntry {
                path,
                bytes: content,
                is_dir,
                unix_mode,
            });
        }

        let manifest_entry = entries
            .iter()
            .find(|entry| entry.path == "module.json" && !entry.is_dir)
            .ok_or(PackageError::ManifestMissing)?;
        let manifest_value: Value =
            serde_json::from_slice(&manifest_entry.bytes).map_err(|error| {
                PackageError::ManifestInvalidJson {
                    detail: error.to_string(),
                }
            })?;
        let manifest = parse_manifest(&manifest_value)?;
        validate_payload_security(&entries, manifest.module_type())?;
        validate_manifest_schema(&manifest_value, manifest.module_type())?;
        validate_package_extension(package, manifest.module_type())?;
        validate_app_version_compatibility(&manifest, &self.current_app_version)?;
        validate_package_compatibility(&manifest, &self.host_api_range)?;
        validate_entrypoint(&manifest, &entries)?;

        Ok(InspectedPackage {
            manifest,
            package_hash,
            entries,
        })
    }

    pub fn install(&self, package: &Path) -> Result<InstalledModule, PackageError> {
        let inspected = self.inspect(package)?;
        let (module_id, module_version, module_type) = manifest_identity(&inspected.manifest);
        let final_root = self
            .store_root
            .join(module_id.as_str())
            .join(module_version.to_string())
            .join(&inspected.package_hash);
        if final_root.exists() {
            self.ensure_default_state(module_id)?;
            self.activate_package(module_id, module_version, &inspected.package_hash)?;
            return self.installed_from_inspection(&inspected, final_root);
        }

        let parent = final_root
            .parent()
            .ok_or_else(|| PackageError::AtomicInstall {
                detail: "module installation path has no parent".to_owned(),
            })?;
        fs::create_dir_all(parent).map_err(PackageError::from)?;
        let staging = parent.join(format!(".staging-{}", Uuid::new_v4()));
        fs::create_dir(&staging).map_err(PackageError::from)?;
        let result = (|| {
            for entry in &inspected.entries {
                let destination = staging.join(&entry.path);
                if entry.is_dir {
                    fs::create_dir_all(&destination).map_err(PackageError::from)?;
                    continue;
                }
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(PackageError::from)?;
                }
                let mut file = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&destination)
                    .map_err(PackageError::from)?;
                file.write_all(&entry.bytes).map_err(PackageError::from)?;
                file.sync_all().map_err(PackageError::from)?;
            }
            sync_directory(&staging)?;
            if final_root.exists() {
                return Ok(());
            }
            fs::rename(&staging, &final_root).map_err(PackageError::from)?;
            sync_directory(parent)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result?;
        self.ensure_default_state(module_id)?;
        self.activate_package(module_id, module_version, &inspected.package_hash)?;
        let _ = module_type;
        self.installed_from_inspection(&inspected, final_root)
    }

    pub fn install_bundled_defaults(
        &self,
        packages: &[PathBuf],
    ) -> Result<Vec<BundledPackageInfo>, PackageError> {
        self.recover_pending_uninstall()?;
        let inspected = packages
            .iter()
            .map(|package| self.inspect(package).map(|value| (package, value)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut state = load_state(&self.store_root)?;
        for (_, package) in &inspected {
            let (module_id, module_version, _) = manifest_identity(&package.manifest);
            state.bundled_catalog.insert(
                module_id.to_string(),
                BundledCatalogEntry {
                    module_version: module_version.to_string(),
                    package_hash: package.package_hash.clone(),
                },
            );
        }
        save_state(&self.store_root, &state)?;
        let installed = self.current_registry()?;
        for (path, package) in inspected {
            let (module_id, _, _) = manifest_identity(&package.manifest);
            let explicitly_disabled = state
                .modules
                .get(module_id.as_str())
                .is_some_and(|enabled| !enabled);
            let already_installed = installed
                .iter()
                .any(|module| &module.module_id == module_id);
            if !explicitly_disabled && !already_installed {
                self.install(path)?;
            }
        }
        self.bundled_catalog()
    }

    pub fn bundled_catalog(&self) -> Result<Vec<BundledPackageInfo>, PackageError> {
        self.recover_pending_uninstall()?;
        let state = load_state(&self.store_root)?;
        state
            .bundled_catalog
            .into_iter()
            .map(|(module_id, entry)| catalog_info(module_id, entry))
            .collect()
    }

    pub fn available_bundled_updates(&self) -> Result<Vec<BundledModuleUpdate>, PackageError> {
        let catalog = self.bundled_catalog()?;
        let installed = self.current_registry()?;
        Ok(catalog
            .into_iter()
            .filter_map(|available| {
                let installed = installed
                    .iter()
                    .filter(|module| module.module_id == available.module_id)
                    .max_by(|left, right| left.module_version.cmp(&right.module_version))
                    .map(|module| BundledPackageInfo {
                        module_id: module.module_id.clone(),
                        module_version: module.module_version.clone(),
                        package_hash: module.package_hash.clone(),
                    });
                if installed
                    .as_ref()
                    .is_some_and(|installed| installed.module_version >= available.module_version)
                {
                    None
                } else {
                    Some(BundledModuleUpdate {
                        available,
                        installed,
                    })
                }
            })
            .collect())
    }

    pub fn list_without_recovery(&self) -> Result<Vec<InstalledModule>, PackageError> {
        self.check_uninstall_fault(UninstallFinalizationFault::BeforeRegistryRefresh)?;
        self.current_registry_without_recovery()
    }

    pub fn set_enabled(&self, id: &ModuleId, enabled: bool) -> Result<(), PackageError> {
        self.recover_pending_uninstall()?;
        let module = self
            .current_registry()?
            .into_iter()
            .find(|module| &module.module_id == id)
            .ok_or_else(|| PackageError::ModuleNotFound {
                module_id: id.to_string(),
            })?;
        if enabled && self.installed_app_compatibility_error(&module).is_some() {
            return Err(PackageError::IncompatibleAppVersion);
        }
        let mut state = load_state(&self.store_root)?;
        state.modules.insert(id.to_string(), enabled);
        save_state(&self.store_root, &state)
    }

    pub fn installed_app_compatibility_error(
        &self,
        module: &InstalledModule,
    ) -> Option<&'static str> {
        validate_app_version_compatibility(&module.manifest, &self.current_app_version)
            .err()
            .map(|error| error.code())
    }

    pub fn uninstall(&self, id: &ModuleId) -> Result<(), PackageError> {
        let mut transaction = self.stage_uninstall(id)?;
        if let Err(error) = self.apply_uninstall(&mut transaction) {
            return self.return_after_rollback(error, &mut transaction);
        }
        if let Err(error) = self.finalize_uninstall(&mut transaction) {
            return self.return_after_rollback(error, &mut transaction);
        }
        Ok(())
    }

    pub fn stage_uninstall(&self, id: &ModuleId) -> Result<UninstallTransaction, PackageError> {
        self.recover_pending_uninstall()?;
        let selected = self
            .current_registry()?
            .into_iter()
            .find(|module| &module.module_id == id)
            .ok_or_else(|| PackageError::ModuleNotFound {
                module_id: id.to_string(),
            })?;
        let original_root = selected.root.clone();
        let version_root = original_root
            .parent()
            .ok_or_else(|| PackageError::AtomicInstall {
                detail: "installed module has no version parent".to_owned(),
            })?
            .to_path_buf();
        let transaction_id = Uuid::new_v4();
        let staged_root = version_root.join(format!(".uninstall-staging-{transaction_id}"));
        let backup_path = self
            .store_root
            .join(format!(".uninstall-backup-{transaction_id}.zip"));
        let previous_state = load_state(&self.store_root)?;
        let mut transaction = UninstallTransaction {
            module_id: id.clone(),
            original_root,
            staged_root,
            backup_path,
            version_root,
            previous_state,
            phase: UninstallPhase::Prepared,
            state_applied: false,
        };
        save_uninstall_journal(&self.store_root, &transaction.journal())?;

        let result = (|| {
            self.check_uninstall_fault(UninstallFinalizationFault::BeforeStageMove)?;
            fs::rename(&transaction.original_root, &transaction.staged_root).map_err(|error| {
                PackageError::AtomicUninstall {
                    detail: format!("move package into uninstall staging failed: {error}"),
                }
            })?;
            transaction.phase = UninstallPhase::Moved;
            self.check_uninstall_fault(UninstallFinalizationFault::AfterStageMoveBeforeSync)?;
            sync_directory(&transaction.version_root).map_err(|error| {
                PackageError::AtomicUninstall {
                    detail: format!("sync uninstall staging directory failed: {error}"),
                }
            })?;
            save_uninstall_journal(&self.store_root, &transaction.journal())?;
            self.backup_package(&transaction.staged_root, &transaction.backup_path)?;
            transaction.phase = UninstallPhase::BackedUp;
            save_uninstall_journal(&self.store_root, &transaction.journal())?;
            Ok::<(), PackageError>(())
        })();
        if let Err(error) = result {
            return self.return_after_rollback(error, &mut transaction);
        }
        Ok(transaction)
    }

    pub fn apply_uninstall(
        &self,
        transaction: &mut UninstallTransaction,
    ) -> Result<(), PackageError> {
        if transaction.phase != UninstallPhase::BackedUp {
            return Err(PackageError::AtomicUninstall {
                detail: "cannot apply an uninstall before its rollback backup is durable"
                    .to_owned(),
            });
        }
        let mut state = transaction.previous_state.clone();
        state
            .modules
            .insert(transaction.module_id.to_string(), false);
        state.active_packages.remove(transaction.module_id.as_str());
        state
            .uninstalled_modules
            .insert(transaction.module_id.to_string());
        self.check_uninstall_fault(UninstallFinalizationFault::BeforeStateSync)?;
        save_state(&self.store_root, &state)?;
        transaction.state_applied = true;
        transaction.phase = UninstallPhase::StateApplied;
        save_uninstall_journal(&self.store_root, &transaction.journal())?;
        Ok(())
    }

    pub fn rollback_uninstall(
        &self,
        transaction: &mut UninstallTransaction,
    ) -> Result<(), PackageError> {
        let backup_exists = path_exists(&transaction.backup_path)?;
        let staged_exists = path_exists(&transaction.staged_root)?;
        let original_exists = path_exists(&transaction.original_root)?;
        if transaction.phase == UninstallPhase::Committed
            && !backup_exists
            && !staged_exists
            && !original_exists
        {
            return Err(PackageError::AtomicUninstall {
                detail:
                    "cannot roll back a committed uninstall after its rollback backup was removed"
                        .to_owned(),
            });
        }
        self.restore_transaction_files(transaction)?;
        save_state(&self.store_root, &transaction.previous_state)?;
        transaction.state_applied = false;
        transaction.phase = UninstallPhase::BackedUp;
        clear_uninstall_journal(&self.store_root)?;
        Ok(())
    }

    pub fn finalize_uninstall(
        &self,
        transaction: &mut UninstallTransaction,
    ) -> Result<(), PackageError> {
        if !transaction.state_applied || transaction.phase != UninstallPhase::StateApplied {
            return Err(PackageError::AtomicUninstall {
                detail: "cannot finalize an unapplied uninstall".to_owned(),
            });
        }
        self.check_uninstall_fault(UninstallFinalizationFault::BeforeDelete)?;
        fs::remove_dir_all(&transaction.staged_root).map_err(|error| {
            PackageError::AtomicUninstall {
                detail: format!("remove staged package failed: {error}"),
            }
        })?;
        self.check_uninstall_fault(UninstallFinalizationFault::AfterDeleteBeforeRead)?;
        sync_directory(&transaction.version_root).map_err(|error| {
            PackageError::AtomicUninstall {
                detail: format!("sync package version directory failed: {error}"),
            }
        })?;
        let mut entries =
            transaction
                .version_root
                .read_dir()
                .map_err(|error| PackageError::AtomicUninstall {
                    detail: format!("read package version directory failed: {error}"),
                })?;
        let version_root_is_empty = entries
            .next()
            .transpose()
            .map_err(|error| PackageError::AtomicUninstall {
                detail: format!("read package version directory entry failed: {error}"),
            })?
            .is_none();
        self.check_uninstall_fault(UninstallFinalizationFault::AfterReadBeforeRemoveVersion)?;
        if version_root_is_empty {
            fs::remove_dir(&transaction.version_root).map_err(|error| {
                PackageError::AtomicUninstall {
                    detail: format!("remove empty package version directory failed: {error}"),
                }
            })?;
        }
        self.check_uninstall_fault(UninstallFinalizationFault::AfterRemoveBeforeSync)?;
        if let Some(parent) = transaction.version_root.parent() {
            sync_directory(parent).map_err(|error| PackageError::AtomicUninstall {
                detail: format!("sync package module directory failed: {error}"),
            })?;
        }
        transaction.phase = UninstallPhase::PackageRemoved;
        save_uninstall_journal(&self.store_root, &transaction.journal())?;
        transaction.phase = UninstallPhase::Committed;
        save_uninstall_journal(&self.store_root, &transaction.journal())?;
        self.check_uninstall_fault(UninstallFinalizationFault::BeforeBackupDelete)?;
        if fs::remove_file(&transaction.backup_path).is_err() {
            return Ok(());
        }
        if self
            .check_uninstall_fault(UninstallFinalizationFault::AfterBackupDeleteBeforeSync)
            .is_err()
        {
            return Ok(());
        }
        if sync_directory(&self.store_root).is_err() {
            return Ok(());
        }
        if self
            .check_uninstall_fault(UninstallFinalizationFault::BeforeJournalClear)
            .is_err()
        {
            return Ok(());
        }
        if clear_uninstall_journal(&self.store_root).is_err() {
            return Ok(());
        }
        Ok(())
    }

    fn return_after_rollback<T>(
        &self,
        error: PackageError,
        transaction: &mut UninstallTransaction,
    ) -> Result<T, PackageError> {
        match self.rollback_uninstall(transaction) {
            Ok(()) => Err(error),
            Err(rollback) => Err(PackageError::AtomicUninstall {
                detail: format!("{error}; rollback failed: {rollback}"),
            }),
        }
    }

    fn check_uninstall_fault(&self, point: UninstallFinalizationFault) -> Result<(), PackageError> {
        if self.uninstall_finalization_fault == Some(point) {
            return Err(PackageError::AtomicUninstall {
                detail: format!("injected uninstall failure at {point:?}"),
            });
        }
        Ok(())
    }

    fn recover_pending_uninstall(&self) -> Result<(), PackageError> {
        let Some(journal) = load_uninstall_journal(&self.store_root)? else {
            return Ok(());
        };
        let transaction = UninstallTransaction::from_journal(&self.store_root, &journal)?;
        let backup_exists = path_exists(&transaction.backup_path)?;
        let staged_exists = path_exists(&transaction.staged_root)?;
        let original_exists = path_exists(&transaction.original_root)?;
        if transaction.phase == UninstallPhase::Committed {
            if staged_exists || original_exists {
                return Err(PackageError::AtomicUninstall {
                    detail: "committed uninstall has recoverable package bytes".to_owned(),
                });
            }
            if backup_exists {
                if fs::remove_file(&transaction.backup_path).is_err() {
                    return Ok(());
                }
                if sync_directory(&self.store_root).is_err() {
                    return Ok(());
                }
            }
            if clear_uninstall_journal(&self.store_root).is_err() {
                return Ok(());
            }
            return Ok(());
        }
        self.restore_transaction_files(&transaction)?;
        save_state(&self.store_root, &transaction.previous_state)?;
        clear_uninstall_journal(&self.store_root)
    }

    fn backup_package(&self, source: &Path, backup: &Path) -> Result<(), PackageError> {
        let parent = backup
            .parent()
            .ok_or_else(|| PackageError::AtomicUninstall {
                detail: "uninstall backup has no parent directory".to_owned(),
            })?;
        fs::create_dir_all(parent).map_err(|error| PackageError::AtomicUninstall {
            detail: format!("create uninstall backup directory failed: {error}"),
        })?;
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(backup)
            .map_err(|error| PackageError::AtomicUninstall {
                detail: format!("create uninstall rollback backup failed: {error}"),
            })?;
        let mut archive = zip::ZipWriter::new(file);
        let mut files = Vec::new();
        collect_package_files(source, Path::new(""), &mut files)?;
        files.sort_by(|left, right| left.0.cmp(&right.0));
        for (relative, path) in files {
            self.check_uninstall_fault(UninstallFinalizationFault::BeforeBackupRead)?;
            let bytes = fs::read(&path).map_err(|error| PackageError::AtomicUninstall {
                detail: format!("read package for uninstall rollback backup failed: {error}"),
            })?;
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            archive
                .start_file(relative.to_string_lossy().as_ref(), options)
                .map_err(|error| PackageError::AtomicUninstall {
                    detail: format!("write uninstall rollback backup entry failed: {error}"),
                })?;
            archive
                .write_all(&bytes)
                .map_err(|error| PackageError::AtomicUninstall {
                    detail: format!("write uninstall rollback backup failed: {error}"),
                })?;
        }
        let file = archive
            .finish()
            .map_err(|error| PackageError::AtomicUninstall {
                detail: format!("finish uninstall rollback backup failed: {error}"),
            })?;
        file.sync_all()
            .map_err(|error| PackageError::AtomicUninstall {
                detail: format!("sync uninstall rollback backup failed: {error}"),
            })?;
        sync_directory(parent).map_err(|error| PackageError::AtomicUninstall {
            detail: format!("sync uninstall rollback backup directory failed: {error}"),
        })
    }

    fn restore_transaction_files(
        &self,
        transaction: &UninstallTransaction,
    ) -> Result<(), PackageError> {
        let staged_exists = path_exists(&transaction.staged_root)?;
        let original_exists = path_exists(&transaction.original_root)?;
        let backup_exists = path_exists(&transaction.backup_path)?;
        let use_staged = matches!(
            transaction.phase,
            UninstallPhase::Prepared | UninstallPhase::Moved
        ) && staged_exists;
        if use_staged {
            if original_exists {
                fs::remove_dir_all(&transaction.original_root).map_err(|error| {
                    PackageError::AtomicUninstall {
                        detail: format!(
                            "remove conflicting package during rollback failed: {error}"
                        ),
                    }
                })?;
            }
            self.check_uninstall_fault(UninstallFinalizationFault::BeforeRestoreMove)?;
            fs::rename(&transaction.staged_root, &transaction.original_root).map_err(|error| {
                PackageError::AtomicUninstall {
                    detail: format!("move staged package back during rollback failed: {error}"),
                }
            })?;
            if let Some(parent) = transaction.original_root.parent() {
                sync_directory(parent).map_err(|error| PackageError::AtomicUninstall {
                    detail: format!("sync restored package directory failed: {error}"),
                })?;
            }
        } else if backup_exists {
            self.restore_from_backup(transaction)?;
        } else if !original_exists {
            return Err(PackageError::AtomicUninstall {
                detail: "uninstall rollback has neither staged package nor rollback backup"
                    .to_owned(),
            });
        }

        if path_exists(&transaction.staged_root)? {
            fs::remove_dir_all(&transaction.staged_root).map_err(|error| {
                PackageError::AtomicUninstall {
                    detail: format!("remove staged package after rollback failed: {error}"),
                }
            })?;
        }

        if path_exists(&transaction.backup_path)? {
            fs::remove_file(&transaction.backup_path).map_err(|error| {
                PackageError::AtomicUninstall {
                    detail: format!(
                        "remove uninstall rollback backup after restore failed: {error}"
                    ),
                }
            })?;
            sync_directory(&self.store_root).map_err(|error| PackageError::AtomicUninstall {
                detail: format!("sync module store after rollback failed: {error}"),
            })?;
        }
        Ok(())
    }

    fn restore_from_backup(&self, transaction: &UninstallTransaction) -> Result<(), PackageError> {
        self.check_uninstall_fault(UninstallFinalizationFault::BeforeRestoreRead)?;
        let bytes =
            fs::read(&transaction.backup_path).map_err(|error| PackageError::AtomicUninstall {
                detail: format!("read uninstall rollback backup failed: {error}"),
            })?;
        let parent =
            transaction
                .original_root
                .parent()
                .ok_or_else(|| PackageError::AtomicUninstall {
                    detail: "uninstall rollback package has no parent directory".to_owned(),
                })?;
        fs::create_dir_all(parent).map_err(|error| PackageError::AtomicUninstall {
            detail: format!("create package parent during rollback failed: {error}"),
        })?;
        let restore_root = parent.join(format!(".uninstall-restore-{}", Uuid::new_v4()));
        let result = (|| {
            fs::create_dir(&restore_root).map_err(|error| PackageError::AtomicUninstall {
                detail: format!("create temporary uninstall restore directory failed: {error}"),
            })?;
            let cursor = Cursor::new(bytes);
            let mut archive =
                ZipArchive::new(cursor).map_err(|error| PackageError::AtomicUninstall {
                    detail: format!("open uninstall rollback backup failed: {error}"),
                })?;
            for index in 0..archive.len() {
                let mut entry =
                    archive
                        .by_index(index)
                        .map_err(|error| PackageError::AtomicUninstall {
                            detail: format!("read uninstall rollback backup entry failed: {error}"),
                        })?;
                let relative = normalize_archive_path(entry.name())?;
                let destination = restore_root.join(Path::new(&relative));
                if entry.is_dir() {
                    fs::create_dir_all(&destination).map_err(|error| {
                        PackageError::AtomicUninstall {
                            detail: format!("create restored package directory failed: {error}"),
                        }
                    })?;
                    continue;
                }
                if let Some(destination_parent) = destination.parent() {
                    fs::create_dir_all(destination_parent).map_err(|error| {
                        PackageError::AtomicUninstall {
                            detail: format!("create restored package parent failed: {error}"),
                        }
                    })?;
                }
                let mut output = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&destination)
                    .map_err(|error| PackageError::AtomicUninstall {
                        detail: format!("create restored package entry failed: {error}"),
                    })?;
                let mut content = Vec::new();
                entry
                    .read_to_end(&mut content)
                    .map_err(|error| PackageError::AtomicUninstall {
                        detail: format!("read restored package entry failed: {error}"),
                    })?;
                output
                    .write_all(&content)
                    .map_err(|error| PackageError::AtomicUninstall {
                        detail: format!("write restored package entry failed: {error}"),
                    })?;
                output
                    .sync_all()
                    .map_err(|error| PackageError::AtomicUninstall {
                        detail: format!("sync restored package entry failed: {error}"),
                    })?;
            }
            sync_directory(&restore_root).map_err(|error| PackageError::AtomicUninstall {
                detail: format!("sync temporary uninstall restore directory failed: {error}"),
            })?;
            if path_exists(&transaction.original_root)? {
                fs::remove_dir_all(&transaction.original_root).map_err(|error| {
                    PackageError::AtomicUninstall {
                        detail: format!(
                            "remove incomplete package during rollback failed: {error}"
                        ),
                    }
                })?;
            }
            self.check_uninstall_fault(UninstallFinalizationFault::BeforeRestoreMove)?;
            fs::rename(&restore_root, &transaction.original_root).map_err(|error| {
                PackageError::AtomicUninstall {
                    detail: format!("move restored package into place failed: {error}"),
                }
            })?;
            sync_directory(parent).map_err(|error| PackageError::AtomicUninstall {
                detail: format!("sync restored package parent failed: {error}"),
            })?;
            Ok::<(), PackageError>(())
        })();
        if let Err(error) = result {
            match fs::remove_dir_all(&restore_root) {
                Ok(()) => {}
                Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => {}
                Err(cleanup) => {
                    return Err(PackageError::AtomicUninstall {
                        detail: format!("{error}; restore temporary cleanup failed: {cleanup}"),
                    });
                }
            }
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn reconstruct_registry(&self) -> Result<Vec<InstalledModule>, PackageError> {
        if !self.store_root.exists() {
            return Ok(Vec::new());
        }
        let state = load_state(&self.store_root)?;
        let mut modules = Vec::new();
        for id_entry in fs::read_dir(&self.store_root).map_err(PackageError::from)? {
            let id_entry = id_entry.map_err(PackageError::from)?;
            let id_path = id_entry.path();
            if !id_path.is_dir() || id_entry.file_name() == "state.json" {
                continue;
            }
            for version_entry in fs::read_dir(&id_path).map_err(PackageError::from)? {
                let version_entry = version_entry.map_err(PackageError::from)?;
                let version_path = version_entry.path();
                if !version_path.is_dir() {
                    continue;
                }
                for hash_entry in fs::read_dir(&version_path).map_err(PackageError::from)? {
                    let hash_entry = hash_entry.map_err(PackageError::from)?;
                    let root = hash_entry.path();
                    if !root.is_dir()
                        || hash_entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".staging-")
                        || hash_entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".uninstall-staging-")
                    {
                        continue;
                    }
                    let manifest_path = root.join("module.json");
                    let manifest_bytes = fs::read(&manifest_path).map_err(|_| {
                        PackageError::InstalledModuleCorrupt {
                            path: manifest_path.clone(),
                        }
                    })?;
                    let value: Value = serde_json::from_slice(&manifest_bytes).map_err(|_| {
                        PackageError::InstalledModuleCorrupt {
                            path: manifest_path.clone(),
                        }
                    })?;
                    let manifest = parse_manifest(&value)?;
                    let (module_id, module_version, module_type) = manifest_identity(&manifest);
                    let package_hash = hash_entry.file_name().to_string_lossy().into_owned();
                    let enabled = state
                        .modules
                        .get(module_id.as_str())
                        .copied()
                        .unwrap_or(true)
                        && validate_app_version_compatibility(&manifest, &self.current_app_version)
                            .is_ok();
                    modules.push(InstalledModule {
                        module_id: module_id.clone(),
                        module_type,
                        module_version: module_version.clone(),
                        package_hash,
                        root,
                        enabled,
                        manifest,
                    });
                }
            }
        }
        modules.sort_by(|left, right| {
            left.module_id
                .cmp(&right.module_id)
                .then_with(|| left.module_version.cmp(&right.module_version))
                .then_with(|| left.package_hash.cmp(&right.package_hash))
        });
        Ok(modules)
    }

    pub(crate) fn current_registry(&self) -> Result<Vec<InstalledModule>, PackageError> {
        self.recover_pending_uninstall()?;
        self.current_registry_without_recovery()
    }

    fn current_registry_without_recovery(&self) -> Result<Vec<InstalledModule>, PackageError> {
        let modules = self.reconstruct_registry()?;
        let mut state = load_state(&self.store_root)?;
        let mut current = std::collections::BTreeMap::<String, InstalledModule>::new();
        for module in modules {
            if let Some(active) = state.active_packages.get(module.module_id.as_str()) {
                if active.module_version == module.module_version.to_string()
                    && active.package_hash == module.package_hash
                {
                    current.insert(module.module_id.to_string(), module);
                }
                continue;
            }
            if state
                .uninstalled_modules
                .contains(module.module_id.as_str())
            {
                continue;
            }
            current
                .entry(module.module_id.to_string())
                .and_modify(|existing| {
                    if (module.module_version.clone(), module.package_hash.clone())
                        > (
                            existing.module_version.clone(),
                            existing.package_hash.clone(),
                        )
                    {
                        *existing = module.clone();
                    }
                })
                .or_insert(module);
        }
        let migrations: Vec<_> = current
            .values()
            .filter(|module| {
                !state
                    .active_packages
                    .contains_key(module.module_id.as_str())
                    && self.installed_app_compatibility_error(module).is_none()
            })
            .map(|module| {
                (
                    module.module_id.to_string(),
                    ActivePackage {
                        module_version: module.module_version.to_string(),
                        package_hash: module.package_hash.clone(),
                    },
                )
            })
            .collect();
        if !migrations.is_empty() {
            for (module_id, active) in migrations {
                state.active_packages.insert(module_id, active);
            }
            save_state(&self.store_root, &state)?;
        }
        Ok(current.into_values().collect())
    }

    fn installed_from_inspection(
        &self,
        inspected: &InspectedPackage,
        root: PathBuf,
    ) -> Result<InstalledModule, PackageError> {
        let (module_id, module_version, module_type) = manifest_identity(&inspected.manifest);
        let state = load_state(&self.store_root)?;
        Ok(InstalledModule {
            module_id: module_id.clone(),
            module_type,
            module_version: module_version.clone(),
            package_hash: inspected.package_hash.clone(),
            root,
            enabled: state
                .modules
                .get(module_id.as_str())
                .copied()
                .unwrap_or(true),
            manifest: inspected.manifest.clone(),
        })
    }

    fn ensure_default_state(&self, module_id: &ModuleId) -> Result<(), PackageError> {
        let mut state = load_state(&self.store_root)?;
        if !state.modules.contains_key(module_id.as_str()) {
            state.modules.insert(module_id.to_string(), true);
            save_state(&self.store_root, &state)?;
        }
        Ok(())
    }

    fn activate_package(
        &self,
        module_id: &ModuleId,
        module_version: &ContractVersion,
        package_hash: &str,
    ) -> Result<(), PackageError> {
        let mut state = load_state(&self.store_root)?;
        state.uninstalled_modules.remove(module_id.as_str());
        state.active_packages.insert(
            module_id.to_string(),
            ActivePackage {
                module_version: module_version.to_string(),
                package_hash: package_hash.to_owned(),
            },
        );
        save_state(&self.store_root, &state)
    }

    pub fn restore_active(&self, module: &InstalledModule) -> Result<(), PackageError> {
        let mut state = load_state(&self.store_root)?;
        state.uninstalled_modules.remove(module.module_id.as_str());
        state
            .modules
            .insert(module.module_id.to_string(), module.enabled);
        state.active_packages.insert(
            module.module_id.to_string(),
            ActivePackage {
                module_version: module.module_version.to_string(),
                package_hash: module.package_hash.clone(),
            },
        );
        save_state(&self.store_root, &state)
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn catalog_info(
    module_id: String,
    entry: BundledCatalogEntry,
) -> Result<BundledPackageInfo, PackageError> {
    let module_id = ModuleId::try_from(module_id).map_err(|error| PackageError::StateInvalid {
        detail: error.to_string(),
    })?;
    let module_version = ContractVersion::try_from(entry.module_version).map_err(|error| {
        PackageError::StateInvalid {
            detail: error.to_string(),
        }
    })?;
    Ok(BundledPackageInfo {
        module_id,
        module_version,
        package_hash: entry.package_hash,
    })
}

fn prefixed_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", digest(bytes))
}

fn path_exists(path: &Path) -> Result<bool, PackageError> {
    match fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(PackageError::AtomicUninstall {
            detail: format!("inspect transactional path failed: {error}"),
        }),
    }
}

fn collect_package_files(
    root: &Path,
    relative_root: &Path,
    files: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), PackageError> {
    let entries = fs::read_dir(root).map_err(|error| PackageError::AtomicUninstall {
        detail: format!("read package during uninstall backup failed: {error}"),
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| PackageError::AtomicUninstall {
            detail: format!("read package entry during uninstall backup failed: {error}"),
        })?;
        let file_type = entry
            .file_type()
            .map_err(|error| PackageError::AtomicUninstall {
                detail: format!("read package entry type during uninstall backup failed: {error}"),
            })?;
        let relative = relative_root.join(entry.file_name());
        let path = entry.path();
        if file_type.is_symlink() {
            return Err(PackageError::AtomicUninstall {
                detail: format!("package contains a symlink during uninstall backup: {path:?}"),
            });
        }
        if file_type.is_dir() {
            collect_package_files(&path, &relative, files)?;
        } else if file_type.is_file() {
            files.push((relative, path));
        } else {
            return Err(PackageError::AtomicUninstall {
                detail: format!(
                    "package contains an unsupported entry during uninstall backup: {path:?}"
                ),
            });
        }
    }
    Ok(())
}

fn normalize_archive_path(path: &str) -> Result<String, PackageError> {
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') {
        return Err(PackageError::AbsolutePath {
            path: path.to_owned(),
        });
    }
    if path.as_bytes().get(1) == Some(&b':') {
        return Err(PackageError::AbsolutePath {
            path: path.to_owned(),
        });
    }
    if path.contains('\\') {
        if path.split(['/', '\\']).any(|component| component == "..") {
            return Err(PackageError::PathTraversal {
                path: path.to_owned(),
            });
        }
        return Err(PackageError::InvalidPath {
            path: path.to_owned(),
        });
    }
    let mut components = Vec::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_string_lossy();
                if value == ".." {
                    return Err(PackageError::PathTraversal {
                        path: path.to_owned(),
                    });
                }
                components.push(value.into_owned());
            }
            Component::ParentDir => {
                return Err(PackageError::PathTraversal {
                    path: path.to_owned(),
                });
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => {
                return Err(PackageError::AbsolutePath {
                    path: path.to_owned(),
                });
            }
        }
    }
    if components.is_empty() {
        return Err(PackageError::InvalidPath {
            path: path.to_owned(),
        });
    }
    Ok(components.join("/"))
}

fn detect_duplicate_archive_entries(bytes: &[u8]) -> Result<(), PackageError> {
    let end_signature = [0x50, 0x4b, 0x05, 0x06];
    let central_signature = [0x50, 0x4b, 0x01, 0x02];
    let Some(end) = bytes
        .windows(end_signature.len())
        .rposition(|window| window == end_signature)
    else {
        return Ok(());
    };
    if end + 22 > bytes.len() {
        return Ok(());
    }
    let central_offset = u32::from_le_bytes([
        bytes[end + 16],
        bytes[end + 17],
        bytes[end + 18],
        bytes[end + 19],
    ]) as usize;
    let central_size = u32::from_le_bytes([
        bytes[end + 12],
        bytes[end + 13],
        bytes[end + 14],
        bytes[end + 15],
    ]) as usize;
    let Some(central_end) = central_offset.checked_add(central_size) else {
        return Ok(());
    };
    if central_offset >= bytes.len() || central_end > bytes.len() || central_end > end {
        return Ok(());
    }

    let mut seen = HashSet::new();
    let mut cursor = central_offset;
    while cursor < central_end {
        if cursor + 46 > central_end
            || bytes[cursor..cursor + central_signature.len()] != central_signature
        {
            return Ok(());
        }
        let name_length = u16::from_le_bytes([bytes[cursor + 28], bytes[cursor + 29]]) as usize;
        let extra_length = u16::from_le_bytes([bytes[cursor + 30], bytes[cursor + 31]]) as usize;
        let comment_length = u16::from_le_bytes([bytes[cursor + 32], bytes[cursor + 33]]) as usize;
        let record_length = 46usize
            .checked_add(name_length)
            .and_then(|value| value.checked_add(extra_length))
            .and_then(|value| value.checked_add(comment_length));
        let Some(record_length) = record_length else {
            return Ok(());
        };
        let Some(record_end) = cursor.checked_add(record_length) else {
            return Ok(());
        };
        if record_end > central_end {
            return Ok(());
        }
        let name_start = cursor + 46;
        let Ok(raw_name) = std::str::from_utf8(&bytes[name_start..name_start + name_length]) else {
            return Ok(());
        };
        let name = normalize_archive_path(raw_name)?;
        if !seen.insert(name.clone()) {
            if name == "module.json" {
                return Err(PackageError::DuplicateManifest);
            }
            return Err(PackageError::DuplicateEntry { path: name });
        }
        cursor = record_end;
    }
    Ok(())
}

fn parse_manifest(value: &Value) -> Result<ModuleManifest, PackageError> {
    let module_type = value
        .get("module_type")
        .and_then(Value::as_str)
        .ok_or_else(|| PackageError::ManifestInvalid {
            detail: "module_type is missing".to_owned(),
        })?;
    match module_type {
        "source" => serde_json::from_value::<SourceManifest>(value.clone())
            .map(ModuleManifest::Source)
            .map_err(|error| PackageError::ManifestInvalid {
                detail: error.to_string(),
            }),
        "dashboard" => serde_json::from_value::<DashboardManifest>(value.clone())
            .map(ModuleManifest::Dashboard)
            .map_err(|error| PackageError::ManifestInvalid {
                detail: error.to_string(),
            }),
        "locale" => serde_json::from_value::<LocaleManifest>(value.clone())
            .map(ModuleManifest::Locale)
            .map_err(|error| PackageError::ManifestInvalid {
                detail: error.to_string(),
            }),
        other => Err(PackageError::ManifestInvalid {
            detail: format!("unknown module_type {other}"),
        }),
    }
}

fn validate_manifest_schema(value: &Value, module_type: ModuleType) -> Result<(), PackageError> {
    let schema = match module_type {
        ModuleType::Source => SOURCE_SCHEMA,
        ModuleType::Dashboard => DASHBOARD_SCHEMA,
        ModuleType::Locale => LOCALE_SCHEMA,
    };
    let schema: Value = serde_json::from_str(schema).expect("bundled manifest schemas are valid");
    let validator = validator_for(&schema).expect("bundled manifest schemas are compilable");
    if let Some(error) = validator.iter_errors(value).next() {
        return Err(PackageError::ManifestSchemaInvalid {
            detail: error.to_string(),
        });
    }
    Ok(())
}

fn validate_package_extension(path: &Path, module_type: ModuleType) -> Result<(), PackageError> {
    let actual = path.extension().and_then(|value| value.to_str());
    let expected = match module_type {
        ModuleType::Source => "mfasource",
        ModuleType::Dashboard => "mfadashboard",
        ModuleType::Locale => "mfalocale",
    };
    if actual != Some(expected) {
        return Err(PackageError::ModuleExtensionMismatch);
    }
    Ok(())
}

fn validate_package_compatibility(
    manifest: &ModuleManifest,
    host_api_range: &VersionReq,
) -> Result<(), PackageError> {
    let package_version = match manifest {
        ModuleManifest::Source(value) => &value.package_format_version,
        ModuleManifest::Dashboard(value) => &value.package_format_version,
        ModuleManifest::Locale(value) => &value.package_format_version,
    };
    let package_range = VersionReq::parse(">=1.0.0, <2.0.0").expect("static package range");
    if !package_range.matches(package_version.as_semver()) {
        return Err(PackageError::IncompatiblePackageFormat);
    }
    match manifest {
        ModuleManifest::Source(value)
            if !host_api_range.matches(value.source_api_version.as_semver()) =>
        {
            Err(PackageError::IncompatibleSourceApi)
        }
        ModuleManifest::Dashboard(value)
            if !host_api_range.matches(value.dashboard_api_version.as_semver()) =>
        {
            Err(PackageError::IncompatibleDashboardApi)
        }
        _ => Ok(()),
    }
}

fn validate_app_version_compatibility(
    manifest: &ModuleManifest,
    current_app_version: &Version,
) -> Result<(), PackageError> {
    let compatible_app_versions = manifest_compatible_app_versions(manifest);
    let mut matches_current_version = false;
    for declared_range in compatible_app_versions {
        let range =
            VersionReq::parse(declared_range).map_err(|_| PackageError::IncompatibleAppVersion)?;
        matches_current_version |= range.matches(current_app_version);
    }
    if matches_current_version {
        Ok(())
    } else {
        Err(PackageError::IncompatibleAppVersion)
    }
}

fn manifest_compatible_app_versions(manifest: &ModuleManifest) -> &[String] {
    match manifest {
        ModuleManifest::Source(value) => &value.compatible_app_versions,
        ModuleManifest::Dashboard(value) => &value.compatible_app_versions,
        ModuleManifest::Locale(value) => &value.compatible_app_versions,
    }
}

fn validate_payload_security(
    entries: &[InspectedEntry],
    module_type: ModuleType,
) -> Result<(), PackageError> {
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        if is_executable(entry) {
            match module_type {
                ModuleType::Locale => {
                    return Err(PackageError::ExecutableLocaleEntry {
                        path: entry.path.clone(),
                    });
                }
                ModuleType::Source | ModuleType::Dashboard if entry.path != ENTRYPOINT_NAME => {
                    return Err(PackageError::UnexpectedExecutable {
                        path: entry.path.clone(),
                    });
                }
                ModuleType::Source | ModuleType::Dashboard => {}
            }
        }
    }
    Ok(())
}

fn is_executable(entry: &InspectedEntry) -> bool {
    let mode_executable = entry
        .unix_mode
        .map(|mode| mode & 0o111 != 0)
        .unwrap_or(false);
    let extension_executable = Path::new(&entry.path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "wasm" | "exe" | "dll" | "dylib" | "so" | "js" | "mjs" | "sh" | "bin"
            )
        })
        .unwrap_or(false);
    mode_executable || extension_executable
}

fn validate_entrypoint(
    manifest: &ModuleManifest,
    entries: &[InspectedEntry],
) -> Result<(), PackageError> {
    match manifest {
        ModuleManifest::Source(value) => validate_wasm_entrypoint(&value.entrypoint_hash, entries),
        ModuleManifest::Dashboard(value) => {
            validate_wasm_entrypoint(&value.entrypoint_hash, entries)
        }
        ModuleManifest::Locale(value) => {
            for file in &value.files {
                let entry = entries
                    .iter()
                    .find(|entry| entry.path == file.path && !entry.is_dir)
                    .ok_or_else(|| PackageError::InstalledModuleCorrupt {
                        path: PathBuf::from(&file.path),
                    })?;
                if prefixed_digest(&entry.bytes) != file.sha256 {
                    return Err(PackageError::LocalePayloadHashMismatch {
                        path: file.path.clone(),
                    });
                }
            }
            Ok(())
        }
    }
}

fn validate_wasm_entrypoint(
    declared_hash: &str,
    entries: &[InspectedEntry],
) -> Result<(), PackageError> {
    let entry = entries
        .iter()
        .find(|entry| entry.path == ENTRYPOINT_NAME && !entry.is_dir)
        .ok_or(PackageError::EntrypointMissing)?;
    if !is_valid_hash(declared_hash) {
        return Err(PackageError::EntrypointHashInvalid);
    }
    if prefixed_digest(&entry.bytes) != declared_hash {
        return Err(PackageError::EntrypointHashMismatch);
    }
    Ok(())
}

fn is_valid_hash(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    !hex.is_empty() && hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn manifest_identity(manifest: &ModuleManifest) -> (&ModuleId, &ContractVersion, ModuleType) {
    match manifest {
        ModuleManifest::Source(value) => {
            (&value.module_id, &value.module_version, ModuleType::Source)
        }
        ModuleManifest::Dashboard(value) => (
            &value.module_id,
            &value.module_version,
            ModuleType::Dashboard,
        ),
        ModuleManifest::Locale(value) => {
            (&value.module_id, &value.module_version, ModuleType::Locale)
        }
    }
}

#[allow(dead_code)]
fn _version_is_supported(version: &Version) -> bool {
    VersionReq::parse(HOST_API_RANGE)
        .expect("static host API range")
        .matches(version)
}

#[cfg(test)]
mod tests {
    use super::{manifest_compatible_app_versions, parse_manifest};
    use mfa_contracts::ModuleManifest;
    use serde_json::json;

    #[test]
    fn every_manifest_kind_exposes_app_compatibility_ranges() {
        let manifests = [
            json!({
                "module_type": "source",
                "module_id": "test-source",
                "module_version": "1.0.0",
                "package_format_version": "1.0.0",
                "source_api_version": "1.0.0",
                "mapping_version": "1.0.0",
                "compatible_app_versions": [">=0.1.0"],
                "provided_capabilities": ["body.weight"],
                "accepted_file_patterns": ["*.csv"],
                "artifact_signatures": ["sha256:0000000000000000000000000000000000000000000000000000000000000000"],
                "extension_contracts": [],
                "settings_schema": {},
                "entrypoint_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                "localization_namespace": "source.test"
            }),
            json!({
                "module_type": "dashboard",
                "module_id": "test-dashboard",
                "module_version": "1.0.0",
                "package_format_version": "1.0.0",
                "dashboard_api_version": "1.0.0",
                "entrypoint_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                "compatible_app_versions": [">=0.1.0"],
                "required_capabilities": [],
                "required_extension_contracts": [],
                "localization_namespace": "dashboard.test"
            }),
            json!({
                "module_type": "locale",
                "module_id": "test-locale",
                "locale": "en",
                "display_name": "Test",
                "module_version": "1.0.0",
                "package_format_version": "1.0.0",
                "compatible_app_versions": [">=0.1.0"],
                "localization_namespace": "locale.test",
                "files": []
            }),
        ];

        for value in manifests {
            let manifest = parse_manifest(&value).unwrap();
            assert!(matches!(
                manifest,
                ModuleManifest::Source(_)
                    | ModuleManifest::Dashboard(_)
                    | ModuleManifest::Locale(_)
            ));
            assert_eq!(
                manifest_compatible_app_versions(&manifest),
                &[">=0.1.0".to_owned()]
            );
        }
    }
}
