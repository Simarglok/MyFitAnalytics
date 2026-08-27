use mfa_contracts::{
    AssetMetadata, ExtensionRequirement, LineageHook, MappingIssue, ReadOnlyAsset, SourceBatch,
    SourceDescriptor, SourceRecord, SourceValidation,
};
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

pub use mfa_contracts::{
    ActivityDay, ActivityEvent, BodyMeasurement, CanonicalObservation, ContractVersion,
    ExtensionRecord, HeartRateObservation, LocalDate, LocalDateTime, ModuleId, NutritionItem,
    SOURCE_API_VERSION, SOURCE_BATCH_CONTRACT_VERSION, WorkoutSession,
};

pub trait GuestAssetReader {
    fn metadata(&self) -> AssetMetadata;
    fn read_at(&mut self, offset: u64, max_bytes: u32) -> Result<Vec<u8>, GuestError>;
}

pub trait SourceGuest {
    fn descriptor() -> SourceDescriptor;
    fn detect(asset: &mut dyn GuestAssetReader) -> u8;
    fn validate(asset: &mut dyn GuestAssetReader) -> Result<SourceValidation, GuestError>;
    fn parse(asset: &mut dyn GuestAssetReader) -> Result<SourceBatch, GuestError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GuestError {
    #[error("asset read failed: {0}")]
    Asset(String),
    #[error("invalid source input: {0}")]
    InvalidInput(String),
    #[error("source mapping failed: {0}")]
    Mapping(String),
}

pub fn source_record_key(sheet_name: Option<&str>, source_row_number: u32) -> String {
    match sheet_name {
        Some(sheet_name) if !sheet_name.trim().is_empty() => {
            format!("{}:{source_row_number}", sheet_name.trim())
        }
        _ => format!("row:{source_row_number}"),
    }
}

pub fn source_record(
    sheet_name: Option<&str>,
    source_row_number: u32,
    raw_payload: Value,
) -> SourceRecord {
    SourceRecord {
        source_record_key: source_record_key(sheet_name, source_row_number),
        sheet_name: sheet_name.map(str::to_owned),
        source_row_number,
        raw_payload,
    }
}

pub fn lineage_hook(
    canonical_entity_type: impl Into<String>,
    canonical_entity_id: impl Into<String>,
    source_record_key: impl Into<String>,
    mapping_version: ContractVersion,
) -> LineageHook {
    LineageHook {
        canonical_entity_type: canonical_entity_type.into(),
        canonical_entity_id: canonical_entity_id.into(),
        source_record_key: source_record_key.into(),
        mapping_version,
    }
}

pub fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let value = serde_json::to_value(value)?;
    serde_json::to_vec(&sort_json(value))
}

pub fn schema_fingerprint<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(canonical_json(value)?)
    ))
}

pub fn validate_finite_json(value: &Value) -> Result<(), GuestError> {
    match value {
        Value::Number(number) if number.as_f64().is_some_and(|number| !number.is_finite()) => Err(
            GuestError::InvalidInput("non-finite JSON number".to_owned()),
        ),
        Value::Array(values) => values.iter().try_for_each(validate_finite_json),
        Value::Object(values) => values.values().try_for_each(validate_finite_json),
        _ => Ok(()),
    }
}

pub fn validate_batch_metadata(batch: &SourceBatch) -> Result<(), GuestError> {
    if batch.source_module_id.trim().is_empty()
        || batch.logical_snapshot_key.trim().is_empty()
        || batch.schema_fingerprint.trim().is_empty()
    {
        return Err(GuestError::InvalidInput(
            "source batch metadata cannot be blank".to_owned(),
        ));
    }
    if batch
        .source_records
        .iter()
        .any(|record| record.source_record_key.trim().is_empty() || record.source_row_number == 0)
    {
        return Err(GuestError::InvalidInput(
            "source record identity and row number are required".to_owned(),
        ));
    }
    for record in &batch.source_records {
        validate_finite_json(&record.raw_payload)?;
    }
    Ok(())
}

pub fn declared_extension(
    namespace: impl Into<String>,
    contract_version: ContractVersion,
) -> ExtensionRequirement {
    ExtensionRequirement {
        namespace: namespace.into(),
        contract_version,
    }
}

pub fn issue(code: impl Into<String>, message: impl Into<String>) -> MappingIssue {
    MappingIssue {
        code: code.into(),
        message: message.into(),
        source_record_key: None,
    }
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(sort_json).collect()),
        Value::Object(values) => {
            let sorted: BTreeMap<_, _> = values
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect();
            let object: Map<String, Value> = sorted.into_iter().collect();
            Value::Object(object)
        }
        value => value,
    }
}

#[allow(dead_code)]
fn _assert_asset_is_host_owned<T: ReadOnlyAsset>() {}
