use mfa_contracts::{ModuleId, ModuleType};
use mfa_module_host::{InstalledModule, ModuleRegistry, PackageInstaller};
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
        "entrypoint_hash": entry_hash,
        "localization_namespace": "source.test"
    })
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
    assert_eq!(installer.list().unwrap().len(), 2);
}

#[test]
fn disable_persists_without_deleting_bytes_and_uninstall_selects_latest_package() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let old = install_fixture(&installer, &packages, "test-source", "1.0.0");
    let new = install_fixture(&installer, &packages, "test-source", "2.0.0");

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
