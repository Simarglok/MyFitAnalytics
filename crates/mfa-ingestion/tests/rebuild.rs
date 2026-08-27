use mfa_archive::{ArchiveCoordinator, ArchiveReconciler, FileFingerprint, StableCandidate};
use mfa_config::WorkspacePaths;
use mfa_contracts::{ModuleId, UtcInstant};
use mfa_db::{DatabaseService, HealthCheck, QueryAttempt};
use mfa_db::{LogicalSnapshotKey, QueryView};
use mfa_ingestion::{
    ArchiveAssetImporter, ArchiveRebuildConfig, ArchiveRebuildConfirmation, ArchiveRebuildService,
    BoxFuture, FailurePoint, RebuildError, RuntimeArchiveImporter, TestFaultInjector,
};
use mfa_module_host::RuntimeLimits;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tempfile::TempDir;

mod support;
use support::{fake_module, fake_runtime, nutrition_batch};

#[derive(Clone)]
struct RecordingImporter {
    calls: Arc<Mutex<Vec<String>>>,
    fail: bool,
}

impl ArchiveAssetImporter for RecordingImporter {
    fn import<'a>(
        &'a self,
        database: DatabaseService,
        asset: mfa_archive::ArchiveRecord,
    ) -> BoxFuture<'a, Result<(), RebuildError>> {
        let calls = Arc::clone(&self.calls);
        let fail = self.fail;
        Box::pin(async move {
            database
                .execute(HealthCheck)
                .await
                .map_err(RebuildError::Database)?;
            calls.lock().unwrap().push(asset.byte_sha256);
            if fail {
                Err(RebuildError::Importer {
                    detail: "synthetic rebuild failure".to_owned(),
                })
            } else {
                Ok(())
            }
        })
    }

    fn validate<'a>(
        &'a self,
        database: DatabaseService,
    ) -> BoxFuture<'a, Result<(), RebuildError>> {
        Box::pin(async move {
            database
                .execute(HealthCheck)
                .await
                .map_err(RebuildError::Database)?;
            Ok(())
        })
    }
}

async fn setup_archive(temp: &TempDir) -> (WorkspacePaths, ModuleId) {
    let source = ModuleId::try_from("fixture-source").unwrap();
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    workspace.enable_source(&source).unwrap();
    let inbox = workspace.source_inbox(&source);
    let path = inbox.join("asset.fixture");
    fs::write(&path, b"rebuild bytes").unwrap();
    let metadata = fs::metadata(&path).unwrap();
    ArchiveCoordinator::new(workspace.clone(), source.clone())
        .archive(
            StableCandidate {
                path,
                fingerprint: FileFingerprint {
                    size: metadata.len(),
                    modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                },
            },
            "2026-08-25T00:00:00Z".parse::<UtcInstant>().unwrap(),
        )
        .unwrap();
    (workspace, source)
}

#[tokio::test]
async fn rebuild_uses_a_temporary_actor_and_keeps_an_immutable_recovery_copy() {
    let temp = TempDir::new().unwrap();
    let (workspace, source) = setup_archive(&temp).await;
    let database_path = temp.path().join("storage.duckdb");
    let database = DatabaseService::start(&database_path, 8).await.unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let importer = RecordingImporter {
        calls: Arc::clone(&calls),
        fail: false,
    };
    let mut service = ArchiveRebuildService::new(
        database,
        ArchiveRebuildConfig {
            database_path: database_path.clone(),
            recovery_root: temp.path().join("recovery"),
            actor_capacity: 8,
        },
        ArchiveCoordinator::new(workspace.clone(), source.clone()).reconciler(),
        Arc::new(importer),
    );

    let plan = service.preflight().await.unwrap();
    assert_eq!(plan.assets.len(), 1);
    assert!(plan.missing_source_packages.is_empty());

    let result = service
        .rebuild(ArchiveRebuildConfirmation::confirmed())
        .await
        .unwrap();

    assert_eq!(result.assets_rebuilt, 1);
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert!(result.recovery_copy.exists());
    assert!(database_path.exists());
    service.database().execute(HealthCheck).await.unwrap();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn failed_rebuild_reopens_the_original_database_without_swapping_it() {
    let temp = TempDir::new().unwrap();
    let (workspace, source) = setup_archive(&temp).await;
    let database_path = temp.path().join("storage.duckdb");
    let database = DatabaseService::start(&database_path, 8).await.unwrap();
    let importer = RecordingImporter {
        calls: Arc::new(Mutex::new(Vec::new())),
        fail: true,
    };
    let mut service = ArchiveRebuildService::new(
        database,
        ArchiveRebuildConfig {
            database_path: database_path.clone(),
            recovery_root: temp.path().join("recovery"),
            actor_capacity: 8,
        },
        ArchiveCoordinator::new(workspace, source).reconciler(),
        Arc::new(importer),
    );

    let error = service
        .rebuild(ArchiveRebuildConfirmation::confirmed())
        .await
        .unwrap_err();
    assert!(matches!(error, RebuildError::Importer { .. }));
    service.database().execute(HealthCheck).await.unwrap();
    assert!(database_path.exists());
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn production_rebuild_importer_replays_archive_through_runtime_and_actor() {
    let temp = TempDir::new().unwrap();
    let (workspace, source) = setup_archive(&temp).await;
    let database_path = temp.path().join("storage.duckdb");
    let database = DatabaseService::start(&database_path, 8).await.unwrap();
    let importer = RuntimeArchiveImporter::new(
        fake_module(&temp, source.clone()),
        fake_runtime(nutrition_batch()),
        RuntimeLimits {
            max_memory_bytes: 8 * 1024 * 1024,
            fuel: 1_000_000,
            timeout: std::time::Duration::from_secs(1),
            max_output_bytes: 64 * 1024,
        },
    );
    let mut service = ArchiveRebuildService::new(
        database,
        ArchiveRebuildConfig {
            database_path,
            recovery_root: temp.path().join("recovery"),
            actor_capacity: 8,
        },
        ArchiveCoordinator::new(workspace, source.clone()).reconciler(),
        Arc::new(importer),
    );

    service
        .rebuild(ArchiveRebuildConfirmation::confirmed())
        .await
        .unwrap();
    let view = service
        .database()
        .execute(QueryView::active_snapshot(
            LogicalSnapshotKey::new("fixture:2026").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(view.counts.total, 1);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn injected_rebuild_swap_failure_reopens_the_original_database() {
    let temp = TempDir::new().unwrap();
    let (workspace, source) = setup_archive(&temp).await;
    let database_path = temp.path().join("storage.duckdb");
    let database = DatabaseService::start(&database_path, 8).await.unwrap();
    let injector = TestFaultInjector::default();
    injector.fail_once(FailurePoint::ActiveSwitch);
    let mut service = ArchiveRebuildService::new_with_fault_injector(
        database,
        ArchiveRebuildConfig {
            database_path: database_path.clone(),
            recovery_root: temp.path().join("recovery"),
            actor_capacity: 8,
        },
        ArchiveCoordinator::new(workspace, source).reconciler(),
        Arc::new(RecordingImporter {
            calls: Arc::new(Mutex::new(Vec::new())),
            fail: false,
        }),
        Arc::new(injector),
    );

    let error = service
        .rebuild(ArchiveRebuildConfirmation::confirmed())
        .await
        .unwrap_err();
    assert!(matches!(error, RebuildError::FaultInjected { .. }));
    service.database().execute(HealthCheck).await.unwrap();
    assert!(database_path.exists());
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn production_rebuild_importer_marks_guest_failure_failed_immediately() {
    let temp = TempDir::new().unwrap();
    let (workspace, source) = setup_archive(&temp).await;
    let database_path = temp.path().join("storage.duckdb");
    let database = DatabaseService::start(&database_path, 8).await.unwrap();
    let runtime = fake_runtime(nutrition_batch());
    runtime.fail_next();
    let importer = RuntimeArchiveImporter::new(
        fake_module(&temp, source.clone()),
        runtime,
        RuntimeLimits {
            max_memory_bytes: 8 * 1024 * 1024,
            fuel: 1_000_000,
            timeout: std::time::Duration::from_secs(1),
            max_output_bytes: 64 * 1024,
        },
    );
    let archived = ArchiveReconciler::new(workspace, source)
        .scan()
        .unwrap()
        .assets
        .into_iter()
        .next()
        .unwrap();

    let error = importer
        .import(database.clone(), archived)
        .await
        .unwrap_err();
    assert!(matches!(error, RebuildError::Importer { .. }));
    let attempt_id = importer.last_attempt_id().unwrap();
    let attempt = database.execute(QueryAttempt { attempt_id }).await.unwrap();
    assert_eq!(attempt.status, "failed");
    assert_eq!(attempt.error_code.as_deref(), Some("module_guest_error"));
    assert!(attempt.finished_at.is_some());
    database.shutdown().await.unwrap();
}
