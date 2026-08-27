use crate::window::DateRange;
use mfa_contracts::LocalDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRef {
    pub logical_snapshot_key: String,
    pub snapshot_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlgorithmVersion(String);

impl AlgorithmVersion {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageEvidence {
    pub requested_days: u64,
    pub observed_days: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedProvenance {
    pub algorithm_version: AlgorithmVersion,
    pub mapping_versions: Vec<String>,
    pub requested: DateRange,
    pub coverage: CoverageEvidence,
    pub snapshot_refs: Vec<SnapshotRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricContext {
    pub requested: DateRange,
    pub as_of: LocalDate,
    pub snapshot_refs: Vec<SnapshotRef>,
    pub algorithm_version: AlgorithmVersion,
}

impl MetricContext {
    pub(crate) fn provenance(&self, observed_days: usize) -> DerivedProvenance {
        DerivedProvenance {
            algorithm_version: self.algorithm_version.clone(),
            mapping_versions: Vec::new(),
            requested: self.requested,
            coverage: CoverageEvidence {
                requested_days: self.requested.len_days(),
                observed_days: observed_days as u64,
            },
            snapshot_refs: self.snapshot_refs.clone(),
        }
    }
}
