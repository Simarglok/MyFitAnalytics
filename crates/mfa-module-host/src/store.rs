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

pub(crate) fn state_path(store_root: &Path) -> PathBuf {
    store_root.join("state.json")
}

pub(crate) fn load_state(store_root: &Path) -> Result<ModuleState, PackageError> {
    let path = state_path(store_root);
    if !path.exists() {
        return Ok(ModuleState::default());
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
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
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
