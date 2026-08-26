use crate::cells::parse_number as parse_cell_number;
use crate::error::MappingError;

pub fn parse_number(raw: &str) -> Result<Option<f64>, MappingError> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    parse_cell_number(raw)
        .map(Some)
        .ok_or_else(|| MappingError::InvalidNumber {
            sheet: "mapping".to_owned(),
            row: 0,
            column: "value".to_owned(),
            value: raw.to_owned(),
        })
}

pub fn parse_non_negative(
    raw: &str,
    sheet: &str,
    row: usize,
    column: &str,
) -> Result<Option<f64>, MappingError> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    let number = parse_cell_number(raw).ok_or_else(|| MappingError::InvalidNumber {
        sheet: sheet.to_owned(),
        row,
        column: column.to_owned(),
        value: raw.to_owned(),
    })?;
    if number < 0.0 {
        return Err(MappingError::NegativeNumber {
            sheet: sheet.to_owned(),
            row,
            column: column.to_owned(),
            value: raw.to_owned(),
        });
    }
    Ok(Some(number))
}

pub fn parse_integral_non_negative(
    raw: &str,
    sheet: &str,
    row: usize,
    column: &str,
) -> Result<Option<u64>, MappingError> {
    let Some(number) = parse_non_negative(raw, sheet, row, column)? else {
        return Ok(None);
    };
    if number.fract() != 0.0 || number > u64::MAX as f64 {
        return Err(MappingError::InvalidNumber {
            sheet: sheet.to_owned(),
            row,
            column: column.to_owned(),
            value: raw.to_owned(),
        });
    }
    Ok(Some(number as u64))
}
