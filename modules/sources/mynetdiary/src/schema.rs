use crate::cells::Cell;
use crate::error::MappingError;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SheetKind {
    Food,
    Measurements,
    Exercise,
    Trackers,
    WaterGlasses,
}

impl SheetKind {
    pub fn workbook_name(self) -> &'static str {
        match self {
            Self::Food => "Food",
            Self::Measurements => "Measurements",
            Self::Exercise => "Exercise",
            Self::Trackers => "Trackers",
            Self::WaterGlasses => "Water Glasses",
        }
    }

    pub fn required() -> &'static [Self] {
        &[Self::Food, Self::Measurements, Self::Exercise]
    }

    pub fn optional() -> &'static [Self] {
        &[Self::Trackers, Self::WaterGlasses]
    }
}

#[derive(Debug, Clone)]
pub struct ValidatedSheet {
    pub kind: SheetKind,
    pub name: String,
    pub header_row: usize,
    pub headers: Vec<String>,
    pub columns: BTreeMap<String, usize>,
    pub rows: Vec<Vec<Cell>>,
}

impl ValidatedSheet {
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.get(name).copied()
    }

    pub fn source_row_number(&self, data_index: usize) -> usize {
        self.header_row + data_index + 1
    }
}

#[derive(Debug, Clone)]
pub struct WorkbookSchema {
    pub sheets: BTreeMap<SheetKind, ValidatedSheet>,
    pub calendar_year: i32,
}

pub fn required_columns(kind: SheetKind) -> &'static [&'static str] {
    match kind {
        SheetKind::Food => &[
            "Date",
            "Time",
            "Food Name",
            "Food ID",
            "Amount",
            "Calories",
            "Protein, g",
            "Fat, g",
            "Carbs, g",
            "Fiber, g",
            "Sugars, g",
            "Sodium, mg",
        ],
        SheetKind::Measurements => &["Date", "Type", "Value", "Unit"],
        SheetKind::Exercise => &[
            "Date",
            "Activity",
            "Duration, min",
            "Distance, km",
            "Calories",
        ],
        SheetKind::Trackers => &["Date", "Type", "Value", "Unit"],
        SheetKind::WaterGlasses => &["Date", "Water, ml", "Glasses"],
    }
}

pub fn validate_headers(
    kind: SheetKind,
    sheet_name: &str,
    rows: &[Vec<Cell>],
) -> Result<(Vec<String>, BTreeMap<String, usize>), MappingError> {
    let header = rows.first().ok_or_else(|| MappingError::MissingColumn {
        sheet: sheet_name.to_owned(),
        column: required_columns(kind)
            .first()
            .unwrap_or(&"Date")
            .to_string(),
    })?;
    let headers: Vec<String> = header
        .iter()
        .map(|cell| cell.display.trim().to_owned())
        .collect();
    let mut columns = BTreeMap::new();
    for (index, header) in headers.iter().enumerate() {
        if header.is_empty() {
            continue;
        }
        if columns.insert(header.clone(), index).is_some() {
            return Err(MappingError::DuplicateColumn {
                sheet: sheet_name.to_owned(),
                column: header.clone(),
            });
        }
    }
    for required in required_columns(kind) {
        if !columns.contains_key(*required) {
            return Err(MappingError::MissingColumn {
                sheet: sheet_name.to_owned(),
                column: (*required).to_owned(),
            });
        }
    }
    Ok((headers, columns))
}
