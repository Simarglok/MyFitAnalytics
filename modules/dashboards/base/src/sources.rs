use mfa_contracts::{AvailabilityState, DashboardBlock, DashboardInput};
use serde_json::json;

use crate::{card, has_capability, status, value_or_missing};

pub fn compose(input: &DashboardInput) -> Vec<DashboardBlock> {
    let available = [
        "body.weight",
        "nutrition.items",
        "activity.days",
        "strength.sessions",
    ]
    .iter()
    .all(|capability| has_capability(input, capability));
    vec![
        card(
            "sources.modules",
            "base.sources.modules",
            json!({"state": if available { "ready" } else { "missing_capability" }}),
        ),
        card(
            "sources.providers",
            "base.sources.providers",
            value_or_missing(input, "body.weight", "base.sources.missing"),
        ),
        card(
            "sources.snapshots",
            "base.sources.snapshots",
            value_or_missing(input, "body.weight", "base.sources.missing"),
        ),
        card(
            "sources.quality",
            "base.sources.quality",
            value_or_missing(input, "nutrition.items", "base.sources.missing"),
        ),
        status(
            "sources.status",
            if available {
                AvailabilityState::Ready
            } else {
                AvailabilityState::MissingCapability
            },
            if available {
                "base.sources.ready"
            } else {
                "base.sources.missing"
            },
        ),
    ]
}
