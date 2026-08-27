use mfa_contracts::{
    AvailabilityState, DashboardBlock, DashboardChart, DashboardInput, DashboardSeries,
};
use serde_json::Value;

use crate::{card, chart, has_capability, status, value_or_missing};

pub fn compose(input: &DashboardInput) -> Vec<DashboardBlock> {
    let body_value = value_or_missing(input, "body.weight", "base.overview.body_weight");
    let nutrition_value = value_or_missing(input, "nutrition.items", "base.overview.nutrition");
    let all_ready = has_capability(input, "body.weight")
        && has_capability(input, "nutrition.items")
        && has_capability(input, "activity.days")
        && has_capability(input, "strength.sessions");
    vec![
        card(
            "overview.body_weight",
            "base.overview.body_weight",
            body_value,
        ),
        card(
            "overview.nutrition",
            "base.overview.nutrition",
            nutrition_value,
        ),
        chart(
            "overview.trend",
            "line",
            "base.overview.trend",
            input,
            "body.weight",
        ),
        status(
            "overview.quality",
            if all_ready {
                AvailabilityState::Ready
            } else {
                AvailabilityState::MissingCapability
            },
            if all_ready {
                "base.overview.quality_ready"
            } else {
                "base.body.missing"
            },
        ),
    ]
}

#[allow(dead_code)]
fn _keep_imports(_: (Value, DashboardChart, DashboardSeries, Value)) {}
