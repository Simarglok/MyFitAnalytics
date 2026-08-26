use mfa_contracts::CanonicalObservation;
use mfa_source_mynetdiary::workbook::validate_workbook;
use mfa_source_mynetdiary::{MappingContext, SheetKind, map_activity};
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
fn approved_activity_maps_and_strength_is_not_an_activity() {
    let schema = validate_workbook(&fixture("valid-full.xls")).unwrap();
    let mapped = map_activity(
        schema.sheets.get(&SheetKind::Exercise).unwrap(),
        &MappingContext::synthetic("activity-asset"),
    )
    .unwrap();
    assert_eq!(mapped.records.len(), 1);
    assert!(mapped.issues.is_empty());
    let CanonicalObservation::ActivityEvent(event) = &mapped.records[0] else {
        panic!()
    };
    assert_eq!(event.activity_type, "walking");
    assert_eq!(event.duration_seconds, Some(1800));
}

#[test]
fn unknown_activity_is_traceable_and_excluded_by_quality_status() {
    let schema = validate_workbook(&fixture("unknown-activity.xls")).unwrap();
    let mapped = map_activity(
        schema.sheets.get(&SheetKind::Exercise).unwrap(),
        &MappingContext::synthetic("unknown-asset"),
    )
    .unwrap();
    assert_eq!(mapped.records.len(), 1);
    assert!(
        mapped
            .issues
            .iter()
            .any(|issue| issue.code == "mynetdiary.unknown_activity")
    );
    let unknown = mapped
        .records
        .iter()
        .find_map(|record| match record {
            CanonicalObservation::ActivityEvent(event) if event.activity_type == "unknown" => {
                Some(event)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(unknown.quality_status, "unknown_mapping");
}
