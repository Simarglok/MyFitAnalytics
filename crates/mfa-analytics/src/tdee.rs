use crate::coverage::{TdeeCoverage, build_coverage};
use crate::phase::excluded_dates;
use crate::provenance::{AlgorithmVersion, DerivedProvenance, MetricContext};
use crate::weight::{TheilSenEstimate, WeightPoint};
use crate::window::DateRange;
use mfa_contracts::{LocalDate, PhaseEvent};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::nutrition::{NutritionDay, NutritionQuality};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TdeeEstimate {
    pub kcal_per_day: f64,
    pub low: f64,
    pub high: f64,
    pub average_intake: f64,
    pub slope: TheilSenEstimate,
    pub coverage: TdeeCoverage,
    pub window: DateRange,
    pub provenance: DerivedProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TdeeResult {
    Ready(TdeeEstimate),
    InsufficientCoverage(TdeeCoverage),
}

pub fn rolling_tdee(
    window: DateRange,
    nutrition: &[NutritionDay],
    weights: &[WeightPoint],
    phases: &[PhaseEvent],
) -> TdeeResult {
    let context = MetricContext {
        requested: window,
        as_of: window.end,
        snapshot_refs: Vec::new(),
        algorithm_version: AlgorithmVersion::new("tdee.rolling@1"),
        mapping_versions: Vec::new(),
    };
    rolling_tdee_with_context(&context, nutrition, weights, phases)
}

pub fn rolling_tdee_with_context(
    context: &MetricContext,
    nutrition: &[NutritionDay],
    weights: &[WeightPoint],
    phases: &[PhaseEvent],
) -> TdeeResult {
    let window = context.requested;
    let excluded = excluded_dates(window, phases);
    let complete_intake = nutrition
        .iter()
        .filter(|day| {
            window.contains(day.local_date)
                && day.quality == NutritionQuality::Complete
                && day.calories_kcal.is_some_and(f64::is_finite)
        })
        .fold(BTreeMap::<LocalDate, f64>::new(), |mut days, day| {
            days.entry(day.local_date)
                .or_insert(day.calories_kcal.expect("complete day has calories"));
            days
        });
    let weight_dates = weights
        .iter()
        .filter(|point| window.contains(point.local_date) && point.value_kg.is_finite())
        .map(|point| point.local_date)
        .collect::<BTreeSet<_>>();
    let weight_values = weights
        .iter()
        .filter(|point| {
            window.contains(point.local_date)
                && !excluded.contains(&point.local_date)
                && point.value_kg.is_finite()
        })
        .fold(
            BTreeMap::<LocalDate, Vec<f64>>::new(),
            |mut values, point| {
                values
                    .entry(point.local_date)
                    .or_default()
                    .push(point.value_kg);
                values
            },
        );
    let weight_points = weight_values
        .iter()
        .map(|(&local_date, values)| WeightPoint {
            local_date,
            value_kg: median(values),
        })
        .collect::<Vec<_>>();
    let slope = theil_sen(window, &weight_points);
    let coverage = build_coverage(
        window,
        &complete_intake.keys().copied().collect(),
        &weight_dates,
        &excluded,
        slope.as_ref(),
    );
    if !coverage.meets_thresholds(window) {
        return TdeeResult::InsufficientCoverage(coverage);
    }

    let average_intake = complete_intake
        .iter()
        .filter(|(date, _)| !excluded.contains(date))
        .map(|(_, calories)| *calories)
        .sum::<f64>()
        / coverage.complete_nutrition_days as f64;
    let slope = slope.expect("coverage requires an available slope");
    let mut provenance = context.provenance(coverage.complete_nutrition_days as usize);
    provenance.algorithm_version = AlgorithmVersion::new("tdee.rolling@1");
    TdeeResult::Ready(TdeeEstimate {
        kcal_per_day: average_intake - 7_700.0 * slope.slope_per_day,
        low: average_intake - 7_700.0 * slope.upper_bound,
        high: average_intake - 7_700.0 * slope.lower_bound,
        average_intake,
        slope,
        coverage,
        window,
        provenance,
    })
}

fn theil_sen(window: DateRange, points: &[WeightPoint]) -> Option<TheilSenEstimate> {
    let mut slopes = Vec::new();
    for (index, left) in points.iter().enumerate() {
        for right in points.iter().skip(index + 1) {
            let days = (right.local_date.0 - left.local_date.0).num_days();
            if days > 0 && window.contains(left.local_date) && window.contains(right.local_date) {
                slopes.push((right.value_kg - left.value_kg) / days as f64);
            }
        }
    }
    if slopes.is_empty() {
        return None;
    }
    slopes.sort_by(f64::total_cmp);
    let middle = slopes.len() / 2;
    let slope_per_day = if slopes.len().is_multiple_of(2) {
        (slopes[middle - 1] + slopes[middle]) / 2.0
    } else {
        slopes[middle]
    };
    Some(TheilSenEstimate {
        slope_per_day,
        lower_bound: slopes[0],
        upper_bound: *slopes.last().expect("non-empty slopes"),
        pair_count: slopes.len() as u32,
    })
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}
