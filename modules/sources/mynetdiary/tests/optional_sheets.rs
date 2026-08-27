use mfa_contracts::CanonicalObservation;
use mfa_source_mynetdiary::cells::Cell;
use mfa_source_mynetdiary::workbook::validate_workbook;
use mfa_source_mynetdiary::{MappingContext, SheetKind, map_trackers, map_water};
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
}

#[test]
fn trackers_import_only_heart_rate_and_keep_optional_raw_fields() {
    let schema = validate_workbook(&fixture("valid-full.xls")).unwrap();
    let mapped = map_trackers(
        schema.sheets.get(&SheetKind::Trackers),
        &MappingContext::synthetic("tracker-asset"),
    )
    .unwrap();
    assert_eq!(mapped.records.len(), 1);
    assert_eq!(mapped.source_records.len(), 1);
    assert!(matches!(
        mapped.records[0],
        CanonicalObservation::HeartRate(_)
    ));
    assert_eq!(
        mapped.source_records[0].raw_payload.get("Notes").unwrap(),
        "fictional tracker"
    );
}

#[test]
fn blank_heart_rate_value_remains_missing_without_failing_the_batch() {
    let schema = validate_workbook(&fixture("valid-full.xls")).unwrap();
    let mut sheet = schema.sheets.get(&SheetKind::Trackers).unwrap().clone();
    let value_column = sheet.column_index("Value").unwrap();
    sheet.rows[0][value_column] = Cell::empty();

    let mapped =
        map_trackers(Some(&sheet), &MappingContext::synthetic("blank-heart-rate")).unwrap();

    assert!(mapped.records.is_empty());
    assert_eq!(mapped.source_records.len(), 1);
}

#[test]
fn water_emits_canonical_water_and_versioned_glass_extension() {
    let schema = validate_workbook(&fixture("valid-full.xls")).unwrap();
    let mapped = map_water(
        schema.sheets.get(&SheetKind::WaterGlasses),
        &MappingContext::synthetic("water-asset"),
    )
    .unwrap();
    assert_eq!(mapped.records.len(), 1);
    assert_eq!(mapped.extensions.len(), 1);
    assert_eq!(mapped.extensions[0].namespace, "mynetdiary.water-glasses");
    assert_eq!(mapped.extensions[0].contract_version.to_string(), "1.0.0");
}

#[test]
fn missing_optional_sheets_are_empty_successes() {
    let schema = validate_workbook(&fixture("optional-sheets-absent.xls")).unwrap();
    assert!(
        map_trackers(None, &MappingContext::synthetic("no-trackers"))
            .unwrap()
            .records
            .is_empty()
    );
    assert!(
        map_water(None, &MappingContext::synthetic("no-water"))
            .unwrap()
            .records
            .is_empty()
    );
    assert!(!schema.sheets.contains_key(&SheetKind::Trackers));
}
