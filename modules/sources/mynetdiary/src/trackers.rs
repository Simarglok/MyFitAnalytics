use crate::datetime::row_local_datetime;
use crate::number::parse_non_negative;
use crate::{MappedRows, MappingContext, add_lineage, add_source_record, deterministic_id, text};
use crate::{MappingError, ValidatedSheet};
use mfa_contracts::{CanonicalObservation, HeartRateObservation};

pub fn map_trackers(
    sheet: Option<&ValidatedSheet>,
    context: &MappingContext,
) -> Result<MappedRows, MappingError> {
    let Some(sheet) = sheet else {
        return Ok(MappedRows::default());
    };
    let date_column = sheet.column_index("Date").expect("validated Date");
    let mut mapped = MappedRows::default();
    for (data_index, row) in sheet.rows.iter().enumerate() {
        let row_number = sheet.source_row_number(data_index);
        let (local_date, observed_local_at) =
            row_local_datetime(&sheet.name, row_number, row, date_column, None)?;
        let source_record_key = add_source_record(&mut mapped, context, sheet, row, data_index);
        if text(sheet, row, "Type") != "Heart Rate" {
            continue;
        }
        let raw = text(sheet, row, "Value");
        let Some(heart_rate_bpm) = parse_non_negative(&raw, &sheet.name, row_number, "Value")?
        else {
            continue;
        };
        let observation = HeartRateObservation {
            heart_rate_observation_id: deterministic_id(
                context,
                "heart_rate_observation",
                &source_record_key,
            ),
            observed_local_at,
            heart_rate_bpm,
            source_record_id: Some(source_record_key.clone()),
        };
        add_lineage(
            &mut mapped,
            context,
            "heart_rate",
            observation.heart_rate_observation_id.to_string(),
            &source_record_key,
        );
        mapped
            .records
            .push(CanonicalObservation::HeartRate(observation));
        let _ = local_date;
    }
    Ok(mapped)
}
