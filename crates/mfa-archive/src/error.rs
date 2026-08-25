use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("source asset is unavailable: {path}: {detail}")]
    SourceUnavailable { path: PathBuf, detail: String },
    #[error("source asset changed after stability observation: {path}")]
    SourceChanged { path: PathBuf },
    #[error("archive operation failed during {operation}: {detail}")]
    Io {
        operation: &'static str,
        detail: String,
    },
    #[error("archive hash verification failed for {path}")]
    HashMismatch { path: PathBuf },
    #[error("archive destination already contains different bytes: {path}")]
    DestinationExists { path: PathBuf },
    #[error("archive contains an unreadable completed asset: {path}: {detail}")]
    CorruptArchive { path: PathBuf, detail: String },
}

impl ArchiveError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::SourceUnavailable { .. } => "source_unavailable",
            Self::SourceChanged { .. } => "source_changed",
            Self::Io { .. } => "archive_io_error",
            Self::HashMismatch { .. } => "archive_hash_mismatch",
            Self::DestinationExists { .. } => "archive_destination_exists",
            Self::CorruptArchive { .. } => "archive_corrupt",
        }
    }
}
