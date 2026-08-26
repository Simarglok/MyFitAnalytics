use chrono::{TimeZone, Timelike, Utc};
use mfa_archive::{
    ArchiveCoordinator, ArchiveDisposition, ArchiveError, FileFingerprint, StableCandidate,
};
use mfa_config::WorkspacePaths;
use mfa_contracts::{ModuleId, UtcInstant};
use std::fs;
use std::path::{Path, PathBuf};
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

fn candidate(path: &Path) -> StableCandidate {
    let metadata = fs::metadata(path).unwrap();
    StableCandidate {
        path: path.to_path_buf(),
        fingerprint: FileFingerprint {
            size: metadata.len(),
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        },
    }
}

fn coordinator(root: &TempDir) -> ArchiveCoordinator {
    ArchiveCoordinator::new(
        WorkspacePaths::new(root.path().join("workspace")),
        ModuleId::try_from("mynetdiary").unwrap(),
    )
}

#[test]
fn archive_copy_is_timestamped_hashed_and_same_directory_temporary() {
    let root = TempDir::new().unwrap();
    let inbox = root.path().join("inbox");
    fs::create_dir_all(&inbox).unwrap();
    let source = inbox.join("Export file (2026).xls");
    fs::write(&source, b"first bytes").unwrap();

    let archived = coordinator(&root)
        .archive(candidate(&source), received_at())
        .unwrap();

    assert_eq!(archived.disposition, ArchiveDisposition::Created);
    assert!(
        archived.archive_path.starts_with(
            root.path()
                .join("workspace/archive/mynetdiary/2026/2026-08-25")
        )
    );
    let name = archived.archive_path.file_name().unwrap().to_string_lossy();
    assert!(name.starts_with("20260825T123456.123456Z--"));
    assert!(name.contains("--Export_file_2026_.xls"));
    assert!(name.split("--").nth(1).is_some_and(|hash| hash.len() == 64));
    assert_eq!(fs::read(&archived.archive_path).unwrap(), b"first bytes");
    let temporary_files: Vec<PathBuf> = fs::read_dir(archived.archive_path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().contains("archive-tmp"))
        .collect();
    assert!(temporary_files.is_empty());
}

#[test]
fn exact_bytes_return_the_existing_identity_and_different_bytes_with_same_name_do_not_collide() {
    let root = TempDir::new().unwrap();
    let inbox = root.path().join("inbox");
    fs::create_dir_all(&inbox).unwrap();
    let first = inbox.join("same.csv");
    fs::write(&first, b"one").unwrap();
    let service = coordinator(&root);
    let created = service.archive(candidate(&first), received_at()).unwrap();

    let duplicate_path = inbox.join("renamed.csv");
    fs::write(&duplicate_path, b"one").unwrap();
    let duplicate_asset = service
        .archive(candidate(&duplicate_path), received_at())
        .unwrap();
    assert_eq!(
        duplicate_asset.disposition,
        ArchiveDisposition::ExistingExactDuplicate
    );
    assert_eq!(duplicate_asset.asset_id, created.asset_id);
    assert_eq!(duplicate_asset.archive_path, created.archive_path);

    fs::write(&duplicate_path, b"two").unwrap();
    let second = service
        .archive(candidate(&duplicate_path), received_at())
        .unwrap();
    assert_eq!(second.disposition, ArchiveDisposition::Created);
    assert_ne!(second.asset_id, created.asset_id);
    assert_ne!(second.archive_path, created.archive_path);
}

#[test]
fn failed_archive_leaves_inbox_untouched() {
    let root = TempDir::new().unwrap();
    let source = root.path().join("missing.csv");
    let error = coordinator(&root)
        .archive(
            StableCandidate {
                path: source.clone(),
                fingerprint: FileFingerprint {
                    size: 1,
                    modified: SystemTime::UNIX_EPOCH,
                },
            },
            received_at(),
        )
        .unwrap_err();

    assert!(matches!(error, ArchiveError::SourceUnavailable { .. }));
    assert!(!source.exists());
}
