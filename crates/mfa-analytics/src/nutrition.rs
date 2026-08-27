use crate::provenance::{DerivedProvenance, MetricContext};
use crate::weight::NullablePoint;
use mfa_contracts::{LocalDate, NutritionItem};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NutritionQuality {
    Complete,
    PartialFields,
    Missing,
    ExcludedByUser,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NutritionDay {
    pub local_date: LocalDate,
    pub calories_kcal: Option<f64>,
    pub protein_g: Option<f64>,
    pub fat_g: Option<f64>,
    pub carbs_g: Option<f64>,
    pub fiber_g: Option<f64>,
    pub logged_item_count: u32,
    pub quality: NutritionQuality,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NutritionAnalytics {
    pub days: Vec<NutritionDay>,
    pub trailing_7d_mean_calories: Vec<NullablePoint>,
    pub provenance: DerivedProvenance,
}

pub fn nutrition_analytics(
    context: &MetricContext,
    items: &[NutritionItem],
    excluded_dates: &BTreeSet<LocalDate>,
) -> NutritionAnalytics {
    let grouped = items
        .iter()
        .filter(|item| context.requested.contains(item.local_date))
        .fold(
            BTreeMap::<LocalDate, Vec<&NutritionItem>>::new(),
            |mut grouped, item| {
                grouped.entry(item.local_date).or_default().push(item);
                grouped
            },
        );

    let days = context
        .requested
        .dates()
        .map(|local_date| {
            let items = grouped.get(&local_date).map(Vec::as_slice).unwrap_or(&[]);
            let quality = if excluded_dates.contains(&local_date) {
                NutritionQuality::ExcludedByUser
            } else if items.is_empty() {
                NutritionQuality::Missing
            } else if items.iter().all(|item| item.calories_kcal.is_some()) {
                NutritionQuality::Complete
            } else {
                NutritionQuality::PartialFields
            };
            NutritionDay {
                local_date,
                calories_kcal: complete_sum(items.iter().map(|item| item.calories_kcal)),
                protein_g: complete_sum(items.iter().map(|item| item.protein_g)),
                fat_g: complete_sum(items.iter().map(|item| item.fat_g)),
                carbs_g: complete_sum(items.iter().map(|item| item.carbs_g)),
                fiber_g: complete_sum(items.iter().map(|item| item.fiber_g)),
                logged_item_count: items.len() as u32,
                quality,
            }
        })
        .collect::<Vec<_>>();

    let trailing_7d_mean_calories = days
        .iter()
        .map(|day| {
            let start = day
                .local_date
                .0
                .checked_sub_days(chrono::Days::new(6))
                .expect("six days before a valid local date");
            let values = days
                .iter()
                .filter(|candidate| {
                    candidate.local_date.0 >= start
                        && candidate.local_date <= day.local_date
                        && candidate.quality == NutritionQuality::Complete
                })
                .filter_map(|candidate| candidate.calories_kcal)
                .collect::<Vec<_>>();
            NullablePoint {
                local_date: day.local_date,
                value: (!values.is_empty())
                    .then(|| values.iter().sum::<f64>() / values.len() as f64),
            }
        })
        .collect();

    NutritionAnalytics {
        days,
        trailing_7d_mean_calories,
        provenance: context.provenance(grouped.len()),
    }
}

fn complete_sum(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    let values = values.collect::<Option<Vec<_>>>()?;
    (!values.is_empty()).then(|| values.into_iter().sum())
}
