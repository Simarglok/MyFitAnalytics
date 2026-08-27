use mfa_config::AppSettings;
use mfa_contracts::{CapabilityId, ModuleId};
use mfa_module_host::{CapabilityRegistry, ModuleRegistry, PackageInstaller};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use zip::write::SimpleFileOptions;

fn package(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../dist/modules")
        .join(name)
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn make_body_weight_provider(path: &Path) {
    let wasm = b"synthetic-body-weight-provider";
    let entry_hash = sha256(wasm);
    let manifest = json!({
        "module_type": "source",
        "module_id": "fixture-weight",
        "module_version": "1.0.0",
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
        "localization_namespace": "source.fixture-weight"
    });
    let file = fs::File::create(path).unwrap();
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    archive.start_file("module.json", options).unwrap();
    archive
        .write_all(serde_json::to_vec(&manifest).unwrap().as_slice())
        .unwrap();
    archive.start_file("module.wasm", options).unwrap();
    archive.write_all(wasm).unwrap();
    archive.finish().unwrap();
}

#[test]
fn bundled_defaults_and_atomic_provider_switch_keep_sources_separate() {
    let temp = TempDir::new().unwrap();
    let packages = TempDir::new().unwrap();
    let installer = PackageInstaller::new(temp.path().join("module-store"));
    let hevy = installer.install(&package("hevy.mfasource")).unwrap();
    let mynetdiary = installer.install(&package("mynetdiary.mfasource")).unwrap();
    let alternate_path = packages.path().join("fixture-weight.mfasource");
    make_body_weight_provider(&alternate_path);
    let alternate = installer.install(&alternate_path).unwrap();

    let modules = installer.list().unwrap();
    assert_eq!(modules.len(), 3);
    assert!(
        modules
            .iter()
            .any(|module| module.module_id == hevy.module_id)
    );
    assert!(
        modules
            .iter()
            .any(|module| module.module_id == mynetdiary.module_id)
    );
    assert!(
        modules
            .iter()
            .any(|module| module.module_id == alternate.module_id)
    );

    let registry = CapabilityRegistry::new();
    let mut settings = AppSettings::default();
    let defaults = registry
        .apply_bundled_defaults(&modules, &mut settings)
        .unwrap();
    assert_eq!(
        defaults
            .active_providers
            .get(&CapabilityId::try_from("nutrition.items").unwrap())
            .unwrap()
            .as_str(),
        "mynetdiary"
    );
    assert_eq!(
        defaults
            .active_providers
            .get(&CapabilityId::try_from("activity.events").unwrap())
            .unwrap()
            .as_str(),
        "mynetdiary"
    );
    assert_eq!(
        defaults
            .active_providers
            .get(&CapabilityId::try_from("body.weight").unwrap())
            .unwrap()
            .as_str(),
        "hevy"
    );
    assert_eq!(
        settings.active_providers.len(),
        defaults.active_providers.len()
    );

    let before_failed_switch = settings.clone();
    let missing = ModuleId::try_from("missing-provider").unwrap();
    assert!(
        registry
            .select_provider(
                &modules,
                &mut settings,
                &CapabilityId::try_from("body.weight").unwrap(),
                &missing,
            )
            .is_err()
    );
    assert_eq!(settings, before_failed_switch);

    let switched = registry
        .select_provider(
            &modules,
            &mut settings,
            &CapabilityId::try_from("body.weight").unwrap(),
            &alternate.module_id,
        )
        .unwrap();
    assert_eq!(
        switched
            .active_providers
            .get(&CapabilityId::try_from("body.weight").unwrap())
            .unwrap(),
        &alternate.module_id
    );
    assert_eq!(
        switched
            .active_providers
            .get(&CapabilityId::try_from("nutrition.items").unwrap())
            .unwrap()
            .as_str(),
        "mynetdiary"
    );
    assert_eq!(installer.list().unwrap().len(), 3);
}
