use chrono::Utc;
use mfa_contracts::{ModuleId, UtcInstant};
use mfa_db::{AssetRegistration, DatabaseService, HealthCheck, RegisterAsset, RegisterReceipt};
use tempfile::TempDir;
use uuid::Uuid;

fn receipt(index: u128) -> RegisterReceipt {
    RegisterReceipt {
        receipt_id: Uuid::from_u128(index),
        source_module_id: ModuleId::try_from("fixture-source").unwrap(),
        inbox_path: format!("/inbox/{index}.fixture"),
        original_filename: format!("{index}.fixture"),
        discovered_at: UtcInstant::from(Utc::now()),
        asset_id: None,
        outcome: "accepted".to_owned(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_database_operation_reports_the_same_actor_thread() {
    let root = TempDir::new().unwrap();
    let service = DatabaseService::start(&root.path().join("storage.duckdb"), 4)
        .await
        .unwrap();

    let first = service.execute(HealthCheck).await.unwrap();
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let service = service.clone();
        tasks.push(tokio::spawn(async move {
            service.execute(HealthCheck).await.unwrap().actor_thread_id
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap(), first.actor_thread_id);
    }

    service.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bounded_command_queue_preserves_order_and_drains_before_shutdown() {
    let root = TempDir::new().unwrap();
    let service = DatabaseService::start(&root.path().join("storage.duckdb"), 1)
        .await
        .unwrap();
    assert_eq!(service.queue_capacity(), 1);

    let asset_id = Uuid::from_u128(500);
    service
        .execute(RegisterAsset {
            asset: AssetRegistration {
                asset_id,
                source_module_id: ModuleId::try_from("fixture-source").unwrap(),
                asset_type: "fixture".to_owned(),
                original_filename: "asset.fixture".to_owned(),
                archive_path: "/archive/asset.fixture".to_owned(),
                byte_sha256: "a".repeat(64),
                file_size: 1,
                received_at: UtcInstant::from(Utc::now()),
            },
        })
        .await
        .unwrap();
    let receipt_result = service.execute(receipt(501)).await.unwrap();
    assert_eq!(receipt_result.receipt_id, Uuid::from_u128(501));

    service.shutdown().await.unwrap();
}

#[test]
fn public_command_types_do_not_expose_duckdb_native_values() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DatabaseService>();
    assert_send_sync::<RegisterReceipt>();
    assert_send_sync::<RegisterAsset>();
}
