use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFingerprint {
    pub size: u64,
    pub modified: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StableCandidate {
    pub path: PathBuf,
    pub fingerprint: FileFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StabilityState {
    Ignored,
    Waiting,
    Stable(StableCandidate),
    Unavailable,
}

#[derive(Debug, Default)]
pub struct StabilityTracker {
    observations: HashMap<PathBuf, Observation>,
}

#[derive(Debug, Clone)]
struct Observation {
    fingerprint: FileFingerprint,
    consecutive_matches: u8,
}

impl StabilityTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(&mut self, path: &Path, fingerprint: FileFingerprint) -> StabilityState {
        if is_ignored_path(path) {
            self.observations.remove(path);
            return StabilityState::Ignored;
        }
        let consecutive_matches = match self.observations.get(path) {
            Some(previous) if previous.fingerprint == fingerprint => {
                previous.consecutive_matches.saturating_add(1)
            }
            _ => 1,
        };
        self.observations.insert(
            path.to_path_buf(),
            Observation {
                fingerprint: fingerprint.clone(),
                consecutive_matches,
            },
        );
        if consecutive_matches >= 2 {
            StabilityState::Stable(StableCandidate {
                path: path.to_path_buf(),
                fingerprint,
            })
        } else {
            StabilityState::Waiting
        }
    }

    pub fn observe_readable(
        &mut self,
        path: &Path,
        fingerprint: FileFingerprint,
    ) -> StabilityState {
        let state = self.observe(path, fingerprint);
        let StabilityState::Stable(candidate) = state else {
            return state;
        };
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(_) => return StabilityState::Unavailable,
        };
        let mut buffer = Vec::new();
        if file.read_to_end(&mut buffer).is_err() {
            return StabilityState::Unavailable;
        }
        StabilityState::Stable(candidate)
    }
}

pub fn is_ignored_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    let lower = name.to_ascii_lowercase();
    name.starts_with('.')
        || name.starts_with("~$")
        || lower.ends_with(".part")
        || lower.ends_with(".tmp")
        || lower.contains(".tmp-")
        || lower.contains(".archive-tmp")
        || lower.contains(".archive.tmp")
        || lower.starts_with(".staging-")
}

pub fn fingerprint(path: &Path) -> Result<FileFingerprint, std::io::Error> {
    let metadata = fs::metadata(path)?;
    Ok(FileFingerprint {
        size: metadata.len(),
        modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
    })
}
