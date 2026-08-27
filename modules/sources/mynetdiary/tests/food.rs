use mfa_contracts::CanonicalObservation;
use mfa_source_mynetdiary::workbook::validate_workbook;
use mfa_source_mynetdiary::{MappingContext, map_food};
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
}

fn context() -> MappingContext {
    MappingContext::synthetic("synthetic-asset")
}

#[test]
fn food_preserves_duplicate_rows_raw_amount_and_local_datetime() {
    let schema = validate_workbook(&fixture("valid-full.xls")).unwrap();
    let mapped = map_food(
        schema
            .sheets
            .get(&mfa_source_mynetdiary::SheetKind::Food)
            .unwrap(),
        &context(),
    )
    .unwrap();
    assert_eq!(mapped.records.len(), 2);
    assert_eq!(mapped.source_records.len(), 2);
    assert_ne!(
        mapped.lineage[0].canonical_entity_id,
        mapped.lineage[1].canonical_entity_id
    );
    assert!(mapped.source_records[0].raw_payload.get("Notes").is_some());
    match &mapped.records[0] {
        CanonicalObservation::NutritionItem(item) => {
            assert_eq!(item.local_date.to_string(), "2026-01-04");
            assert_eq!(
                item.occurred_local_at.unwrap().to_string(),
                "2026-01-04T08:15:00"
            );
            assert_eq!(item.amount_raw, "1 serving");
        }
        other => panic!("unexpected record: {other:?}"),
    }
    let CanonicalObservation::NutritionItem(second) = &mapped.records[1] else {
        panic!()
    };
    assert_eq!(second.protein_g, None);
}

#[test]
fn food_numeric_parser_accepts_decimal_comma_and_nbsp() {
    let schema = validate_workbook(&fixture("decimal-comma-nbsp.xls")).unwrap();
    let mapped = map_food(
        schema
            .sheets
            .get(&mfa_source_mynetdiary::SheetKind::Food)
            .unwrap(),
        &context(),
    )
    .unwrap();
    let CanonicalObservation::NutritionItem(item) = &mapped.records[0] else {
        panic!()
    };
    assert_eq!(item.calories_kcal, Some(1234.5));
    assert_eq!(item.protein_g, Some(12.5));
}
