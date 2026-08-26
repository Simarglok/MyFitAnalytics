use mfa_archive::{ArchiveCoordinator, ArchiveReconciler, ScanReason, ScanRequest};
use mfa_config::WorkspacePaths;
use mfa_contracts::{ModuleId, UtcInstant};
use mfa_db::{
    DatabaseFailurePoint, DatabaseService, LogicalSnapshotKey, QueryView, TestDatabaseFaultInjector,
};
use mfa_ingestion::{FailurePoint, IngestionCoordinator, IngestionDependencies, TestFaultInjector};
use mfa_module_host::RuntimeLimits;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::Duration;

mod support;
use support::{fake_module, fake_runtime, nutrition_batch};

fn request() -> ScanRequest {
    ScanRequest::new(
        ScanReason::Manual,
        "2026-08-25T00:00:00Z".parse::<UtcInstant>().unwrap(),
    )
}

async fn coordinator(temp: &TempDir) -> (IngestionCoordinator, DatabaseService, ModuleId) {
    let source = ModuleId::try_from("fixture-source").unwrap();
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    workspace.enable_source(&source).unwrap();
    let database = DatabaseService::start(&temp.path().join("storage.duckdb"), 8)
        .await
        .unwrap();
    let dependencies = IngestionDependencies {
        workspace: workspace.clone(),
        source_module: fake_module(temp, source.clone()),
        archive: ArchiveCoordinator::new(workspace, source.clone()),
        database: database.clone(),
        runtime: fake_runtime(nutrition_batch()),
        limits: RuntimeLimits {
            max_memory_bytes: 8 * 1024 * 1024,
            fuel: 1_000_000,
            timeout: Duration::from_secs(1),
            max_output_bytes: 64 * 1024,
        },
        queue_capacity: 2,
    };
    (
        IngestionCoordinator::start(dependencies).unwrap(),
        database,
        source,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn registration_failure_keeps_archive_for_a_later_retry() {
    let temp = TempDir::new().unwrap();
    let (coordinator, database, source) = coordinator(&temp).await;
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    let inbox = workspace.source_inbox(&source);
    let source_path = inbox.join("retry.fixture");
    std::fs::write(&source_path, b"retryable asset").unwrap();
    let injector = TestFaultInjector::default();
    injector.fail_once(FailurePoint::AssetRegistration);
    coordinator.set_fault_injector(Arc::new(injector.clone()));

    coordinator.request_scan(request()).await.unwrap();
    let failed = coordinator.request_scan(request()).await.unwrap();
    assert_eq!(failed.coalesced_requests, 0);
    assert!(!source_path.exists());
    let inventory = ArchiveReconciler::new(workspace.clone(), source.clone())
        .scan()
        .unwrap();
    assert_eq!(inventory.assets.len(), 1);
    let active = database
        .execute(QueryView::active_snapshot(
            LogicalSnapshotKey::new("fixture:2026").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(active.counts.total, 0);

    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();
    assert!(!source_path.exists());
    let active = database
        .execute(QueryView::active_snapshot(
            LogicalSnapshotKey::new("fixture:2026").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(active.counts.total, 1);
    database.shutdown().await.unwrap();
    coordinator.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_retry_replays_an_archived_asset_with_a_new_attempt_identity() {
    let temp = TempDir::new().unwrap();
    let (coordinator, database, source) = coordinator(&temp).await;
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    let source_path = workspace.source_inbox(&source).join("retry.fixture");
    std::fs::write(&source_path, b"retryable asset").unwrap();
    coordinator.set_runtime_failure_for_next_asset();
    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();

    let archived = ArchiveReconciler::new(workspace.clone(), source.clone())
        .scan()
        .unwrap()
        .assets
        .into_iter()
        .next()
        .unwrap();
    let retry = coordinator.retry_asset(archived.asset_id).await.unwrap();
    assert_eq!(retry.asset_id, archived.asset_id);
    assert_ne!(retry.attempt_id, archived.asset_id);
    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn canonical_transaction_failure_retries_a_registered_asset_after_archive_verification() {
    let temp = TempDir::new().unwrap();
    let source = ModuleId::try_from("fixture-source").unwrap();
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    workspace.enable_source(&source).unwrap();
    let database_fault = TestDatabaseFaultInjector::default();
    database_fault.fail_once(DatabaseFailurePoint::CanonicalInsert);
    let database = DatabaseService::start_with_fault_injector(
        &temp.path().join("storage.duckdb"),
        8,
        Arc::new(database_fault),
    )
    .await
    .unwrap();
    let dependencies = IngestionDependencies {
        workspace: workspace.clone(),
        source_module: support::fake_module(&temp, source.clone()),
        archive: ArchiveCoordinator::new(workspace.clone(), source.clone()),
        database: database.clone(),
        runtime: support::fake_runtime(support::nutrition_batch()),
        limits: RuntimeLimits {
            max_memory_bytes: 8 * 1024 * 1024,
            fuel: 1_000_000,
            timeout: Duration::from_secs(1),
            max_output_bytes: 64 * 1024,
        },
        queue_capacity: 2,
    };
    let coordinator = IngestionCoordinator::start(dependencies).unwrap();
    let source_path = workspace.source_inbox(&source).join("transaction.fixture");
    std::fs::write(&source_path, b"transaction retry").unwrap();
    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();
    assert!(!source_path.exists());
    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();
    let active = database
        .execute(QueryView::active_snapshot(
            LogicalSnapshotKey::new("fixture:2026").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(active.counts.total, 1);
    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();
}
