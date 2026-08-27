use mfa_source_mynetdiary::{
    MappingError, SheetKind, detect_mynetdiary, infer_calendar_year, validate_workbook,
};
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
fn probe_accepts_biff_structure_without_using_filename() {
    let bytes = fixture("valid-full.xls");
    assert_eq!(detect_mynetdiary(&bytes), 100);
}

#[test]
fn probe_rejects_non_biff_and_arbitrary_biff_content() {
    assert_eq!(
        detect_mynetdiary(b"Date,Food Name\n2026-01-01,Synthetic"),
        0
    );
    assert_eq!(detect_mynetdiary(b"PK\x03\x04not-an-xlsx"), 0);
    let mut corrupt = fixture("valid-full.xls");
    corrupt[0] = 0;
    assert_eq!(detect_mynetdiary(&corrupt), 0);
    assert_eq!(detect_mynetdiary(&fixture("missing-required-sheet.xls")), 0);
}

#[test]
fn schema_requires_food_measurements_and_exercise_and_infers_content_year() {
    let schema = validate_workbook(&fixture("valid-full.xls")).unwrap();
    assert!(schema.sheets.contains_key(&SheetKind::Food));
    assert!(schema.sheets.contains_key(&SheetKind::Measurements));
    assert!(schema.sheets.contains_key(&SheetKind::Exercise));
    assert_eq!(infer_calendar_year(&schema).unwrap(), 2026);
}

#[test]
fn optional_sheets_may_be_absent() {
    let schema = validate_workbook(&fixture("optional-sheets-absent.xls")).unwrap();
    assert!(!schema.sheets.contains_key(&SheetKind::Trackers));
    assert!(!schema.sheets.contains_key(&SheetKind::WaterGlasses));
}

#[test]
fn missing_sheet_and_schema_drift_have_stable_diagnostics() {
    assert!(matches!(
        validate_workbook(&fixture("missing-required-sheet.xls")),
        Err(MappingError::MissingSheet { sheet }) if sheet == "Exercise"
    ));
    assert!(matches!(
        validate_workbook(&fixture("schema-drift.xls")),
        Err(MappingError::MissingColumn { sheet, column }) if sheet == "Food" && column == "Calories"
    ));
}

#[test]
fn mixed_calendar_year_is_rejected_even_when_filename_is_yearless() {
    assert!(matches!(
        validate_workbook(&fixture("mixed-year.xls")),
        Err(MappingError::MixedCalendarYear { .. })
    ));
}
