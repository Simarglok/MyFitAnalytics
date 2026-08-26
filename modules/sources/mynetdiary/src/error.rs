use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum MappingError {
    #[error("invalid BIFF workbook: {detail}")]
    InvalidBiff { detail: String },
    #[error("required sheet `{sheet}` is missing")]
    MissingSheet { sheet: String },
    #[error("required column `{column}` is missing from `{sheet}`")]
    MissingColumn { sheet: String, column: String },
    #[error("column `{column}` is duplicated in `{sheet}`")]
    DuplicateColumn { sheet: String, column: String },
    #[error("invalid date `{value}` in `{sheet}` row {row}")]
    InvalidDate {
        sheet: String,
        row: usize,
        value: String,
    },
    #[error("invalid number `{value}` in `{sheet}` row {row}, column `{column}`")]
    InvalidNumber {
        sheet: String,
        row: usize,
        column: String,
        value: String,
    },
    #[error("negative number `{value}` in `{sheet}` row {row}, column `{column}`")]
    NegativeNumber {
        sheet: String,
        row: usize,
        column: String,
        value: String,
    },
    #[error("workbook dates span multiple calendar years: {years:?}")]
    MixedCalendarYear { years: Vec<i32> },
    #[error("workbook contains no calendar year")]
    MissingCalendarYear,
}

impl MappingError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidBiff { .. } => "mynetdiary.invalid_biff",
            Self::MissingSheet { .. } => "mynetdiary.missing_sheet",
            Self::MissingColumn { .. } => "mynetdiary.missing_column",
            Self::DuplicateColumn { .. } => "mynetdiary.duplicate_column",
            Self::InvalidDate { .. } => "mynetdiary.invalid_date",
            Self::InvalidNumber { .. } => "mynetdiary.invalid_number",
            Self::NegativeNumber { .. } => "mynetdiary.negative_number",
            Self::MixedCalendarYear { .. } => "mynetdiary.mixed_calendar_year",
            Self::MissingCalendarYear => "mynetdiary.missing_calendar_year",
        }
    }

    pub fn detail(&self) -> String {
        self.to_string()
    }
}
