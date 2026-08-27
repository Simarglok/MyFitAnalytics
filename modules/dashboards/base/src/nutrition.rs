use mfa_contracts::{AvailabilityState, DashboardBlock, DashboardInput};

use crate::{card, chart, has_capability, status, value_or_missing};

pub fn compose(input: &DashboardInput) -> Vec<DashboardBlock> {
    let available = has_capability(input, "nutrition.items");
    vec![
        card(
            "nutrition.calories",
            "base.nutrition.calories",
            value_or_missing(input, "nutrition.items", "base.nutrition.missing"),
        ),
        card(
            "nutrition.macros",
            "base.nutrition.macros",
            value_or_missing(input, "nutrition.items", "base.nutrition.missing"),
        ),
        card(
            "nutrition.trailing_mean",
            "base.nutrition.trailing_mean",
            value_or_missing(input, "nutrition.items", "base.nutrition.missing"),
        ),
        card(
            "nutrition.tdee",
            "base.nutrition.tdee",
            value_or_missing(input, "nutrition.items", "base.nutrition.missing"),
        ),
        chart(
            "nutrition.calorie.trend",
            "line",
            "base.nutrition.trailing_mean",
            input,
            "nutrition.items",
        ),
        status(
            "nutrition.status",
            if available {
                AvailabilityState::Ready
            } else {
                AvailabilityState::MissingCapability
            },
            if available {
                "base.nutrition.ready"
            } else {
                "base.nutrition.missing"
            },
        ),
    ]
}
