use crate::csv_input::parse_headers;
use crate::error::MappingError;
use crate::{CsvInput, MappingContext, deterministic_id, schema_fingerprint, source_record_key};
use chrono::{Datelike, NaiveDate, NaiveDateTime};
use csv::StringRecord;
use mfa_contracts::{
    BodyMeasurement, CanonicalObservation, ExtensionRecord, LineageHook, SourceBatch, SourceRecord,
};
use serde_json::{Map, Value};

pub fn parse_measurements(
    input: CsvInput,
    context: &MappingContext,
) -> Result<SourceBatch, MappingError> {
    let headers = parse_headers(&input.bytes)?;
    for required in ["date", "weight_kg"] {
        if !headers.iter().any(|header| header == required) {
            return Err(MappingError::MissingColumn {
                column: required.to_owned(),
            });
        }
    }
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(input.bytes.as_slice());
    let mut source_records = Vec::new();
    let mut records = Vec::new();
    let mut lineage = Vec::new();
    let mut extensions = Vec::new();
    for (index, result) in reader.records().enumerate() {
        let row = result.map_err(|error| MappingError::InvalidCsv {
            detail: error.to_string(),
        })?;
        let row_number = index + 2;
        let source_key = source_record_key(context, row_number);
        source_records.push(SourceRecord {
            source_record_key: source_key.clone(),
            sheet_name: Some("measurements".to_owned()),
            source_row_number: row_number as u32,
            raw_payload: raw_payload(&headers, &row),
        });
        let date_raw = value(&headers, &row, "date");
        let weight_raw = value(&headers, &row, "weight_kg");
        if weight_raw.trim().is_empty() {
            continue;
        }
        let weight_kg = parse_number(&weight_raw, "weight_kg", row_number)?;
        if weight_kg <= 0.0 {
            return Err(MappingError::InvalidWeight {
                value: weight_raw,
                row: row_number,
            });
        }
        let local_date = parse_date(&date_raw, row_number)?;
        let body_fat_pct = optional_number(
            &value(&headers, &row, "fat_percent"),
            "fat_percent",
            row_number,
        )?;
        let measurement_id = deterministic_id(context, "body_measurement", &source_key);
        records.push(CanonicalObservation::BodyMeasurement(BodyMeasurement {
            body_measurement_id: measurement_id,
            local_date,
            weight_kg,
            body_fat_pct,
            source_record_id: Some(source_key.clone()),
        }));
        lineage.push(LineageHook {
            canonical_entity_type: "body_measurement".to_owned(),
            canonical_entity_id: measurement_id.to_string(),
            source_record_key: source_key.clone(),
            mapping_version: context.mapping_version.clone(),
        });
        let circumference = [
            ("waist_cm", "waist_cm"),
            ("neck_cm", "neck_cm"),
            ("hip_cm", "hip_cm"),
        ];
        let mut payload = Map::new();
        let mut has_circumference = false;
        for (column, key) in circumference {
            let raw = value(&headers, &row, column);
            if raw.trim().is_empty() {
                continue;
            }
            payload.insert(
                key.to_owned(),
                Value::from(parse_number(&raw, column, row_number)?),
            );
            has_circumference = true;
        }
        if has_circumference {
            extensions.push(ExtensionRecord {
                namespace: "hevy.body-circumference".to_owned(),
                contract_version: "1.0.0".parse().expect("valid extension version"),
                record_type: "circumference_cm".to_owned(),
                source_record_key: source_key,
                occurred_local_at: None,
                local_date: Some(local_date),
                payload: Value::Object(payload),
            });
        }
    }
    Ok(SourceBatch {
        contract_version: "1.0.0".parse().expect("valid contract version"),
        source_module_id: context.module_id.clone(),
        source_api_version: "1.0.0".parse().expect("valid source API version"),
        mapping_version: context.mapping_version.clone(),
        schema_fingerprint: if context.schema_fingerprint.starts_with("sha256:") {
            context.schema_fingerprint.clone()
        } else {
            schema_fingerprint(&headers)
        },
        logical_snapshot_key: context.logical_snapshot_key.clone(),
        source_records,
        lineage,
        records,
        extensions,
        issues: Vec::new(),
    })
}

pub fn context_for_measurements(input: &CsvInput) -> Result<MappingContext, MappingError> {
    let headers = parse_headers(&input.bytes)?;
    let date_index = headers
        .iter()
        .position(|header| header == "date")
        .ok_or_else(|| MappingError::MissingColumn {
            column: "date".to_owned(),
        })?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(input.bytes.as_slice());
    let first_date = reader
        .records()
        .filter_map(Result::ok)
        .map(|row| row.get(date_index).unwrap_or_default().to_owned())
        .find(|raw| !raw.trim().is_empty())
        .ok_or(MappingError::InvalidDate {
            value: String::new(),
            row: 2,
        })?;
    let year = parse_date(&first_date, 2)?.0.year();
    Ok(MappingContext::for_measurements(
        input.asset_id.clone(),
        year,
        crate::schema_fingerprint(&headers),
    ))
}

fn raw_payload(headers: &[String], row: &StringRecord) -> Value {
    let mut payload = Map::new();
    for (index, header) in headers.iter().enumerate() {
        let raw = row.get(index).unwrap_or_default();
        payload.insert(
            header.clone(),
            if raw.is_empty() {
                Value::Null
            } else {
                Value::String(raw.to_owned())
            },
        );
    }
    Value::Object(payload)
}

fn value(headers: &[String], row: &StringRecord, column: &str) -> String {
    headers
        .iter()
        .position(|header| header == column)
        .and_then(|index| row.get(index))
        .unwrap_or_default()
        .to_owned()
}

fn parse_number(raw: &str, column: &str, row: usize) -> Result<f64, MappingError> {
    let value = raw
        .trim()
        .parse::<f64>()
        .map_err(|_| MappingError::InvalidNumber {
            value: raw.to_owned(),
            column: column.to_owned(),
            row,
        })?;
    if !value.is_finite() {
        return Err(MappingError::InvalidNumber {
            value: raw.to_owned(),
            column: column.to_owned(),
            row,
        });
    }
    Ok(value)
}

fn optional_number(raw: &str, column: &str, row: usize) -> Result<Option<f64>, MappingError> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(parse_number(raw, column, row)?))
}

fn parse_date(raw: &str, row: usize) -> Result<mfa_contracts::LocalDate, MappingError> {
    let raw = raw.trim();
    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .or_else(|_| {
            NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S").map(|value| value.date())
        })
        .or_else(|_| {
            NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S").map(|value| value.date())
        })
        .map_err(|_| MappingError::InvalidDate {
            value: raw.to_owned(),
            row,
        })?;
    Ok(date.into())
}
