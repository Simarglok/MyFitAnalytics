use chrono::{TimeZone, Timelike, Utc};
use mfa_archive::{ArchiveCoordinator, ArchiveReconciler, FileFingerprint, StableCandidate};
use mfa_config::WorkspacePaths;
use mfa_contracts::{ModuleId, UtcInstant};
use mfa_db::{ArchiveAssetRecord, DatabaseService, ReconcileArchiveInventory, RegisterAsset};
use std::fs;
use std::time::SystemTime;
use tempfile::TempDir;

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
