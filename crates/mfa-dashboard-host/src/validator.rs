use crate::document::{DashboardOutput, ModuleErrorView};
use mfa_contracts::{DashboardBlock, DashboardDocument, DashboardInput};
use serde_json::Value;
use std::collections::BTreeSet;
use thiserror::Error;

const MAX_POINTS_PER_SERIES: usize = 5_000;
const MAX_POINTS_PER_DOCUMENT: usize = 10_000;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DocumentValidationError {
    #[error("dashboard document is malformed")]
    MalformedDocument,
    #[error("dashboard document contains an unknown node type")]
    UnknownNodeType,
    #[error("dashboard chart type is not supported: {chart_type}")]
    UnknownChartType { chart_type: String },
    #[error("dashboard document contains an unsafe string")]
    UnsafeString,
    #[error("dashboard document contains a forbidden key: {key}")]
    ForbiddenKey { key: String },
    #[error("dashboard document contains a non-finite number")]
    NonFiniteNumber,
    #[error("dashboard document contains too many chart points")]
    SeriesTooLarge,
    #[error("dashboard document references an undeclared dataset: {dataset}")]
    UndeclaredDataset { dataset: String },
    #[error("dashboard document references an undeclared localization key: {key}")]
    UndeclaredLocalization { key: String },
}

impl DocumentValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MalformedDocument => "malformed_document",
            Self::UnknownNodeType => "unknown_node_type",
            Self::UnknownChartType { .. } => "unknown_chart_type",
            Self::UnsafeString => "unsafe_string",
            Self::ForbiddenKey { .. } => "forbidden_key",
            Self::NonFiniteNumber => "non_finite_number",
            Self::SeriesTooLarge => "series_too_large",
            Self::UndeclaredDataset { .. } => "undeclared_dataset",
            Self::UndeclaredLocalization { .. } => "undeclared_localization",
        }
    }
}

pub fn validate_document(
    document: &DashboardDocument,
    grant: &DashboardInput,
    localization_keys: &BTreeSet<String>,
) -> Result<(), DocumentValidationError> {
    localization_key(&document.title_key, localization_keys)?;
    let mut point_count = 0usize;
    for block in &document.blocks {
        match block {
            DashboardBlock::Card(card) => {
                localization_key(&card.label, localization_keys)?;
                validate_value(&card.value, grant)?;
            }
            DashboardBlock::Table(table) => {
                for column in &table.columns {
                    localization_key(column, localization_keys)?;
                }
                for row in &table.rows {
                    for value in row {
                        validate_value(value, grant)?;
                    }
                }
            }
            DashboardBlock::StatusPanel(panel) => {
                localization_key(&panel.message_key, localization_keys)?;
            }
            DashboardBlock::Chart(chart) => {
                if !matches!(
                    chart.chart_type.as_str(),
                    "line" | "bar" | "scatter" | "calendar_heatmap"
                ) {
                    return Err(DocumentValidationError::UnknownChartType {
                        chart_type: chart.chart_type.clone(),
                    });
                }
                for series in &chart.series {
                    localization_key(&series.name, localization_keys)?;
                    if series.points.len() > MAX_POINTS_PER_SERIES {
                        return Err(DocumentValidationError::SeriesTooLarge);
                    }
                    point_count = point_count.saturating_add(series.points.len());
                    for (label, value) in &series.points {
                        safe_string(label)?;
                        if let Some(value) = value {
                            if !value.is_finite() {
                                return Err(DocumentValidationError::NonFiniteNumber);
                            }
                        }
                    }
                }
            }
        }
    }
    if point_count > MAX_POINTS_PER_DOCUMENT {
        return Err(DocumentValidationError::SeriesTooLarge);
    }
    Ok(())
}

pub fn validate_document_json(
    raw: &Value,
    grant: &DashboardInput,
    localization_keys: &BTreeSet<String>,
) -> Result<DashboardDocument, DocumentValidationError> {
    reject_unknown_node_types(raw)?;
    let document: DashboardDocument = serde_json::from_value(raw.clone())
        .map_err(|_| DocumentValidationError::MalformedDocument)?;
    validate_document(&document, grant, localization_keys)?;
    Ok(document)
}

pub fn validate_or_error(
    document: DashboardDocument,
    grant: &DashboardInput,
    localization_keys: &BTreeSet<String>,
) -> DashboardOutput {
    validate_or_error_result::<String>(Ok(document), grant, localization_keys)
}

pub fn validate_or_error_result<E: Into<String>>(
    result: Result<DashboardDocument, E>,
    grant: &DashboardInput,
    localization_keys: &BTreeSet<String>,
) -> DashboardOutput {
    match result {
        Ok(document) => match validate_document(&document, grant, localization_keys) {
            Ok(()) => DashboardOutput::Document(document),
            Err(error) => DashboardOutput::ModuleError(ModuleErrorView::from_code(error.code())),
        },
        Err(error) => DashboardOutput::ModuleError(ModuleErrorView::from_code(error)),
    }
}

fn reject_unknown_node_types(raw: &Value) -> Result<(), DocumentValidationError> {
    let Some(blocks) = raw.get("blocks").and_then(Value::as_array) else {
        return Ok(());
    };
    for block in blocks {
        if let Some(object) = block.as_object()
            && object.keys().any(|key| key != "type" && key != "value")
        {
            return Err(DocumentValidationError::MalformedDocument);
        }
        let Some(node_type) = block.get("type").and_then(Value::as_str) else {
            continue;
        };
        if !matches!(node_type, "card" | "table" | "status_panel" | "chart") {
            return Err(DocumentValidationError::UnknownNodeType);
        }
    }
    Ok(())
}

fn localization_key(
    key: &str,
    localization_keys: &BTreeSet<String>,
) -> Result<(), DocumentValidationError> {
    safe_string(key)?;
    if !localization_keys.contains(key) {
        return Err(DocumentValidationError::UndeclaredLocalization {
            key: key.to_owned(),
        });
    }
    Ok(())
}

fn validate_value(value: &Value, grant: &DashboardInput) -> Result<(), DocumentValidationError> {
    match value {
        Value::Null => Ok(()),
        Value::Bool(_) => Ok(()),
        Value::Number(number) => {
            if number.as_f64().is_some_and(|value| !value.is_finite()) {
                Err(DocumentValidationError::NonFiniteNumber)
            } else {
                Ok(())
            }
        }
        Value::String(value) => safe_string(value),
        Value::Array(values) => values
            .iter()
            .try_for_each(|value| validate_value(value, grant)),
        Value::Object(values) => {
            for (key, value) in values {
                let normalized = key.to_ascii_lowercase();
                if normalized == "html"
                    || normalized == "script"
                    || normalized == "css"
                    || normalized == "url"
                    || normalized == "sql"
                    || normalized == "command"
                    || normalized.starts_with("on")
                {
                    return Err(DocumentValidationError::ForbiddenKey { key: key.clone() });
                }
                if normalized == "dataset" {
                    let dataset = value.as_str().ok_or_else(|| {
                        DocumentValidationError::UndeclaredDataset {
                            dataset: "non_string".to_owned(),
                        }
                    })?;
                    let capability_granted = grant
                        .capabilities
                        .keys()
                        .any(|capability| capability.as_str() == dataset);
                    let extension_granted = grant.extensions.contains_key(dataset);
                    if !capability_granted && !extension_granted {
                        return Err(DocumentValidationError::UndeclaredDataset {
                            dataset: dataset.to_owned(),
                        });
                    }
                }
                safe_string(key)?;
                validate_value(value, grant)?;
            }
            Ok(())
        }
    }
}

fn safe_string(value: &str) -> Result<(), DocumentValidationError> {
    let lower = value.to_ascii_lowercase();
    if lower.contains('<')
        || lower.contains('>')
        || lower.contains("javascript:")
        || lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("select ")
        || lower.contains("insert ")
        || lower.contains("update ")
        || lower.contains("delete ")
        || lower.contains("drop ")
        || lower.contains("style=")
    {
        return Err(DocumentValidationError::UnsafeString);
    }
    Ok(())
}
