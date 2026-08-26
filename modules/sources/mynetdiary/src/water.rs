use crate::datetime::row_local_date;
use crate::number::parse_non_negative;
use crate::{MappedRows, MappingContext, add_lineage, add_source_record, deterministic_id, text};
use crate::{MappingError, ValidatedSheet};
use mfa_contracts::{ActivityDay, CanonicalObservation, ExtensionRecord};
use serde_json::json;

pub fn map_water(
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
        let local_date = row_local_date(&sheet.name, row_number, row, date_column)?;
        let source_record_key = add_source_record(&mut mapped, context, sheet, row, data_index);
        let water_ml = parse_non_negative(
            &text(sheet, row, "Water, ml"),
            &sheet.name,
            row_number,
            "Water, ml",
        )?;
        let glasses = parse_non_negative(
            &text(sheet, row, "Glasses"),
            &sheet.name,
            row_number,
            "Glasses",
        )?;
        let activity_day = ActivityDay {
            local_date,
            steps: None,
            water_ml,
            heart_rate_observation_count: 0,
            activity_duration_seconds: 0,
            activity_distance_km: 0.0,
            estimated_activity_calories_kcal: 0.0,
        };
        let activity_day_id = deterministic_id(context, "water_activity_day", &source_record_key);
        add_lineage(
            &mut mapped,
            context,
            "activity_day",
            activity_day_id.to_string(),
            &source_record_key,
        );
        mapped
            .records
            .push(CanonicalObservation::ActivityDay(activity_day));
        mapped.extensions.push(ExtensionRecord {
            namespace: "mynetdiary.water-glasses".to_owned(),
            contract_version: "1.0.0".parse().expect("valid extension version"),
            record_type: "water_glasses".to_owned(),
            source_record_key: source_record_key.clone(),
            occurred_local_at: None,
            local_date: Some(local_date),
            payload: json!({
                "glasses": glasses,
                "water_ml": water_ml,
            }),
        });
    }
    Ok(mapped)
}
