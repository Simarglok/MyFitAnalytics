use crate::datetime::row_local_date;
use crate::number::parse_integral_non_negative;
use crate::{MappedRows, MappingContext, add_lineage, add_source_record, text};
use crate::{MappingError, ValidatedSheet};
use mfa_contracts::{ActivityDay, CanonicalObservation};

pub fn map_measurements(
    sheet: &ValidatedSheet,
    context: &MappingContext,
) -> Result<MappedRows, MappingError> {
    let date_column = sheet.column_index("Date").expect("validated Date");
    let type_column = sheet.column_index("Type").expect("validated Type");
    let value_column = sheet.column_index("Value").expect("validated Value");
    let mut mapped = MappedRows::default();
    for (data_index, row) in sheet.rows.iter().enumerate() {
        let row_number = sheet.source_row_number(data_index);
        let local_date = row_local_date(&sheet.name, row_number, row, date_column)?;
        let source_record_key = add_source_record(&mut mapped, context, sheet, row, data_index);
        if text(sheet, row, "Type") != "Daily Steps Count" {
            continue;
        }
        let steps = parse_integral_non_negative(
            row.get(value_column)
                .map(|cell| cell.display.as_str())
                .unwrap_or_default(),
            &sheet.name,
            row_number,
            &sheet.headers[type_column],
        )?;
        let activity_day = ActivityDay {
            local_date,
            steps,
            water_ml: None,
            heart_rate_observation_count: 0,
            activity_duration_seconds: 0,
            activity_distance_km: 0.0,
            estimated_activity_calories_kcal: 0.0,
        };
        add_lineage(
            &mut mapped,
            context,
            "activity_day",
            source_record_key.clone(),
            &source_record_key,
        );
        mapped
            .records
            .push(CanonicalObservation::ActivityDay(activity_day));
    }
    Ok(mapped)
}
