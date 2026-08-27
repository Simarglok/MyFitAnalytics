use mfa_analytics::{
    ActivityAnalytics, AlgorithmVersion, DateRange, MetricContext, SnapshotRef, activity_analytics,
};
use mfa_contracts::{ActivityDay, ActivityEvent, HeartRateObservation, LocalDate, LocalDateTime};
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
        requested: DateRange::inclusive(date("2026-01-01"), date("2026-01-08")),
        as_of: date("2026-01-08"),
        snapshot_refs: vec![SnapshotRef {
            logical_snapshot_key: "mynetdiary:2026".to_owned(),
            snapshot_id: "snapshot-activity".to_owned(),
        }],
        algorithm_version: AlgorithmVersion::new("activity.aggregate@1"),
    }
}

fn day(local_date: &str, steps: Option<u64>, water_ml: Option<f64>) -> ActivityDay {
    ActivityDay {
        local_date: date(local_date),
        steps,
        water_ml,
        heart_rate_observation_count: 0,
        activity_duration_seconds: 0,
        activity_distance_km: 0.0,
        estimated_activity_calories_kcal: 0.0,
    }
}

fn event(
    id: u128,
    local_date: &str,
    activity_type: &str,
    duration_seconds: Option<u32>,
    distance_km: Option<f64>,
    calories: Option<f64>,
    quality_status: &str,
) -> ActivityEvent {
    ActivityEvent {
        activity_event_id: Uuid::from_u128(id),
        occurred_local_at: datetime(&format!("{local_date}T08:00:00")),
        local_date: date(local_date),
        activity_type: activity_type.to_owned(),
        source_name: activity_type.to_owned(),
        duration_seconds,
        distance_km,
        estimated_calories_kcal: calories,
        origin_hint: None,
        quality_status: quality_status.to_owned(),
        source_record_id: Some(format!("record-{id}")),
    }
}

#[test]
fn activity_analytics_keeps_series_separate_and_excludes_unknown_mappings() {
    let result: ActivityAnalytics = activity_analytics(
        &context(),
        &[
            day("2026-01-01", Some(1000), None),
            day("2026-01-02", None, Some(500.0)),
            day("2026-01-03", Some(3000), None),
            day("2026-01-08", Some(7000), None),
        ],
        &[
            event(
                1,
                "2026-01-01",
                "walking",
                Some(1800),
                Some(2.0),
                Some(100.0),
                "accepted",
            ),
            event(
                2,
                "2026-01-02",
                "stretching",
                Some(600),
                Some(1.0),
                None,
                "accepted",
            ),
            event(
                3,
                "2026-01-03",
                "unknown",
                Some(999),
                Some(9.0),
                Some(900.0),
                "unknown_mapping",
            ),
            event(
                4,
                "2026-01-04",
                "cycling",
                None,
                Some(5.0),
                Some(250.0),
                "accepted",
            ),
            event(
                5,
                "2026-01-09",
                "walking",
                Some(10),
                Some(1.0),
                Some(10.0),
                "accepted",
            ),
        ],
        &[
            HeartRateObservation {
                heart_rate_observation_id: Uuid::from_u128(20),
                observed_local_at: datetime("2026-01-01T08:00:00"),
                heart_rate_bpm: 120.0,
                source_record_id: Some("hr-20".to_owned()),
            },
            HeartRateObservation {
                heart_rate_observation_id: Uuid::from_u128(21),
                observed_local_at: datetime("2026-01-03T08:00:00"),
                heart_rate_bpm: 130.0,
                source_record_id: Some("hr-21".to_owned()),
            },
            HeartRateObservation {
                heart_rate_observation_id: Uuid::from_u128(22),
                observed_local_at: datetime("2026-01-03T09:00:00"),
                heart_rate_bpm: 140.0,
                source_record_id: Some("hr-22".to_owned()),
            },
        ],
    );

    assert_eq!(result.steps.len(), 8);
    assert_eq!(result.steps[0].value, Some(1000.0));
    assert_eq!(result.steps[1].value, None);
    assert_eq!(result.steps[7].value, Some(7000.0));
    assert_eq!(result.mean_steps_7d[7].value, Some(5000.0));
    assert_eq!(result.mean_steps_28d[7].value, Some(11_000.0 / 3.0));

    assert_eq!(result.events[0].accepted_event_count, 1);
    assert_eq!(result.events[0].duration_seconds, Some(1800));
    assert_eq!(result.events[0].distance_km, Some(2.0));
    assert_eq!(result.events[0].estimated_calories_kcal, Some(100.0));
    assert_eq!(result.events[1].duration_seconds, Some(600));
    assert_eq!(result.events[1].distance_km, None);
    assert_eq!(result.events[2].accepted_event_count, 0);
    assert_eq!(result.events[2].unknown_event_count, 1);
    assert_eq!(result.events[3].duration_seconds, None);
    assert_eq!(result.events[3].distance_km, Some(5.0));
    assert_eq!(result.events[3].estimated_calories_kcal, Some(250.0));
    assert!(
        result
            .events
            .iter()
            .all(|summary| summary.local_date <= date("2026-01-08"))
    );

    assert_eq!(result.water[1].value, Some(500.0));
    assert_eq!(result.water[0].value, None);
    assert_eq!(result.heart_rate[0].value, Some(120.0));
    assert_eq!(result.heart_rate[2].value, Some(135.0));
    assert_eq!(
        result.provenance.snapshot_refs[0].snapshot_id,
        "snapshot-activity"
    );
}
