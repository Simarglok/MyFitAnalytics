use mfa_contracts::{AvailabilityState, DashboardBlock, DashboardInput};

use crate::{
    availability_message_key, card, chart, has_capability, page_availability_state, status,
    value_or_missing,
};

pub fn compose(input: &DashboardInput) -> Vec<DashboardBlock> {
    let available = has_capability(input, "body.weight");
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
            "body.raw_weight",
            "base.body.raw_weight",
            value_or_missing(input, "body.weight", "base.body.missing"),
        ),
        card(
            "body.daily_median",
            "base.body.daily_median",
            value_or_missing(input, "body.weight", "base.body.missing"),
        ),
        card(
            "body.trailing_mean",
            "base.body.trailing_mean",
            value_or_missing(input, "body.weight", "base.body.missing"),
        ),
        card(
            "body.phase_overlay",
            "base.body.phase_overlay",
            value_or_missing(input, "body.weight", "base.body.missing"),
        ),
        chart(
            "body.weight.trend",
            "line",
            "base.body.trend",
            input,
            "body.weight",
        ),
        chart(
            "body.fat.trend",
            "line",
            "base.body.fat_trend",
            input,
            "body.fat_percentage",
        ),
        status(
            "body.status",
            availability_state.clone(),
            availability_message_key(&availability_state, "base.body.ready", "base.body.missing"),
        ),
    ]
}
