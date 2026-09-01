use crate::provenance::{DerivedProvenance, MetricContext};
use crate::weight::NullablePoint;
use mfa_contracts::{ActivityDay, ActivityEvent, HeartRateObservation, LocalDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivitySummary {
    pub local_date: LocalDate,
    pub accepted_event_count: u32,
    pub duration_seconds: Option<u64>,
    pub distance_km: Option<f64>,
    pub estimated_calories_kcal: Option<f64>,
    pub unknown_event_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityAnalytics {
    pub steps: Vec<NullablePoint>,
    pub mean_steps_7d: Vec<NullablePoint>,
    pub mean_steps_28d: Vec<NullablePoint>,
    pub events: Vec<ActivitySummary>,
    pub heart_rate: Vec<NullablePoint>,
    pub water: Vec<NullablePoint>,
    pub provenance: DerivedProvenance,
}

pub fn activity_analytics(
    context: &MetricContext,
    activity_days: &[ActivityDay],
    activity_events: &[ActivityEvent],
    heart_rate_observations: &[HeartRateObservation],
) -> ActivityAnalytics {
    let steps_by_date = first_present_by_date(activity_days.iter().filter_map(|day| {
        day.steps
            .filter(|value| *value <= f64::MAX as u64)
            .map(|value| (day.local_date, value as f64))
    }));
    let water_by_date = first_present_by_date(
        activity_days
            .iter()
            .filter_map(|day| day.water_ml.map(|value| (day.local_date, value))),
    );
    let heart_rate_by_date =
        mean_by_date(heart_rate_observations.iter().filter_map(|observation| {
            observation.heart_rate_bpm.is_finite().then_some((
                observation.observed_local_at.0.date().into(),
                observation.heart_rate_bpm,
            ))
        }));

    let steps = points_for_range(&context.requested, &steps_by_date);
    let water = points_for_range(&context.requested, &water_by_date);
    let heart_rate = points_for_range(&context.requested, &heart_rate_by_date);
    let mean_steps_7d = rolling_mean(&steps, 7);
    let mean_steps_28d = rolling_mean(&steps, 28);
    let events = context
        .requested
        .dates()
        .map(|local_date| summarize_events(local_date, activity_events))
        .collect();

    ActivityAnalytics {
        steps,
        mean_steps_7d,
        mean_steps_28d,
        events,
        heart_rate,
        water,
        provenance: context.provenance(steps_by_date.len().max(water_by_date.len())),
    }
}

fn first_present_by_date<I>(values: I) -> BTreeMap<LocalDate, f64>
where
    I: IntoIterator<Item = (LocalDate, f64)>,
{
    values
        .into_iter()
        .fold(BTreeMap::new(), |mut result, (date, value)| {
            if value.is_finite() {
                result.entry(date).or_insert(value);
            }
            result
        })
}

fn mean_by_date<I>(values: I) -> BTreeMap<LocalDate, f64>
where
    I: IntoIterator<Item = (LocalDate, f64)>,
{
    let mut grouped = BTreeMap::<LocalDate, Vec<f64>>::new();
    for (date, value) in values {
        if value.is_finite() {
            grouped.entry(date).or_default().push(value);
        }
    }
    grouped
        .into_iter()
        .map(|(date, values)| (date, values.iter().sum::<f64>() / values.len() as f64))
        .collect()
}

fn points_for_range(
    range: &crate::DateRange,
    values: &BTreeMap<LocalDate, f64>,
) -> Vec<NullablePoint> {
    range
        .dates()
        .map(|local_date| NullablePoint {
            local_date,
            value: values.get(&local_date).copied(),
        })
        .collect()
}

fn rolling_mean(points: &[NullablePoint], days: i64) -> Vec<NullablePoint> {
    points
        .iter()
        .map(|point| {
            let start = point
                .local_date
                .0
                .checked_sub_days(chrono::Days::new((days - 1) as u64))
                .expect("rolling window starts at a valid local date");
            let values = points
                .iter()
                .filter(|candidate| {
                    candidate.local_date.0 >= start && candidate.local_date <= point.local_date
                })
                .filter_map(|candidate| candidate.value)
                .collect::<Vec<_>>();
            NullablePoint {
                local_date: point.local_date,
                value: (!values.is_empty())
                    .then(|| values.iter().sum::<f64>() / values.len() as f64),
            }
        })
        .collect()
}

fn summarize_events(local_date: LocalDate, events: &[ActivityEvent]) -> ActivitySummary {
    let mut accepted_event_count = 0;
    let mut unknown_event_count = 0;
    let mut duration_seconds = Vec::new();
    let mut distance_km = Vec::new();
    let mut estimated_calories_kcal = Vec::new();
    for event in events.iter().filter(|event| event.local_date == local_date) {
        let is_unknown =
            event.quality_status == "unknown_mapping" || event.activity_type == "unknown";
        if is_unknown {
            unknown_event_count += 1;
            continue;
        }
        if event.quality_status != "accepted" {
            continue;
        }
        accepted_event_count += 1;
        if let Some(value) = event.duration_seconds {
            duration_seconds.push(u64::from(value));
        }
        if is_distance_allowed(&event.activity_type)
            && let Some(value) = event.distance_km.filter(|value| value.is_finite())
        {
            distance_km.push(value);
        }
        if let Some(value) = event
            .estimated_calories_kcal
            .filter(|value| value.is_finite())
        {
            estimated_calories_kcal.push(value);
        }
    }
    ActivitySummary {
        local_date,
        accepted_event_count,
        duration_seconds: sum_u64(&duration_seconds),
        distance_km: sum_f64(&distance_km),
        estimated_calories_kcal: sum_f64(&estimated_calories_kcal),
        unknown_event_count,
    }
}

fn is_distance_allowed(activity_type: &str) -> bool {
    matches!(
        activity_type,
        "walking" | "running" | "hiking" | "cycling" | "treadmill"
    )
}

fn sum_u64(values: &[u64]) -> Option<u64> {
    (!values.is_empty()).then(|| values.iter().sum())
}

fn sum_f64(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum())
}
