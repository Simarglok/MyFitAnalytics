use mfa_analytics::{
    AlgorithmVersion, DateRange, MetricContext, SnapshotRef, StrengthAnalytics, strength_analytics,
};
use mfa_contracts::{ExerciseSet, LocalDate, LocalDateTime, WorkoutSession};
use std::str::FromStr;
use uuid::Uuid;

fn date(value: &str) -> LocalDate {
    LocalDate::from_str(value).unwrap()
}

fn datetime(value: &str) -> LocalDateTime {
    LocalDateTime::from_str(value).unwrap()
}

fn context() -> MetricContext {
    MetricContext {
        requested: DateRange::inclusive(date("2026-01-01"), date("2026-01-28")),
        as_of: date("2026-01-28"),
        snapshot_refs: vec![SnapshotRef {
            logical_snapshot_key: "hevy:workouts:2026".to_owned(),
            snapshot_id: "snapshot-strength".to_owned(),
        }],
        algorithm_version: AlgorithmVersion::new("strength.epley@1"),
    }
}

fn session(id: u128, local_date: &str, duration_seconds: u32) -> WorkoutSession {
    let start = format!("{local_date}T08:00:00");
    WorkoutSession {
        workout_session_id: Uuid::from_u128(id),
        title: format!("Session {id}"),
        started_local_at: datetime(&start),
        ended_local_at: datetime(&format!("{local_date}T09:00:00")),
        duration_seconds: Some(duration_seconds),
        source_record_group_key: format!("group-{id}"),
    }
}

fn set(
    id: u128,
    session_id: u128,
    exercise_key: &str,
    set_type: &str,
    load_type: &str,
    weight_kg: Option<f64>,
    reps: Option<u32>,
) -> ExerciseSet {
    ExerciseSet {
        exercise_set_id: Uuid::from_u128(id),
        workout_session_id: Uuid::from_u128(session_id),
        exercise_title_raw: exercise_key.to_owned(),
        exercise_key: exercise_key.to_owned(),
        exercise_block_ordinal: 1,
        set_index: id as u32,
        set_type: set_type.to_owned(),
        load_type: load_type.to_owned(),
        weight_kg,
        reps,
        duration_seconds: None,
        rpe: None,
        source_record_id: Some(format!("record-{id}")),
    }
}

#[test]
fn strength_analytics_counts_calendar_windows_and_filters_epley_inputs() {
    let result: StrengthAnalytics = strength_analytics(
        &context(),
        &[
            session(1, "2026-01-01", 3600),
            session(2, "2026-01-15", 2700),
            session(3, "2026-01-22", 1800),
            session(4, "2026-01-28", 2400),
            session(5, "2025-12-31", 1000),
        ],
        &[
            set(
                101,
                1,
                "bench press",
                "normal",
                "external",
                Some(80.0),
                Some(4),
            ),
            set(
                102,
                2,
                "bench press",
                "normal",
                "external",
                Some(90.0),
                Some(4),
            ),
            set(
                103,
                3,
                "bench press",
                "warmup",
                "external",
                Some(120.0),
                Some(5),
            ),
            set(
                104,
                3,
                "bench press",
                "normal",
                "external",
                Some(100.0),
                Some(5),
            ),
            set(105, 3, "push-up", "failure", "bodyweight", None, Some(10)),
            set(106, 3, "mystery", "normal", "unknown", Some(200.0), Some(5)),
            set(
                107,
                4,
                "bench press",
                "failure",
                "external",
                Some(105.0),
                Some(5),
            ),
            set(
                108,
                4,
                "bench press",
                "normal",
                "external",
                Some(105.0),
                Some(13),
            ),
            set(
                109,
                4,
                "bench press",
                "normal",
                "assisted",
                Some(300.0),
                Some(5),
            ),
        ],
    );

    assert_eq!(result.session_counts.seven_day, 2);
    assert_eq!(result.session_counts.fourteen_day, 3);
    assert_eq!(result.session_counts.twenty_eight_day, 4);
    assert_eq!(result.session_durations.len(), 4);
    assert_eq!(result.session_durations[0].duration_seconds, Some(3600));
    assert_eq!(
        result
            .working_sets
            .iter()
            .find(|value| value.exercise_key == "bench press")
            .map(|value| value.count),
        Some(6)
    );
    assert_eq!(
        result
            .working_sets
            .iter()
            .find(|value| value.exercise_key == "push-up")
            .map(|value| value.count),
        Some(1)
    );
    assert!(
        result
            .working_sets
            .iter()
            .all(|value| value.exercise_key != "mystery")
    );

    assert_eq!(result.weekly_best_e1rm.len(), 4);
    assert_eq!(result.weekly_best_e1rm[0].week_start, date("2025-12-29"));
    assert_eq!(
        result.weekly_best_e1rm[0].value_kg,
        80.0 * (1.0 + 4.0 / 30.0)
    );
    assert_eq!(result.weekly_best_e1rm[1].week_start, date("2026-01-12"));
    assert_eq!(
        result.weekly_best_e1rm[1].value_kg,
        90.0 * (1.0 + 4.0 / 30.0)
    );
    assert_eq!(result.weekly_best_e1rm[2].week_start, date("2026-01-19"));
    assert_eq!(
        result.weekly_best_e1rm[2].value_kg,
        100.0 * (1.0 + 5.0 / 30.0)
    );
    assert_eq!(result.weekly_best_e1rm[3].week_start, date("2026-01-26"));
    assert_eq!(
        result.weekly_best_e1rm[3].value_kg,
        105.0 * (1.0 + 5.0 / 30.0)
    );
    assert_eq!(
        result.provenance.snapshot_refs[0].snapshot_id,
        "snapshot-strength"
    );
}
