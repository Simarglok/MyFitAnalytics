use chrono::Utc;
use mfa_archive::ArchiveReconciler;
use mfa_archive::{ArchiveCoordinator, FileFingerprint, StableCandidate};
use mfa_config::WorkspacePaths;
use mfa_contracts::{ModuleId, UtcInstant};
use mfa_db::{AssetRegistration, DatabaseService, RegisterAsset, StartAttempt};
use mfa_ingestion::{RecoveryError, RecoveryGate, RecoveryMode, RecoveryService, recover_sources};
use std::fs;
use std::time::SystemTime;
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn startup_recovery_marks_interrupted_attempts_before_releasing_ingestion_gate() {
    let temp = TempDir::new().unwrap();
    let source = ModuleId::try_from("fixture-source").unwrap();
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    workspace.enable_source(&source).unwrap();
    let inbox = workspace.source_inbox(&source);
    let source_path = inbox.join("orphan.fixture");
    fs::write(&source_path, b"orphan bytes").unwrap();
    let metadata = fs::metadata(&source_path).unwrap();
    let candidate = StableCandidate {
        path: source_path.clone(),
        fingerprint: FileFingerprint {
            size: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        },
    };
    let archive = ArchiveCoordinator::new(workspace.clone(), source.clone());
    archive
        .archive(candidate, UtcInstant::from(Utc::now()))
        .unwrap();

    let database = DatabaseService::start(&temp.path().join("storage.duckdb"), 8)
        .await
        .unwrap();
    let asset_id = Uuid::from_u128(7000);
    database
        .execute(RegisterAsset {
            asset: AssetRegistration {
                asset_id,
                source_module_id: source.clone(),
                asset_type: "fixture".to_owned(),
                original_filename: "old.fixture".to_owned(),
                archive_path: "/missing/old.fixture".to_owned(),
                byte_sha256: "a".repeat(64),
                file_size: 1,
                received_at: UtcInstant::from(Utc::now()),
            },
        })
        .await
        .unwrap();
    database
        .execute(StartAttempt {
            attempt_id: Uuid::from_u128(7001),
            asset_id,
            source_module_id: source.clone(),
            source_module_version: "1.0.0".to_owned(),
            source_module_package_sha256: "b".repeat(64),
            source_api_version: "1.0.0".to_owned(),
            mapping_version: "1.0.0".to_owned(),
            schema_fingerprint: "schema".to_owned(),
            logical_snapshot_key: "fixture-source:default".to_owned(),
            started_at: UtcInstant::from(Utc::now()),
        })
        .await
        .unwrap();

    let gate = RecoveryGate::new();
    assert_eq!(gate.mode(), RecoveryMode::Recovery);
    let service = RecoveryService::new(
        database.clone(),
        ArchiveReconciler::new(workspace, source),
        gate.clone(),
    );
    let report = service.startup().await.unwrap();

    assert_eq!(report.interrupted_attempts, 1);
    assert_eq!(report.registered_assets, 1);
    assert_eq!(report.assets_to_ingest, 1);
    assert_eq!(gate.mode(), RecoveryMode::Normal);
    assert!(gate.ensure_ingestion_allowed().is_ok());
    database.shutdown().await.unwrap();
}

#[test]
fn recovery_gate_rejects_ingestion_while_recovery_is_in_progress() {
    let gate = RecoveryGate::new();
    assert_eq!(gate.mode(), RecoveryMode::Recovery);
    assert!(matches!(
        gate.ensure_ingestion_allowed(),
        Err(mfa_ingestion::IngestionError::CriticalFailure { code, .. })
            if code == "recovery_in_progress"
    ));
    gate.complete();
    assert_eq!(gate.mode(), RecoveryMode::Normal);
}

fn archive_fixture(workspace: &WorkspacePaths, source: &ModuleId, name: &str, bytes: &[u8]) {
    let inbox_path = workspace.source_inbox(source).join(name);
    fs::write(&inbox_path, bytes).unwrap();
    let metadata = fs::metadata(&inbox_path).unwrap();
    ArchiveCoordinator::new(workspace.clone(), source.clone())
        .archive(
            StableCandidate {
                path: inbox_path.clone(),
                fingerprint: FileFingerprint {
                    size: metadata.len(),
                    modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                },
            },
            UtcInstant::from(Utc::now()),
        )
        .unwrap();
    fs::remove_file(inbox_path).unwrap();
}

#[tokio::test]
async fn recover_sources_marks_interrupted_once_orders_reports_and_releases_gate() {
    let temp = TempDir::new().unwrap();
    let first_source = ModuleId::try_from("first-source").unwrap();
    let second_source = ModuleId::try_from("second-source").unwrap();
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    workspace.enable_source(&first_source).unwrap();
    workspace.enable_source(&second_source).unwrap();
    archive_fixture(&workspace, &first_source, "first.fixture", b"first archive");
    let database = DatabaseService::start(&temp.path().join("storage.duckdb"), 8)
        .await
        .unwrap();
    let asset_id = Uuid::from_u128(7100);
    database
        .execute(RegisterAsset {
            asset: AssetRegistration {
                asset_id,
                source_module_id: first_source.clone(),
                asset_type: "fixture".to_owned(),
                original_filename: "running.fixture".to_owned(),
                archive_path: "/missing/running.fixture".to_owned(),
                byte_sha256: "c".repeat(64),
                file_size: 1,
                received_at: UtcInstant::from(Utc::now()),
            },
        })
        .await
        .unwrap();
    database
        .execute(StartAttempt {
            attempt_id: Uuid::from_u128(7101),
            asset_id,
            source_module_id: first_source.clone(),
            source_module_version: "1.0.0".to_owned(),
            source_module_package_sha256: "d".repeat(64),
            source_api_version: "1.0.0".to_owned(),
            mapping_version: "1.0.0".to_owned(),
            schema_fingerprint: "schema".to_owned(),
            logical_snapshot_key: "first-source:default".to_owned(),
            started_at: UtcInstant::from(Utc::now()),
        })
        .await
        .unwrap();

    let gate = RecoveryGate::new();
    let reports = recover_sources(
        database.clone(),
        vec![
            ArchiveReconciler::new(workspace.clone(), first_source),
            ArchiveReconciler::new(workspace, second_source),
        ],
        gate.clone(),
    )
    .await
    .unwrap();

    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].interrupted_attempts, 1);
    assert_eq!(reports[1].interrupted_attempts, 0);
    assert_eq!(
        reports
            .iter()
            .map(|report| report.interrupted_attempts)
            .sum::<u64>(),
        1
    );
    assert_eq!(reports[0].registered_assets, 1);
    assert_eq!(reports[0].assets_to_ingest, 1);
    assert_eq!(reports[1].registered_assets, 0);
    assert_eq!(reports[1].assets_to_ingest, 0);
    assert_eq!(gate.mode(), RecoveryMode::Normal);
    database.shutdown().await.unwrap();
}

#[tokio::test]
async fn recover_sources_keeps_gate_blocked_when_second_reconciliation_fails() {
    let temp = TempDir::new().unwrap();
    let first_source = ModuleId::try_from("first-source").unwrap();
    let second_source = ModuleId::try_from("second-source").unwrap();
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    workspace.enable_source(&first_source).unwrap();
    workspace.enable_source(&second_source).unwrap();
    archive_fixture(&workspace, &first_source, "first.fixture", b"first archive");
    let second_archive = workspace.source_archive(&second_source);
    fs::remove_dir_all(&second_archive).unwrap();
    fs::write(&second_archive, b"not an archive directory").unwrap();
    let database = DatabaseService::start(&temp.path().join("storage.duckdb"), 8)
        .await
        .unwrap();
    let gate = RecoveryGate::new();

    let result = recover_sources(
        database.clone(),
        vec![
            ArchiveReconciler::new(workspace.clone(), first_source),
            ArchiveReconciler::new(workspace, second_source),
        ],
        gate.clone(),
    )
    .await;

    assert!(matches!(result, Err(RecoveryError::Archive(_))));
    assert_eq!(gate.mode(), RecoveryMode::Recovery);
    database.shutdown().await.unwrap();
}
