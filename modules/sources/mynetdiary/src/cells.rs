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
        parse_local_date(&self.display)
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
    for format in ["%Y-%m-%d", "%Y/%m/%d", "%m/%d/%Y", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(date) = NaiveDate::parse_from_str(value, format) {
            return Some(date);
        }
        if let Ok(date_time) = NaiveDateTime::parse_from_str(value, format) {
            return Some(date_time.date());
        }
    }
    value
        .get(..10)
        .and_then(|prefix| NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok())
}
