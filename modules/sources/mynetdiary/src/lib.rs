pub mod activity;
pub mod cells;
pub mod datetime;
pub mod error;
pub mod food;
pub mod measurements;
pub mod number;
pub mod schema;
pub mod trackers;
pub mod water;
pub mod workbook;

#[cfg(target_arch = "wasm32")]
mod component;

use crate::schema::WorkbookSchema;
use mfa_contracts::{
    CanonicalObservation, ContractVersion, ExtensionRecord, LineageHook, MappingIssue, SourceBatch,
    SourceRecord,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub use activity::map_activity;
pub use error::MappingError;
pub use food::map_food;
pub use measurements::map_measurements;
pub use schema::{SheetKind, ValidatedSheet, WorkbookSchema as PublicWorkbookSchema};
pub use trackers::map_trackers;
pub use water::map_water;
pub use workbook::{detect_mynetdiary, infer_calendar_year, validate_workbook};

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
            module_id: "mynetdiary".to_owned(),
            asset_id: asset_id.into(),
            mapping_version: "1.0.0".parse().expect("valid mapping version"),
            schema_fingerprint:
                "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
            logical_snapshot_key: "mynetdiary:2026".to_owned(),
        }
    }

    pub fn for_workbook(asset_id: impl Into<String>, schema: &WorkbookSchema) -> Self {
        let mut fingerprint_input = String::new();
        for sheet in schema.sheets.values() {
            fingerprint_input.push_str(sheet.kind.workbook_name());
            fingerprint_input.push('|');
            fingerprint_input.push_str(&sheet.headers.join("\u{1f}"));
            fingerprint_input.push('\n');
        }
        Self {
            module_id: "mynetdiary".to_owned(),
            asset_id: asset_id.into(),
            mapping_version: "1.0.0".parse().expect("valid mapping version"),
            schema_fingerprint: format!("sha256:{:x}", Sha256::digest(fingerprint_input)),
            logical_snapshot_key: format!("mynetdiary:{}", schema.calendar_year),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct MappedRows {
    pub records: Vec<CanonicalObservation>,
    pub source_records: Vec<SourceRecord>,
    pub lineage: Vec<LineageHook>,
    pub extensions: Vec<ExtensionRecord>,
    pub issues: Vec<MappingIssue>,
}

impl MappedRows {
    pub fn append(&mut self, mut other: Self) {
        self.records.append(&mut other.records);
        self.source_records.append(&mut other.source_records);
        self.lineage.append(&mut other.lineage);
        self.extensions.append(&mut other.extensions);
        self.issues.append(&mut other.issues);
    }
}

pub fn parse_workbook(
    bytes: &[u8],
    asset_id: impl Into<String>,
) -> Result<SourceBatch, MappingError> {
    let schema = validate_workbook(bytes)?;
    let context = MappingContext::for_workbook(asset_id, &schema);
    let mut mapped = MappedRows::default();
    mapped.append(food::map_food(
        schema.sheets.get(&SheetKind::Food).expect("validated Food"),
        &context,
    )?);
    mapped.append(measurements::map_measurements(
        schema
            .sheets
            .get(&SheetKind::Measurements)
            .expect("validated Measurements"),
        &context,
    )?);
    mapped.append(activity::map_activity(
        schema
            .sheets
            .get(&SheetKind::Exercise)
            .expect("validated Exercise"),
        &context,
    )?);
    mapped.append(trackers::map_trackers(
        schema.sheets.get(&SheetKind::Trackers),
        &context,
    )?);
    mapped.append(water::map_water(
        schema.sheets.get(&SheetKind::WaterGlasses),
        &context,
    )?);
    Ok(SourceBatch {
        contract_version: "1.0.0".parse().expect("valid contract version"),
        source_module_id: context.module_id,
        source_api_version: "1.0.0".parse().expect("valid source API version"),
        mapping_version: context.mapping_version,
        schema_fingerprint: context.schema_fingerprint,
        logical_snapshot_key: context.logical_snapshot_key,
        source_records: mapped.source_records,
        lineage: mapped.lineage,
        records: mapped.records,
        extensions: mapped.extensions,
        issues: mapped.issues,
    })
}

pub(crate) fn source_key(context: &MappingContext, sheet: &str, row_number: usize) -> String {
    format!("{}:{}:{}", context.asset_id, sheet, row_number)
}

pub(crate) fn raw_payload(sheet: &ValidatedSheet, row: &[crate::cells::Cell]) -> Value {
    let mut payload = Map::new();
    for (index, header) in sheet.headers.iter().enumerate() {
        let value = row.get(index).map(|cell| cell.display.trim());
        payload.insert(
            header.clone(),
            match value {
                Some(value) if !value.is_empty() => Value::String(value.to_owned()),
                _ => Value::Null,
            },
        );
    }
    Value::Object(payload)
}

pub(crate) fn add_source_record(
    mapped: &mut MappedRows,
    context: &MappingContext,
    sheet: &ValidatedSheet,
    row: &[crate::cells::Cell],
    data_index: usize,
) -> String {
    let row_number = sheet.source_row_number(data_index);
    let key = source_key(context, sheet.kind.workbook_name(), row_number);
    mapped.source_records.push(SourceRecord {
        source_record_key: key.clone(),
        sheet_name: Some(sheet.name.clone()),
        source_row_number: row_number as u32,
        raw_payload: raw_payload(sheet, row),
    });
    key
}

pub(crate) fn add_lineage(
    mapped: &mut MappedRows,
    context: &MappingContext,
    entity_type: &str,
    entity_id: impl Into<String>,
    source_record_key: &str,
) {
    mapped.lineage.push(LineageHook {
        canonical_entity_type: entity_type.to_owned(),
        canonical_entity_id: entity_id.into(),
        source_record_key: source_record_key.to_owned(),
        mapping_version: context.mapping_version.clone(),
    });
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

pub(crate) fn text(sheet: &ValidatedSheet, row: &[crate::cells::Cell], column: &str) -> String {
    sheet
        .column_index(column)
        .and_then(|index| row.get(index))
        .map(|cell| cell.display.trim().to_owned())
        .unwrap_or_default()
}

pub(crate) fn optional_text(
    sheet: &ValidatedSheet,
    row: &[crate::cells::Cell],
    column: &str,
) -> Option<String> {
    let value = text(sheet, row, column);
    (!value.is_empty()).then_some(value)
}
