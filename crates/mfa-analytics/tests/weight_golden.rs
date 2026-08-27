use mfa_analytics::{
    AlgorithmVersion, DateRange, MetricContext, SnapshotRef, WeightObservation, weight_analytics,
};
use mfa_contracts::LocalDate;
use std::str::FromStr;

fn date(value: &str) -> LocalDate {
    LocalDate::from_str(value).unwrap()
}

fn context(start: &str, end: &str) -> MetricContext {
    MetricContext {
        requested: DateRange::inclusive(date(start), date(end)),
        as_of: date(end),
        snapshot_refs: vec![SnapshotRef {
            logical_snapshot_key: "hevy:measurements:2026".to_owned(),
            snapshot_id: "snapshot-1".to_owned(),
        }],
        algorithm_version: AlgorithmVersion::new("weight.theil_sen@1"),
        mapping_versions: Vec::new(),
    }
}

#[test]
fn weight_analytics_uses_daily_medians_calendar_windows_and_exact_dates() {
    let observations = vec![
        WeightObservation {
            observation_id: "z-outlier".to_owned(),
            local_date: date("2026-01-01"),
            weight_kg: 40.0,
        },
        WeightObservation {
            observation_id: "b".to_owned(),
            local_date: date("2026-01-03"),
            weight_kg: 82.0,
        },
        WeightObservation {
            observation_id: "a".to_owned(),
            local_date: date("2026-01-01"),
            weight_kg: 80.0,
        },
        WeightObservation {
            observation_id: "outside".to_owned(),
            local_date: date("2026-01-11"),
            weight_kg: 1.0,
        },
        WeightObservation {
            observation_id: "c".to_owned(),
            local_date: date("2026-01-10"),
            weight_kg: 81.0,
        },
    ];

    let result = weight_analytics(&context("2026-01-01", "2026-01-10"), &observations);

    assert_eq!(
        result
            .daily_median
            .iter()
            .map(|point| (point.local_date.to_string(), point.value_kg))
            .collect::<Vec<_>>(),
        vec![
            ("2026-01-01".to_owned(), 60.0),
            ("2026-01-03".to_owned(), 82.0),
            ("2026-01-10".to_owned(), 81.0),
        ]
    );
    assert_eq!(result.trailing_7d_mean.len(), 10);
    assert_eq!(result.trailing_7d_mean[0].value, Some(60.0));
    assert_eq!(result.trailing_7d_mean[1].value, Some(60.0));
    assert_eq!(result.trailing_7d_mean[2].value, Some(71.0));
    assert_eq!(result.trailing_7d_mean[8].value, Some(82.0));
    assert_eq!(result.trailing_7d_mean[9].value, Some(81.0));

    let slope = result
        .slope_28d
        .expect("three observed dates provide a slope");
    assert!((slope.slope_per_day - (21.0 / 9.0)).abs() < 1e-12);
    assert!((slope.lower_bound - (-1.0 / 7.0)).abs() < 1e-12);
    assert!((slope.upper_bound - 11.0).abs() < 1e-12);
    assert_eq!(slope.pair_count, 3);
    assert_eq!(
        result.provenance.requested,
        context("2026-01-01", "2026-01-10").requested
    );
    assert_eq!(result.provenance.snapshot_refs[0].snapshot_id, "snapshot-1");
}

#[test]
fn weight_analytics_keeps_empty_calendar_windows_as_null_and_requires_two_dates() {
    let result = weight_analytics(
        &context("2026-01-01", "2026-01-20"),
        &[WeightObservation {
            observation_id: "only".to_owned(),
            local_date: date("2026-01-03"),
            weight_kg: 80.0,
        }],
    );

    assert_eq!(result.slope_28d, None);
    assert_eq!(result.trailing_7d_mean[2].value, Some(80.0));
    assert_eq!(result.trailing_7d_mean[8].value, Some(80.0));
    assert_eq!(result.trailing_7d_mean[9].value, None);
    assert_eq!(result.trailing_7d_mean[19].value, None);
}
