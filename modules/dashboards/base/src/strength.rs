use mfa_contracts::{AvailabilityState, DashboardBlock, DashboardInput};

use crate::{card, chart, has_capability, status, value_or_missing};

pub fn compose(input: &DashboardInput) -> Vec<DashboardBlock> {
    let available = has_capability(input, "strength.sessions");
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
            if available {
                AvailabilityState::Ready
            } else {
                AvailabilityState::MissingCapability
            },
            if available {
                "base.strength.ready"
            } else {
                "base.strength.missing"
            },
        ),
    ]
}
