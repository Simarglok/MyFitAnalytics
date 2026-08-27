use crate::provenance::{DerivedProvenance, MetricContext};
use mfa_contracts::LocalDate;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightObservation {
    pub observation_id: String,
    pub local_date: LocalDate,
    pub weight_kg: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightPoint {
    pub local_date: LocalDate,
    pub value_kg: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NullablePoint {
    pub local_date: LocalDate,
    pub value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TheilSenEstimate {
    pub slope_per_day: f64,
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub pair_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeightAnalytics {
    pub observations: Vec<WeightPoint>,
    pub daily_median: Vec<WeightPoint>,
    pub trailing_7d_mean: Vec<NullablePoint>,
    pub slope_28d: Option<TheilSenEstimate>,
    pub provenance: DerivedProvenance,
}

pub fn weight_analytics(
    context: &MetricContext,
    observations: &[WeightObservation],
) -> WeightAnalytics {
    let mut clipped = observations
        .iter()
        .filter(|observation| {
            context.requested.contains(observation.local_date) && observation.weight_kg.is_finite()
        })
        .cloned()
        .collect::<Vec<_>>();
    clipped.sort_by(|left, right| {
        left.local_date
            .cmp(&right.local_date)
            .then_with(|| left.observation_id.cmp(&right.observation_id))
    });

    let mut values_by_date: BTreeMap<LocalDate, Vec<f64>> = BTreeMap::new();
    for observation in &clipped {
        values_by_date
            .entry(observation.local_date)
            .or_default()
            .push(observation.weight_kg);
    }
    let daily_median = values_by_date
        .iter()
        .map(|(&local_date, values)| WeightPoint {
            local_date,
            value_kg: median(values),
        })
        .collect::<Vec<_>>();

    let trailing_7d_mean = context
        .requested
        .dates()
        .map(|local_date| {
            let start = local_date
                .0
                .checked_sub_days(chrono::Days::new(6))
                .expect("six days before a valid local date");
            let values = daily_median
                .iter()
                .filter(|point| point.local_date.0 >= start && point.local_date <= local_date)
                .map(|point| point.value_kg)
                .collect::<Vec<_>>();
            NullablePoint {
                local_date,
                value: (!values.is_empty())
                    .then(|| values.iter().sum::<f64>() / values.len() as f64),
            }
        })
        .collect::<Vec<_>>();

    let slope_28d = theil_sen(context.as_of, &daily_median);
    WeightAnalytics {
        observations: clipped
            .iter()
            .map(|observation| WeightPoint {
                local_date: observation.local_date,
                value_kg: observation.weight_kg,
            })
            .collect(),
        daily_median,
        trailing_7d_mean,
        slope_28d,
        provenance: context.provenance(values_by_date.len()),
    }
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}

fn theil_sen(as_of: LocalDate, daily_median: &[WeightPoint]) -> Option<TheilSenEstimate> {
    let window_start = as_of
        .0
        .checked_sub_days(chrono::Days::new(27))
        .expect("27 days before a valid local date");
    let points = daily_median
        .iter()
        .filter(|point| point.local_date.0 >= window_start && point.local_date <= as_of)
        .collect::<Vec<_>>();
    let mut slopes = Vec::new();
    for (index, left) in points.iter().enumerate() {
        for right in points.iter().skip(index + 1) {
            let days = (right.local_date.0 - left.local_date.0).num_days();
            if days > 0 {
                slopes.push((right.value_kg - left.value_kg) / days as f64);
            }
        }
    }
    if slopes.is_empty() {
        return None;
    }
    slopes.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    Some(TheilSenEstimate {
        slope_per_day: median(&slopes),
        lower_bound: *slopes.first().expect("non-empty slopes"),
        upper_bound: *slopes.last().expect("non-empty slopes"),
        pair_count: slopes.len() as u32,
    })
}
