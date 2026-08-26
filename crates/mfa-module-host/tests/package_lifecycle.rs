use mfa_contracts::{ModuleId, ModuleType};
use mfa_module_host::{InstalledModule, ModuleRegistry, PackageInstaller};
use semver::Version;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

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

#[allow(dead_code)]
fn _cursor_package(bytes: Vec<u8>) -> Cursor<Vec<u8>> {
    Cursor::new(bytes)
}

#[allow(dead_code)]
fn _path_for(module: &InstalledModule) -> PathBuf {
    module.root.clone()
}
