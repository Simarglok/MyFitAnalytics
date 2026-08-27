use mfa_contracts::CanonicalObservation;
use mfa_source_hevy::{
    CsvInput, HevyArtifact, MappingContext, ProbeResult, detect_hevy, parse_measurements,
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

struct MemoryAsset {
    bytes: Vec<u8>,
}

impl mfa_source_hevy::GuestAssetReader for MemoryAsset {
    fn read_all(&mut self) -> Result<Vec<u8>, String> {
        Ok(self.bytes.clone())
    }
}

#[test]
fn probe_matches_measurements_by_headers_not_filename() {
    let mut asset = MemoryAsset {
        bytes: fixture("measurement_data.csv"),
    };
    assert_eq!(
        detect_hevy(&mut asset),
        ProbeResult::Match(HevyArtifact::Measurements)
    );

    let mut renamed = MemoryAsset {
        bytes: fixture("measurement_data.csv"),
    };
    assert_eq!(
        detect_hevy(&mut renamed),
        ProbeResult::Match(HevyArtifact::Measurements)
    );

    let mut workout = MemoryAsset {
        bytes: fixture("workout_data.csv"),
    };
    assert_eq!(
        detect_hevy(&mut workout),
        ProbeResult::Match(HevyArtifact::Workouts)
    );
}

#[test]
fn probe_rejects_invalid_utf8_duplicate_headers_and_unknown_csv() {
    let mut invalid_utf8 = MemoryAsset {
        bytes: b"date,weight_kg\n2026-01-01,\xFF".to_vec(),
    };
    assert_eq!(detect_hevy(&mut invalid_utf8), ProbeResult::InvalidUtf8);

    let mut duplicate = MemoryAsset {
        bytes: b"date,weight_kg,weight_kg\n2026-01-01,80,81".to_vec(),
    };
    assert_eq!(detect_hevy(&mut duplicate), ProbeResult::InvalidSchema);

    let mut unknown = MemoryAsset {
        bytes: b"foo,bar\n1,2".to_vec(),
    };
    assert_eq!(detect_hevy(&mut unknown), ProbeResult::NoMatch);
}

#[test]
fn mapping_preserves_raw_rows_and_emits_local_date_measurements_and_circumference() {
    let batch = parse_measurements(
        CsvInput::new(fixture("measurement_data.csv"), "measurement-asset"),
        &MappingContext::synthetic("measurement-asset"),
    )
    .unwrap();
    assert_eq!(batch.source_module_id, "hevy");
    assert_eq!(batch.logical_snapshot_key, "hevy:measurements:2026");
    assert_eq!(batch.source_records.len(), 3);
    assert_eq!(batch.records.len(), 2);
    assert_eq!(batch.extensions.len(), 2);
    assert_eq!(batch.lineage.len(), 2);

    let CanonicalObservation::BodyMeasurement(first) = &batch.records[0] else {
        panic!()
    };
    assert_eq!(first.local_date.to_string(), "2026-02-01");
    assert_eq!(first.weight_kg, 81.4);
    assert_eq!(first.body_fat_pct, Some(18.2));
    assert_eq!(batch.extensions[0].namespace, "hevy.body-circumference");
    assert_eq!(batch.extensions[0].payload["waist_cm"], 86.0);
}

#[test]
fn mapping_rejects_non_positive_weight_and_keeps_blank_fat_null() {
    let input = b"date,weight_kg,fat_percent\n2026-02-01,0,\n2026-02-02,81.0,".to_vec();
    let error = parse_measurements(
        CsvInput::new(input, "measurement-asset"),
        &MappingContext::synthetic("measurement-asset"),
    )
    .unwrap_err();
    assert_eq!(error.code(), "hevy.invalid_weight");
}
