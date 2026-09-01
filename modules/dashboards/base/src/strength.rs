use mfa_contracts::{AvailabilityState, DashboardBlock, DashboardInput};

use crate::{
    availability_message_key, card, chart, has_capability, page_availability_state, status,
    value_or_missing,
};

pub fn compose(input: &DashboardInput) -> Vec<DashboardBlock> {
    let available = has_capability(input, "strength.sessions");
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
            "strength.sessions",
            "base.strength.sessions",
            value_or_missing(input, "strength.sessions", "base.strength.missing"),
        ),
        card(
            "strength.duration",
            "base.strength.duration",
            value_or_missing(input, "strength.sessions", "base.strength.missing"),
        ),
        card(
            "strength.sets",
            "base.strength.sets",
            value_or_missing(input, "strength.sets", "base.strength.missing"),
        ),
        card(
            "strength.e1rm",
            "base.strength.e1rm",
            value_or_missing(input, "strength.sets", "base.strength.missing"),
        ),
        chart(
            "strength.sessions.calendar",
            "calendar_heatmap",
            "base.strength.sessions",
            input,
            "strength.sessions",
        ),
        status(
            "strength.status",
            availability_state.clone(),
            availability_message_key(
                &availability_state,
                "base.strength.ready",
                "base.strength.missing",
            ),
        ),
    ]
}
