use mfa_archive::ArchiveCoordinator;
use mfa_config::WorkspacePaths;
use mfa_contracts::UtcInstant;
use mfa_db::{DatabaseService, HealthCheck, LogicalSnapshotKey, QueryView};
use mfa_ingestion::{
    CoreEvent, IngestionCoordinator, IngestionDependencies, ScanReason, ScanRequest,
};
use mfa_module_host::RuntimeLimits;
use tempfile::TempDir;
use tokio::time::{Duration, timeout};

mod support;
use support::{fake_module, fake_runtime, nutrition_batch};

fn request() -> ScanRequest {
    ScanRequest::new(
        ScanReason::Manual,
        "2026-08-25T00:00:00Z".parse::<UtcInstant>().unwrap(),
    )
}

async fn coordinator(
    temp: &TempDir,
) -> (
    IngestionCoordinator,
    DatabaseService,
    mfa_contracts::ModuleId,
) {
    let source = mfa_contracts::ModuleId::try_from("fixture-source").unwrap();
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    workspace.enable_source(&source).unwrap();
    let app_data = temp.path().join("app-data");
    std::fs::create_dir_all(&app_data).unwrap();
    let database = DatabaseService::start(&app_data.join("db.duckdb"), 8)
        .await
        .unwrap();
    let module = fake_module(temp, source.clone());
    let dependencies = IngestionDependencies {
        workspace,
        source_module: module,
        archive: ArchiveCoordinator::new(
            WorkspacePaths::new(temp.path().join("workspace")),
            source.clone(),
        ),
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
async fn successful_asset_follows_archive_parse_commit_and_data_changed_order() {
    let temp = TempDir::new().unwrap();
    let (coordinator, database, source) = coordinator(&temp).await;
    let inbox = WorkspacePaths::new(temp.path().join("workspace")).source_inbox(&source);
    std::fs::write(inbox.join("export.fixture"), b"synthetic asset").unwrap();
    let mut events = coordinator.subscribe();
    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();

    let mut stages = Vec::new();
    loop {
        match timeout(Duration::from_secs(2), events.recv())
            .await
            .unwrap()
            .unwrap()
        {
            CoreEvent::Stage(stage) => stages.push(stage),
            CoreEvent::DataChanged { .. } => break,
            CoreEvent::QualityChanged | CoreEvent::WorkStateChanged(_) => {}
        }
    }
    assert_eq!(
        stages,
        vec![
            "stable",
            "archive_verified",
            "receipt_registered",
            "attempt_started",
            "guest_parsed",
            "host_validated",
            "snapshot_committed",
            "inbox_removed",
        ]
    );
    assert!(!inbox.join("export.fixture").exists());
    let view = database
        .execute(QueryView::active_snapshot(
            LogicalSnapshotKey::new("fixture-source:default").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(view.counts.total, 1);
    database.shutdown().await.unwrap();
    coordinator.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exact_duplicate_records_receipt_removes_inbox_and_skips_guest() {
    let temp = TempDir::new().unwrap();
    let (coordinator, database, source) = coordinator(&temp).await;
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    let inbox = workspace.source_inbox(&source);
    std::fs::write(inbox.join("first.fixture"), b"same bytes").unwrap();
    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();
    let first_count = coordinator.runtime_invocations();
    std::fs::write(inbox.join("second.fixture"), b"same bytes").unwrap();
    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();
    assert!(!inbox.join("second.fixture").exists());
    assert_eq!(coordinator.runtime_invocations(), first_count);
    assert!(coordinator.duplicate_receipts() >= 1);
    let health = database.execute(HealthCheck).await.unwrap();
    assert_eq!(health.schema_version, 3);
    database.shutdown().await.unwrap();
    coordinator.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_asset_failure_does_not_stop_next_asset_after_archive_cleanup() {
    let temp = TempDir::new().unwrap();
    let (coordinator, database, source) = coordinator(&temp).await;
    let inbox = WorkspacePaths::new(temp.path().join("workspace")).source_inbox(&source);
    std::fs::write(inbox.join("bad.fixture"), b"bad").unwrap();
    std::fs::write(inbox.join("good.fixture"), b"good").unwrap();
    coordinator.set_runtime_failure_for_next_asset();
    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();
    assert!(inbox.join("bad.fixture").exists());
    assert!(!inbox.join("good.fixture").exists());
    assert!(coordinator.completed_assets() >= 1);
    database.shutdown().await.unwrap();
    coordinator.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_failure_keeps_inbox_and_records_failed_before_archive_receipt() {
    let temp = TempDir::new().unwrap();
    let (coordinator, database, source) = coordinator(&temp).await;
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    let archive_root = workspace.source_archive(&source);
    std::fs::remove_dir_all(&archive_root).unwrap();
    std::fs::write(&archive_root, b"archive destination is blocked").unwrap();
    let inbox = workspace.source_inbox(&source);
    let source_path = inbox.join("blocked.fixture");
    std::fs::write(&source_path, b"blocked archive").unwrap();

    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();

    assert!(source_path.exists());
    assert_eq!(coordinator.failed_before_archive_receipts(), 1);
    database.shutdown().await.unwrap();
    coordinator.shutdown().await.unwrap();
}
