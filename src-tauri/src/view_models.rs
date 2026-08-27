use mfa_contracts::{AvailabilityState, DashboardBlock, LocalDate};
use mfa_dashboard_host::DashboardOutput;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DateRangeView {
    pub start: String,
    pub end: String,
}

impl DateRangeView {
    pub fn synthetic_default() -> Self {
        Self {
            start: "2026-01-01".to_owned(),
            end: "2026-01-31".to_owned(),
        }
    }

    pub fn parse(&self) -> Result<(LocalDate, LocalDate), String> {
        let start = self
            .start
            .parse::<LocalDate>()
            .map_err(|error| error.to_string())?;
        let end = self
            .end
            .parse::<LocalDate>()
            .map_err(|error| error.to_string())?;
        if start > end {
            return Err("date range start must not be after end".to_owned());
        }
        Ok((start, end))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityView {
    pub state: AvailabilityState,
    pub reason_key: String,
    pub required_capabilities: Vec<String>,
    pub required_dependencies: Vec<String>,
    pub freshness: mfa_dashboard_host::Freshness,
    pub action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CoverageView {
    pub expected_days: u64,
    pub observed_days: u64,
    pub ratio: f64,
    pub sufficient: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FreshnessView {
    pub latest_observation_date: Option<String>,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NavigationItemView {
    pub id: String,
    pub page_id: String,
    pub title_key: String,
    pub module_id: String,
    pub availability: AvailabilityView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NavigationView {
    pub items: Vec<NavigationItemView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardPageView {
    pub module_id: String,
    pub page_id: String,
    pub title_key: String,
    pub document: DashboardOutput,
    pub availability: AvailabilityView,
    pub coverage: CoverageView,
    pub freshness: FreshnessView,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    pub capability: String,
    pub module_id: String,
    pub active_providers: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PhaseEventInput {
    pub phase_event_id: Option<String>,
    pub event_type: String,
    pub start_date: String,
    pub end_date: String,
    pub description: Option<String>,
    pub exclude_from_tdee: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PhaseEventView {
    pub phase_event_id: String,
    pub event_type: String,
    pub start_date: String,
    pub end_date: String,
    pub description: Option<String>,
    pub exclude_from_tdee: bool,
}

impl From<mfa_contracts::PhaseEvent> for PhaseEventView {
    fn from(event: mfa_contracts::PhaseEvent) -> Self {
        Self {
            phase_event_id: event.phase_event_id.to_string(),
            event_type: event.event_type,
            start_date: event.start_date.to_string(),
            end_date: event.end_date.to_string(),
            description: event.description,
            exclude_from_tdee: event.exclude_from_tdee,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardDocumentView {
    pub blocks: Vec<DashboardBlock>,
}
