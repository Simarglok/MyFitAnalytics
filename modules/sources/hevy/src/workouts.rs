use crate::csv_input::parse_headers;
use crate::error::MappingError;
use crate::{CsvInput, MappingContext, deterministic_id};
use chrono::{Datelike, NaiveDateTime};
use csv::StringRecord;
use mfa_contracts::{
    CanonicalObservation, ExerciseSet, LineageHook, MappingIssue, SourceBatch, SourceRecord,
    WorkoutSession,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct WorkoutRow {
    pub title: String,
    pub start_local_at: mfa_contracts::LocalDateTime,
    pub end_local_at: mfa_contracts::LocalDateTime,
    pub exercise_title: String,
    pub set_index: u32,
    pub set_type: String,
    pub weight_kg: Option<f64>,
    pub reps: Option<u32>,
    pub rpe: Option<f64>,
    pub duration_seconds: Option<u32>,
    pub notes: String,
    pub source_row_number: usize,
    pub source_record_key: String,
    pub raw_payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkoutGroup {
    pub group_key: String,
    pub title: String,
    pub start_local_at: mfa_contracts::LocalDateTime,
    pub end_local_at: mfa_contracts::LocalDateTime,
    pub rows: Vec<WorkoutRow>,
}

impl WorkoutGroup {
    pub fn duration_seconds(&self) -> Option<u32> {
        (self.end_local_at.0 - self.start_local_at.0)
            .num_seconds()
            .try_into()
            .ok()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExerciseMapping {
    entries: BTreeMap<String, String>,
}

impl Default for ExerciseMapping {
    fn default() -> Self {
        let entries: BTreeMap<String, String> =
            serde_json::from_str(include_str!("exercise_mapping.json"))
                .expect("valid Hevy exercise mapping");
        Self { entries }
    }
}

impl ExerciseMapping {
    pub fn load_type(&self, exercise_title: &str) -> Option<&str> {
        self.entries
            .get(&normalize_exercise(exercise_title))
            .map(String::as_str)
    }
}

pub fn parse_workout_rows(input: CsvInput) -> Result<Vec<WorkoutRow>, MappingError> {
    let headers = parse_headers(&input.bytes)?;
    for required in [
        "title",
        "start_time",
        "end_time",
        "exercise_title",
        "set_index",
        "set_type",
    ] {
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
    let mut rows = Vec::new();
    for (index, result) in reader.records().enumerate() {
        let row = result.map_err(|error| MappingError::InvalidCsv {
            detail: error.to_string(),
        })?;
        let source_row_number = index + 2;
        let start = parse_datetime(&value(&headers, &row, "start_time"), source_row_number)?;
        let end = parse_datetime(&value(&headers, &row, "end_time"), source_row_number)?;
        if end.0 < start.0 {
            return Err(MappingError::InvalidSessionTime {
                row: source_row_number,
            });
        }
        let set_index = value(&headers, &row, "set_index")
            .trim()
            .parse::<u32>()
            .map_err(|_| MappingError::InvalidSetValue {
                value: value(&headers, &row, "set_index"),
                column: "set_index".to_owned(),
                row: source_row_number,
            })?;
        let source_record_key = format!("{}:workouts:{source_row_number}", input.asset_id);
        rows.push(WorkoutRow {
            title: value(&headers, &row, "title"),
            start_local_at: start,
            end_local_at: end,
            exercise_title: value(&headers, &row, "exercise_title"),
            set_index,
            set_type: value(&headers, &row, "set_type"),
            weight_kg: optional_number(&headers, &row, "weight_kg", source_row_number)?,
            reps: optional_u32(&headers, &row, "reps", source_row_number)?,
            rpe: optional_number(&headers, &row, "rpe", source_row_number)?,
            duration_seconds: optional_u32(&headers, &row, "duration_seconds", source_row_number)?,
            notes: value(&headers, &row, "notes"),
            source_row_number,
            source_record_key,
            raw_payload: raw_payload(&headers, &row),
        });
    }
    Ok(rows)
}

pub fn context_for_workouts(input: &CsvInput) -> Result<MappingContext, MappingError> {
    let headers = parse_headers(&input.bytes)?;
    let start_index = headers
        .iter()
        .position(|header| header == "start_time")
        .ok_or_else(|| MappingError::MissingColumn {
            column: "start_time".to_owned(),
        })?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(input.bytes.as_slice());
    let first_start = reader
        .records()
        .filter_map(Result::ok)
        .map(|row| row.get(start_index).unwrap_or_default().to_owned())
        .find(|raw| !raw.trim().is_empty())
        .ok_or(MappingError::InvalidDate {
            value: String::new(),
            row: 2,
        })?;
    let start = NaiveDateTime::parse_from_str(first_start.trim(), "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(first_start.trim(), "%Y-%m-%dT%H:%M:%S"))
        .map_err(|_| MappingError::InvalidDate {
            value: first_start,
            row: 2,
        })?;
    Ok(MappingContext::for_workouts(
        input.asset_id.clone(),
        start.year(),
        crate::schema_fingerprint(&headers),
    ))
}

pub fn group_sessions(rows: Vec<WorkoutRow>) -> Result<Vec<WorkoutGroup>, MappingError> {
    let mut groups: BTreeMap<String, WorkoutGroup> = BTreeMap::new();
    for row in rows {
        if row.end_local_at.0 < row.start_local_at.0 {
            return Err(MappingError::InvalidSessionTime {
                row: row.source_row_number,
            });
        }
        let group_key = format!("{}|{}|{}", row.title, row.start_local_at, row.end_local_at);
        groups
            .entry(group_key.clone())
            .and_modify(|group| group.rows.push(row.clone()))
            .or_insert_with(|| WorkoutGroup {
                group_key,
                title: row.title.clone(),
                start_local_at: row.start_local_at,
                end_local_at: row.end_local_at,
                rows: vec![row],
            });
    }
    let mut groups = groups.into_values().collect::<Vec<_>>();
    groups.sort_by_key(|group| (group.start_local_at, group.title.clone()));
    Ok(groups)
}

pub fn assign_exercise_blocks(rows: &[WorkoutRow]) -> Vec<u32> {
    let mut ordinal = 0;
    let mut previous = None;
    rows.iter()
        .map(|row| {
            if previous.as_deref() != Some(row.exercise_title.as_str()) {
                ordinal += 1;
                previous = Some(row.exercise_title.clone());
            }
            ordinal
        })
        .collect()
}

pub fn parse_workouts(
    input: CsvInput,
    mapping: &ExerciseMapping,
    context: &MappingContext,
) -> Result<SourceBatch, MappingError> {
    let rows = parse_workout_rows(input)?;
    let groups = group_sessions(rows.clone())?;
    let mut source_records = rows
        .iter()
        .map(|row| SourceRecord {
            source_record_key: row.source_record_key.clone(),
            sheet_name: Some("workouts".to_owned()),
            source_row_number: row.source_row_number as u32,
            raw_payload: row.raw_payload.clone(),
        })
        .collect::<Vec<_>>();
    source_records.sort_by_key(|record| record.source_row_number);
    let mut records = Vec::new();
    let mut lineage = Vec::new();
    let mut issues = Vec::new();
    for group in groups {
        let session_id = deterministic_id(context, "workout_session", &group.group_key);
        let session_source_key = group
            .rows
            .first()
            .map(|row| row.source_record_key.clone())
            .expect("group has rows");
        records.push(CanonicalObservation::WorkoutSession(WorkoutSession {
            workout_session_id: session_id,
            title: group.title.clone(),
            started_local_at: group.start_local_at,
            ended_local_at: group.end_local_at,
            duration_seconds: group.duration_seconds(),
            source_record_group_key: group.group_key.clone(),
        }));
        lineage.push(LineageHook {
            canonical_entity_type: "workout_session".to_owned(),
            canonical_entity_id: session_id.to_string(),
            source_record_key: session_source_key,
            mapping_version: context.mapping_version.clone(),
        });
        let block_ordinals = assign_exercise_blocks(&group.rows);
        for (row, block_ordinal) in group.rows.iter().zip(block_ordinals) {
            let mapped_load_type = mapping.load_type(&row.exercise_title);
            let load_type = mapped_load_type.unwrap_or("unknown");
            if mapped_load_type.is_none() {
                issues.push(MappingIssue {
                    code: "hevy.unknown_exercise".to_owned(),
                    message: format!("unknown exercise `{}`", row.exercise_title),
                    source_record_key: Some(row.source_record_key.clone()),
                });
            }
            let normalized_set_type = row.set_type.trim().to_ascii_lowercase();
            if !matches!(
                normalized_set_type.as_str(),
                "warmup" | "normal" | "failure"
            ) {
                issues.push(MappingIssue {
                    code: "hevy.unknown_set_type".to_owned(),
                    message: format!("set type `{}` is excluded from e1RM", row.set_type),
                    source_record_key: Some(row.source_record_key.clone()),
                });
            }
            let set_id = deterministic_id(
                context,
                "exercise_set",
                &format!(
                    "{}:{}:{}:{}",
                    session_id, block_ordinal, row.set_index, row.source_row_number
                ),
            );
            let set = ExerciseSet {
                exercise_set_id: set_id,
                workout_session_id: session_id,
                exercise_title_raw: row.exercise_title.clone(),
                exercise_key: normalize_exercise(&row.exercise_title),
                exercise_block_ordinal: block_ordinal,
                set_index: row.set_index,
                set_type: row.set_type.clone(),
                load_type: load_type.to_owned(),
                weight_kg: if matches!(load_type, "external" | "unknown") {
                    row.weight_kg
                } else {
                    None
                },
                reps: row.reps,
                duration_seconds: row.duration_seconds,
                rpe: row.rpe,
                source_record_id: Some(row.source_record_key.clone()),
            };
            records.push(CanonicalObservation::ExerciseSet(set));
            lineage.push(LineageHook {
                canonical_entity_type: "exercise_set".to_owned(),
                canonical_entity_id: set_id.to_string(),
                source_record_key: row.source_record_key.clone(),
                mapping_version: context.mapping_version.clone(),
            });
        }
    }
    Ok(SourceBatch {
        contract_version: "1.0.0".parse().expect("valid contract version"),
        source_module_id: context.module_id.clone(),
        source_api_version: "1.0.0".parse().expect("valid source API version"),
        mapping_version: context.mapping_version.clone(),
        schema_fingerprint: context.schema_fingerprint.clone(),
        logical_snapshot_key: context.logical_snapshot_key.clone(),
        source_records,
        lineage,
        records,
        extensions: Vec::new(),
        issues,
    })
}

fn normalize_exercise(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn value(headers: &[String], row: &StringRecord, column: &str) -> String {
    headers
        .iter()
        .position(|header| header == column)
        .and_then(|index| row.get(index))
        .unwrap_or_default()
        .to_owned()
}

fn parse_datetime(raw: &str, row: usize) -> Result<mfa_contracts::LocalDateTime, MappingError> {
    let parsed = NaiveDateTime::parse_from_str(raw.trim(), "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(raw.trim(), "%Y-%m-%dT%H:%M:%S"))
        .map_err(|_| MappingError::InvalidDate {
            value: raw.to_owned(),
            row,
        })?;
    Ok(parsed.into())
}

fn optional_number(
    headers: &[String],
    row: &StringRecord,
    column: &str,
    row_number: usize,
) -> Result<Option<f64>, MappingError> {
    let raw = value(headers, row, column);
    if raw.trim().is_empty() {
        return Ok(None);
    }
    let parsed = raw
        .trim()
        .parse::<f64>()
        .map_err(|_| MappingError::InvalidSetValue {
            value: raw.clone(),
            column: column.to_owned(),
            row: row_number,
        })?;
    if !parsed.is_finite() {
        return Err(MappingError::InvalidSetValue {
            value: raw,
            column: column.to_owned(),
            row: row_number,
        });
    }
    Ok(Some(parsed))
}

fn optional_u32(
    headers: &[String],
    row: &StringRecord,
    column: &str,
    row_number: usize,
) -> Result<Option<u32>, MappingError> {
    let raw = value(headers, row, column);
    if raw.trim().is_empty() {
        return Ok(None);
    }
    raw.trim()
        .parse::<u32>()
        .map(Some)
        .map_err(|_| MappingError::InvalidSetValue {
            value: raw,
            column: column.to_owned(),
            row: row_number,
        })
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
