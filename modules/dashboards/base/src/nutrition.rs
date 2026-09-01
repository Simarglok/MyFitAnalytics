use mfa_contracts::{AvailabilityState, DashboardBlock, DashboardInput};

use crate::{
    availability_message_key, card, chart, has_capability, page_availability_state, status,
    value_or_missing, value_or_missing_field,
};

pub fn compose(input: &DashboardInput) -> Vec<DashboardBlock> {
    let available = has_capability(input, "nutrition.items");
    let availability_state = page_availability_state(
        input,
        if available {
            AvailabilityState::Ready
        } else {
            AvailabilityState::MissingCapability
        },
    );
    vec![
        card(
            "nutrition.calories",
            "base.nutrition.calories",
            value_or_missing(input, "nutrition.items", "base.nutrition.missing"),
        ),
        card(
            "nutrition.macros",
            "base.nutrition.macros",
            value_or_missing_field(input, "nutrition.items", "days", "base.nutrition.missing"),
        ),
        card(
            "nutrition.trailing_mean",
            "base.nutrition.trailing_mean",
            value_or_missing_field(
                input,
                "nutrition.items",
                "trailing7dMeanCalories",
                "base.nutrition.missing",
            ),
        ),
        card(
            "nutrition.tdee",
            "base.nutrition.tdee",
            value_or_missing_field(input, "nutrition.items", "tdee", "base.nutrition.missing"),
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
            availability_state.clone(),
            availability_message_key(
                &availability_state,
                "base.nutrition.ready",
                "base.nutrition.missing",
            ),
        ),
    ]
}
