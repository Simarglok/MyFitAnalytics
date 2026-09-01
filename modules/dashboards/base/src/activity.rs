use mfa_contracts::{AvailabilityState, DashboardBlock, DashboardInput};

use crate::{
    availability_message_key, card, chart, has_capability, page_availability_state, status,
    value_or_missing,
};

pub fn compose(input: &DashboardInput) -> Vec<DashboardBlock> {
    let available = has_capability(input, "activity.days");
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
            "activity.steps",
            "base.activity.steps",
            value_or_missing(input, "activity.days", "base.activity.missing"),
        ),
        card(
            "activity.events",
            "base.activity.events",
            value_or_missing(input, "activity.events", "base.activity.missing"),
        ),
        card(
            "activity.heart_rate",
            "base.activity.heart_rate",
            value_or_missing(input, "heart_rate.observations", "base.activity.missing"),
        ),
        card(
            "activity.water",
            "base.activity.water",
            value_or_missing(input, "activity.days", "base.activity.missing"),
        ),
        chart(
            "activity.steps.trend",
            "bar",
            "base.activity.steps",
            input,
            "activity.days",
        ),
        status(
            "activity.status",
            availability_state.clone(),
            availability_message_key(
                &availability_state,
                "base.activity.ready",
                "base.activity.missing",
            ),
        ),
    ]
}
