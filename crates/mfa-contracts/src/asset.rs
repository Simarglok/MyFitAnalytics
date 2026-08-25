use crate::AssetReadError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetMetadata {
    pub asset_id: Uuid,
    pub file_name: String,
    pub media_type: String,
    pub byte_len: u64,
}

pub trait ReadOnlyAsset: Send + Sync {
    fn metadata(&self) -> AssetMetadata;
    fn read_at(&self, offset: u64, max_bytes: u32) -> Result<Vec<u8>, AssetReadError>;
}
