use crate::datetime::row_local_datetime;
use crate::number::parse_non_negative;
use crate::{
    MappedRows, MappingContext, add_lineage, add_source_record, deterministic_id, optional_text,
    text,
};
use crate::{MappingError, ValidatedSheet};
use mfa_contracts::{CanonicalObservation, NutritionItem};

pub fn map_food(
    sheet: &ValidatedSheet,
    context: &MappingContext,
) -> Result<MappedRows, MappingError> {
    let date_column = sheet.column_index("Date").expect("validated Date");
    let time_column = sheet.column_index("Time").expect("validated Time");
    let mut mapped = MappedRows::default();
    for (data_index, row) in sheet.rows.iter().enumerate() {
        let row_number = sheet.source_row_number(data_index);
        let (local_date, occurred_local_at) =
            row_local_datetime(&sheet.name, row_number, row, date_column, Some(time_column))?;
        let source_record_key = add_source_record(&mut mapped, context, sheet, row, data_index);
        let nutrition_item_id = deterministic_id(context, "nutrition_item", &source_record_key);
        let item = NutritionItem {
            nutrition_item_id,
            occurred_local_at: Some(occurred_local_at),
            local_date,
            meal: optional_text(sheet, row, "Meal").unwrap_or_else(|| "unknown".to_owned()),
            food_source_id: {
                let value = text(sheet, row, "Food ID");
                if value.is_empty() {
                    "unknown".to_owned()
                } else {
                    value
                }
            },
            name: text(sheet, row, "Food Name"),
            amount_raw: text(sheet, row, "Amount"),
            calories_kcal: numeric(sheet, row, "Calories", row_number)?,
            protein_g: numeric(sheet, row, "Protein, g", row_number)?,
            fat_g: numeric(sheet, row, "Fat, g", row_number)?,
            carbs_g: numeric(sheet, row, "Carbs, g", row_number)?,
            fiber_g: numeric(sheet, row, "Fiber, g", row_number)?,
            sugars_g: numeric(sheet, row, "Sugars, g", row_number)?,
            sodium_mg: numeric(sheet, row, "Sodium, mg", row_number)?,
            source_record_id: Some(source_record_key.clone()),
        };
        add_lineage(
            &mut mapped,
            context,
            "nutrition_item",
            item.nutrition_item_id.to_string(),
            &source_record_key,
        );
        mapped
            .records
            .push(CanonicalObservation::NutritionItem(item));
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
