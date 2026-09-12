use chrono::{NaiveDate, NaiveDateTime};

#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub display: String,
    pub value: CellValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Empty,
    Text(String),
    Number(f64),
    Boolean(bool),
    DateTime(String),
    Error(String),
}

impl Cell {
    pub fn empty() -> Self {
        Self {
            display: String::new(),
            value: CellValue::Empty,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self.value {
            CellValue::Number(value) => value.is_finite().then_some(value),
            CellValue::Text(ref value) => parse_number(value),
            _ => None,
        }
    }

    pub fn as_date(&self) -> Option<NaiveDate> {
        self.as_datetime()
            .map(|date_time| date_time.date())
            .or_else(|| parse_local_date(&self.display))
    }

    pub fn as_datetime(&self) -> Option<NaiveDateTime> {
        match &self.value {
            CellValue::DateTime(value) => parse_local_datetime(value),
            _ => parse_local_datetime(&self.display),
        }
    }
}

pub fn parse_number(raw: &str) -> Option<f64> {
    let mut value = raw.trim().replace(['\u{00a0}', ' '], "");
    if value.contains(',') && value.contains('.') {
        value = value.replace('.', "").replace(',', ".");
    } else if value.contains(',') {
        value = value.replace(',', ".");
    }
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

pub fn parse_local_date(raw: &str) -> Option<NaiveDate> {
    let value = raw.trim();
    for format in ["%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y"] {
        if let Ok(date) = NaiveDate::parse_from_str(value, format) {
            return Some(date);
        }
    }
    if let Some(date_time) = parse_local_datetime(value) {
        return Some(date_time.date());
    }
    value
        .get(..10)
        .and_then(|prefix| NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok())
}

pub fn parse_local_datetime(raw: &str) -> Option<NaiveDateTime> {
    let value = raw.trim();
    [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
        "%m/%d/%Y %H:%M:%S",
        "%m/%d/%Y %H:%M",
        "%m/%d/%Y %I:%M %p",
    ]
    .iter()
    .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
}
