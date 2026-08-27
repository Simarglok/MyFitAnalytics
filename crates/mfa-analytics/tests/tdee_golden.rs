use mfa_analytics::{
    AlgorithmVersion, DateRange, MetricContext, NutritionDay, NutritionQuality, SnapshotRef,
    TdeeResult, WeightPoint, rolling_tdee, rolling_tdee_with_context,
};
use mfa_contracts::{LocalDate, PhaseEvent};
use std::str::FromStr;
use uuid::Uuid;

fn date(value: &str) -> LocalDate {
    LocalDate::from_str(value).unwrap()
}

fn window() -> DateRange {
    DateRange::inclusive(date("2026-01-01"), date("2026-01-28"))
}

#[test]
fn trailing_tdee_window_contains_exactly_28_local_dates() {
    let display_range = DateRange::inclusive(date("2026-01-01"), date("2026-01-31"));
    let tdee_range = display_range
        .trailing_ending(display_range.end, 28)
        .expect("positive trailing window");

    assert_eq!(tdee_range.start, date("2026-01-04"));
    assert_eq!(tdee_range.end, date("2026-01-31"));
    assert_eq!(tdee_range.len_days(), 28);
}

fn nutrition_days(complete_days: usize) -> Vec<NutritionDay> {
    let complete_days = complete_days as u64;
    (0..28)
        .map(|offset| NutritionDay {
            local_date: date("2026-01-01")
                .0
                .checked_add_days(chrono::Days::new(offset))
                .unwrap()
                .into(),
            calories_kcal: (offset < complete_days).then_some(2_500.0),
            protein_g: (offset < complete_days).then_some(150.0),
            fat_g: Some(80.0),
            carbs_g: Some(250.0),
            fiber_g: Some(30.0),
            logged_item_count: if offset < complete_days { 1 } else { 0 },
            quality: if offset < complete_days {
                NutritionQuality::Complete
            } else {
                NutritionQuality::Missing
            },
        })
        .collect()
}

fn weights(offsets: &[i64], values: &[f64]) -> Vec<WeightPoint> {
    offsets
        .iter()
        .zip(values.iter())
        .map(|(&offset, &value_kg)| WeightPoint {
            local_date: date("2026-01-01")
                .0
                .checked_add_days(chrono::Days::new(offset as u64))
                .unwrap()
                .into(),
            value_kg,
        })
        .collect()
}

fn phase(start: &str, end: &str, exclude_from_tdee: bool) -> PhaseEvent {
    PhaseEvent {
        phase_event_id: Uuid::from_u128(1),
        event_type: "cut".to_owned(),
        start_date: date(start),
        end_date: date(end),
        description: Some("synthetic phase".to_owned()),
        exclude_from_tdee,
    }
}

fn assert_insufficient(result: TdeeResult, check: impl FnOnce(&mfa_analytics::TdeeCoverage)) {
    match result {
        TdeeResult::InsufficientCoverage(coverage) => check(&coverage),
        TdeeResult::Ready(_) => panic!("expected insufficient coverage"),
    }
}

#[test]
fn coverage_rejects_fewer_than_21_complete_nutrition_days() {
    let result = rolling_tdee(
        window(),
        &nutrition_days(20),
        &weights(&[0, 3, 6, 9, 12, 15, 20, 27], &[80.0; 8]),
        &[],
    );
    assert_insufficient(result, |coverage| {
        assert_eq!(coverage.complete_nutrition_days, 20);
    });
}

#[test]
fn coverage_rejects_fewer_than_8_weight_dates() {
    let result = rolling_tdee(
        window(),
        &nutrition_days(21),
        &weights(&[0, 3, 6, 9, 12, 15, 27], &[80.0; 7]),
        &[],
    );
    assert_insufficient(result, |coverage| {
        assert_eq!(coverage.weight_days, 7);
    });
}

#[test]
fn coverage_requires_weight_in_the_first_seven_calendar_days() {
    let result = rolling_tdee(
        window(),
        &nutrition_days(21),
        &weights(&[7, 11, 15, 19, 21, 23, 25, 27], &[80.0; 8]),
        &[],
    );
    assert_insufficient(result, |coverage| {
        assert_eq!(coverage.first_7d_weight_days, 0);
        assert!(coverage.last_7d_weight_days > 0);
    });
}

#[test]
fn coverage_requires_weight_in_the_last_seven_calendar_days() {
    let result = rolling_tdee(
        window(),
        &nutrition_days(21),
        &weights(&[0, 2, 4, 6, 8, 10, 12, 14], &[80.0; 8]),
        &[],
    );
    assert_insufficient(result, |coverage| {
        assert_eq!(coverage.last_7d_weight_days, 0);
        assert!(coverage.first_7d_weight_days > 0);
    });
}

#[test]
fn coverage_counts_excluded_phase_dates_against_complete_days() {
    let result = rolling_tdee(
        window(),
        &nutrition_days(21),
        &weights(&[0, 3, 6, 9, 12, 15, 20, 27], &[80.0; 8]),
        &[phase("2026-01-21", "2026-01-21", true)],
    );
    assert_insufficient(result, |coverage| {
        assert_eq!(coverage.complete_nutrition_days, 20);
        assert_eq!(coverage.excluded_days, 1);
    });
}

#[test]
fn coverage_reports_unavailable_slope_without_returning_numeric_tdee() {
    let result = rolling_tdee(
        window(),
        &nutrition_days(21),
        &weights(&[0, 3, 6, 9, 12, 15, 20, 27], &[f64::NAN; 8]),
        &[],
    );
    assert_insufficient(result, |coverage| {
        assert_eq!(coverage.weight_days, 0);
        assert!(!coverage.slope_available);
    });
}

#[test]
fn non_finite_weights_do_not_satisfy_weight_coverage_boundaries() {
    let result = rolling_tdee(
        window(),
        &nutrition_days(21),
        &weights(
            &[0, 3, 6, 9, 12, 15, 20, 24, 27],
            &[
                80.0,
                80.0,
                80.0,
                80.0,
                80.0,
                80.0,
                80.0,
                f64::NAN,
                f64::INFINITY,
            ],
        ),
        &[],
    );
    assert_insufficient(result, |coverage| {
        assert_eq!(coverage.weight_days, 7);
        assert_eq!(coverage.first_7d_weight_days, 3);
        assert_eq!(coverage.last_7d_weight_days, 0);
    });
}

#[test]
fn estimates_follow_the_weight_change_sign_and_interval_formula() {
    for (slope_per_day, expected_tdee) in [(-0.1, 3_270.0), (0.0, 2_500.0), (0.1, 1_730.0)] {
        let values = [0, 3, 6, 9, 12, 15, 20, 27]
            .iter()
            .map(|offset| 80.0 + slope_per_day * *offset as f64)
            .collect::<Vec<_>>();
        let result = rolling_tdee(
            window(),
            &nutrition_days(28),
            &weights(&[0, 3, 6, 9, 12, 15, 20, 27], &values),
            &[],
        );
        let TdeeResult::Ready(estimate) = result else {
            panic!("expected a TDEE estimate")
        };
        assert!((estimate.average_intake - 2_500.0).abs() < 1e-9);
        assert!((estimate.kcal_per_day - expected_tdee).abs() < 1e-9);
        assert!((estimate.low - expected_tdee).abs() < 1e-9);
        assert!((estimate.high - expected_tdee).abs() < 1e-9);
    }
}

#[test]
fn ready_tdee_estimate_carries_derived_provenance() {
    let result = rolling_tdee(
        window(),
        &nutrition_days(28),
        &weights(&[0, 3, 6, 9, 12, 15, 20, 27], &[80.0; 8]),
        &[],
    );
    let TdeeResult::Ready(estimate) = result else {
        panic!("expected a TDEE estimate")
    };
    assert_eq!(estimate.provenance.requested, window());
    assert_eq!(estimate.provenance.snapshot_refs.len(), 0);
}

#[test]
fn contextual_tdee_preserves_snapshot_and_mapping_provenance() {
    let context = MetricContext {
        requested: window(),
        as_of: date("2026-01-28"),
        snapshot_refs: vec![SnapshotRef {
            logical_snapshot_key: "mynetdiary:active".to_owned(),
            snapshot_id: "snapshot-1".to_owned(),
        }],
        algorithm_version: AlgorithmVersion::new("base-analytics-v1"),
        mapping_versions: vec!["mynetdiary@1.0.0".to_owned()],
    };
    let result = rolling_tdee_with_context(
        &context,
        &nutrition_days(21),
        &weights(&[0, 3, 6, 9, 12, 15, 20, 24, 27], &[80.0; 9]),
        &[],
    );

    let TdeeResult::Ready(estimate) = result else {
        panic!("expected ready TDEE");
    };
    assert_eq!(
        estimate.provenance.snapshot_refs[0].snapshot_id,
        "snapshot-1"
    );
    assert_eq!(
        estimate.provenance.mapping_versions,
        vec!["mynetdiary@1.0.0"]
    );
}
