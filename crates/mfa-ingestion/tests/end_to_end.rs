use mfa_archive::{ArchiveCoordinator, ArchiveReconciler};
use mfa_config::WorkspacePaths;
use mfa_contracts::{CanonicalObservation, ExtensionRecord, SourceBatch, UtcInstant};
use mfa_db::{
    DatabaseFailurePoint, DatabaseService, HealthCheck, LogicalSnapshotKey, QueryAttempt,
    QueryView, TestDatabaseFaultInjector,
};
use mfa_ingestion::{
    CoreEvent, FailurePoint, IngestionCoordinator, IngestionDependencies, ScanReason, ScanRequest,
    SourceInvoker, TestFaultInjector,
};
use mfa_module_host::RuntimeLimits;
use std::sync::Arc;
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
    coordinator_with_runtime(temp, fake_runtime(nutrition_batch())).await
}

async fn coordinator_with_runtime(
    temp: &TempDir,
    runtime: Arc<dyn SourceInvoker>,
) -> (
    IngestionCoordinator,
    DatabaseService,
    mfa_contracts::ModuleId,
) {
    let app_data = temp.path().join("app-data");
    std::fs::create_dir_all(&app_data).unwrap();
    let database = DatabaseService::start(&app_data.join("db.duckdb"), 8)
        .await
        .unwrap();
    coordinator_with_database(temp, runtime, database)
}

fn coordinator_with_database(
    temp: &TempDir,
    runtime: Arc<dyn SourceInvoker>,
    database: DatabaseService,
) -> (
    IngestionCoordinator,
    DatabaseService,
    mfa_contracts::ModuleId,
) {
    let source = mfa_contracts::ModuleId::try_from("fixture-source").unwrap();
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    workspace.enable_source(&source).unwrap();
    let module = fake_module(temp, source.clone());
    let dependencies = IngestionDependencies {
        workspace,
        source_module: module,
        archive: ArchiveCoordinator::new(
            WorkspacePaths::new(temp.path().join("workspace")),
            source.clone(),
        ),
        database: database.clone(),
        runtime,
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
            "inbox_removed",
            "receipt_registered",
            "attempt_started",
            "guest_parsed",
            "host_validated",
            "snapshot_committed",
        ]
    );
    assert!(!inbox.join("export.fixture").exists());
    let view = database
        .execute(QueryView::active_snapshot(
            LogicalSnapshotKey::new("fixture:2026").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(view.counts.total, 1);
    database.shutdown().await.unwrap();
    coordinator.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_scan_autonomously_ingests_preexisting_stable_file() {
    let temp = TempDir::new().unwrap();
    let source = mfa_contracts::ModuleId::try_from("fixture-source").unwrap();
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    workspace.enable_source(&source).unwrap();
    let inbox = workspace.source_inbox(&source);
    let source_path = inbox.join("preexisting.fixture");
    std::fs::write(&source_path, b"preexisting stable asset").unwrap();

    let app_data = temp.path().join("app-data");
    std::fs::create_dir_all(&app_data).unwrap();
    let database = DatabaseService::start(&app_data.join("db.duckdb"), 8)
        .await
        .unwrap();
    let dependencies = IngestionDependencies {
        workspace: workspace.clone(),
        source_module: fake_module(&temp, source.clone()),
        archive: ArchiveCoordinator::new(workspace.clone(), source.clone()),
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
    let coordinator = IngestionCoordinator::start(dependencies).unwrap();

    coordinator
        .request_scan(ScanRequest::new(
            ScanReason::Startup,
            "2026-08-25T00:00:00Z".parse::<UtcInstant>().unwrap(),
        ))
        .await
        .unwrap();

    timeout(Duration::from_secs(3), async {
        loop {
            let archive = ArchiveReconciler::new(workspace.clone(), source.clone())
                .scan()
                .unwrap();
            let view = database
                .execute(QueryView::active_snapshot(
                    LogicalSnapshotKey::new("fixture:2026").unwrap(),
                ))
                .await
                .unwrap();
            if !source_path.exists() && archive.assets.len() == 1 && view.counts.total == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("one startup scan should converge a stable preexisting file");

    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();
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
    assert!(!inbox.join("bad.fixture").exists());
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guest_parse_failure_marks_the_attempt_failed_without_restart() {
    let temp = TempDir::new().unwrap();
    let (coordinator, database, source) = coordinator(&temp).await;
    let inbox = WorkspacePaths::new(temp.path().join("workspace")).source_inbox(&source);
    std::fs::write(inbox.join("parse.fixture"), b"parse failure").unwrap();
    coordinator.set_runtime_failure_for_next_asset();

    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();
    let attempt_id = coordinator.last_attempt_id().unwrap();
    let attempt = database.execute(QueryAttempt { attempt_id }).await.unwrap();

    assert_eq!(attempt.status, "failed");
    assert_eq!(attempt.error_code.as_deref(), Some("module_guest_error"));
    assert!(attempt.finished_at.is_some());
    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_retry_guest_failure_marks_its_attempt_failed_immediately() {
    let temp = TempDir::new().unwrap();
    let runtime = fake_runtime(nutrition_batch());
    let (coordinator, database, source) = coordinator_with_runtime(&temp, runtime.clone()).await;
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    let inbox = workspace.source_inbox(&source);
    std::fs::write(inbox.join("retry.fixture"), b"retry bytes").unwrap();
    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();
    let asset_id = ArchiveReconciler::new(workspace, source)
        .scan()
        .unwrap()
        .assets
        .into_iter()
        .next()
        .unwrap()
        .asset_id;

    runtime.fail_next();
    let error = coordinator.retry_asset(asset_id).await.unwrap_err();
    assert!(matches!(
        error,
        mfa_ingestion::IngestionError::AssetFailure { .. }
    ));
    let attempt_id = coordinator.last_attempt_id().unwrap();
    let attempt = database.execute(QueryAttempt { attempt_id }).await.unwrap();
    assert_eq!(attempt.status, "failed");
    assert_eq!(attempt.error_code.as_deref(), Some("module_guest_error"));
    assert!(attempt.finished_at.is_some());
    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();
}

async fn assert_batch_failure_marks_attempt_failed(batch: SourceBatch, expected_code: &str) {
    let temp = TempDir::new().unwrap();
    let runtime = fake_runtime(batch);
    let (coordinator, database, source) = coordinator_with_runtime(&temp, runtime).await;
    let inbox = WorkspacePaths::new(temp.path().join("workspace")).source_inbox(&source);
    std::fs::write(inbox.join("invalid.fixture"), b"invalid bytes").unwrap();
    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();

    let attempt_id = coordinator.last_attempt_id().unwrap();
    let attempt = database.execute(QueryAttempt { attempt_id }).await.unwrap();
    assert_eq!(attempt.status, "failed");
    assert_eq!(attempt.error_code.as_deref(), Some(expected_code));
    assert!(attempt.finished_at.is_some());
    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();
}

#[tokio::test]
async fn build_and_validation_failures_mark_attempt_failed_immediately() {
    let mut serialization_batch = nutrition_batch();
    if let CanonicalObservation::NutritionItem(item) = &mut serialization_batch.records[0] {
        item.calories_kcal = Some(f64::NAN);
    }
    assert_batch_failure_marks_attempt_failed(serialization_batch, "non_finite_value").await;

    let mut validation_batch = nutrition_batch();
    if let CanonicalObservation::NutritionItem(item) = &mut validation_batch.records[0] {
        item.calories_kcal = Some(-1.0);
    }
    assert_batch_failure_marks_attempt_failed(validation_batch, "invalid_domain_value").await;
}

#[tokio::test]
async fn build_batch_failure_marks_attempt_failed_immediately() {
    let temp = TempDir::new().unwrap();
    let runtime = fake_runtime(nutrition_batch());
    let (coordinator, database, source) = coordinator_with_runtime(&temp, runtime).await;
    let injector = TestFaultInjector::default();
    injector.fail_once(FailurePoint::BuildBatch);
    coordinator.set_fault_injector(Arc::new(injector));
    let inbox = WorkspacePaths::new(temp.path().join("workspace")).source_inbox(&source);
    std::fs::write(inbox.join("build.fixture"), b"build bytes").unwrap();
    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();

    let attempt_id = coordinator.last_attempt_id().unwrap();
    let attempt = database.execute(QueryAttempt { attempt_id }).await.unwrap();
    assert_eq!(attempt.status, "failed");
    assert_eq!(
        attempt.error_code.as_deref(),
        Some("fault_injected_buildbatch")
    );
    assert!(attempt.finished_at.is_some());
    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();
}

#[tokio::test]
async fn extension_contract_registration_failure_marks_attempt_failed_immediately() {
    let mut batch = nutrition_batch();
    batch.extensions.push(ExtensionRecord {
        namespace: "fixture.extension".to_owned(),
        contract_version: "1.0.0".parse().unwrap(),
        record_type: "fixture.record".to_owned(),
        source_record_key: "source-1".to_owned(),
        occurred_local_at: None,
        local_date: None,
        payload: serde_json::json!({"value": 1}),
    });
    let temp = TempDir::new().unwrap();
    let app_data = temp.path().join("app-data");
    std::fs::create_dir_all(&app_data).unwrap();
    let injector = TestDatabaseFaultInjector::default();
    injector.fail_once(DatabaseFailurePoint::ExtensionContractRegistration);
    let database = DatabaseService::start_with_fault_injector(
        &app_data.join("db.duckdb"),
        8,
        Arc::new(injector),
    )
    .await
    .unwrap();
    let runtime = fake_runtime(batch);
    let (coordinator, database, source) = coordinator_with_database(&temp, runtime, database);
    let inbox = WorkspacePaths::new(temp.path().join("workspace")).source_inbox(&source);
    std::fs::write(inbox.join("extension.fixture"), b"extension bytes").unwrap();
    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();

    let attempt_id = coordinator.last_attempt_id().unwrap();
    let attempt = database.execute(QueryAttempt { attempt_id }).await.unwrap();
    assert_eq!(attempt.status, "failed");
    assert_eq!(
        attempt.error_code.as_deref(),
        Some("database_fault_injected")
    );
    assert!(attempt.finished_at.is_some());
    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();
}
