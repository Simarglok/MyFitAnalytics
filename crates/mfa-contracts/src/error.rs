use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractError {
    pub code: String,
    pub field: Option<String>,
    pub detail: String,
}

impl ContractError {
    pub fn new(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            field: None,
            detail: detail.into(),
        }
    }

    pub fn for_field(
        code: impl Into<String>,
        field: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            field: Some(field.into()),
            detail: detail.into(),
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for ContractError {}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AssetReadError {
    #[error("asset read range is invalid: offset={offset}, max_bytes={max_bytes}")]
    InvalidRange { offset: u64, max_bytes: u32 },
    #[error("asset read failed: {detail}")]
    Unavailable { detail: String },
}

impl AssetReadError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRange { .. } => "asset_invalid_range",
            Self::Unavailable { .. } => "asset_unavailable",
        }
    }
}
