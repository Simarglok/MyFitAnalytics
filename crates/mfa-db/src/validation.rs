use crate::provenance::{
    DataQualityItem, RecordCounts, ValidatedSnapshotBatch, canonical_entity_key, canonical_identity,
};
use mfa_contracts::CanonicalObservation;
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValidationError {
    #[error("snapshot logical key does not match attempt")]
    LogicalKeyMismatch,
    #[error("source record id is duplicated: {id}")]
    DuplicateSourceRecord { id: String },
    #[error("source record key is duplicated: {key}")]
    DuplicateSourceRecordKey { key: String },
    #[error("source record row number must be positive: {id}")]
    InvalidSourceRow { id: String },
    #[error("source record identity cannot be blank")]
    BlankSourceIdentity,
    #[error("canonical entity id is duplicated: {entity_type}:{entity_id}")]
    DuplicateCanonicalEntity {
        entity_type: String,
        entity_id: String,
    },
    #[error("canonical entity has no source record: {entity_type}:{entity_id}")]
    MissingSourceRecord {
        entity_type: String,
        entity_id: String,
    },
    #[error("canonical entity has no lineage: {entity_type}:{entity_id}")]
    MissingLineage {
        entity_type: String,
        entity_id: String,
    },
    #[error("lineage references an unknown source record: {source_record_id}")]
    UnknownLineageSource { source_record_id: String },
    #[error("lineage mapping version is blank")]
    BlankLineageMapping,
    #[error("canonical field is not finite: {field}")]
    NonFinite { field: String },
    #[error("canonical field is outside its domain: {field}")]
    InvalidDomain { field: String },
    #[error("extension record is invalid: {detail}")]
    InvalidExtension { detail: String },
    #[error("data-quality item is invalid: {detail}")]
    InvalidQualityItem { detail: String },
}

impl ValidationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::LogicalKeyMismatch => "logical_key_mismatch",
            Self::DuplicateSourceRecord { .. } => "duplicate_source_record",
            Self::DuplicateSourceRecordKey { .. } => "duplicate_source_record_key",
            Self::InvalidSourceRow { .. } => "invalid_source_row",
            Self::BlankSourceIdentity => "blank_source_identity",
            Self::DuplicateCanonicalEntity { .. } => "duplicate_canonical_entity",
            Self::MissingSourceRecord { .. } => "lineage_missing_source_record",
            Self::MissingLineage { .. } => "lineage_missing",
            Self::UnknownLineageSource { .. } => "lineage_unknown_source",
            Self::BlankLineageMapping => "lineage_blank_mapping",
            Self::NonFinite { .. } => "non_finite_value",
            Self::InvalidDomain { .. } => "invalid_domain_value",
            Self::InvalidExtension { .. } => "invalid_extension",
            Self::InvalidQualityItem { .. } => "invalid_quality_item",
        }
    }
}

pub fn validate_batch(batch: &ValidatedSnapshotBatch) -> Result<(), ValidationError> {
    if batch.logical_key != batch.attempt.logical_snapshot_key {
        return Err(ValidationError::LogicalKeyMismatch);
    }
    let mut source_ids = BTreeSet::new();
    let mut source_keys = BTreeSet::new();
    for source in &batch.source_records {
        if source.source_record_id.trim().is_empty() || source.source_record_key.trim().is_empty() {
            return Err(ValidationError::BlankSourceIdentity);
        }
        if source.source_row_number == 0 {
            return Err(ValidationError::InvalidSourceRow {
                id: source.source_record_id.clone(),
            });
        }
        if !source_ids.insert(&source.source_record_id) {
            return Err(ValidationError::DuplicateSourceRecord {
                id: source.source_record_id.clone(),
            });
        }
        if !source_keys.insert(&source.source_record_key) {
            return Err(ValidationError::DuplicateSourceRecordKey {
                key: source.source_record_key.clone(),
            });
        }
    }

    let mut entities = BTreeSet::new();
    for observation in &batch.observations {
        validate_observation_values(observation)?;
        let (entity_type, entity_id) = canonical_entity_key(observation);
        if !entities.insert((entity_type.clone(), entity_id.clone())) {
            return Err(ValidationError::DuplicateCanonicalEntity {
                entity_type,
                entity_id,
            });
        }
        if let Ok((entity_type, entity_id, source_record_id)) = canonical_identity(observation)
            && !source_ids.contains(&source_record_id)
        {
            return Err(ValidationError::MissingLineage {
                entity_type,
                entity_id,
            });
        }
    }

    let mut lineages = BTreeSet::new();
    for lineage in &batch.lineage {
        if lineage.mapping_version.trim().is_empty() {
            return Err(ValidationError::BlankLineageMapping);
        }
        if !source_ids.contains(&lineage.source_record_id) {
            return Err(ValidationError::UnknownLineageSource {
                source_record_id: lineage.source_record_id.clone(),
            });
        }
        lineages.insert((
            lineage.canonical_entity_type.clone(),
            lineage.canonical_entity_id.clone(),
            lineage.source_record_id.clone(),
        ));
    }
    for (entity_type, entity_id) in entities {
        if !lineages
            .iter()
            .any(|(kind, id, _)| kind == &entity_type && id == &entity_id)
        {
            return Err(ValidationError::MissingLineage {
                entity_type,
                entity_id,
            });
        }
    }

    for extension in &batch.extensions {
        if extension.extension_record_id.trim().is_empty()
            || extension.source_record_id.trim().is_empty()
            || extension.contract_id.trim().is_empty()
            || extension.contract_version.trim().is_empty()
        {
            return Err(ValidationError::InvalidExtension {
                detail: "extension identity and contract fields are required".to_owned(),
            });
        }
        if !source_ids.contains(&extension.source_record_id) {
            return Err(ValidationError::UnknownLineageSource {
                source_record_id: extension.source_record_id.clone(),
            });
        }
    }

    for issue in &batch.issues {
        validate_quality_item(issue)?;
    }
    Ok(())
}

pub fn record_counts(batch: &ValidatedSnapshotBatch) -> RecordCounts {
    let mut counts = RecordCounts::zero();
    counts.total = batch.observations.len() as u64;
    counts.extensions = batch.extensions.len() as u64;
    counts.quality_items = batch.issues.len() as u64;
    for observation in &batch.observations {
        match observation {
            CanonicalObservation::NutritionItem(_) => counts.nutrition_items += 1,
            CanonicalObservation::BodyMeasurement(_) => counts.body_measurements += 1,
            CanonicalObservation::ActivityEvent(_) => counts.activity_events += 1,
            CanonicalObservation::ActivityDay(_) => counts.activity_days += 1,
            CanonicalObservation::HeartRate(_) => counts.heart_rate_observations += 1,
            CanonicalObservation::WorkoutSession(_) => counts.workout_sessions += 1,
            CanonicalObservation::ExerciseSet(_) => counts.exercise_sets += 1,
            CanonicalObservation::PhaseEvent(_) => counts.phase_events += 1,
        }
    }
    counts
}

fn validate_observation_values(observation: &CanonicalObservation) -> Result<(), ValidationError> {
    let values: &[(&str, Option<f64>)] = match observation {
        CanonicalObservation::NutritionItem(value) => &[
            ("calories_kcal", value.calories_kcal),
            ("protein_g", value.protein_g),
            ("fat_g", value.fat_g),
            ("carbs_g", value.carbs_g),
            ("fiber_g", value.fiber_g),
            ("sugars_g", value.sugars_g),
            ("sodium_mg", value.sodium_mg),
        ],
        CanonicalObservation::BodyMeasurement(value) => &[
            ("weight_kg", Some(value.weight_kg)),
            ("body_fat_pct", value.body_fat_pct),
        ],
        CanonicalObservation::ActivityEvent(value) => &[
            ("distance_km", value.distance_km),
            ("estimated_calories_kcal", value.estimated_calories_kcal),
        ],
        CanonicalObservation::ActivityDay(value) => &[
            ("water_ml", value.water_ml),
            ("activity_distance_km", Some(value.activity_distance_km)),
            (
                "estimated_activity_calories_kcal",
                Some(value.estimated_activity_calories_kcal),
            ),
        ],
        CanonicalObservation::HeartRate(value) => &[("heart_rate_bpm", Some(value.heart_rate_bpm))],
        CanonicalObservation::WorkoutSession(_) => &[],
        CanonicalObservation::ExerciseSet(value) => {
            &[("weight_kg", value.weight_kg), ("rpe", value.rpe)]
        }
        CanonicalObservation::PhaseEvent(_) => &[],
    };
    for (field, value) in values {
        if value.is_some_and(|value| !value.is_finite()) {
            return Err(ValidationError::NonFinite {
                field: (*field).to_owned(),
            });
        }
    }
    match observation {
        CanonicalObservation::NutritionItem(value)
            if [
                value.calories_kcal,
                value.protein_g,
                value.fat_g,
                value.carbs_g,
                value.fiber_g,
                value.sugars_g,
                value.sodium_mg,
            ]
            .into_iter()
            .flatten()
            .any(|value| value < 0.0) =>
        {
            Err(ValidationError::InvalidDomain {
                field: "nutrition_value".to_owned(),
            })
        }
        CanonicalObservation::BodyMeasurement(value) if value.weight_kg <= 0.0 => {
            Err(ValidationError::InvalidDomain {
                field: "weight_kg".to_owned(),
            })
        }
        CanonicalObservation::BodyMeasurement(value)
            if value
                .body_fat_pct
                .is_some_and(|value| !(0.0..=100.0).contains(&value)) =>
        {
            Err(ValidationError::InvalidDomain {
                field: "body_fat_pct".to_owned(),
            })
        }
        CanonicalObservation::HeartRate(value) if value.heart_rate_bpm <= 0.0 => {
            Err(ValidationError::InvalidDomain {
                field: "heart_rate_bpm".to_owned(),
            })
        }
        CanonicalObservation::ActivityEvent(value)
            if value.distance_km.is_some_and(|value| value < 0.0)
                || value
                    .estimated_calories_kcal
                    .is_some_and(|value| value < 0.0) =>
        {
            Err(ValidationError::InvalidDomain {
                field: "activity_event".to_owned(),
            })
        }
        CanonicalObservation::ActivityDay(value)
            if value.water_ml.is_some_and(|value| value < 0.0)
                || value.activity_distance_km < 0.0
                || value.estimated_activity_calories_kcal < 0.0 =>
        {
            Err(ValidationError::InvalidDomain {
                field: "activity_day".to_owned(),
            })
        }
        CanonicalObservation::ExerciseSet(value)
            if value.weight_kg.is_some_and(|value| value < 0.0)
                || value.rpe.is_some_and(|value| value < 0.0) =>
        {
            Err(ValidationError::InvalidDomain {
                field: "exercise_set".to_owned(),
            })
        }
        _ => Ok(()),
    }
}

fn validate_quality_item(item: &DataQualityItem) -> Result<(), ValidationError> {
    if item.data_quality_item_id.trim().is_empty()
        || item.item_type.trim().is_empty()
        || item.message.trim().is_empty()
        || !matches!(
            item.severity.as_str(),
            "info" | "warning" | "error" | "critical"
        )
        || !matches!(item.status.as_str(), "open" | "resolved")
    {
        return Err(ValidationError::InvalidQualityItem {
            detail: "quality item fields are invalid".to_owned(),
        });
    }
    Ok(())
}
