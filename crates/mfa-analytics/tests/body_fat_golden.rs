use chrono::NaiveDate;
use mfa_analytics::{AlgorithmVersion, DateRange, MetricContext, SnapshotRef, body_fat_analytics};
use mfa_contracts::BodyMeasurement;
use uuid::Uuid;

fn date(value: &str) -> mfa_contracts::LocalDate {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap().into()
}

fn measurement(day: &str, body_fat_pct: Option<f64>, suffix: u128) -> BodyMeasurement {
    BodyMeasurement {
        body_measurement_id: Uuid::from_u128(suffix),
        local_date: date(day),
        weight_kg: 80.0,
        body_fat_pct,
        source_record_id: None,
    }
}

fn context() -> MetricContext {
    MetricContext {
        requested: DateRange {
            start: date("2026-01-01"),
            end: date("2026-01-03"),
        },
        as_of: date("2026-01-03"),
        snapshot_refs: vec![SnapshotRef {
            logical_snapshot_key: "hevy:measurements:2026".to_owned(),
            snapshot_id: "snapshot-body-fat".to_owned(),
        }],
        algorithm_version: AlgorithmVersion::new("body-fat.median@1"),
        mapping_versions: vec!["hevy@1.0.0".to_owned()],
    }
}

#[test]
fn body_fat_analytics_preserves_optional_dates_without_using_weight() {
    let result = body_fat_analytics(
        &context(),
        &[
            measurement("2026-01-01", Some(18.0), 1),
            measurement("2026-01-01", Some(20.0), 2),
            measurement("2026-01-02", None, 3),
            measurement("2026-01-03", Some(17.5), 4),
        ],
    );

    assert_eq!(
        result
            .observations
            .iter()
            .map(|point| point.value)
            .collect::<Vec<_>>(),
        vec![Some(19.0), None, Some(17.5)]
    );
    assert_eq!(result.provenance.coverage.observed_days, 2);
    assert_eq!(
        result.provenance.snapshot_refs[0].snapshot_id,
        "snapshot-body-fat"
    );
}

#[test]
fn body_fat_analytics_ignores_nonfinite_optional_values() {
    let result = body_fat_analytics(&context(), &[measurement("2026-01-01", Some(f64::NAN), 1)]);
    assert!(
        result
            .observations
            .iter()
            .all(|point| point.value.is_none())
    );
    assert_eq!(result.provenance.coverage.observed_days, 0);
}
