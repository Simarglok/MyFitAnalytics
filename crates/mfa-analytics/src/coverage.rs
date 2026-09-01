use crate::weight::TheilSenEstimate;
use crate::window::DateRange;
use mfa_contracts::LocalDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TdeeCoverage {
    pub complete_nutrition_days: u32,
    pub weight_days: u32,
    pub first_7d_weight_days: u32,
    pub last_7d_weight_days: u32,
    pub excluded_days: u32,
    pub slope_available: bool,
}

impl TdeeCoverage {
    pub fn meets_thresholds(&self, window: DateRange) -> bool {
        window.len_days() == 28
            && self.complete_nutrition_days >= 21
            && self.weight_days >= 8
            && self.first_7d_weight_days > 0
            && self.last_7d_weight_days > 0
            && self.slope_available
    }
}

pub(crate) fn build_coverage(
    window: DateRange,
    complete_nutrition_dates: &BTreeSet<LocalDate>,
    weight_dates: &BTreeSet<LocalDate>,
    excluded: &BTreeSet<LocalDate>,
    slope: Option<&TheilSenEstimate>,
) -> TdeeCoverage {
    let complete_nutrition_days = complete_nutrition_dates
        .iter()
        .filter(|date| window.contains(**date) && !excluded.contains(date))
        .count() as u32;
    let usable_weight_dates = weight_dates
        .iter()
        .filter(|date| window.contains(**date) && !excluded.contains(date))
        .copied()
        .collect::<BTreeSet<_>>();
    let first_window = DateRange::inclusive(
        window.start,
        LocalDate::from(
            window
                .start
                .0
                .checked_add_days(chrono::Days::new(6))
                .expect("first TDEE coverage window is valid"),
        ),
    );
    let last_window = DateRange::inclusive(
        LocalDate::from(
            window
                .end
                .0
                .checked_sub_days(chrono::Days::new(6))
                .expect("last TDEE coverage window is valid"),
        ),
        window.end,
    );
    TdeeCoverage {
        complete_nutrition_days,
        weight_days: usable_weight_dates.len() as u32,
        first_7d_weight_days: usable_weight_dates
            .iter()
            .filter(|date| first_window.contains(**date))
            .count() as u32,
        last_7d_weight_days: usable_weight_dates
            .iter()
            .filter(|date| last_window.contains(**date))
            .count() as u32,
        excluded_days: excluded.len() as u32,
        slope_available: slope.is_some(),
    }
}
