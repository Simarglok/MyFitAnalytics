use crate::{CapabilityId, ContractError, ContractVersion, LocalDate, LocalDateTime};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NutritionItem {
    pub nutrition_item_id: Uuid,
    pub occurred_local_at: Option<LocalDateTime>,
    pub local_date: LocalDate,
    pub meal: String,
    pub food_source_id: String,
    pub name: String,
    pub amount_raw: String,
    pub calories_kcal: Option<f64>,
    pub protein_g: Option<f64>,
    pub fat_g: Option<f64>,
    pub carbs_g: Option<f64>,
    pub fiber_g: Option<f64>,
    pub sugars_g: Option<f64>,
    pub sodium_mg: Option<f64>,
    pub source_record_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BodyMeasurement {
    pub body_measurement_id: Uuid,
    pub local_date: LocalDate,
    pub weight_kg: f64,
    pub body_fat_pct: Option<f64>,
    pub source_record_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub activity_event_id: Uuid,
    pub occurred_local_at: LocalDateTime,
    pub local_date: LocalDate,
    pub activity_type: String,
    pub source_name: String,
    pub duration_seconds: Option<u32>,
    pub distance_km: Option<f64>,
    pub estimated_calories_kcal: Option<f64>,
    pub origin_hint: Option<String>,
    pub quality_status: String,
    pub source_record_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityDay {
    pub local_date: LocalDate,
    pub steps: Option<u64>,
    pub water_ml: Option<f64>,
    pub heart_rate_observation_count: u32,
    pub activity_duration_seconds: u64,
    pub activity_distance_km: f64,
    pub estimated_activity_calories_kcal: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeartRateObservation {
    pub heart_rate_observation_id: Uuid,
    pub observed_local_at: LocalDateTime,
    pub heart_rate_bpm: f64,
    pub source_record_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkoutSession {
    pub workout_session_id: Uuid,
    pub title: String,
    pub started_local_at: LocalDateTime,
    pub ended_local_at: LocalDateTime,
    pub duration_seconds: Option<u32>,
    pub source_record_group_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExerciseSet {
    pub exercise_set_id: Uuid,
    pub workout_session_id: Uuid,
    pub exercise_title_raw: String,
    pub exercise_key: String,
    pub exercise_block_ordinal: u32,
    pub set_index: u32,
    pub set_type: String,
    pub load_type: String,
    pub weight_kg: Option<f64>,
    pub reps: Option<u32>,
    pub duration_seconds: Option<u32>,
    pub rpe: Option<f64>,
    pub source_record_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseEvent {
    pub phase_event_id: Uuid,
    pub event_type: String,
    pub start_date: LocalDate,
    pub end_date: LocalDate,
    pub description: Option<String>,
    pub exclude_from_tdee: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum CanonicalObservation {
    NutritionItem(NutritionItem),
    BodyMeasurement(BodyMeasurement),
    ActivityEvent(ActivityEvent),
    ActivityDay(ActivityDay),
    HeartRate(HeartRateObservation),
    WorkoutSession(WorkoutSession),
    ExerciseSet(ExerciseSet),
    PhaseEvent(PhaseEvent),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingIssue {
    pub code: String,
    pub message: String,
    pub source_record_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionRecord {
    pub namespace: String,
    pub contract_version: ContractVersion,
    pub record_type: String,
    pub source_record_key: String,
    pub occurred_local_at: Option<LocalDateTime>,
    pub local_date: Option<LocalDate>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceRecord {
    pub source_record_key: String,
    pub sheet_name: Option<String>,
    pub source_row_number: u32,
    pub raw_payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageHook {
    pub canonical_entity_type: String,
    pub canonical_entity_id: String,
    pub source_record_key: String,
    pub mapping_version: ContractVersion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceBatch {
    pub contract_version: ContractVersion,
    pub source_module_id: String,
    pub source_api_version: ContractVersion,
    pub mapping_version: ContractVersion,
    pub schema_fingerprint: String,
    pub logical_snapshot_key: String,
    pub source_records: Vec<SourceRecord>,
    pub lineage: Vec<LineageHook>,
    pub records: Vec<CanonicalObservation>,
    pub extensions: Vec<ExtensionRecord>,
    pub issues: Vec<MappingIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDescriptor {
    pub module_id: String,
    pub module_version: ContractVersion,
    pub source_api_version: ContractVersion,
    pub mapping_version: ContractVersion,
    pub provided_capabilities: Vec<CapabilityId>,
    pub extension_contracts: Vec<ExtensionRequirement>,
    pub localization_namespace: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceValidation {
    pub valid: bool,
    pub issues: Vec<MappingIssue>,
    pub source_module_id: String,
    pub source_api_version: ContractVersion,
    pub logical_snapshot_key: String,
    pub schema_fingerprint: String,
    pub mapping_version: ContractVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionRequirement {
    pub namespace: String,
    pub contract_version: ContractVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocaleBundle {
    pub locale: String,
    pub namespace: String,
    pub messages: BTreeMap<String, String>,
}

impl LocaleBundle {
    pub fn message(&self, key: &str) -> Option<&str> {
        self.messages.get(key).map(String::as_str)
    }
}

impl TryFrom<&str> for ExtensionRequirement {
    type Error = ContractError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (namespace, version) = value
            .rsplit_once('@')
            .ok_or_else(|| ContractError::new("invalid_extension_requirement", value))?;
        if namespace.trim().is_empty() {
            return Err(ContractError::new(
                "invalid_extension_requirement",
                "extension namespace cannot be blank",
            ));
        }
        Ok(Self {
            namespace: namespace.to_owned(),
            contract_version: ContractVersion::from_str(version)?,
        })
    }
}

#[allow(dead_code)]
fn _assert_capability_is_displayable(value: &CapabilityId) -> impl fmt::Display + '_ {
    value
}
