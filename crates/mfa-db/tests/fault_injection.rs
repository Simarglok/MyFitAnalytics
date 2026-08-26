use chrono::{NaiveDate, NaiveDateTime, Utc};
use mfa_contracts::{CanonicalObservation, LocalDate, LocalDateTime, ModuleId, NutritionItem};
use mfa_db::{
    CommitSnapshot, DatabaseFailurePoint, DatabaseService, LineageLink, LogicalSnapshotKey,
    QueryView, SourceRecord, TestDatabaseFaultInjector, ValidatedSnapshotBatch,
};
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

fn attempt(number: u128) -> mfa_db::AttemptIdentity {
    mfa_db::AttemptIdentity {
        attempt_id: Uuid::from_u128(number),
        asset_id: Uuid::from_u128(10_000 + number),
        source_module_id: ModuleId::try_from("fixture-source").unwrap(),
        source_module_version: "1.0.0".to_owned(),
        source_module_package_sha256: "f".repeat(64),
        source_api_version: "1.0.0".to_owned(),
        mapping_version: "1.0.0".to_owned(),
        schema_fingerprint: "schema-v1".to_owned(),
        logical_snapshot_key: LogicalSnapshotKey::new("fixture:2026").unwrap(),
        started_at: mfa_contracts::UtcInstant::from(Utc::now()),
    }
}

fn batch(attempt: mfa_db::AttemptIdentity, name: &str) -> ValidatedSnapshotBatch {
    let source_record_id = format!("{}-record", attempt.attempt_id);
    let observation = CanonicalObservation::NutritionItem(NutritionItem {
        nutrition_item_id: Uuid::from_u128(attempt.attempt_id.as_u128() + 100),
        occurred_local_at: Some(LocalDateTime::from(
            NaiveDateTime::parse_from_str("2026-01-02T12:00:00", "%Y-%m-%dT%H:%M:%S").unwrap(),
        )),
        local_date: LocalDate::from(NaiveDate::from_ymd_opt(2026, 1, 2).unwrap()),
        meal: "Lunch".to_owned(),
        food_source_id: "food".to_owned(),
        name: name.to_owned(),
        amount_raw: "1 serving".to_owned(),
        calories_kcal: Some(100.0),
        protein_g: None,
        fat_g: None,
        carbs_g: None,
        fiber_g: None,
        sugars_g: None,
        sodium_mg: None,
        source_record_id: Some(source_record_id.clone()),
    });
    ValidatedSnapshotBatch {
        logical_key: attempt.logical_snapshot_key.clone(),
        attempt: attempt.clone(),
        source_records: vec![SourceRecord {
            source_record_id,
            sheet_name: Some("Food".to_owned()),
            source_row_number: 1,
            source_record_key: format!("{}:Food:1", attempt.asset_id),
            raw_payload: serde_json::json!({"name": name}),
        }],
        observations: vec![observation.clone()],
        extensions: Vec::new(),
        lineage: vec![LineageLink::for_observation(&observation, "1.0.0".to_owned()).unwrap()],
        issues: Vec::new(),
    }
}

async fn prepare(service: &DatabaseService, attempt: &mfa_db::AttemptIdentity) {
    service
        .execute(mfa_db::RegisterAsset {
            asset: mfa_db::AssetRegistration {
                asset_id: attempt.asset_id,
                source_module_id: attempt.source_module_id.clone(),
                asset_type: "fixture".to_owned(),
                original_filename: "fixture".to_owned(),
                archive_path: format!("/archive/{}", attempt.asset_id),
                byte_sha256: format!("{:064x}", attempt.asset_id.as_u128()),
                file_size: 1,
                received_at: attempt.started_at.clone(),
            },
        })
        .await
        .unwrap();
    service.execute(attempt.start_command()).await.unwrap();
}

#[tokio::test]
async fn active_snapshot_failure_keeps_the_previous_snapshot_visible() {
    let root = TempDir::new().unwrap();
    let injector = TestDatabaseFaultInjector::default();
    let service = DatabaseService::start_with_fault_injector(
        &root.path().join("storage.duckdb"),
        8,
        Arc::new(injector.clone()),
    )
    .await
    .unwrap();
    let first = attempt(1);
    prepare(&service, &first).await;
    let first_result = service
        .execute(CommitSnapshot(Arc::new(batch(first.clone(), "old"))))
        .await
        .unwrap();

    let second = attempt(2);
    prepare(&service, &second).await;
    injector.fail_once(DatabaseFailurePoint::ActiveSwitch);
    let error = service
        .execute(CommitSnapshot(Arc::new(batch(second, "new"))))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "database_fault_injected");

    let view = service
        .execute(QueryView::active_snapshot(first.logical_snapshot_key))
        .await
        .unwrap();
    assert_eq!(view.snapshot_id, Some(first_result.snapshot_id));
    assert_eq!(view.counts.total, 1);
    service.shutdown().await.unwrap();
}
