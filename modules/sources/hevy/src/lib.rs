pub mod csv_input;
pub mod error;
pub mod measurements;

#[cfg(target_arch = "wasm32")]
mod component;

use mfa_contracts::ContractVersion;
use sha2::{Digest, Sha256};

pub use csv_input::{CsvInput, GuestAssetReader, HevyArtifact, ProbeResult, detect_hevy};
pub use error::MappingError;
pub use measurements::{context_for_measurements, parse_measurements};

#[derive(Debug, Clone)]
pub struct MappingContext {
    pub module_id: String,
    pub asset_id: String,
    pub mapping_version: ContractVersion,
    pub schema_fingerprint: String,
    pub logical_snapshot_key: String,
}

impl MappingContext {
    pub fn synthetic(asset_id: impl Into<String>) -> Self {
        Self {
            module_id: "hevy".to_owned(),
            asset_id: asset_id.into(),
            mapping_version: "1.0.0".parse().expect("valid mapping version"),
            schema_fingerprint:
                "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
            logical_snapshot_key: "hevy:measurements:2026".to_owned(),
        }
    }

    pub fn for_measurements(
        asset_id: impl Into<String>,
        year: i32,
        schema_fingerprint: String,
    ) -> Self {
        Self {
            module_id: "hevy".to_owned(),
            asset_id: asset_id.into(),
            mapping_version: "1.0.0".parse().expect("valid mapping version"),
            schema_fingerprint,
            logical_snapshot_key: format!("hevy:measurements:{year}"),
        }
    }
}

pub(crate) fn source_record_key(context: &MappingContext, row_number: usize) -> String {
    format!("{}:measurements:{row_number}", context.asset_id)
}

pub(crate) fn deterministic_id(
    context: &MappingContext,
    kind: &str,
    source_key: &str,
) -> uuid::Uuid {
    let digest = Sha256::digest(format!("{}:{kind}:{source_key}", context.module_id));
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

pub(crate) fn schema_fingerprint(headers: &[String]) -> String {
    format!("sha256:{:x}", Sha256::digest(headers.join("\u{1f}")))
}
