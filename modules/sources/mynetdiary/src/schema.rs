use crate::cells::Cell;
use crate::error::MappingError;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaProfile {
    Legacy,
    Current,
}

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
    pub profile: SchemaProfile,
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
    pub profile: SchemaProfile,
    pub calendar_year: i32,
}

type ValidatedHeaders = (SchemaProfile, Vec<String>, BTreeMap<String, usize>);

fn profile_columns(
    kind: SheetKind,
    profile: SchemaProfile,
) -> &'static [(&'static str, &'static str)] {
    match (kind, profile) {
        (SheetKind::Food, SchemaProfile::Legacy) => &[
            ("Date", "Date"),
            ("Time", "Time"),
            ("Food Name", "Food Name"),
            ("Food ID", "Food ID"),
            ("Amount", "Amount"),
            ("Calories", "Calories"),
            ("Protein, g", "Protein, g"),
            ("Fat, g", "Fat, g"),
            ("Carbs, g", "Carbs, g"),
            ("Fiber, g", "Fiber, g"),
            ("Sugars, g", "Sugars, g"),
            ("Sodium, mg", "Sodium, mg"),
        ],
        (SheetKind::Food, SchemaProfile::Current) => &[
            ("Date", "Date & Time"),
            ("Time", "Date & Time"),
            ("Food Name", "Name"),
            ("Food ID", "Food ID"),
            ("Amount", "Amount"),
            ("Calories", "Calories, cals"),
            ("Protein, g", "Protein, g"),
            ("Fat, g", "Total Fat, g"),
            ("Carbs, g", "Total Carbs, g"),
            ("Fiber, g", "Dietary Fiber, g"),
            ("Sugars, g", "Total Sugars, g"),
            ("Sodium, mg", "Sodium, mg"),
        ],
        (SheetKind::Measurements, SchemaProfile::Legacy) => &[
            ("Date", "Date"),
            ("Type", "Type"),
            ("Value", "Value"),
            ("Unit", "Unit"),
        ],
        (SheetKind::Measurements, SchemaProfile::Current) => &[
            ("Date", "Date"),
            ("Type", "Measurement"),
            ("Value", "Value"),
            ("Unit", "Unit"),
        ],
        (SheetKind::Exercise, SchemaProfile::Legacy) => &[
            ("Date", "Date"),
            ("Activity", "Activity"),
            ("Duration, min", "Duration, min"),
            ("Distance, km", "Distance, km"),
            ("Calories", "Calories"),
        ],
        (SheetKind::Exercise, SchemaProfile::Current) => &[
            ("Date", "Date & Time"),
            ("Time", "Date & Time"),
            ("Activity", "Name"),
            ("Calories", "Calories"),
        ],
        (SheetKind::Trackers, SchemaProfile::Legacy) => &[
            ("Date", "Date"),
            ("Type", "Type"),
            ("Value", "Value"),
            ("Unit", "Unit"),
        ],
        (SheetKind::Trackers, SchemaProfile::Current) => &[
            ("Date", "Date and Time"),
            ("Time", "Date and Time"),
            ("Type", "Tracker"),
            ("Value", "Value"),
            ("Unit", "Unit"),
        ],
        (SheetKind::WaterGlasses, SchemaProfile::Legacy) => &[
            ("Date", "Date"),
            ("Water, ml", "Water, ml"),
            ("Glasses", "Glasses"),
        ],
        (SheetKind::WaterGlasses, SchemaProfile::Current) => &[
            ("Date", "Date"),
            ("Water, ml", "Water, ml"),
            ("Glasses", "Water Glasses Count"),
        ],
    }
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
) -> Result<ValidatedHeaders, MappingError> {
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
    let mut physical_columns = BTreeMap::new();
    for (index, header) in headers.iter().enumerate() {
        if header.is_empty() {
            continue;
        }
        if physical_columns.insert(header.clone(), index).is_some() {
            return Err(MappingError::DuplicateColumn {
                sheet: sheet_name.to_owned(),
                column: header.clone(),
            });
        }
    }
    let candidates = [SchemaProfile::Legacy, SchemaProfile::Current]
        .into_iter()
        .filter(|profile| {
            profile_columns(kind, *profile)
                .iter()
                .all(|(_, actual)| physical_columns.contains_key(*actual))
        })
        .collect::<Vec<_>>();
    let Some(profile) = candidates.first().copied() else {
        let missing = profile_columns(kind, SchemaProfile::Legacy)
            .iter()
            .find(|(_, actual)| !physical_columns.contains_key(*actual))
            .map(|(semantic, _)| *semantic)
            .unwrap_or("required field");
        return Err(MappingError::MissingColumn {
            sheet: sheet_name.to_owned(),
            column: missing.to_owned(),
        });
    };
    if candidates.len() != 1 {
        return Err(MappingError::DuplicateColumn {
            sheet: sheet_name.to_owned(),
            column: "ambiguous schema profile".to_owned(),
        });
    }
    let columns = profile_columns(kind, profile)
        .iter()
        .map(|(semantic, actual)| {
            (
                (*semantic).to_owned(),
                *physical_columns
                    .get(*actual)
                    .expect("selected schema profile must have every declared column"),
            )
        })
        .collect();
    Ok((profile, headers, columns))
}
