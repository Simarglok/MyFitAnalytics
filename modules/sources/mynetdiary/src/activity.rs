use crate::datetime::row_local_datetime;
use crate::number::parse_non_negative;
use crate::{MappedRows, MappingContext, add_lineage, add_source_record, deterministic_id, text};
use crate::{MappingError, ValidatedSheet};
use mfa_contracts::{ActivityEvent, CanonicalObservation};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
struct ActivityRule {
    activity_type: String,
}

fn rules() -> BTreeMap<String, ActivityRule> {
    serde_json::from_str(include_str!("activity_mapping.json")).expect("valid activity mapping")
}

pub fn map_activity(
    sheet: &ValidatedSheet,
    context: &MappingContext,
) -> Result<MappedRows, MappingError> {
    let date_column = sheet.column_index("Date").expect("validated Date");
    let mut mapped = MappedRows::default();
    let rules = rules();
    for (data_index, row) in sheet.rows.iter().enumerate() {
        let row_number = sheet.source_row_number(data_index);
        let (local_date, occurred_local_at) =
            row_local_datetime(&sheet.name, row_number, row, date_column, None)?;
        let source_record_key = add_source_record(&mut mapped, context, sheet, row, data_index);
        let source_name = text(sheet, row, "Activity");
        if source_name == "Traditional Strength Training" {
            continue;
        }
        let (activity_type, quality_status) = match rules.get(&source_name) {
            Some(rule) => (rule.activity_type.clone(), "accepted".to_owned()),
            None => {
                mapped.issues.push(mfa_contracts::MappingIssue {
                    code: "mynetdiary.unknown_activity".to_owned(),
                    message: format!("unknown exercise activity `{source_name}`"),
                    source_record_key: Some(source_record_key.clone()),
                });
                ("unknown".to_owned(), "unknown_mapping".to_owned())
            }
        };
        let duration_minutes = numeric(sheet, row, "Duration, min", row_number)?;
        let duration_seconds = duration_minutes
            .map(|value| (value * 60.0).round())
            .and_then(|value| (value >= 0.0 && value <= u32::MAX as f64).then_some(value as u32));
        let event = ActivityEvent {
            activity_event_id: deterministic_id(context, "activity_event", &source_record_key),
            occurred_local_at,
            local_date,
            activity_type,
            source_name,
            duration_seconds,
            distance_km: numeric(sheet, row, "Distance, km", row_number)?,
            estimated_calories_kcal: numeric(sheet, row, "Calories", row_number)?,
            origin_hint: None,
            quality_status,
            source_record_id: Some(source_record_key.clone()),
        };
        add_lineage(
            &mut mapped,
            context,
            "activity_event",
            event.activity_event_id.to_string(),
            &source_record_key,
        );
        mapped
            .records
            .push(CanonicalObservation::ActivityEvent(event));
    }
    Ok(mapped)
}

fn numeric(
    sheet: &ValidatedSheet,
    row: &[crate::cells::Cell],
    column: &str,
    row_number: usize,
) -> Result<Option<f64>, MappingError> {
    let raw = text(sheet, row, column);
    parse_non_negative(&raw, &sheet.name, row_number, column)
}
