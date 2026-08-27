use crate::error::MappingError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HevyArtifact {
    Measurements,
    Workouts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeResult<T> {
    Match(T),
    NoMatch,
    InvalidUtf8,
    InvalidSchema,
}

pub trait GuestAssetReader {
    fn read_all(&mut self) -> Result<Vec<u8>, String>;
}

#[derive(Debug, Clone)]
pub struct CsvInput {
    pub bytes: Vec<u8>,
    pub asset_id: String,
}

impl CsvInput {
    pub fn new(bytes: Vec<u8>, asset_id: impl Into<String>) -> Self {
        Self {
            bytes,
            asset_id: asset_id.into(),
        }
    }
}

pub fn parse_headers(bytes: &[u8]) -> Result<Vec<String>, MappingError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);
    let headers = reader.headers().map_err(|error| MappingError::InvalidCsv {
        detail: error.to_string(),
    })?;
    let mut output = Vec::with_capacity(headers.len());
    for header in headers {
        let header = header.trim().to_owned();
        if output.iter().any(|existing| existing == &header) {
            return Err(MappingError::DuplicateColumn { column: header });
        }
        output.push(header);
    }
    Ok(output)
}

pub fn detect_hevy(asset: &mut dyn GuestAssetReader) -> ProbeResult<HevyArtifact> {
    let bytes = match asset.read_all() {
        Ok(bytes) => bytes,
        Err(_) => return ProbeResult::InvalidSchema,
    };
    if std::str::from_utf8(&bytes).is_err() {
        return ProbeResult::InvalidUtf8;
    }
    let headers = match parse_headers(&bytes) {
        Ok(headers) => headers,
        Err(MappingError::DuplicateColumn { .. }) => return ProbeResult::InvalidSchema,
        Err(_) => return ProbeResult::NoMatch,
    };
    if headers.iter().any(|header| header == "date")
        && headers.iter().any(|header| header == "weight_kg")
    {
        ProbeResult::Match(HevyArtifact::Measurements)
    } else if [
        "title",
        "start_time",
        "end_time",
        "exercise_title",
        "set_index",
        "set_type",
    ]
    .iter()
    .all(|required| headers.iter().any(|header| header == required))
    {
        ProbeResult::Match(HevyArtifact::Workouts)
    } else {
        ProbeResult::NoMatch
    }
}
