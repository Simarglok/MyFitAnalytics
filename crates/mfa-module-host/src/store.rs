use crate::error::PackageError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ModuleState {
    #[serde(default)]
    pub modules: BTreeMap<String, bool>,
    #[serde(default)]
    pub active_packages: BTreeMap<String, ActivePackage>,
    #[serde(default)]
    pub uninstalled_modules: BTreeSet<String>,
    #[serde(default)]
    pub bundled_catalog: BTreeMap<String, BundledCatalogEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ActivePackage {
    pub module_version: String,
    pub package_hash: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct BundledCatalogEntry {
    pub module_version: String,
    pub package_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum UninstallPhase {
    Prepared,
    Moved,
    BackedUp,
    StateApplied,
    PackageRemoved,
    Committed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct UninstallJournal {
    pub module_id: String,
    pub original_root: PathBuf,
    pub staged_root: PathBuf,
    pub backup_path: PathBuf,
    pub version_root: PathBuf,
    pub previous_state: ModuleState,
    pub phase: UninstallPhase,
}

pub(crate) fn state_path(store_root: &Path) -> PathBuf {
    store_root.join("state.json")
}

pub(crate) fn uninstall_journal_path(store_root: &Path) -> PathBuf {
    store_root.join(".uninstall-transaction.json")
}

pub(crate) fn load_state(store_root: &Path) -> Result<ModuleState, PackageError> {
    let path = state_path(store_root);
    match fs::metadata(&path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ModuleState::default());
        }
        Err(error) => return Err(PackageError::from(error)),
    }
    let bytes = fs::read(path).map_err(PackageError::from)?;
    serde_json::from_slice(&bytes).map_err(|error| PackageError::StateInvalid {
        detail: error.to_string(),
    })
}

pub(crate) fn save_state(store_root: &Path, state: &ModuleState) -> Result<(), PackageError> {
    fs::create_dir_all(store_root).map_err(PackageError::from)?;
    let temporary = store_root.join(format!("state.json.tmp-{}", Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(PackageError::from)?;
        let encoded =
            serde_json::to_vec_pretty(state).map_err(|error| PackageError::StateInvalid {
                detail: error.to_string(),
            })?;
        file.write_all(&encoded).map_err(PackageError::from)?;
        file.sync_all().map_err(PackageError::from)?;
        fs::rename(&temporary, state_path(store_root)).map_err(PackageError::from)?;
        sync_directory(store_root)
    })();
    match result {
        Ok(()) => Ok(()),
        Err(error) => match fs::remove_file(&temporary) {
            Ok(()) => Err(error),
            Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => Err(error),
            Err(cleanup) => Err(PackageError::AtomicUninstall {
                detail: format!("{error}; temporary state cleanup failed: {cleanup}"),
            }),
        },
    }
}

pub(crate) fn load_uninstall_journal(
    store_root: &Path,
) -> Result<Option<UninstallJournal>, PackageError> {
    let path = uninstall_journal_path(store_root);
    match fs::metadata(&path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(PackageError::from(error)),
    }
    let bytes = fs::read(path).map_err(PackageError::from)?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| PackageError::StateInvalid {
            detail: format!("uninstall journal is invalid: {error}"),
        })
}

pub(crate) fn save_uninstall_journal(
    store_root: &Path,
    journal: &UninstallJournal,
) -> Result<(), PackageError> {
    fs::create_dir_all(store_root).map_err(PackageError::from)?;
    let temporary = store_root.join(format!(".uninstall-transaction.tmp-{}", Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(PackageError::from)?;
        let encoded =
            serde_json::to_vec_pretty(journal).map_err(|error| PackageError::StateInvalid {
                detail: error.to_string(),
            })?;
        file.write_all(&encoded).map_err(PackageError::from)?;
        file.sync_all().map_err(PackageError::from)?;
        fs::rename(&temporary, uninstall_journal_path(store_root)).map_err(PackageError::from)?;
        sync_directory(store_root)
    })();
    match result {
        Ok(()) => Ok(()),
        Err(error) => match fs::remove_file(&temporary) {
            Ok(()) => Err(error),
            Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => Err(error),
            Err(cleanup) => Err(PackageError::AtomicUninstall {
                detail: format!("{error}; temporary uninstall journal cleanup failed: {cleanup}"),
            }),
        },
    }
}

pub(crate) fn clear_uninstall_journal(store_root: &Path) -> Result<(), PackageError> {
    let path = uninstall_journal_path(store_root);
    match fs::remove_file(path) {
        Ok(()) => sync_directory(store_root),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(PackageError::from(error)),
    }
}

pub(crate) fn sync_directory(path: &Path) -> Result<(), PackageError> {
    #[cfg(unix)]
    {
        File::open(path)
            .map_err(PackageError::from)?
            .sync_all()
            .map_err(PackageError::from)?;
    }
    Ok(())
}
