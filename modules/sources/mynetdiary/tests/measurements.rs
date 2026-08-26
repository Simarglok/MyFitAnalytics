use mfa_contracts::CanonicalObservation;
use mfa_source_mynetdiary::workbook::validate_workbook;
use mfa_source_mynetdiary::{MappingContext, SheetKind, map_measurements};
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
fn only_daily_steps_emit_activity_day_and_other_measurements_stay_raw() {
    let schema = validate_workbook(&fixture("valid-full.xls")).unwrap();
    let mapped = map_measurements(
        schema.sheets.get(&SheetKind::Measurements).unwrap(),
        &MappingContext::synthetic("measurement-asset"),
    )
    .unwrap();
    assert_eq!(mapped.records.len(), 1);
    assert_eq!(mapped.source_records.len(), 2);
    assert!(
        mapped
            .source_records
            .iter()
            .any(|record| record.raw_payload.get("Type").unwrap() == "Weight")
    );
    assert!(matches!(
        mapped.records[0],
        CanonicalObservation::ActivityDay(_)
    ));
}
