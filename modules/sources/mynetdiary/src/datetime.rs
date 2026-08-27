use crate::cells::{Cell, parse_local_date};
use crate::error::MappingError;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use mfa_contracts::{LocalDate, LocalDateTime};

pub fn row_local_date(
    sheet: &str,
    row_number: usize,
    row: &[Cell],
    date_column: usize,
) -> Result<LocalDate, MappingError> {
    let value = row
        .get(date_column)
        .map(|cell| cell.display.as_str())
        .unwrap_or_default();
    parse_local_date(value)
        .map(LocalDate::from)
        .ok_or_else(|| MappingError::InvalidDate {
            sheet: sheet.to_owned(),
            row: row_number,
            value: value.to_owned(),
        })
}

pub fn row_local_datetime(
    sheet: &str,
    row_number: usize,
    row: &[Cell],
    date_column: usize,
    time_column: Option<usize>,
) -> Result<(LocalDate, LocalDateTime), MappingError> {
    let date = row_local_date(sheet, row_number, row, date_column)?;
    let time = time_column
        .and_then(|column| row.get(column))
        .and_then(|cell| parse_time(&cell.display))
        .unwrap_or(NaiveTime::MIN);
    let date_time = NaiveDateTime::new(date.as_naive(), time);
    Ok((date, LocalDateTime::from(date_time)))
}

fn parse_time(value: &str) -> Option<NaiveTime> {
    let value = value.trim();
    ["%H:%M:%S", "%H:%M", "%H:%M:%S%.f"]
        .iter()
        .find_map(|format| NaiveTime::parse_from_str(value, format).ok())
}

#[allow(dead_code)]
fn _date_from_naive(value: NaiveDate) -> LocalDate {
    LocalDate::from(value)
}
