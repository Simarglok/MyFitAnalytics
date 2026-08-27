use crate::provenance::{
    DataQualityItem, ExtensionRecord, LineageLink, LogicalSnapshotKey, RecordCounts, SourceRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct QueryView {
    pub request: ViewRequest,
}

impl QueryView {
    pub fn active_snapshot(logical_snapshot_key: LogicalSnapshotKey) -> Self {
        Self {
            request: ViewRequest::ActiveSnapshot {
                logical_snapshot_key: logical_snapshot_key.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuerySnapshot {
    pub logical_snapshot_key: LogicalSnapshotKey,
}

impl QuerySnapshot {
    pub fn active(logical_snapshot_key: LogicalSnapshotKey) -> Self {
        Self {
            logical_snapshot_key,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ViewRequest {
    ActiveSnapshot { logical_snapshot_key: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewResponse {
    pub logical_snapshot_key: String,
    pub snapshot_id: Option<Uuid>,
    pub counts: RecordCounts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotResponse {
    pub logical_snapshot_key: String,
    pub snapshot_id: Option<Uuid>,
    pub counts: RecordCounts,
    pub canonical_records: Vec<Value>,
    pub source_records: Vec<SourceRecord>,
    pub historical_source_records: Vec<SourceRecord>,
    pub lineage: Vec<LineageLink>,
    pub extensions: Vec<ExtensionRecord>,
    pub issues: Vec<DataQualityItem>,
}
