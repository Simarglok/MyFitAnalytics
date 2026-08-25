use crate::error::ArchiveError;
use crate::naming::archive_filename;
use mfa_config::WorkspacePaths;
use mfa_contracts::{ModuleId, UtcInstant};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveDisposition {
    Created,
    ExistingExactDuplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivedAsset {
    pub asset_id: Uuid,
    pub source_module_id: ModuleId,
    pub original_filename: String,
    pub archive_path: PathBuf,
    pub byte_sha256: String,
    pub file_size: u64,
    pub received_at: UtcInstant,
    pub disposition: ArchiveDisposition,
}

#[derive(Debug, Clone)]
pub struct ArchiveCoordinator {
    workspace: WorkspacePaths,
    source_module_id: ModuleId,
}

impl ArchiveCoordinator {
    pub fn new(workspace: WorkspacePaths, source_module_id: ModuleId) -> Self {
        Self {
            workspace,
            source_module_id,
        }
    }

    pub fn workspace(&self) -> &WorkspacePaths {
        &self.workspace
    }

    pub fn source_module_id(&self) -> &ModuleId {
        &self.source_module_id
    }

    pub fn archive(
        &self,
        candidate: crate::StableCandidate,
        received_at: UtcInstant,
    ) -> Result<ArchivedAsset, ArchiveError> {
        let source_path = &candidate.path;
        let source_metadata =
            fs::metadata(source_path).map_err(|error| ArchiveError::SourceUnavailable {
                path: source_path.clone(),
                detail: error.to_string(),
            })?;
        let modified = source_metadata
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if source_metadata.len() != candidate.fingerprint.size
            || modified != candidate.fingerprint.modified
        {
            return Err(ArchiveError::SourceChanged {
                path: source_path.clone(),
            });
        }
        let source_bytes =
            fs::read(source_path).map_err(|error| ArchiveError::SourceUnavailable {
                path: source_path.clone(),
                detail: error.to_string(),
            })?;
        let hash = digest(&source_bytes);
        if let Some(existing) = self.find_exact_duplicate(&hash)? {
            return self.asset_from_path(
                existing,
                hash,
                ArchiveDisposition::ExistingExactDuplicate,
                received_at,
            );
        }

        let original_filename = source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unnamed-asset")
            .to_owned();
        let timestamp = received_at.as_datetime();
        let archive_directory = self
            .workspace
            .source_archive(&self.source_module_id)
            .join(timestamp.format("%Y").to_string())
            .join(timestamp.format("%Y-%m-%d").to_string());
        fs::create_dir_all(&archive_directory).map_err(|error| ArchiveError::Io {
            operation: "create_archive_directory",
            detail: error.to_string(),
        })?;
        let final_path =
            archive_directory.join(archive_filename(timestamp, &hash, &original_filename));
        if final_path.exists() {
            let existing_hash =
                hash_file(&final_path).map_err(|error| ArchiveError::CorruptArchive {
                    path: final_path.clone(),
                    detail: error.to_string(),
                })?;
            if existing_hash == hash {
                return self.asset_from_path(
                    final_path,
                    hash,
                    ArchiveDisposition::ExistingExactDuplicate,
                    received_at,
                );
            }
            return Err(ArchiveError::DestinationExists { path: final_path });
        }

        let temporary_path = archive_directory.join(format!(
            ".{}.archive-tmp-{}",
            original_filename,
            Uuid::new_v4()
        ));
        let result = (|| {
            let mut temporary = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary_path)
                .map_err(|error| ArchiveError::Io {
                    operation: "create_archive_temporary",
                    detail: error.to_string(),
                })?;
            temporary
                .write_all(&source_bytes)
                .map_err(|error| ArchiveError::Io {
                    operation: "write_archive_temporary",
                    detail: error.to_string(),
                })?;
            temporary.sync_all().map_err(|error| ArchiveError::Io {
                operation: "sync_archive_temporary",
                detail: error.to_string(),
            })?;
            drop(temporary);
            let copied_hash = hash_file(&temporary_path).map_err(|error| ArchiveError::Io {
                operation: "hash_archive_temporary",
                detail: error.to_string(),
            })?;
            if copied_hash != hash {
                return Err(ArchiveError::HashMismatch {
                    path: temporary_path.clone(),
                });
            }
            install_without_overwrite(&temporary_path, &final_path)?;
            sync_directory(&archive_directory).map_err(|error| ArchiveError::Io {
                operation: "sync_archive_directory",
                detail: error.to_string(),
            })?;
            Ok::<(), ArchiveError>(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        result?;

        Ok(ArchivedAsset {
            asset_id: asset_id_from_hash(&hash),
            source_module_id: self.source_module_id.clone(),
            original_filename,
            archive_path: final_path,
            byte_sha256: hash,
            file_size: source_bytes.len() as u64,
            received_at,
            disposition: ArchiveDisposition::Created,
        })
    }

    fn find_exact_duplicate(&self, hash: &str) -> Result<Option<PathBuf>, ArchiveError> {
        let root = self.workspace.source_archive(&self.source_module_id);
        if !root.exists() {
            return Ok(None);
        }
        let mut paths = Vec::new();
        collect_files(&root, &mut paths).map_err(|error| ArchiveError::Io {
            operation: "scan_archive",
            detail: error.to_string(),
        })?;
        paths.sort();
        for path in paths {
            if crate::is_ignored_path(&path) {
                continue;
            }
            let candidate_hash =
                hash_file(&path).map_err(|error| ArchiveError::CorruptArchive {
                    path: path.clone(),
                    detail: error.to_string(),
                })?;
            if candidate_hash == hash {
                return Ok(Some(path));
            }
        }
        Ok(None)
    }

    fn asset_from_path(
        &self,
        path: PathBuf,
        hash: String,
        disposition: ArchiveDisposition,
        received_at: UtcInstant,
    ) -> Result<ArchivedAsset, ArchiveError> {
        let metadata = fs::metadata(&path).map_err(|error| ArchiveError::CorruptArchive {
            path: path.clone(),
            detail: error.to_string(),
        })?;
        let original_filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| {
                name.split_once("--")
                    .and_then(|(_, rest)| rest.split_once("--"))
            })
            .map(|(_, name)| name.to_owned())
            .unwrap_or_else(|| "unnamed-asset".to_owned());
        Ok(ArchivedAsset {
            asset_id: asset_id_from_hash(&hash),
            source_module_id: self.source_module_id.clone(),
            original_filename,
            archive_path: path,
            byte_sha256: hash,
            file_size: metadata.len(),
            received_at,
            disposition,
        })
    }
}

fn install_without_overwrite(temporary: &Path, final_path: &Path) -> Result<(), ArchiveError> {
    match fs::hard_link(temporary, final_path) {
        Ok(()) => {
            fs::remove_file(temporary).map_err(|error| ArchiveError::Io {
                operation: "remove_archive_temporary",
                detail: error.to_string(),
            })?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(ArchiveError::DestinationExists {
                path: final_path.to_path_buf(),
            })
        }
        Err(error) => Err(ArchiveError::Io {
            operation: "claim_archive_destination",
            detail: error.to_string(),
        }),
    }
}

fn collect_files(root: &Path, output: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, output)?;
        } else if path.is_file() {
            output.push(path);
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn asset_id_from_hash(hash: &str) -> Uuid {
    let mut bytes = [0u8; 16];
    for (index, pair) in hash.as_bytes().chunks_exact(2).take(16).enumerate() {
        bytes[index] = (hex(pair[0]) << 4) | hex(pair[1]);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn hex(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        b'A'..=b'F' => value - b'A' + 10,
        _ => 0,
    }
}

fn sync_directory(path: &Path) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()?;
    }
    Ok(())
}
