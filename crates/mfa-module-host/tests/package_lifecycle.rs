use mfa_contracts::{ModuleId, ModuleType};
use mfa_module_host::{InstalledModule, ModuleRegistry, PackageInstaller};
use semver::Version;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use uuid::Uuid;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt, symlink};

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn source_manifest(module_id: &str, version: &str, entry_hash: &str) -> serde_json::Value {
    json!({
        "module_type": "source",
        "module_id": module_id,
        "module_version": version,
        "package_format_version": "1.0.0",
        "source_api_version": "1.0.0",
        "mapping_version": "1.0.0",
        "compatible_app_versions": [">=0.1.0"],
        "provided_capabilities": ["body.weight"],
        "accepted_file_patterns": ["*.csv"],
        "artifact_signatures": [entry_hash],
        "extension_contracts": [],
        "settings_schema": {},
        "entrypoint_hash": entry_hash,
        "localization_namespace": "source.test"
    })
}

fn source_manifest_with_compatible_versions(
    module_id: &str,
    version: &str,
    entry_hash: &str,
    compatible_app_versions: &[&str],
) -> serde_json::Value {
    let mut manifest = source_manifest(module_id, version, entry_hash);
    manifest["compatible_app_versions"] = json!(compatible_app_versions);
    manifest
}

fn make_package(path: &Path, extension: &str, manifest: serde_json::Value, wasm: &[u8]) {
    let file = fs::File::create(path).unwrap();
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    archive.start_file("module.json", options).unwrap();
    archive
        .write_all(serde_json::to_vec(&manifest).unwrap().as_slice())
        .unwrap();
    archive.start_file("module.wasm", options).unwrap();
    archive.write_all(wasm).unwrap();
    archive.finish().unwrap();
    assert_eq!(
        path.extension().and_then(|value| value.to_str()),
        Some(extension)
    );
}

fn install_fixture(
    installer: &PackageInstaller,
    dir: &TempDir,
    id: &str,
    version: &str,
) -> InstalledModule {
    let wasm = format!("component-{id}-{version}").into_bytes();
    let path = dir.path().join(format!("{id}-{version}.mfasource"));
    let manifest = source_manifest(id, version, &sha256(&wasm));
    make_package(&path, "mfasource", manifest, &wasm);
    installer.install(&path).unwrap()
}

#[test]
fn reopened_and_refreshed_packages_reject_newer_incompatible_app_versions() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installed_under_old_app =
        PackageInstaller::with_app_version(store.path(), Version::parse("0.1.0").unwrap());
    let upgraded = install_fixture(
        &installed_under_old_app,
        &packages,
        "upgraded-source",
        "1.0.0",
    );
    let upgraded_manifest_path = upgraded.root.join("module.json");
    let mut upgraded_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&upgraded_manifest_path).unwrap()).unwrap();
    upgraded_manifest["compatible_app_versions"] = json!(["<2.0.0"]);
    fs::write(
        &upgraded_manifest_path,
        serde_json::to_vec(&upgraded_manifest).unwrap(),
    )
    .unwrap();
    let invalid = install_fixture(
        &installed_under_old_app,
        &packages,
        "invalid-installed-source",
        "1.0.0",
    );
    let invalid_manifest_path = invalid.root.join("module.json");
    let mut invalid_manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&invalid_manifest_path).unwrap()).unwrap();
    invalid_manifest["compatible_app_versions"] = json!(["not-a-semver-range"]);
    fs::write(
        &invalid_manifest_path,
        serde_json::to_vec(&invalid_manifest).unwrap(),
    )
    .unwrap();

    let reopened =
        PackageInstaller::with_app_version(store.path(), Version::parse("2.0.0").unwrap());
    let startup = reopened.list().unwrap();
    assert_eq!(startup.len(), 2);
    assert!(startup.iter().all(|module| !module.enabled));
    assert_eq!(
        reopened
            .set_enabled(&upgraded.module_id, true)
            .unwrap_err()
            .code(),
        "incompatible_app_version"
    );
    assert_eq!(
        reopened
            .set_enabled(&invalid.module_id, true)
            .unwrap_err()
            .code(),
        "incompatible_app_version"
    );
    let refreshed = reopened.list().unwrap();
    assert_eq!(refreshed.len(), 2);
    assert!(refreshed.iter().all(|module| !module.enabled));
}

#[test]
fn install_is_content_addressed_atomic_and_idempotent() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let first = install_fixture(&installer, &packages, "test-source", "1.0.0");

    assert_eq!(first.module_id, ModuleId::try_from("test-source").unwrap());
    assert_eq!(first.module_type, ModuleType::Source);
    assert!(first.root.exists());
    assert!(
        first
            .root
            .ends_with(format!("test-source/1.0.0/{}", first.package_hash))
    );

    let second = installer
        .install(&packages.path().join("test-source-1.0.0.mfasource"))
        .unwrap();
    assert_eq!(second.package_hash, first.package_hash);
    assert_eq!(second.root, first.root);
    assert_eq!(installer.list().unwrap().len(), 1);
    assert!(!store.path().join("test-source/1.0.0/.staging").exists());
}

#[test]
fn app_version_compatibility_rejects_nonmatching_and_invalid_ranges_before_install() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer =
        PackageInstaller::with_app_version(store.path(), Version::parse("0.1.0").unwrap());
    let wasm = b"incompatible-component";
    let mismatch_path = packages.path().join("mismatch.mfasource");
    make_package(
        &mismatch_path,
        "mfasource",
        source_manifest_with_compatible_versions(
            "mismatch-source",
            "1.0.0",
            &sha256(wasm),
            &[">=9.0.0"],
        ),
        wasm,
    );
    assert_eq!(
        installer.install(&mismatch_path).unwrap_err().code(),
        "incompatible_app_version"
    );

    let invalid_path = packages.path().join("invalid-range.mfasource");
    make_package(
        &invalid_path,
        "mfasource",
        source_manifest_with_compatible_versions(
            "invalid-range-source",
            "1.0.0",
            &sha256(wasm),
            &["not-a-semver-range"],
        ),
        wasm,
    );
    assert_eq!(
        installer.install(&invalid_path).unwrap_err().code(),
        "incompatible_app_version"
    );
    assert!(!store.path().join("mismatch-source").exists());
    assert!(!store.path().join("invalid-range-source").exists());
}

#[test]
fn legacy_disabled_state_migrates_to_the_latest_active_package() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let old = install_fixture(&installer, &packages, "legacy-source", "1.0.0");
    let latest = install_fixture(&installer, &packages, "legacy-source", "2.0.0");

    fs::write(
        store.path().join("state.json"),
        serde_json::to_vec_pretty(&json!({
            "modules": {"legacy-source": false}
        }))
        .unwrap(),
    )
    .unwrap();

    let migrated = installer.list().unwrap();
    assert_eq!(migrated.len(), 1);
    assert_eq!(migrated[0].module_version.to_string(), "2.0.0");
    assert!(!migrated[0].enabled);
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path().join("state.json")).unwrap()).unwrap();
    assert_eq!(
        state["active_packages"]["legacy-source"]["module_version"],
        "2.0.0"
    );
    assert_eq!(
        state["active_packages"]["legacy-source"]["package_hash"],
        latest.package_hash
    );

    installer
        .set_enabled(&ModuleId::try_from("legacy-source").unwrap(), true)
        .unwrap();
    assert!(
        installer
            .resolve_active(&ModuleId::try_from("legacy-source").unwrap())
            .is_ok()
    );
    installer
        .set_enabled(&ModuleId::try_from("legacy-source").unwrap(), false)
        .unwrap();
    installer
        .uninstall(&ModuleId::try_from("legacy-source").unwrap())
        .unwrap();
    assert!(!latest.root.exists());
    assert!(old.root.exists());
}

#[test]
fn failed_update_keeps_old_version_and_valid_update_adds_new_version() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let old = install_fixture(&installer, &packages, "test-source", "1.0.0");

    let bad_bytes = b"new-component";
    let bad_path = packages.path().join("test-source-2.0.0.mfasource");
    make_package(
        &bad_path,
        "mfasource",
        source_manifest(
            "test-source",
            "2.0.0",
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        ),
        bad_bytes,
    );
    assert_eq!(
        installer.install(&bad_path).unwrap_err().code(),
        "entrypoint_hash_mismatch"
    );
    assert!(old.root.exists());
    assert_eq!(installer.list().unwrap().len(), 1);
    assert_eq!(
        installer
            .resolve_active(&ModuleId::try_from("test-source").unwrap())
            .unwrap()
            .module_version
            .to_string(),
        "1.0.0"
    );

    let good_path = packages.path().join("test-source-2.0.0-good.mfasource");
    make_package(
        &good_path,
        "mfasource",
        source_manifest("test-source", "2.0.0", &sha256(b"new-component")),
        bad_bytes,
    );
    let new = installer.install(&good_path).unwrap();
    assert!(old.root.exists());
    assert!(new.root.exists());
    assert_eq!(installer.list().unwrap().len(), 1);
    assert_eq!(
        installer.list().unwrap()[0].module_version.to_string(),
        "2.0.0"
    );
}

#[test]
fn disable_persists_without_deleting_bytes_and_uninstall_selects_latest_package() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let old = install_fixture(&installer, &packages, "test-source", "1.0.0");
    let new = install_fixture(&installer, &packages, "test-source", "2.0.0");
    let current = installer.list().unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].module_version.to_string(), "2.0.0");
    assert_eq!(
        installer
            .resolve_active(&ModuleId::try_from("test-source").unwrap())
            .unwrap()
            .module_version
            .to_string(),
        "2.0.0"
    );

    installer
        .set_enabled(&ModuleId::try_from("test-source").unwrap(), false)
        .unwrap();
    let disabled = installer.list().unwrap();
    assert!(disabled.iter().all(|module| !module.enabled));
    assert!(old.root.exists());
    assert!(new.root.exists());
    assert_eq!(
        installer
            .resolve_active(&ModuleId::try_from("test-source").unwrap())
            .unwrap_err()
            .code(),
        "no_active_module"
    );

    installer
        .uninstall(&ModuleId::try_from("test-source").unwrap())
        .unwrap();
    assert!(!new.root.exists());
    assert!(old.root.exists());
    assert!(installer.list().unwrap().is_empty());
    assert_eq!(
        installer
            .set_enabled(&ModuleId::try_from("test-source").unwrap(), true)
            .unwrap_err()
            .code(),
        "module_not_found"
    );
}

#[test]
fn registry_reconstructs_from_manifests_without_mutable_index() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let installed = install_fixture(&installer, &packages, "test-source", "1.0.0");
    let state = store.path().join("state.json");
    assert!(state.exists());
    fs::remove_file(state).unwrap();

    let reconstructed = installer.list().unwrap();
    assert_eq!(reconstructed.len(), 1);
    assert_eq!(reconstructed[0].package_hash, installed.package_hash);
    assert_eq!(reconstructed[0].module_id, installed.module_id);
}

#[test]
fn bundled_catalog_installs_first_profile_defaults_and_reports_updates_without_reinstalling() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let old_wasm = b"component-bundled-source-1.0.0";
    let old_path = packages.path().join("bundled-source-1.0.0.mfasource");
    make_package(
        &old_path,
        "mfasource",
        source_manifest("bundled-source", "1.0.0", &sha256(old_wasm)),
        old_wasm,
    );

    let catalog = installer
        .install_bundled_defaults(std::slice::from_ref(&old_path))
        .unwrap();
    assert_eq!(catalog.len(), 1);
    assert_eq!(installer.list().unwrap().len(), 1);
    assert_eq!(
        installer.list().unwrap()[0].module_version.to_string(),
        "1.0.0"
    );

    let new_path = packages.path().join("bundled-source-2.0.0.mfasource");
    make_package(
        &new_path,
        "mfasource",
        source_manifest("bundled-source", "2.0.0", &sha256(b"new-bundled-component")),
        b"new-bundled-component",
    );
    installer
        .install_bundled_defaults(std::slice::from_ref(&new_path))
        .unwrap();
    let updates = installer.available_bundled_updates().unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].available.module_version.to_string(), "2.0.0");
    assert_eq!(installer.list().unwrap().len(), 1);

    installer
        .uninstall(&ModuleId::try_from("bundled-source").unwrap())
        .unwrap();
    installer
        .install_bundled_defaults(std::slice::from_ref(&new_path))
        .unwrap();
    assert!(installer.list().unwrap().is_empty());
}

#[test]
fn checked_in_valid_source_fixture_is_installable() {
    let store = TempDir::new().unwrap();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid-source.mfasource");
    let installer = PackageInstaller::new(store.path());

    let inspected = installer.inspect(&fixture).unwrap();
    assert_eq!(
        *inspected.manifest.module_id(),
        ModuleId::try_from("fixture-source").unwrap()
    );
    let installed = installer.install(&fixture).unwrap();
    assert!(installed.root.join("module.wasm").exists());
}

#[cfg(unix)]
#[test]
fn uninstall_reports_package_delete_failure_and_preserves_the_previous_state() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let installed = install_fixture(&installer, &packages, "delete-failure-source", "1.0.0");
    let module_id = installed.module_id.clone();
    installer.set_enabled(&module_id, false).unwrap();

    let mut transaction = installer.stage_uninstall(&module_id).unwrap();
    let staged_root = fs::read_dir(installed.root.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".uninstall-staging-"))
        })
        .unwrap();
    fs::set_permissions(&staged_root, fs::Permissions::from_mode(0o500)).unwrap();

    installer.apply_uninstall(&mut transaction).unwrap();
    let result = installer.finalize_uninstall(&mut transaction);

    fs::set_permissions(&staged_root, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err(), "package deletion failure was suppressed");
    installer.rollback_uninstall(&mut transaction).unwrap();
    assert!(installed.root.exists());
    assert_eq!(installer.list().unwrap().len(), 1);
    assert!(installer.resolve_active(&module_id).is_err());
    assert!(!has_uninstall_staging_directory(
        installed.root.parent().unwrap()
    ));
}

#[test]
fn reopening_after_an_interrupted_uninstall_restores_the_previous_package_and_state() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let installed = install_fixture(
        &installer,
        &packages,
        "interrupted-uninstall-source",
        "1.0.0",
    );
    let module_id = installed.module_id.clone();
    installer.set_enabled(&module_id, false).unwrap();

    let mut transaction = installer.stage_uninstall(&module_id).unwrap();
    installer.apply_uninstall(&mut transaction).unwrap();
    drop(transaction);
    drop(installer);

    let reopened = PackageInstaller::new(store.path());
    let modules = reopened.list().unwrap();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].module_id, module_id);
    assert!(!modules[0].enabled);
    assert!(installed.root.exists());
    assert!(reopened.resolve_active(&module_id).is_err());
}

#[test]
fn post_delete_read_failure_restores_package_bytes_and_registry_state() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path()).with_uninstall_finalization_fault(
        mfa_module_host::UninstallFinalizationFault::AfterDeleteBeforeRead,
    );
    let installed = install_fixture(&installer, &packages, "post-delete-read-source", "1.0.0");
    let module_id = installed.module_id.clone();
    installer.set_enabled(&module_id, false).unwrap();

    let error = installer.uninstall(&module_id).unwrap_err();

    assert_eq!(error.code(), "atomic_uninstall_failed");
    assert!(installed.root.exists());
    assert_eq!(installer.list().unwrap().len(), 1);
    assert!(installer.resolve_active(&module_id).is_err());
    assert!(!has_uninstall_staging_directory(
        installed.root.parent().unwrap()
    ));
}

#[test]
fn post_read_remove_failure_restores_package_bytes_and_registry_state() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path()).with_uninstall_finalization_fault(
        mfa_module_host::UninstallFinalizationFault::AfterReadBeforeRemoveVersion,
    );
    let installed = install_fixture(&installer, &packages, "post-read-remove-source", "1.0.0");
    let module_id = installed.module_id.clone();
    installer.set_enabled(&module_id, false).unwrap();

    let error = installer.uninstall(&module_id).unwrap_err();

    assert_eq!(error.code(), "atomic_uninstall_failed");
    assert!(installed.root.exists());
    assert_eq!(installer.list().unwrap().len(), 1);
    assert!(installer.resolve_active(&module_id).is_err());
}

#[test]
fn post_remove_sync_failure_restores_package_bytes_and_registry_state() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path()).with_uninstall_finalization_fault(
        mfa_module_host::UninstallFinalizationFault::AfterRemoveBeforeSync,
    );
    let installed = install_fixture(&installer, &packages, "post-remove-sync-source", "1.0.0");
    let module_id = installed.module_id.clone();
    installer.set_enabled(&module_id, false).unwrap();

    let error = installer.uninstall(&module_id).unwrap_err();

    assert_eq!(error.code(), "atomic_uninstall_failed");
    assert!(installed.root.exists());
    assert_eq!(installer.list().unwrap().len(), 1);
    assert!(installer.resolve_active(&module_id).is_err());
}

#[test]
fn corrupt_uninstall_journal_is_reported_without_panicking_during_recovery() {
    let store = TempDir::new().unwrap();
    fs::create_dir_all(store.path()).unwrap();
    fs::write(
        store.path().join(".uninstall-transaction.json"),
        r#"{
            "module_id": "",
            "original_root": "/tmp/original",
            "staged_root": "/tmp/staged",
            "backup_path": "/tmp/backup.zip",
            "version_root": "/tmp/version",
            "previous_state": {},
            "phase": "Prepared"
        }"#,
    )
    .unwrap();

    let result = std::panic::catch_unwind(|| PackageInstaller::new(store.path()).list());

    assert!(result.is_ok(), "corrupt journal recovery panicked");
    assert!(result.unwrap().is_err());
}

#[test]
fn malicious_uninstall_journal_cannot_modify_external_sentinel() {
    let store = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    let module_id = "malicious-journal-source";
    let package_hash = "a".repeat(64);
    let transaction_id = Uuid::new_v4();
    let external_version_root = external.path().join(module_id).join("1.0.0");
    let external_staged_root =
        external_version_root.join(format!(".uninstall-staging-{transaction_id}"));
    fs::create_dir_all(&external_staged_root).unwrap();
    let sentinel = external_staged_root.join("sentinel");
    fs::write(&sentinel, b"do-not-touch").unwrap();
    let external_original_root = external_version_root.join(&package_hash);
    let external_backup_path = external
        .path()
        .join(format!(".uninstall-backup-{transaction_id}.zip"));
    let journal = json!({
        "module_id": module_id,
        "original_root": external_original_root,
        "staged_root": external_staged_root,
        "backup_path": external_backup_path,
        "version_root": external_version_root,
        "previous_state": {
            "modules": {module_id: false},
            "active_packages": {
                module_id: {
                    "module_version": "1.0.0",
                    "package_hash": package_hash
                }
            },
            "uninstalled_modules": [],
            "bundled_catalog": {}
        },
        "phase": "Prepared"
    });
    fs::write(
        store.path().join(".uninstall-transaction.json"),
        serde_json::to_vec_pretty(&journal).unwrap(),
    )
    .unwrap();

    let result = PackageInstaller::new(store.path()).list();

    assert_eq!(result.unwrap_err().code(), "atomic_uninstall_failed");
    assert!(external_staged_root.exists());
    assert_eq!(fs::read(&sentinel).unwrap(), b"do-not-touch");
}

#[cfg(unix)]
#[test]
fn symlinked_uninstall_journal_path_cannot_modify_external_sentinel() {
    let store = TempDir::new().unwrap();
    let external = TempDir::new().unwrap();
    let module_id = "symlinked-journal-source";
    let package_hash = "b".repeat(64);
    let transaction_id = Uuid::new_v4();
    let external_module_root = external.path().join(module_id);
    let external_version_root = external_module_root.join("1.0.0");
    let external_staged_root =
        external_version_root.join(format!(".uninstall-staging-{transaction_id}"));
    fs::create_dir_all(&external_staged_root).unwrap();
    let sentinel = external_staged_root.join("sentinel");
    fs::write(&sentinel, b"do-not-touch").unwrap();
    fs::create_dir_all(store.path()).unwrap();
    symlink(&external_module_root, store.path().join(module_id)).unwrap();
    let store_version_root = store.path().join(module_id).join("1.0.0");
    let journal = json!({
        "module_id": module_id,
        "original_root": store_version_root.join(&package_hash),
        "staged_root": store_version_root.join(format!(".uninstall-staging-{transaction_id}")),
        "backup_path": store
            .path()
            .join(format!(".uninstall-backup-{transaction_id}.zip")),
        "version_root": store_version_root,
        "previous_state": {
            "modules": {module_id: false},
            "active_packages": {
                module_id: {
                    "module_version": "1.0.0",
                    "package_hash": package_hash
                }
            },
            "uninstalled_modules": [],
            "bundled_catalog": {}
        },
        "phase": "Prepared"
    });
    fs::write(
        store.path().join(".uninstall-transaction.json"),
        serde_json::to_vec_pretty(&journal).unwrap(),
    )
    .unwrap();

    let result = PackageInstaller::new(store.path()).list();

    assert_eq!(result.unwrap_err().code(), "atomic_uninstall_failed");
    assert!(external_staged_root.exists());
    assert_eq!(fs::read(&sentinel).unwrap(), b"do-not-touch");
}

#[test]
fn stage_move_failure_preserves_package_and_previous_state() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path()).with_uninstall_finalization_fault(
        mfa_module_host::UninstallFinalizationFault::BeforeStageMove,
    );
    let installed = install_fixture(&installer, &packages, "stage-move-failure-source", "1.0.0");
    let module_id = installed.module_id.clone();
    installer.set_enabled(&module_id, false).unwrap();

    let error = installer.uninstall(&module_id).unwrap_err();

    assert_eq!(error.code(), "atomic_uninstall_failed");
    assert!(installed.root.exists());
    assert_eq!(installer.list().unwrap().len(), 1);
    assert!(installer.resolve_active(&module_id).is_err());
    assert_uninstall_artifacts_absent(&store, installed.root.parent().unwrap());
}

#[test]
fn post_stage_move_sync_failure_restores_package_and_previous_state() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path()).with_uninstall_finalization_fault(
        mfa_module_host::UninstallFinalizationFault::AfterStageMoveBeforeSync,
    );
    let installed = install_fixture(&installer, &packages, "post-stage-sync-source", "1.0.0");
    let module_id = installed.module_id.clone();
    installer.set_enabled(&module_id, false).unwrap();

    let error = installer.uninstall(&module_id).unwrap_err();

    assert_eq!(error.code(), "atomic_uninstall_failed");
    assert!(installed.root.exists());
    assert_eq!(installer.list().unwrap().len(), 1);
    assert!(installer.resolve_active(&module_id).is_err());
    assert_uninstall_artifacts_absent(&store, installed.root.parent().unwrap());
}

#[test]
fn backup_read_failure_restores_staged_package_and_previous_state() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path()).with_uninstall_finalization_fault(
        mfa_module_host::UninstallFinalizationFault::BeforeBackupRead,
    );
    let installed = install_fixture(&installer, &packages, "backup-read-failure-source", "1.0.0");
    let module_id = installed.module_id.clone();
    installer.set_enabled(&module_id, false).unwrap();

    let error = installer.uninstall(&module_id).unwrap_err();

    assert_eq!(error.code(), "atomic_uninstall_failed");
    assert!(installed.root.exists());
    assert_eq!(installer.list().unwrap().len(), 1);
    assert_uninstall_artifacts_absent(&store, installed.root.parent().unwrap());
}

#[test]
fn backup_delete_failure_restores_package_and_previous_state() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path()).with_uninstall_finalization_fault(
        mfa_module_host::UninstallFinalizationFault::BeforeBackupDelete,
    );
    let installed = install_fixture(
        &installer,
        &packages,
        "backup-delete-failure-source",
        "1.0.0",
    );
    let module_id = installed.module_id.clone();
    installer.set_enabled(&module_id, false).unwrap();

    let error = installer.uninstall(&module_id).unwrap_err();

    assert_eq!(error.code(), "atomic_uninstall_failed");
    assert!(installed.root.exists());
    assert_eq!(installer.list().unwrap().len(), 1);
    assert_uninstall_artifacts_absent(&store, installed.root.parent().unwrap());
}

#[test]
fn state_sync_failure_preserves_package_and_previous_state() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path()).with_uninstall_finalization_fault(
        mfa_module_host::UninstallFinalizationFault::BeforeStateSync,
    );
    let installed = install_fixture(&installer, &packages, "state-sync-failure-source", "1.0.0");
    let module_id = installed.module_id.clone();
    installer.set_enabled(&module_id, false).unwrap();

    let error = installer.uninstall(&module_id).unwrap_err();

    assert_eq!(error.code(), "atomic_uninstall_failed");
    assert!(installed.root.exists());
    assert_eq!(installer.list().unwrap().len(), 1);
    assert!(installer.resolve_active(&module_id).is_err());
    assert_uninstall_artifacts_absent(&store, installed.root.parent().unwrap());
}

#[test]
fn recovery_restore_move_failure_keeps_the_transaction_for_a_clean_restart() {
    assert_recovery_fault_restores(
        mfa_module_host::UninstallFinalizationFault::BeforeRestoreMove,
        "recovery-restore-move-source",
    );
}

#[test]
fn recovery_restore_read_failure_keeps_the_transaction_for_a_clean_restart() {
    assert_recovery_fault_restores(
        mfa_module_host::UninstallFinalizationFault::BeforeRestoreRead,
        "recovery-restore-read-source",
    );
}

fn assert_recovery_fault_restores(
    fault: mfa_module_host::UninstallFinalizationFault,
    module_name: &str,
) {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let installed = install_fixture(&installer, &packages, module_name, "1.0.0");
    let module_id = installed.module_id.clone();
    let expected_wasm = fs::read(installed.root.join("module.wasm")).unwrap();
    installer.set_enabled(&module_id, false).unwrap();

    let mut transaction = installer.stage_uninstall(&module_id).unwrap();
    installer.apply_uninstall(&mut transaction).unwrap();
    drop(transaction);
    drop(installer);

    let faulted = PackageInstaller::new(store.path()).with_uninstall_finalization_fault(fault);
    let error = faulted.list().unwrap_err();
    assert_eq!(error.code(), "atomic_uninstall_failed");

    let reopened = PackageInstaller::new(store.path());
    let modules = reopened.list().unwrap();
    assert_eq!(modules.len(), 1);
    assert!(!modules[0].enabled);
    assert_eq!(
        fs::read(installed.root.join("module.wasm")).unwrap(),
        expected_wasm
    );
    assert_uninstall_artifacts_absent(&store, installed.root.parent().unwrap());
}

fn has_uninstall_staging_directory(version_root: &Path) -> bool {
    version_root.read_dir().unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".uninstall-staging-")
    })
}

fn assert_uninstall_artifacts_absent(store: &TempDir, version_root: &Path) {
    assert!(!store.path().join(".uninstall-transaction.json").exists());
    assert!(!store.path().read_dir().unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".uninstall-backup-")
    }));
    assert!(!has_uninstall_staging_directory(version_root));
}

#[allow(dead_code)]
fn _cursor_package(bytes: Vec<u8>) -> Cursor<Vec<u8>> {
    Cursor::new(bytes)
}

#[allow(dead_code)]
fn _path_for(module: &InstalledModule) -> PathBuf {
    module.root.clone()
}
