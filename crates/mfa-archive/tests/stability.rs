use chrono::{TimeZone, Utc};
use mfa_archive::{
    FileFingerprint, ScanReason, ScanRequest, StabilityState, StabilityTracker, is_ignored_path,
};
use mfa_contracts::UtcInstant;
use std::fs;
use std::time::SystemTime;
use tempfile::TempDir;

fn fingerprint(path: &std::path::Path) -> FileFingerprint {
    let metadata = fs::metadata(path).unwrap();
    FileFingerprint {
        size: metadata.len(),
        modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
    }
}

#[test]
fn temporary_and_hidden_names_are_not_candidates() {
    for name in [
        ".hidden.csv",
        "~$export.xls",
        "export.xls.part",
        "export.xls.tmp",
        "export.archive-tmp",
        "export.archive.tmp",
    ] {
        assert!(is_ignored_path(std::path::Path::new(name)), "{name}");
    }
    assert!(!is_ignored_path(std::path::Path::new("export.xls")));
}

#[test]
fn a_file_becomes_stable_after_two_equal_readable_observations() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("export.xls");
    fs::write(&path, b"stable bytes").unwrap();
    let observed = fingerprint(&path);
    let mut tracker = StabilityTracker::new();

    assert!(matches!(
        tracker.observe(&path, observed.clone()),
        StabilityState::Waiting
    ));
    assert!(matches!(
        tracker.observe_readable(&path, observed),
        StabilityState::Stable(candidate) if candidate.path == path
    ));
}

#[test]
fn an_unreadable_candidate_never_becomes_stable() {
    let root = TempDir::new().unwrap();
    let directory = root.path().join("not-a-file.csv");
    fs::create_dir(&directory).unwrap();
    let observed = FileFingerprint {
        size: 0,
        modified: SystemTime::UNIX_EPOCH,
    };
    let mut tracker = StabilityTracker::new();

    assert!(matches!(
        tracker.observe(&directory, observed.clone()),
        StabilityState::Waiting
    ));
    assert!(matches!(
        tracker.observe_readable(&directory, observed),
        StabilityState::Unavailable
    ));
}

#[test]
fn watcher_and_periodic_scans_share_the_same_typed_request() {
    fn accepts_scan_request(_: ScanRequest) {}
    let requested_at = UtcInstant::from(Utc.with_ymd_and_hms(2026, 8, 25, 12, 0, 0).unwrap());

    accepts_scan_request(ScanRequest::new(ScanReason::Watcher, requested_at.clone()));
    accepts_scan_request(ScanRequest::new(ScanReason::Periodic, requested_at));
}
