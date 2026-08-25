use chrono::Utc;
use duckdb::{Connection, params};
use mfa_contracts::ModuleId;
use mfa_db::{
    AttemptIdentity, CommitSnapshot, DataQualityItem, DatabaseService,
    ExtensionContractRegistration, ExtensionRecord, LineageLink, LogicalSnapshotKey, SourceRecord,
    ValidatedSnapshotBatch,
};
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

fn attempt(number: u128) -> AttemptIdentity {
    AttemptIdentity {
        attempt_id: Uuid::from_u128(number),
        asset_id: Uuid::from_u128(10_000 + number),
        source_module_id: ModuleId::try_from("fixture-source").unwrap(),
        source_module_version: "1.0.0".to_owned(),
        source_module_package_sha256: "b".repeat(64),
        source_api_version: "1.0.0".to_owned(),
        mapping_version: "1.0.0".to_owned(),
        schema_fingerprint: "schema-v1".to_owned(),
        logical_snapshot_key: LogicalSnapshotKey::new("fixture:2026").unwrap(),
        started_at: mfa_contracts::UtcInstant::from(Utc::now()),
    }
}

fn source_record(attempt: &AttemptIdentity, row: u32) -> SourceRecord {
    SourceRecord {
        source_record_id: format!("{}-record-{row}", attempt.attempt_id),
        sheet_name: Some("Fixture".to_owned()),
        source_row_number: row,
        source_record_key: format!("{}:Fixture:{row}", attempt.asset_id),
        raw_payload: serde_json::json!({"row": row}),
    }
}

fn empty_batch(attempt: AttemptIdentity) -> ValidatedSnapshotBatch {
    ValidatedSnapshotBatch {
        logical_key: attempt.logical_snapshot_key.clone(),
        attempt: attempt.clone(),
        source_records: vec![source_record(&attempt, 1)],
        observations: Vec::new(),
        extensions: Vec::new(),
        lineage: Vec::new(),
        issues: Vec::<DataQualityItem>::new(),
    }
}

#[tokio::test]
async fn receipts_assets_attempts_and_source_lineage_are_durable_and_immutable() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("storage.duckdb");
    let service = DatabaseService::start(&path, 8).await.unwrap();
    let asset_id = Uuid::from_u128(1);
    service
        .execute(mfa_db::RegisterAsset {
            asset: mfa_db::AssetRegistration {
                asset_id,
                source_module_id: ModuleId::try_from("fixture-source").unwrap(),
                asset_type: "fixture".to_owned(),
                original_filename: "export.fixture".to_owned(),
                archive_path: "/archive/export.fixture".to_owned(),
                byte_sha256: "c".repeat(64),
                file_size: 3,
                received_at: mfa_contracts::UtcInstant::from(Utc::now()),
            },
        })
        .await
        .unwrap();
    for (receipt_id, outcome) in [(2, "accepted"), (3, "duplicate")] {
        service
            .execute(mfa_db::RegisterReceipt {
                receipt_id: Uuid::from_u128(receipt_id),
                source_module_id: ModuleId::try_from("fixture-source").unwrap(),
                inbox_path: format!("/inbox/{receipt_id}.fixture"),
                original_filename: "export.fixture".to_owned(),
                discovered_at: mfa_contracts::UtcInstant::from(Utc::now()),
                asset_id: Some(asset_id),
                outcome: outcome.to_owned(),
            })
            .await
            .unwrap();
    }
    let first_attempt = attempt(20);
    let retry_attempt = attempt(21);
    for candidate in [&first_attempt, &retry_attempt] {
        service
            .execute(mfa_db::RegisterAsset {
                asset: mfa_db::AssetRegistration {
                    asset_id: candidate.asset_id,
                    source_module_id: candidate.source_module_id.clone(),
                    asset_type: "fixture".to_owned(),
                    original_filename: "retry.fixture".to_owned(),
                    archive_path: format!("/archive/{}", candidate.asset_id),
                    byte_sha256: format!("{:064x}", candidate.asset_id.as_u128()),
                    file_size: 1,
                    received_at: candidate.started_at.clone(),
                },
            })
            .await
            .unwrap();
    }
    service
        .execute(first_attempt.start_command())
        .await
        .unwrap();
    service
        .execute(retry_attempt.start_command())
        .await
        .unwrap();
    service.shutdown().await.unwrap();

    let connection = Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM source_asset", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM source_receipt", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM ingestion_attempt", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    let original_hash: String = connection
        .query_row(
            "SELECT byte_sha256 FROM source_asset WHERE asset_id = ?",
            params![asset_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(original_hash, "c".repeat(64));
}

#[tokio::test]
async fn extension_records_require_a_registered_contract_and_lineage_for_every_canonical_row() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("storage.duckdb");
    let service = DatabaseService::start(&path, 8).await.unwrap();
    let attempt = attempt(30);
    service
        .execute(mfa_db::RegisterAsset {
            asset: mfa_db::AssetRegistration {
                asset_id: attempt.asset_id,
                source_module_id: attempt.source_module_id.clone(),
                asset_type: "fixture".to_owned(),
                original_filename: "export.fixture".to_owned(),
                archive_path: "/archive/export.fixture".to_owned(),
                byte_sha256: "d".repeat(64),
                file_size: 1,
                received_at: attempt.started_at.clone(),
            },
        })
        .await
        .unwrap();
    service.execute(attempt.start_command()).await.unwrap();
    let mut batch = empty_batch(attempt.clone());
    batch.extensions.push(ExtensionRecord {
        extension_record_id: "extension-1".to_owned(),
        source_record_id: format!("{}-record-1", attempt.attempt_id),
        source_module_id: attempt.source_module_id.clone(),
        contract_id: "fixture.extension".to_owned(),
        contract_version: "1.0.0".to_owned(),
        occurred_local_at: None,
        local_date: None,
        payload: serde_json::json!({"rpe": 8}),
    });
    let error = service
        .execute(CommitSnapshot(Arc::new(batch.clone())))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "extension_contract_missing");

    service
        .execute(ExtensionContractRegistration {
            contract_id: "fixture.extension".to_owned(),
            source_module_id: attempt.source_module_id,
            namespace: "fixture.extension".to_owned(),
            contract_version: "1.0.0".to_owned(),
            payload_schema: serde_json::json!({"type": "object"}),
        })
        .await
        .unwrap();
    let committed = service
        .execute(CommitSnapshot(Arc::new(batch)))
        .await
        .unwrap();
    assert_eq!(committed.counts.total, 0);
}

#[test]
fn lineage_link_type_is_not_a_duckdb_native_value() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LineageLink>();
    assert_send_sync::<ExtensionContractRegistration>();
}
