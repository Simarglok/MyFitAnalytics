wit_bindgen::generate!({
    path: "../../sdk/wit/dashboard-api.wit",
    world: "dashboard-module",
});

use mfa_contracts::{
    AvailabilityState, CapabilityId, DashboardBlock, DashboardCard, DashboardCardPresentation,
    DashboardChart, DashboardDocument, DashboardInput, DashboardSeries, DashboardStatusPanel,
};
use serde::Deserialize;
use serde_json::{Value, json};

mod activity;
mod body;
mod nutrition;
mod overview;
mod sources;
mod strength;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasePage {
    Overview,
    Body,
    Nutrition,
    Activity,
    Strength,
    Sources,
}

impl BasePage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Body => "body",
            Self::Nutrition => "nutrition",
            Self::Activity => "activity",
            Self::Strength => "strength",
            Self::Sources => "sources",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "overview" => Self::Overview,
            "body" => Self::Body,
            "nutrition" => Self::Nutrition,
            "activity" => Self::Activity,
            "strength" => Self::Strength,
            "sources" => Self::Sources,
            _ => return None,
        })
    }
}

pub fn compose_page(page: BasePage, input: &DashboardInput) -> DashboardDocument {
    let blocks = match page {
        BasePage::Overview => overview::compose(input),
        BasePage::Body => body::compose(input),
        BasePage::Nutrition => nutrition::compose(input),
        BasePage::Activity => activity::compose(input),
        BasePage::Strength => strength::compose(input),
        BasePage::Sources => sources::compose(input),
    };
    DashboardDocument {
        title_key: format!("base.{}.title", page.as_str()),
        blocks,
    }
}

pub fn describe_module() -> String {
    r#"{"module_id":"base","module_version":"1.0.0","dashboard_api_version":"1.0.0","required_capabilities":["activity.days","activity.events","body.fat_percentage","body.weight","heart_rate.observations","nutrition.items","strength.sessions","strength.sets"],"required_extension_contracts":[],"localization_namespace":"base"}"#.to_owned()
}

pub fn compose_json(input_json: &str) -> Result<String, String> {
    let raw: Value =
        serde_json::from_str(input_json).map_err(|error| format!("invalid_input:{error}"))?;
    let page = raw
        .get("page_id")
        .and_then(Value::as_str)
        .unwrap_or("overview");
    let page = BasePage::parse(page).ok_or_else(|| "unknown_page".to_owned())?;
    let input: DashboardInput =
        serde_json::from_value(raw).map_err(|error| format!("invalid_input:{error}"))?;
    serde_json::to_string(&compose_page(page, &input)).map_err(|error| error.to_string())
}

struct Component;

export!(Component);

impl Guest for Component {
    fn describe() -> String {
        describe_module()
    }

    fn compose(input_json: String) -> Result<String, String> {
        compose_json(&input_json)
    }
}

pub(crate) fn has_capability(input: &DashboardInput, name: &str) -> bool {
    input
        .capabilities
        .keys()
        .any(|capability| capability.as_str() == name)
}

pub(crate) fn page_availability_state(
    input: &DashboardInput,
    fallback: AvailabilityState,
) -> AvailabilityState {
    input.availability_state.clone().unwrap_or(fallback)
}

pub(crate) fn availability_message_key<'a>(
    state: &AvailabilityState,
    ready_key: &'a str,
    missing_key: &'a str,
) -> &'a str {
    match state {
        AvailabilityState::Ready => ready_key,
        AvailabilityState::MissingCapability => missing_key,
        AvailabilityState::MissingDependency => "base.state.missing_dependency",
        AvailabilityState::IncompatibleContract => "base.state.incompatible_contract",
        AvailabilityState::WaitingForData => "base.state.waiting_for_data",
        AvailabilityState::InsufficientCoverage => "base.state.insufficient_coverage",
        AvailabilityState::DisabledByUser => "base.state.disabled_by_user",
    }
}

pub(crate) fn value_or_missing(
    input: &DashboardInput,
    capability: &str,
    message_key: &str,
) -> Value {
    match input
        .capabilities
        .iter()
        .find(|(id, _)| id.as_str() == capability)
    {
        Some((_, value)) => json!({"available": true, "value": value}),
        None => json!({
            "available": false,
            "state": {"type": "missing_capability"},
            "message_key": message_key,
            "action": "import_data"
        }),
    }
}

pub(crate) fn value_or_missing_field(
    input: &DashboardInput,
    capability: &str,
    field: &str,
    message_key: &str,
) -> Value {
    match input
        .capabilities
        .iter()
        .find(|(id, _)| id.as_str() == capability)
    {
        Some((_, value)) => json!({
            "available": true,
            "value": value.get(field).cloned().unwrap_or(Value::Null),
        }),
        None => json!({
            "available": false,
            "state": {"type": "missing_capability"},
            "message_key": message_key,
            "action": "import_data"
        }),
    }
}

pub(crate) fn card(key: &str, label: &str, value: Value) -> DashboardBlock {
    let presentation = reviewed_card_presentation(&value);
    DashboardBlock::Card(DashboardCard {
        key: key.to_owned(),
        label: label.to_owned(),
        value,
        presentation,
    })
}

fn reviewed_card_presentation(value: &Value) -> Option<DashboardCardPresentation> {
    let available = value
        .get("available")
        .and_then(Value::as_bool)
        .or_else(|| {
            value
                .get("state")
                .and_then(Value::as_str)
                .map(|state| state == "ready")
        })
        .unwrap_or(false);
    available.then(|| DashboardCardPresentation {
        summary_key: "base.card_available".to_owned(),
        summary_value: None,
    })
}

pub(crate) fn status(key: &str, state: AvailabilityState, message_key: &str) -> DashboardBlock {
    DashboardBlock::StatusPanel(DashboardStatusPanel {
        key: key.to_owned(),
        state,
        message_key: message_key.to_owned(),
    })
}

pub(crate) fn chart(
    key: &str,
    chart_type: &str,
    name: &str,
    input: &DashboardInput,
    capability: &str,
) -> DashboardBlock {
    DashboardBlock::Chart(DashboardChart {
        key: key.to_owned(),
        chart_type: chart_type.to_owned(),
        series: vec![DashboardSeries {
            name: name.to_owned(),
            points: points(input, capability),
        }],
    })
}

fn points(input: &DashboardInput, capability: &str) -> Vec<(String, Option<f64>)> {
    let Some(value) = input
        .capabilities
        .iter()
        .find(|(id, _)| id.as_str() == capability)
        .map(|(_, value)| value)
    else {
        return Vec::new();
    };
    let values = value.as_array().or_else(|| {
        [
            "trailing7dMeanKg",
            "dailyMedianKg",
            "observations",
            "days",
            "steps",
            "mean_steps_7d",
            "session_durations",
        ]
        .into_iter()
        .find_map(|key| value.get(key).and_then(Value::as_array))
    });
    values
        .map(|values| parse_points(values))
        .unwrap_or_default()
}

fn parse_points(values: &[Value]) -> Vec<(String, Option<f64>)> {
    values
        .iter()
        .filter_map(|value| {
            let object = value.as_object()?;
            let label = ["date", "local_date", "week_start", "label"]
                .into_iter()
                .find_map(|key| object.get(key).and_then(Value::as_str))?
                .to_owned();
            let number = [
                "value",
                "value_kg",
                "calories_kcal",
                "steps",
                "duration_seconds",
                "water_ml",
            ]
            .into_iter()
            .find_map(|key| object.get(key))
            .map(|value| value.as_f64().filter(|number| number.is_finite()));
            Some((label, number.flatten()))
        })
        .collect()
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct _InputPageProbe {
    #[serde(default)]
    page_id: Option<String>,
}

#[allow(dead_code)]
fn _keep_types(_: (CapabilityId, DashboardDocument)) {}
