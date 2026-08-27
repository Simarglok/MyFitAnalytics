wit_bindgen::generate!({
    path: "../../sdk/wit/dashboard-api.wit",
    world: "dashboard-module",
});

use mfa_contracts::{
    AvailabilityState, CapabilityId, DashboardBlock, DashboardCard, DashboardChart,
    DashboardDocument, DashboardInput, DashboardSeries, DashboardStatusPanel,
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
    r#"{"module_id":"base","module_version":"1.0.0","dashboard_api_version":"1.0.0","required_capabilities":["activity.days","body.fat_percentage","body.weight","nutrition.items","strength.sessions","strength.sets"],"required_extension_contracts":[],"localization_namespace":"base"}"#.to_owned()
}

pub fn compose_json(input_json: &str) -> Result<String, String> {
    let raw: Value =
        serde_json::from_str(input_json).map_err(|error| format!("invalid_input:{error}"))?;
    let page = raw
        .get("page_id")
        .and_then(Value::as_str)
        .or_else(|| {
            raw.get("capabilities")
                .and_then(Value::as_object)
                .and_then(|capabilities| capabilities.get("dashboard.page"))
                .and_then(Value::as_str)
        })
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

pub(crate) fn card(key: &str, label: &str, value: Value) -> DashboardBlock {
    DashboardBlock::Card(DashboardCard {
        key: key.to_owned(),
        label: label.to_owned(),
        value,
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

fn points(input: &DashboardInput, capability: &str) -> Vec<(String, f64)> {
    let Some(value) = input
        .capabilities
        .iter()
        .find(|(id, _)| id.as_str() == capability)
        .map(|(_, value)| value)
    else {
        return Vec::new();
    };
    let Some(values) = value.as_array() else {
        return Vec::new();
    };
    values
        .iter()
        .filter_map(|value| {
            let object = value.as_object()?;
            let label = object.get("date")?.as_str()?.to_owned();
            let number = object.get("value")?.as_f64()?;
            number.is_finite().then_some((label, number))
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
