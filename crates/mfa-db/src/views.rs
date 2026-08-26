use crate::provenance::{LogicalSnapshotKey, RecordCounts};
use serde::{Deserialize, Serialize};
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
pub enum ViewRequest {
    ActiveSnapshot { logical_snapshot_key: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewResponse {
    pub logical_snapshot_key: String,
    pub snapshot_id: Option<Uuid>,
    pub counts: RecordCounts,
}
