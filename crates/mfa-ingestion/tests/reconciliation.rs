use chrono::{TimeZone, Timelike, Utc};
use mfa_archive::{ArchiveCoordinator, ArchiveReconciler, FileFingerprint, StableCandidate};
use mfa_config::WorkspacePaths;
use mfa_contracts::{ModuleId, UtcInstant};
use mfa_db::{
    ArchiveAssetRecord, DatabaseService, LogicalSnapshotKey, QueryView, ReconcileArchiveInventory,
    RegisterAsset,
};
use mfa_ingestion::{IngestionCoordinator, IngestionDependencies, ScanReason, ScanRequest};
use mfa_module_host::RuntimeLimits;
use std::fs;
use std::time::SystemTime;
use tempfile::TempDir;

mod support;
use support::{fake_module, fake_runtime, nutrition_batch};

fn received_at() -> UtcInstant {
    UtcInstant::from(
        Utc.with_ymd_and_hms(2026, 8, 25, 12, 34, 56)
            .unwrap()
            .with_nanosecond(123_456_000)
            .unwrap(),
    )
}

#[test]
fn reconciliation_lists_orphaned_completed_assets_and_ignores_temporary_copies() {
    let root = TempDir::new().unwrap();
    let workspace = WorkspacePaths::new(root.path().join("workspace"));
    let source = ModuleId::try_from("fixture-source").unwrap();
    workspace.enable_source(&source).unwrap();
    let inbox = workspace.source_inbox(&source);
    let input = inbox.join("renamed export.fixture");
    fs::write(&input, b"orphaned bytes").unwrap();
    let metadata = fs::metadata(&input).unwrap();
    let archived = ArchiveCoordinator::new(workspace.clone(), source.clone())
        .archive(
            StableCandidate {
                path: input,
                fingerprint: FileFingerprint {
                    size: metadata.len(),
                    modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                },
            },
            received_at(),
        )
        .unwrap();

    let temporary = archived
        .archive_path
        .parent()
        .unwrap()
        .join(".leftover.archive-tmp-copy");
    fs::write(temporary, b"partial bytes").unwrap();

    let inventory = ArchiveReconciler::new(workspace, source).scan().unwrap();

    assert_eq!(inventory.assets.len(), 1);
    assert_eq!(inventory.assets[0].asset_id, archived.asset_id);
    assert_eq!(inventory.assets[0].byte_sha256, archived.byte_sha256);
    assert!(inventory.ignored_temporary_files >= 1);
}

#[tokio::test]
async fn reconciliation_registers_orphans_idempotently_and_marks_missing_bytes_critical() {
    let root = TempDir::new().unwrap();
    let workspace = WorkspacePaths::new(root.path().join("workspace"));
    let source = ModuleId::try_from("fixture-source").unwrap();
    workspace.enable_source(&source).unwrap();
    let archive_path = workspace
        .source_archive(&source)
        .join("2026/2026-08-25/orphan");
    fs::create_dir_all(archive_path.parent().unwrap()).unwrap();
    fs::write(&archive_path, b"orphaned bytes").unwrap();
    let database = DatabaseService::start(&root.path().join("app-data/storage.duckdb"), 4)
        .await
        .unwrap();
    let orphan = ArchiveAssetRecord {
        asset_id: uuid::Uuid::from_u128(1),
        source_module_id: source.clone(),
        asset_type: "source_export".to_owned(),
        original_filename: "orphan.fixture".to_owned(),
        archive_path: archive_path.to_string_lossy().into_owned(),
        byte_sha256: "a".repeat(64),
        file_size: 14,
        received_at: received_at(),
    };

    let first = database
        .execute(ReconcileArchiveInventory {
            source_module_id: source.clone(),
            assets: vec![orphan.clone()],
        })
        .await
        .unwrap();
    let second = database
        .execute(ReconcileArchiveInventory {
            source_module_id: source.clone(),
            assets: vec![orphan],
        })
        .await
        .unwrap();

    assert_eq!(first.registered_assets, 1);
    assert_eq!(second.registered_assets, 0);
    assert_eq!(first.assets_to_ingest.len(), 1);
    assert_eq!(second.assets_to_ingest.len(), 1);

    database
        .execute(RegisterAsset {
            asset: mfa_db::AssetRegistration {
                asset_id: uuid::Uuid::from_u128(2),
                source_module_id: source.clone(),
                asset_type: "source_export".to_owned(),
                original_filename: "missing.fixture".to_owned(),
                archive_path: root
                    .path()
                    .join("workspace/archive/fixture-source/missing")
                    .to_string_lossy()
                    .into_owned(),
                byte_sha256: "b".repeat(64),
                file_size: 1,
                received_at: received_at(),
            },
        })
        .await
        .unwrap();
    let mismatch = database
        .execute(ReconcileArchiveInventory {
            source_module_id: source,
            assets: Vec::new(),
        })
        .await
        .unwrap();
    assert_eq!(mismatch.missing_assets, 2);
    assert_eq!(mismatch.missing_asset_ids.len(), 2);
    database.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_and_refresh_reconcile_archive_only_assets_into_active_snapshots() {
    let root = TempDir::new().unwrap();
    let workspace = WorkspacePaths::new(root.path().join("workspace"));
    let source = ModuleId::try_from("fixture-source").unwrap();
    workspace.enable_source(&source).unwrap();
    let database = DatabaseService::start(&root.path().join("app-data/storage.duckdb"), 8)
        .await
        .unwrap();
    let dependencies = IngestionDependencies {
        workspace: workspace.clone(),
        source_module: fake_module(&root, source.clone()),
        archive: ArchiveCoordinator::new(workspace.clone(), source.clone()),
        database: database.clone(),
        runtime: fake_runtime(nutrition_batch()),
        limits: RuntimeLimits {
            max_memory_bytes: 8 * 1024 * 1024,
            fuel: 1_000_000,
            timeout: std::time::Duration::from_secs(1),
            max_output_bytes: 64 * 1024,
        },
        queue_capacity: 4,
    };
    let coordinator = IngestionCoordinator::start(dependencies).unwrap();

    let archive_orphan = |name: &str, bytes: &[u8]| {
        let inbox_path = workspace.source_inbox(&source).join(name);
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
                received_at(),
            )
            .unwrap();
        fs::remove_file(inbox_path).unwrap();
    };

    archive_orphan("startup.fixture", b"startup orphan");
    coordinator
        .request_scan(ScanRequest::new(ScanReason::Startup, received_at()))
        .await
        .unwrap();
    let first = database
        .execute(QueryView::active_snapshot(
            LogicalSnapshotKey::new("fixture:2026").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(first.counts.total, 1);

    archive_orphan("refresh.fixture", b"refresh orphan");
    coordinator
        .request_scan(ScanRequest::new(ScanReason::Manual, received_at()))
        .await
        .unwrap();
    let second = database
        .execute(QueryView::active_snapshot(
            LogicalSnapshotKey::new("fixture:2026").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(second.counts.total, 1);
    assert_ne!(second.snapshot_id, first.snapshot_id);

    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();
}
