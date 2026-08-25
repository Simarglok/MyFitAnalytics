use crate::command::StartAttempt;
use crate::error::DatabaseError;
use mfa_contracts::{CanonicalObservation, CapabilityId, ModuleId, UtcInstant};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LogicalSnapshotKey(String);

impl LogicalSnapshotKey {
    pub fn new(value: impl Into<String>) -> Result<Self, DatabaseError> {
        let value = value.into();
        if value.trim().is_empty() || value.contains('\0') {
            return Err(DatabaseError::Command {
                detail: "logical snapshot key cannot be blank or contain NUL".to_owned(),
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LogicalSnapshotKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for LogicalSnapshotKey {
    type Err = DatabaseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptIdentity {
    pub attempt_id: Uuid,
    pub asset_id: Uuid,
    pub source_module_id: ModuleId,
    pub source_module_version: String,
    pub source_module_package_sha256: String,
    pub source_api_version: String,
    pub mapping_version: String,
    pub schema_fingerprint: String,
    pub logical_snapshot_key: LogicalSnapshotKey,
    pub started_at: UtcInstant,
}

impl AttemptIdentity {
    pub fn start_command(&self) -> StartAttempt {
        StartAttempt {
            attempt_id: self.attempt_id,
            asset_id: self.asset_id,
            source_module_id: self.source_module_id.clone(),
            source_module_version: self.source_module_version.clone(),
            source_module_package_sha256: self.source_module_package_sha256.clone(),
            source_api_version: self.source_api_version.clone(),
            mapping_version: self.mapping_version.clone(),
            schema_fingerprint: self.schema_fingerprint.clone(),
            logical_snapshot_key: self.logical_snapshot_key.to_string(),
            started_at: self.started_at.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceRecord {
    pub source_record_id: String,
    pub sheet_name: Option<String>,
    pub source_row_number: u32,
    pub source_record_key: String,
    pub raw_payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageLink {
    pub canonical_entity_type: String,
    pub canonical_entity_id: String,
    pub source_record_id: String,
    pub mapping_version: String,
}

impl LineageLink {
    pub fn for_observation(
        observation: &CanonicalObservation,
        mapping_version: String,
    ) -> Result<Self, crate::validation::ValidationError> {
        let (canonical_entity_type, canonical_entity_id, source_record_id) =
            canonical_identity(observation)?;
        Ok(Self {
            canonical_entity_type,
            canonical_entity_id,
            source_record_id,
            mapping_version,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionRecord {
    pub extension_record_id: String,
    pub source_record_id: String,
    pub source_module_id: ModuleId,
    pub contract_id: String,
    pub contract_version: String,
    pub occurred_local_at: Option<mfa_contracts::LocalDateTime>,
    pub local_date: Option<mfa_contracts::LocalDate>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataQualityItem {
    pub data_quality_item_id: String,
    pub item_type: String,
    pub source_asset_id: Option<Uuid>,
    pub source_record_id: Option<String>,
    pub severity: String,
    pub message: String,
    pub status: String,
    pub created_at: UtcInstant,
    pub resolved_at: Option<UtcInstant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatedSnapshotBatch {
    pub logical_key: LogicalSnapshotKey,
    pub attempt: AttemptIdentity,
    pub source_records: Vec<SourceRecord>,
    pub observations: Vec<CanonicalObservation>,
    pub extensions: Vec<ExtensionRecord>,
    pub lineage: Vec<LineageLink>,
    pub issues: Vec<DataQualityItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordCounts {
    pub total: u64,
    pub nutrition_items: u64,
    pub body_measurements: u64,
    pub activity_events: u64,
    pub activity_days: u64,
    pub heart_rate_observations: u64,
    pub workout_sessions: u64,
    pub exercise_sets: u64,
    pub phase_events: u64,
    pub extensions: u64,
    pub quality_items: u64,
}

impl RecordCounts {
    pub fn zero() -> Self {
        Self {
            total: 0,
            nutrition_items: 0,
            body_measurements: 0,
            activity_events: 0,
            activity_days: 0,
            heart_rate_observations: 0,
            workout_sessions: 0,
            exercise_sets: 0,
            phase_events: 0,
            extensions: 0,
            quality_items: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotCommitResult {
    pub snapshot_id: Uuid,
    pub changed_capabilities: Vec<CapabilityId>,
    pub counts: RecordCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionContractRegistration {
    pub contract_id: String,
    pub source_module_id: ModuleId,
    pub namespace: String,
    pub contract_version: String,
    pub payload_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionContractRegistrationResult {
    pub contract_id: String,
}

pub fn canonical_identity(
    observation: &CanonicalObservation,
) -> Result<(String, String, String), crate::validation::ValidationError> {
    let (entity_type, entity_id) = canonical_entity_key(observation);
    let source_record_id = match observation {
        CanonicalObservation::NutritionItem(value) => value.source_record_id.as_deref(),
        CanonicalObservation::BodyMeasurement(value) => value.source_record_id.as_deref(),
        CanonicalObservation::ActivityEvent(value) => value.source_record_id.as_deref(),
        CanonicalObservation::ActivityDay(_)
        | CanonicalObservation::WorkoutSession(_)
        | CanonicalObservation::PhaseEvent(_) => None,
        CanonicalObservation::HeartRate(value) => value.source_record_id.as_deref(),
        CanonicalObservation::ExerciseSet(value) => value.source_record_id.as_deref(),
    };
    source_record_id
        .map(|source_record_id| {
            (
                entity_type.clone(),
                entity_id.clone(),
                source_record_id.to_owned(),
            )
        })
        .ok_or(crate::validation::ValidationError::MissingSourceRecord {
            entity_type,
            entity_id,
        })
}

pub fn canonical_entity_key(observation: &CanonicalObservation) -> (String, String) {
    match observation {
        CanonicalObservation::NutritionItem(value) => (
            "nutrition_item".to_owned(),
            value.nutrition_item_id.to_string(),
        ),
        CanonicalObservation::BodyMeasurement(value) => (
            "body_measurement".to_owned(),
            value.body_measurement_id.to_string(),
        ),
        CanonicalObservation::ActivityEvent(value) => (
            "activity_event".to_owned(),
            value.activity_event_id.to_string(),
        ),
        CanonicalObservation::ActivityDay(value) => {
            ("activity_day".to_owned(), value.local_date.to_string())
        }
        CanonicalObservation::HeartRate(value) => (
            "heart_rate_observation".to_owned(),
            value.heart_rate_observation_id.to_string(),
        ),
        CanonicalObservation::WorkoutSession(value) => (
            "workout_session".to_owned(),
            value.workout_session_id.to_string(),
        ),
        CanonicalObservation::ExerciseSet(value) => {
            ("exercise_set".to_owned(), value.exercise_set_id.to_string())
        }
        CanonicalObservation::PhaseEvent(value) => {
            ("phase_event".to_owned(), value.phase_event_id.to_string())
        }
    }
}
