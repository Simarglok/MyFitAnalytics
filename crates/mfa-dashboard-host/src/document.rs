use mfa_contracts::{
    DashboardBlock, DashboardCard, DashboardChart, DashboardDocument, DashboardStatusPanel,
    DashboardTable,
};
use serde::{Deserialize, Serialize};

pub type DashboardNode = DashboardBlock;
pub type CardNode = DashboardCard;
pub type LineChartNode = DashboardChart;
pub type BarChartNode = DashboardChart;
pub type ScatterChartNode = DashboardChart;
pub type CalendarHeatmapNode = DashboardChart;
pub type TableNode = DashboardTable;
pub type StatusNode = DashboardStatusPanel;
pub type SectionNode = DashboardDocument;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleErrorView {
    pub code: String,
    pub message_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DashboardOutput {
    Document(DashboardDocument),
    ModuleError(ModuleErrorView),
}

impl ModuleErrorView {
    pub fn from_code(code: impl Into<String>) -> Self {
        let code = code.into();
        Self {
            message_key: format!("dashboard.module_error.{code}"),
            code,
        }
    }
}
