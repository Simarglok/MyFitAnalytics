use mfa_contracts::CanonicalObservation;
use mfa_source_mynetdiary::parse_workbook;
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
fn whole_workbook_output_is_deterministic_and_lineage_complete() {
    let bytes = fixture("valid-full.xls");
    let first = parse_workbook(&bytes, "asset-1").unwrap();
    let second = parse_workbook(&bytes, "asset-1").unwrap();
    assert_eq!(first, second);
    assert_eq!(first.source_module_id, "mynetdiary");
    assert_eq!(first.logical_snapshot_key, "mynetdiary:2026");
    assert_eq!(first.source_records.len(), 8);
    assert_eq!(first.records.len(), 6);
    assert_eq!(first.lineage.len(), first.records.len());
    assert_eq!(first.extensions.len(), 1);
    assert!(first.issues.is_empty());
    let keys = first
        .source_records
        .iter()
        .map(|record| record.source_record_key.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(keys.len(), first.source_records.len());
    for hook in &first.lineage {
        assert!(keys.contains(&hook.source_record_key));
    }
    assert!(first.records.iter().any(|record| matches!(
        record,
        CanonicalObservation::ActivityDay(day) if day.steps == Some(6400)
    )));
}
