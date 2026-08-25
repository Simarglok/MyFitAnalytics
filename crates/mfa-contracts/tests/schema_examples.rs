use jsonschema::validator_for;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

fn schema(path: &str) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modules/sdk/schemas");
    serde_json::from_str(&fs::read_to_string(root.join(path)).unwrap()).unwrap()
}

fn assert_valid(schema_name: &str, example: Value) {
    let validator = validator_for(&schema(schema_name)).unwrap();
    let errors: Vec<_> = validator.iter_errors(&example).collect();
    assert!(errors.is_empty(), "{}: {errors:#?}", schema_name);
}

#[test]
fn source_manifest_example_requires_the_complete_contract() {
    assert_valid(
        "source-manifest.schema.json",
        json!({
            "module_type": "source",
            "module_id": "mynetdiary",
            "module_version": "1.0.0",
            "package_format_version": "1.0.0",
            "source_api_version": "1.0.0",
            "mapping_version": "1.0.0",
            "compatible_app_versions": [">=0.1.0"],
            "provided_capabilities": ["nutrition.items"],
            "accepted_file_patterns": ["*.xls"],
            "entrypoint_hash": "sha256:abc",
            "localization_namespace": "source.mynetdiary"
        }),
    );
}

#[test]
fn source_manifest_rejects_missing_security_fields() {
    let invalid = json!({
        "module_type": "source",
        "module_id": "mynetdiary",
        "module_version": "1.0.0"
    });
    let validator = validator_for(&schema("source-manifest.schema.json")).unwrap();
    assert!(!validator.is_valid(&invalid));
}

#[test]
fn dashboard_manifest_declares_base_and_extension_dependencies() {
    assert_valid(
        "dashboard-manifest.schema.json",
        json!({
            "module_type": "dashboard",
            "module_id": "base",
            "module_version": "1.0.0",
            "package_format_version": "1.0.0",
            "dashboard_api_version": "1.0.0",
            "entrypoint_hash": "sha256:abc",
            "compatible_app_versions": [">=0.1.0"],
            "required_capabilities": [{"capability": "body.weight"}],
            "required_extension_contracts": [{
                "namespace": "hevy.set-rpe",
                "contract_version": "1.0.0"
            }],
            "localization_namespace": "dashboard.base"
        }),
    );
}

#[test]
fn locale_manifest_rejects_executable_entries() {
    let validator = validator_for(&schema("locale-manifest.schema.json")).unwrap();
    let executable = json!({
        "module_type": "locale",
        "module_id": "ru",
        "locale": "ru",
        "display_name": "Русский",
        "module_version": "1.0.0",
        "package_format_version": "1.0.0",
        "compatible_app_versions": [">=0.1.0"],
        "localization_namespace": "locale.ru",
        "files": [{"path": "module.wasm", "sha256": "sha256:bad", "executable": true}]
    });
    assert!(!validator.is_valid(&executable));
}
