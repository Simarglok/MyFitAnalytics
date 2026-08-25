use crate::archive::{asset_id_from_hash, collect_files, hash_file};
use crate::stability::is_ignored_path;
use mfa_config::WorkspacePaths;
use mfa_contracts::{ModuleId, UtcInstant};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveRecord {
    pub asset_id: Uuid,
    pub source_module_id: ModuleId,
    pub original_filename: String,
    pub archive_path: PathBuf,
    pub byte_sha256: String,
    pub file_size: u64,
    pub received_at: UtcInstant,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArchiveInventory {
    pub assets: Vec<ArchiveRecord>,
    pub ignored_temporary_files: u64,
}

#[derive(Debug, Error)]
pub enum ReconciliationError {
    #[error("archive reconciliation I/O failed: {detail}")]
    Io { detail: String },
    #[error("archive filename is invalid: {path}")]
    InvalidFilename { path: PathBuf },
    #[error("archive filename contains an invalid SHA-256: {path}")]
    InvalidHash { path: PathBuf },
    #[error("archive bytes do not match their immutable filename hash: {path}")]
    HashMismatch { path: PathBuf },
    #[error("archive timestamp is invalid: {path}")]
    InvalidTimestamp { path: PathBuf },
}

impl ReconciliationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "archive_reconciliation_io",
            Self::InvalidFilename { .. } => "archive_filename_invalid",
            Self::InvalidHash { .. } => "archive_hash_invalid",
            Self::HashMismatch { .. } => "archive_hash_mismatch",
            Self::InvalidTimestamp { .. } => "archive_timestamp_invalid",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArchiveReconciler {
    workspace: WorkspacePaths,
    source_module_id: ModuleId,
}

impl ArchiveReconciler {
    pub fn new(workspace: WorkspacePaths, source_module_id: ModuleId) -> Self {
        Self {
            workspace,
            source_module_id,
        }
    }

    pub fn scan(&self) -> Result<ArchiveInventory, ReconciliationError> {
        let root = self.workspace.source_archive(&self.source_module_id);
        if !root.exists() {
            return Ok(ArchiveInventory::default());
        }
        let mut paths = Vec::new();
        collect_files(&root, &mut paths).map_err(|error| ReconciliationError::Io {
            detail: error.to_string(),
        })?;
        paths.sort();
        let mut inventory = ArchiveInventory::default();
        for path in paths {
            if is_ignored_path(&path) {
                inventory.ignored_temporary_files += 1;
                continue;
            }
            inventory
                .assets
                .push(parse_asset(&path, &self.source_module_id)?);
        }
        Ok(inventory)
    }

    pub fn source_module_id(&self) -> &ModuleId {
        &self.source_module_id
    }
}

impl ArchiveRecord {
    pub fn into_archived_asset(self) -> crate::ArchivedAsset {
        crate::ArchivedAsset {
            asset_id: self.asset_id,
            source_module_id: self.source_module_id,
            original_filename: self.original_filename,
            archive_path: self.archive_path,
            byte_sha256: self.byte_sha256,
            file_size: self.file_size,
            received_at: self.received_at,
            disposition: crate::ArchiveDisposition::Created,
        }
    }
}

fn parse_asset(
    path: &std::path::Path,
    source_module_id: &ModuleId,
) -> Result<ArchiveRecord, ReconciliationError> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ReconciliationError::InvalidFilename {
            path: path.to_path_buf(),
        })?;
    let mut pieces = name.splitn(3, "--");
    let timestamp = pieces.next();
    let hash = pieces.next();
    let original_filename = pieces.next();
    let (Some(timestamp), Some(hash), Some(original_filename)) =
        (timestamp, hash, original_filename)
    else {
        return Err(ReconciliationError::InvalidFilename {
            path: path.to_path_buf(),
        });
    };
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ReconciliationError::InvalidHash {
            path: path.to_path_buf(),
        });
    }
    let naive =
        chrono::NaiveDateTime::parse_from_str(timestamp, "%Y%m%dT%H%M%S%.fZ").map_err(|_| {
            ReconciliationError::InvalidTimestamp {
                path: path.to_path_buf(),
            }
        })?;
    let received_at = UtcInstant::from(chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
        naive,
        chrono::Utc,
    ));
    let actual_hash = hash_file(path).map_err(|error| ReconciliationError::Io {
        detail: error.to_string(),
    })?;
    if actual_hash != hash.to_ascii_lowercase() {
        return Err(ReconciliationError::HashMismatch {
            path: path.to_path_buf(),
        });
    }
    let file_size = fs::metadata(path)
        .map_err(|error| ReconciliationError::Io {
            detail: error.to_string(),
        })?
        .len();
    Ok(ArchiveRecord {
        asset_id: asset_id_from_hash(hash),
        source_module_id: source_module_id.clone(),
        original_filename: original_filename.to_owned(),
        archive_path: path.to_path_buf(),
        byte_sha256: hash.to_ascii_lowercase(),
        file_size,
        received_at,
    })
}

pub type ReconciledAsset = ArchiveRecord;
