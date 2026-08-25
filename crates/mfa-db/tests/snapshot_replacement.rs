use chrono::{NaiveDate, NaiveDateTime, Utc};
use duckdb::{Connection, params};
use mfa_contracts::{CanonicalObservation, LocalDate, LocalDateTime, ModuleId, NutritionItem};
use mfa_db::{
    CommitSnapshot, DatabaseService, LineageLink, LogicalSnapshotKey, QueryView, SourceRecord,
    ValidatedSnapshotBatch,
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
        source_module_package_sha256: "e".repeat(64),
        source_api_version: "1.0.0".to_owned(),
        mapping_version: "1.0.0".to_owned(),
        schema_fingerprint: "schema-v1".to_owned(),
        logical_snapshot_key: LogicalSnapshotKey::new("fixture:2026").unwrap(),
        started_at: mfa_contracts::UtcInstant::from(Utc::now()),
    }
}

fn nutrition(id: u128, source_record_id: &str, name: &str) -> CanonicalObservation {
    CanonicalObservation::NutritionItem(NutritionItem {
        nutrition_item_id: Uuid::from_u128(id),
        occurred_local_at: Some(LocalDateTime::from(
            NaiveDateTime::parse_from_str("2026-01-02T12:00:00", "%Y-%m-%dT%H:%M:%S").unwrap(),
        )),
        local_date: LocalDate::from(NaiveDate::from_ymd_opt(2026, 1, 2).unwrap()),
        meal: "Lunch".to_owned(),
        food_source_id: "food-1".to_owned(),
        name: name.to_owned(),
        amount_raw: "1 serving".to_owned(),
        calories_kcal: Some(500.0),
        protein_g: Some(30.0),
        fat_g: Some(20.0),
        carbs_g: Some(40.0),
        fiber_g: None,
        sugars_g: None,
        sodium_mg: None,
        source_record_id: Some(source_record_id.to_owned()),
    })
}

fn batch(attempt: mfa_db::AttemptIdentity, names: &[&str]) -> ValidatedSnapshotBatch {
    let source_records: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(index, _)| SourceRecord {
            source_record_id: format!("{}-record-{index}", attempt.attempt_id),
            sheet_name: Some("Food".to_owned()),
            source_row_number: (index + 1) as u32,
            source_record_key: format!("{}:Food:{}", attempt.asset_id, index + 1),
            raw_payload: serde_json::json!({"row": index + 1}),
        })
        .collect();
    let observations: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            nutrition(
                100 + index as u128,
                &format!("{}-record-{index}", attempt.attempt_id),
                name,
            )
        })
        .collect();
    let lineage = observations
        .iter()
        .map(|observation| LineageLink::for_observation(observation, "1.0.0".to_owned()).unwrap())
        .collect();
    ValidatedSnapshotBatch {
        logical_key: attempt.logical_snapshot_key.clone(),
        attempt,
        source_records,
        observations,
        extensions: Vec::new(),
        lineage,
        issues: Vec::new(),
    }
}

async fn start_attempt(service: &DatabaseService, attempt: &mfa_db::AttemptIdentity) {
    service
        .execute(mfa_db::RegisterAsset {
            asset: mfa_db::AssetRegistration {
                asset_id: attempt.asset_id,
                source_module_id: attempt.source_module_id.clone(),
                asset_type: "fixture".to_owned(),
                original_filename: "export.fixture".to_owned(),
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
async fn a_new_snapshot_replaces_only_the_active_logical_view_and_preserves_history() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("storage.duckdb");
    let service = DatabaseService::start(&path, 8).await.unwrap();
    let first = attempt(1);
    let second = attempt(2);
    start_attempt(&service, &first).await;
    let first_result = service
        .execute(CommitSnapshot(Arc::new(batch(first.clone(), &["old"]))))
        .await
        .unwrap();
    start_attempt(&service, &second).await;
    let second_result = service
        .execute(CommitSnapshot(Arc::new(batch(second.clone(), &["new"]))))
        .await
        .unwrap();

    let view = service
        .execute(QueryView::active_snapshot(
            first.logical_snapshot_key.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(view.snapshot_id, Some(second_result.snapshot_id));
    assert_eq!(view.counts.total, 1);
    assert!(first_result.snapshot_id != second_result.snapshot_id);
    service.shutdown().await.unwrap();

    let connection = Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM nutrition_item WHERE logical_snapshot_key = ?",
                params![first.logical_snapshot_key.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        2
    );
    let active_name: String = connection
        .query_row("SELECT name FROM active_nutrition_items", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(active_name, "new");
}

#[tokio::test]
async fn identical_rows_keep_multiplicity_and_invalid_snapshot_does_not_switch_active_state() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("storage.duckdb");
    let service = DatabaseService::start(&path, 8).await.unwrap();
    let first = attempt(11);
    start_attempt(&service, &first).await;
    service
        .execute(CommitSnapshot(Arc::new(batch(
            first.clone(),
            &["same", "same"],
        ))))
        .await
        .unwrap();
    let active_before = service
        .execute(QueryView::active_snapshot(
            first.logical_snapshot_key.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(active_before.counts.total, 2);

    let invalid = attempt(12);
    start_attempt(&service, &invalid).await;
    let invalid_batch = batch(invalid.clone(), &["invalid"]);
    let mut invalid_batch = invalid_batch;
    invalid_batch.observations[0] = nutrition(999, "missing-record", "invalid");
    let error = service
        .execute(CommitSnapshot(Arc::new(invalid_batch)))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "lineage_missing");
    let active_after = service
        .execute(QueryView::active_snapshot(first.logical_snapshot_key))
        .await
        .unwrap();
    assert_eq!(active_after.snapshot_id, active_before.snapshot_id);
    assert_eq!(active_after.counts.total, 2);
    service.shutdown().await.unwrap();
}
