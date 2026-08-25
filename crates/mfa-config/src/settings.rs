use crate::atomic_file::{AtomicFileError, atomic_write, recover_temporary};
use mfa_contracts::{CapabilityId, ModuleId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    pub schema_version: u32,
    pub locale: String,
    #[serde(default)]
    pub workspace_root: Option<std::path::PathBuf>,
    #[serde(default)]
    pub app_data_root: Option<std::path::PathBuf>,
    #[serde(default)]
    pub active_providers: BTreeMap<CapabilityId, ModuleId>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            locale: "en-US".to_owned(),
            workspace_root: None,
            app_data_root: None,
            active_providers: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("settings I/O failed: {detail}")]
    Io { detail: String },
    #[error("settings JSON is invalid: {detail}")]
    InvalidJson { detail: String },
    #[error("settings schema version {found} is unsupported; expected {expected}")]
    UnsupportedSchemaVersion { found: u32, expected: u32 },
    #[error("settings value is invalid: {detail}")]
    InvalidSettings { detail: String },
}

impl SettingsError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "settings_io",
            Self::InvalidJson { .. } => "invalid_settings_json",
            Self::UnsupportedSchemaVersion { .. } => "unsupported_schema_version",
            Self::InvalidSettings { .. } => "invalid_settings",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<AppSettings, SettingsError> {
        recover_temporary(&self.path).map_err(map_atomic)?;
        if !self.path.exists() {
            return Ok(AppSettings::default());
        }
        let bytes = fs::read(&self.path).map_err(|error| SettingsError::Io {
            detail: error.to_string(),
        })?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|error| SettingsError::InvalidJson {
                detail: error.to_string(),
            })?;
        let found = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| SettingsError::InvalidSettings {
                detail: "schema_version is required".to_owned(),
            })? as u32;
        if found != CURRENT_SCHEMA_VERSION {
            return Err(SettingsError::UnsupportedSchemaVersion {
                found,
                expected: CURRENT_SCHEMA_VERSION,
            });
        }
        serde_json::from_value(value).map_err(|error| SettingsError::InvalidSettings {
            detail: error.to_string(),
        })
    }

    pub fn save(&self, settings: &AppSettings) -> Result<(), SettingsError> {
        if settings.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(SettingsError::UnsupportedSchemaVersion {
                found: settings.schema_version,
                expected: CURRENT_SCHEMA_VERSION,
            });
        }
        let bytes = serde_json::to_vec_pretty(settings).map_err(|error| {
            SettingsError::InvalidSettings {
                detail: error.to_string(),
            }
        })?;
        atomic_write(&self.path, &bytes).map_err(map_atomic)
    }
}

fn map_atomic(error: AtomicFileError) -> SettingsError {
    SettingsError::Io {
        detail: error.to_string(),
    }
}
