use mfa_contracts::{AvailabilityState, DashboardBlock, DashboardInput};

use crate::{card, chart, has_capability, status, value_or_missing};

pub fn compose(input: &DashboardInput) -> Vec<DashboardBlock> {
    let available = has_capability(input, "body.weight");
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
            if available {
                AvailabilityState::Ready
            } else {
                AvailabilityState::MissingCapability
            },
            if available {
                "base.body.ready"
            } else {
                "base.body.missing"
            },
        ),
    ]
}
