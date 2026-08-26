use chrono::Utc;
use mfa_contracts::{CanonicalObservation, ModuleId, NutritionItem, UtcInstant};
use mfa_db::{
    AttemptIdentity, LineageLink, LogicalSnapshotKey, SourceRecord, ValidatedSnapshotBatch,
};
use serde_json::json;
use uuid::Uuid;

fn attempt() -> AttemptIdentity {
    AttemptIdentity {
        attempt_id: Uuid::from_u128(500),
        asset_id: Uuid::from_u128(501),
        source_module_id: ModuleId::try_from("fixture-source").unwrap(),
        source_module_version: "1.0.0".to_owned(),
        source_module_package_sha256: "a".repeat(64),
        source_api_version: "1.0.0".to_owned(),
        mapping_version: "1.0.0".to_owned(),
        schema_fingerprint: "schema".to_owned(),
        logical_snapshot_key: LogicalSnapshotKey::new("fixture:2026").unwrap(),
        started_at: UtcInstant::from(Utc::now()),
    }
}

fn batch(observation: CanonicalObservation, source_record_id: &str) -> ValidatedSnapshotBatch {
    let attempt = attempt();
    ValidatedSnapshotBatch {
        logical_key: attempt.logical_snapshot_key.clone(),
        attempt,
        source_records: vec![SourceRecord {
            source_record_id: source_record_id.to_owned(),
            sheet_name: Some("Sheet".to_owned()),
            source_row_number: 1,
            source_record_key: "asset:Sheet:1".to_owned(),
            raw_payload: json!({"value": 1}),
        }],
        observations: vec![observation],
        extensions: Vec::new(),
        lineage: vec![LineageLink {
            canonical_entity_type: "nutrition_item".to_owned(),
            canonical_entity_id: Uuid::from_u128(600).to_string(),
            source_record_id: source_record_id.to_owned(),
            mapping_version: "1.0.0".to_owned(),
        }],
        issues: Vec::new(),
    }
}

#[test]
fn negative_nutrition_values_are_rejected_before_sql() {
    let observation = CanonicalObservation::NutritionItem(NutritionItem {
        nutrition_item_id: Uuid::from_u128(600),
        occurred_local_at: None,
        local_date: "2026-01-01".parse().unwrap(),
        meal: "Lunch".to_owned(),
        food_source_id: "food".to_owned(),
        name: "Impossible food".to_owned(),
        amount_raw: "1".to_owned(),
        calories_kcal: Some(-1.0),
        protein_g: None,
        fat_g: None,
        carbs_g: None,
        fiber_g: None,
        sugars_g: None,
        sodium_mg: None,
        source_record_id: Some("record-1".to_owned()),
    });
    let error = mfa_db::validation::validate_batch(&batch(observation, "record-1")).unwrap_err();
    assert_eq!(error.code(), "invalid_domain_value");
}

#[test]
fn blank_source_identity_is_rejected_before_sql() {
    let observation = CanonicalObservation::NutritionItem(NutritionItem {
        nutrition_item_id: Uuid::from_u128(600),
        occurred_local_at: None,
        local_date: "2026-01-01".parse().unwrap(),
        meal: "Lunch".to_owned(),
        food_source_id: "food".to_owned(),
        name: "Missing source".to_owned(),
        amount_raw: "1".to_owned(),
        calories_kcal: Some(1.0),
        protein_g: None,
        fat_g: None,
        carbs_g: None,
        fiber_g: None,
        sugars_g: None,
        sodium_mg: None,
        source_record_id: Some("".to_owned()),
    });
    let error = mfa_db::validation::validate_batch(&batch(observation, "")).unwrap_err();
    assert_eq!(error.code(), "blank_source_identity");
}
