use mfa_module_host::PackageInstaller;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use tempfile::TempDir;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn hash(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn source_manifest(entry_hash: &str, api: &str) -> serde_json::Value {
    json!({
        "module_type": "source",
        "module_id": "security-source",
        "module_version": "1.0.0",
        "package_format_version": "1.0.0",
        "source_api_version": api,
        "mapping_version": "1.0.0",
        "compatible_app_versions": [">=0.1.0"],
        "provided_capabilities": ["body.weight"],
        "accepted_file_patterns": ["*.csv"],
        "artifact_signatures": [entry_hash],
        "extension_contracts": [],
        "settings_schema": {},
        "entrypoint_hash": entry_hash,
        "localization_namespace": "source.security"
    })
}

fn dashboard_manifest(entry_hash: &str) -> serde_json::Value {
    json!({
        "module_type": "dashboard",
        "module_id": "security-dashboard",
        "module_version": "1.0.0",
        "package_format_version": "1.0.0",
        "dashboard_api_version": "1.0.0",
        "entrypoint_hash": entry_hash,
        "compatible_app_versions": [">=0.1.0"],
        "required_capabilities": [{"capability": "body.weight"}],
        "required_extension_contracts": [],
        "localization_namespace": "dashboard.security"
    })
}

fn locale_manifest(executable: bool) -> serde_json::Value {
    json!({
        "module_type": "locale",
        "module_id": "security-locale",
        "locale": "en",
        "display_name": "English",
        "module_version": "1.0.0",
        "package_format_version": "1.0.0",
        "compatible_app_versions": [">=0.1.0"],
        "localization_namespace": "locale.security",
        "files": [{"path": "messages.json", "sha256": "sha256:aa", "executable": executable}]
    })
}

fn write_zip(path: &Path, entries: Vec<(&str, &[u8], Option<u32>)>) {
    let file = fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    for (name, bytes, mode) in entries {
        let mut options =
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        if let Some(mode) = mode {
            options = options.unix_permissions(mode);
        }
        writer.start_file(name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
}

fn write_symlink_zip(path: &Path) {
    let mut cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(&mut cursor);
    for (name, bytes) in [
        ("module.json", b"{}".as_slice()),
        ("module.wasm", b"component".as_slice()),
        ("link", b"module.wasm".as_slice()),
    ] {
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        writer.start_file(name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
    let mut bytes = cursor.into_inner();
    let signature = [0x50, 0x4b, 0x01, 0x02];
    let mut position = 0;
    while let Some(relative) = bytes[position..]
        .windows(signature.len())
        .position(|window| window == signature)
    {
        let central = position + relative;
        let name_length = u16::from_le_bytes([bytes[central + 28], bytes[central + 29]]) as usize;
        let name_start = central + 46;
        if &bytes[name_start..name_start + name_length] == b"link" {
            bytes[central + 4..central + 6].copy_from_slice(&(3u16 << 8 | 20).to_le_bytes());
            bytes[central + 38..central + 42].copy_from_slice(&(0o120777u32 << 16).to_le_bytes());
            break;
        }
        position = central + 46 + name_length;
    }
    fs::write(path, bytes).unwrap();
}

fn write_duplicate_manifest_zip(path: &Path, manifest: &[u8], wasm: &[u8]) {
    write_zip(
        path,
        vec![("module.json", manifest, None), ("module.wasm", wasm, None)],
    );
    let mut bytes = fs::read(path).unwrap();
    let central_signature = [0x50, 0x4b, 0x01, 0x02];
    let end_signature = [0x50, 0x4b, 0x05, 0x06];
    let end = bytes
        .windows(end_signature.len())
        .rposition(|window| window == end_signature)
        .unwrap();
    let mut position = 0;
    let duplicate = loop {
        let relative = bytes[position..]
            .windows(central_signature.len())
            .position(|window| window == central_signature)
            .unwrap();
        let central = position + relative;
        let name_length = u16::from_le_bytes([bytes[central + 28], bytes[central + 29]]) as usize;
        let extra_length = u16::from_le_bytes([bytes[central + 30], bytes[central + 31]]) as usize;
        let comment_length =
            u16::from_le_bytes([bytes[central + 32], bytes[central + 33]]) as usize;
        let name_start = central + 46;
        let record_length = 46 + name_length + extra_length + comment_length;
        if &bytes[name_start..name_start + name_length] == b"module.json" {
            break (central, record_length);
        }
        position = central + record_length;
    };
    let record = bytes[duplicate.0..duplicate.0 + duplicate.1].to_vec();
    bytes.splice(end..end, record.iter().copied());
    let new_end = end + record.len();
    for offset in [8usize, 10usize] {
        let count = u16::from_le_bytes([bytes[new_end + offset], bytes[new_end + offset + 1]]);
        bytes[new_end + offset..new_end + offset + 2].copy_from_slice(&(count + 1).to_le_bytes());
    }
    let size = u32::from_le_bytes([
        bytes[new_end + 12],
        bytes[new_end + 13],
        bytes[new_end + 14],
        bytes[new_end + 15],
    ]);
    bytes[new_end + 12..new_end + 16].copy_from_slice(&(size + record.len() as u32).to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn assert_code(installer: &PackageInstaller, path: &Path, expected: &str) {
    let error = installer.inspect(path).unwrap_err();
    assert_eq!(error.code(), expected, "expected {expected}, got {error:?}");
}

#[test]
fn traversal_and_absolute_paths_are_rejected() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let wasm = b"component";
    let manifest = serde_json::to_vec(&source_manifest(&hash(wasm), "1.0.0")).unwrap();
    let traversal = packages.path().join("traversal.mfasource");
    write_zip(
        &traversal,
        vec![
            ("../escape", b"x", None),
            ("module.json", &manifest, None),
            ("module.wasm", wasm, None),
        ],
    );
    assert_code(
        &PackageInstaller::new(store.path()),
        &traversal,
        "path_traversal",
    );

    let absolute = packages.path().join("absolute.mfasource");
    write_zip(
        &absolute,
        vec![
            ("/escape", b"x", None),
            ("module.json", &manifest, None),
            ("module.wasm", wasm, None),
        ],
    );
    assert_code(
        &PackageInstaller::new(store.path()),
        &absolute,
        "absolute_path",
    );

    let windows_traversal = packages.path().join("windows-traversal.mfasource");
    write_zip(
        &windows_traversal,
        vec![
            ("..\\escape", b"x", None),
            ("module.json", &manifest, None),
            ("module.wasm", wasm, None),
        ],
    );
    assert_code(
        &PackageInstaller::new(store.path()),
        &windows_traversal,
        "path_traversal",
    );

    let windows_drive = packages.path().join("windows-drive.mfasource");
    write_zip(
        &windows_drive,
        vec![
            ("C:\\escape", b"x", None),
            ("module.json", &manifest, None),
            ("module.wasm", wasm, None),
        ],
    );
    assert_code(
        &PackageInstaller::new(store.path()),
        &windows_drive,
        "absolute_path",
    );
}

#[test]
fn symlink_duplicate_manifest_and_hash_mismatch_have_distinct_codes() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let wasm = b"component";
    let manifest = serde_json::to_vec(&source_manifest(&hash(wasm), "1.0.0")).unwrap();

    let symlink = packages.path().join("symlink.mfasource");
    write_symlink_zip(&symlink);
    assert_code(&installer, &symlink, "symlink_entry");

    let duplicate = packages.path().join("duplicate.mfasource");
    write_duplicate_manifest_zip(&duplicate, &manifest, wasm);
    assert_code(&installer, &duplicate, "duplicate_manifest");

    let mismatch = packages.path().join("mismatch.mfasource");
    let bad_manifest = serde_json::to_vec(&source_manifest(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "1.0.0",
    ))
    .unwrap();
    write_zip(
        &mismatch,
        vec![
            ("module.json", &bad_manifest, None),
            ("module.wasm", wasm, None),
        ],
    );
    assert_code(&installer, &mismatch, "entrypoint_hash_mismatch");
}

#[test]
fn dashboard_entrypoint_hash_is_checked_before_installation() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let wasm = b"dashboard-component";
    let wrong_hash = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let bad_manifest = serde_json::to_vec(&dashboard_manifest(wrong_hash)).unwrap();
    let bad_path = packages.path().join("dashboard-bad.mfadashboard");
    write_zip(
        &bad_path,
        vec![
            ("module.json", &bad_manifest, None),
            ("module.wasm", wasm, None),
        ],
    );
    assert_code(&installer, &bad_path, "entrypoint_hash_mismatch");

    let good_manifest = serde_json::to_vec(&dashboard_manifest(&hash(wasm))).unwrap();
    let good_path = packages.path().join("dashboard-good.mfadashboard");
    write_zip(
        &good_path,
        vec![
            ("module.json", &good_manifest, None),
            ("module.wasm", wasm, None),
        ],
    );
    assert!(installer.inspect(&good_path).is_ok());
}

#[test]
fn extension_api_executable_locale_and_size_limits_are_rejected() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let wasm = b"component";
    let manifest = serde_json::to_vec(&source_manifest(&hash(wasm), "1.0.0")).unwrap();

    let wrong_extension = packages.path().join("wrong.mfadashboard");
    write_zip(
        &wrong_extension,
        vec![
            ("module.json", &manifest, None),
            ("module.wasm", wasm, None),
        ],
    );
    assert_code(&installer, &wrong_extension, "module_extension_mismatch");

    let incompatible = packages.path().join("incompatible.mfasource");
    let bad_api = serde_json::to_vec(&source_manifest(&hash(wasm), "2.0.0")).unwrap();
    write_zip(
        &incompatible,
        vec![("module.json", &bad_api, None), ("module.wasm", wasm, None)],
    );
    assert_code(&installer, &incompatible, "incompatible_source_api");

    let locale = packages.path().join("executable.mfalocale");
    let locale_manifest = serde_json::to_vec(&locale_manifest(true)).unwrap();
    write_zip(
        &locale,
        vec![
            ("module.json", &locale_manifest, None),
            ("messages.json", b"{}", Some(0o100755)),
        ],
    );
    assert_code(&installer, &locale, "executable_locale_entry");

    let oversized = packages.path().join("oversized.mfasource");
    write_zip(
        &oversized,
        vec![
            ("module.json", &manifest, None),
            ("module.wasm", &[b'x'; 32], None),
        ],
    );
    let limited = PackageInstaller::new(store.path()).with_max_uncompressed_bytes(16);
    assert_code(&limited, &oversized, "uncompressed_size_limit");
}

#[test]
fn malformed_manifest_and_missing_entrypoint_are_rejected() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());

    let malformed = packages.path().join("malformed.mfasource");
    write_zip(&malformed, vec![("module.json", b"not-json", None)]);
    assert_code(&installer, &malformed, "manifest_invalid_json");

    let missing = packages.path().join("missing-entrypoint.mfasource");
    let manifest = serde_json::to_vec(&source_manifest("sha256:aa", "1.0.0")).unwrap();
    write_zip(&missing, vec![("module.json", &manifest, None)]);
    assert_code(&installer, &missing, "entrypoint_missing");
}

#[cfg(unix)]
#[test]
fn package_path_permissions_are_not_used_as_authority() {
    let store = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let path = packages.path().join("normal.mfasource");
    let wasm = b"component";
    let manifest = serde_json::to_vec(&source_manifest(&hash(wasm), "1.0.0")).unwrap();
    write_zip(
        &path,
        vec![
            ("module.json", &manifest, None),
            ("module.wasm", wasm, None),
        ],
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(PackageInstaller::new(store.path()).inspect(&path).is_ok());
}

#[test]
fn checked_in_traversal_fixture_is_rejected() {
    let store = TempDir::new().unwrap();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/traversal-source.mfasource");
    assert_code(
        &PackageInstaller::new(store.path()),
        &fixture,
        "path_traversal",
    );
}
