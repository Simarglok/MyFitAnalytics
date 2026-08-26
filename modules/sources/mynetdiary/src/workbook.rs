use crate::cells::{Cell, CellValue};
use crate::error::MappingError;
use crate::schema::{SheetKind, ValidatedSheet, WorkbookSchema, validate_headers};
use calamine::{Data, Reader, Xls};
use chrono::Datelike;
use std::collections::BTreeSet;
use std::io::Cursor;

pub const BIFF8_SIGNATURE: [u8; 8] = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];

pub fn detect_mynetdiary(bytes: &[u8]) -> u8 {
    if !bytes.starts_with(&BIFF8_SIGNATURE) {
        return 0;
    }
    let mut workbook = match Xls::new(Cursor::new(bytes.to_vec())) {
        Ok(workbook) => workbook,
        Err(_) => return 0,
    };
    let names = workbook.sheet_names().to_vec();
    if SheetKind::required()
        .iter()
        .any(|kind| !names.iter().any(|name| name.trim() == kind.workbook_name()))
    {
        return 0;
    }
    if SheetKind::required()
        .iter()
        .any(|kind| workbook.worksheet_range(kind.workbook_name()).is_err())
    {
        return 0;
    }
    100
}

pub fn validate_workbook(bytes: &[u8]) -> Result<WorkbookSchema, MappingError> {
    if !bytes.starts_with(&BIFF8_SIGNATURE) {
        return Err(MappingError::InvalidBiff {
            detail: "CDFV2/BIFF8 signature is missing".to_owned(),
        });
    }
    let mut workbook =
        Xls::new(Cursor::new(bytes.to_vec())).map_err(|error| MappingError::InvalidBiff {
            detail: error.to_string(),
        })?;
    let names = workbook.sheet_names().to_vec();
    let mut sheets = std::collections::BTreeMap::new();
    for kind in SheetKind::required()
        .iter()
        .chain(SheetKind::optional().iter())
    {
        let Some(sheet_name) = names
            .iter()
            .find(|name| name.trim() == kind.workbook_name())
            .cloned()
        else {
            if SheetKind::required().contains(kind) {
                return Err(MappingError::MissingSheet {
                    sheet: kind.workbook_name().to_owned(),
                });
            }
            continue;
        };
        let range =
            workbook
                .worksheet_range(&sheet_name)
                .map_err(|error| MappingError::InvalidBiff {
                    detail: format!("{}: {error}", kind.workbook_name()),
                })?;
        let rows = range
            .rows()
            .map(|row| row.iter().map(cell_from_calamine).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let (headers, columns) = validate_headers(*kind, kind.workbook_name(), &rows)?;
        let data_rows = rows.into_iter().skip(1).collect();
        sheets.insert(
            *kind,
            ValidatedSheet {
                kind: *kind,
                name: sheet_name,
                header_row: 1,
                headers,
                columns,
                rows: data_rows,
            },
        );
    }
    let provisional = WorkbookSchema {
        sheets,
        calendar_year: 0,
    };
    let calendar_year = infer_calendar_year(&provisional)?;
    Ok(WorkbookSchema {
        calendar_year,
        ..provisional
    })
}

pub fn infer_calendar_year(schema: &WorkbookSchema) -> Result<i32, MappingError> {
    let mut years = BTreeSet::new();
    for kind in SheetKind::required() {
        let sheet = schema
            .sheets
            .get(kind)
            .expect("validated required sheet must exist");
        let date_column =
            sheet
                .column_index("Date")
                .ok_or_else(|| MappingError::MissingColumn {
                    sheet: kind.workbook_name().to_owned(),
                    column: "Date".to_owned(),
                })?;
        for (index, row) in sheet.rows.iter().enumerate() {
            let Some(cell) = row.get(date_column) else {
                continue;
            };
            if cell.display.trim().is_empty() {
                continue;
            }
            let Some(date) = cell.as_date().or_else(|| parse_numeric_excel_date(cell)) else {
                return Err(MappingError::InvalidDate {
                    sheet: kind.workbook_name().to_owned(),
                    row: sheet.source_row_number(index),
                    value: cell.display.clone(),
                });
            };
            years.insert(date.year());
        }
    }
    if years.is_empty() {
        return Err(MappingError::MissingCalendarYear);
    }
    if years.len() > 1 {
        return Err(MappingError::MixedCalendarYear {
            years: years.into_iter().collect(),
        });
    }
    Ok(*years.first().expect("non-empty years"))
}

fn parse_numeric_excel_date(cell: &Cell) -> Option<chrono::NaiveDate> {
    let value = cell.as_f64()?;
    if !(1.0..=100_000.0).contains(&value) {
        return None;
    }
    chrono::NaiveDate::from_ymd_opt(1899, 12, 30)
        .and_then(|origin| origin.checked_add_days(chrono::Days::new(value.floor() as u64)))
}

fn cell_from_calamine(cell: &Data) -> Cell {
    match cell {
        Data::Empty => Cell::empty(),
        Data::String(value) => Cell {
            display: value.clone(),
            value: CellValue::Text(value.clone()),
        },
        Data::Float(value) => Cell {
            display: value.to_string(),
            value: CellValue::Number(*value),
        },
        Data::Int(value) => Cell {
            display: value.to_string(),
            value: CellValue::Number(*value as f64),
        },
        Data::Bool(value) => Cell {
            display: value.to_string(),
            value: CellValue::Boolean(*value),
        },
        Data::DateTime(value) => {
            let display = value.to_string();
            Cell {
                display: display.clone(),
                value: CellValue::DateTime(display),
            }
        }
        Data::DateTimeIso(value) | Data::DurationIso(value) => Cell {
            display: value.clone(),
            value: CellValue::DateTime(value.clone()),
        },
        Data::Error(value) => Cell {
            display: format!("{value:?}"),
            value: CellValue::Error(format!("{value:?}")),
        },
    }
}
