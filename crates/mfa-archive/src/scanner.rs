use crate::stability::{
    StabilityState, StabilityTracker, StableCandidate, fingerprint, is_ignored_path,
};
use mfa_contracts::UtcInstant;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanReason {
    Startup,
    Watcher,
    Periodic,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRequest {
    pub reason: ScanReason,
    pub requested_at: UtcInstant,
}

impl ScanRequest {
    pub fn new(reason: ScanReason, requested_at: UtcInstant) -> Self {
        Self {
            reason,
            requested_at,
        }
    }
}

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("inbox scan failed: {detail}")]
    Io { detail: String },
}

impl ScanError {
    pub fn code(&self) -> &'static str {
        "inbox_scan_failed"
    }
}

#[derive(Debug, Default)]
pub struct StableScanner {
    tracker: StabilityTracker,
}

impl StableScanner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scan(&mut self, inbox: &Path) -> Result<Vec<StableCandidate>, ScanError> {
        let mut paths = Vec::new();
        let entries = fs::read_dir(inbox).map_err(|error| ScanError::Io {
            detail: error.to_string(),
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| ScanError::Io {
                detail: error.to_string(),
            })?;
            let path = entry.path();
            if !is_ignored_path(&path) && path.is_file() {
                paths.push(path);
            }
        }
        paths.sort();
        let mut stable = Vec::new();
        for path in paths {
            let observed = fingerprint(&path).map_err(|error| ScanError::Io {
                detail: error.to_string(),
            })?;
            if let StabilityState::Stable(candidate) =
                self.tracker.observe_readable(&path, observed)
            {
                stable.push(candidate);
            }
        }
        Ok(stable)
    }

    pub fn tracker_mut(&mut self) -> &mut StabilityTracker {
        &mut self.tracker
    }

    pub fn pending_paths(&self) -> Vec<PathBuf> {
        Vec::new()
    }
}
