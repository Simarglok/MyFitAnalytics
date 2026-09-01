use crate::provenance::{DerivedProvenance, MetricContext};
use crate::weight::NullablePoint;
use mfa_contracts::{BodyMeasurement, LocalDate};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BodyFatAnalytics {
    pub observations: Vec<NullablePoint>,
    pub provenance: DerivedProvenance,
}

pub fn body_fat_analytics(
    context: &MetricContext,
    measurements: &[BodyMeasurement],
) -> BodyFatAnalytics {
    let mut values_by_date = BTreeMap::<LocalDate, Vec<f64>>::new();
    for measurement in measurements {
        if !context.requested.contains(measurement.local_date) {
            continue;
        }
        if let Some(value) = measurement.body_fat_pct.filter(|value| value.is_finite()) {
            values_by_date
                .entry(measurement.local_date)
                .or_default()
                .push(value);
        }
    }
    let observations = context
        .requested
        .dates()
        .map(|local_date| NullablePoint {
            local_date,
            value: values_by_date.get(&local_date).map(|values| median(values)),
        })
        .collect();
    BodyFatAnalytics {
        observations,
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
