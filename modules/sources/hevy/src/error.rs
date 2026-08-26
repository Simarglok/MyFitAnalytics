use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum MappingError {
    #[error("invalid UTF-8 CSV input")]
    InvalidUtf8,
    #[error("invalid CSV: {detail}")]
    InvalidCsv { detail: String },
    #[error("missing required Hevy column `{column}`")]
    MissingColumn { column: String },
    #[error("duplicate Hevy column `{column}`")]
    DuplicateColumn { column: String },
    #[error("invalid Hevy date `{value}` at row {row}")]
    InvalidDate { value: String, row: usize },
    #[error("invalid numeric value `{value}` for `{column}` at row {row}")]
    InvalidNumber {
        value: String,
        column: String,
        row: usize,
    },
    #[error("weight must be positive at row {row}, got `{value}`")]
    InvalidWeight { value: String, row: usize },
    #[error("workout session ends before it starts at row {row}")]
    InvalidSessionTime { row: usize },
    #[error("invalid set value `{value}` for `{column}` at row {row}")]
    InvalidSetValue {
        value: String,
        column: String,
        row: usize,
    },
}

impl MappingError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidUtf8 => "hevy.invalid_utf8",
            Self::InvalidCsv { .. } => "hevy.invalid_csv",
            Self::MissingColumn { .. } => "hevy.missing_column",
            Self::DuplicateColumn { .. } => "hevy.duplicate_column",
            Self::InvalidDate { .. } => "hevy.invalid_date",
            Self::InvalidNumber { .. } => "hevy.invalid_number",
            Self::InvalidWeight { .. } => "hevy.invalid_weight",
            Self::InvalidSessionTime { .. } => "hevy.invalid_session_time",
            Self::InvalidSetValue { .. } => "hevy.invalid_set_value",
        }
    }

    pub fn detail(&self) -> String {
        self.to_string()
    }
}
