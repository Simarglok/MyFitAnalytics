use crate::{CapabilityId, ContractVersion, ExtensionRequirement};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardRequirement {
    pub capability: CapabilityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension: Option<ExtensionRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum AvailabilityState {
    MissingCapability,
    MissingDependency,
    IncompatibleContract,
    WaitingForData,
    InsufficientCoverage,
    Ready,
    DisabledByUser,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    #[serde(default)]
    pub capabilities: BTreeMap<CapabilityId, Value>,
    #[serde(default)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum DashboardBlock {
    Card(DashboardCard),
    Table(DashboardTable),
    StatusPanel(DashboardStatusPanel),
    Chart(DashboardChart),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardCard {
    pub key: String,
    pub label: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardTable {
    pub key: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardStatusPanel {
    pub key: String,
    pub state: AvailabilityState,
    pub message_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardChart {
    pub key: String,
    pub chart_type: String,
    pub series: Vec<DashboardSeries>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardSeries {
    pub name: String,
    pub points: Vec<(String, Option<f64>)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardDocument {
    pub title_key: String,
    pub blocks: Vec<DashboardBlock>,
}

impl DashboardDocument {
    pub fn is_declarative(&self) -> bool {
        serde_json::to_value(self)
            .map(|value| {
                let text = value.to_string().to_ascii_lowercase();
                ![
                    "javascript:",
                    "<script",
                    "select ",
                    "insert ",
                    "update ",
                    "delete ",
                ]
                .iter()
                .any(|forbidden| text.contains(forbidden))
            })
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageRule {
    pub minimum_records: u64,
    pub minimum_days: u64,
}

#[allow(dead_code)]
fn _keep_contract_version_in_module(value: &ContractVersion) -> &ContractVersion {
    value
}
