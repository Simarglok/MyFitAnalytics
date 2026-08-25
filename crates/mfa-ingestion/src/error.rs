use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum IngestionError {
    #[error("ingestion queue is closed")]
    QueueClosed,
    #[error("asset failure ({code}): {detail}")]
    AssetFailure { code: String, detail: String },
    #[error("transient ingestion failure ({code}): {detail}")]
    TransientFailure { code: String, detail: String },
    #[error("critical ingestion failure ({code}): {detail}")]
    CriticalFailure { code: String, detail: String },
    #[error("ingestion scan failed: {detail}")]
    ScanFailed { detail: String },
}

impl IngestionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::QueueClosed => "ingestion_queue_closed",
            Self::AssetFailure { .. } => "asset_failure",
            Self::TransientFailure { .. } => "transient_failure",
            Self::CriticalFailure { .. } => "critical_failure",
            Self::ScanFailed { .. } => "scan_failed",
        }
    }
}
