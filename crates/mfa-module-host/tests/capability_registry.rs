use mfa_config::AppSettings;
use mfa_contracts::{
    CapabilityId, ContractVersion, ModuleId, ModuleManifest, ModuleType, SourceManifest,
};
use mfa_module_host::CapabilityRegistry;
use serde_json::json;
use std::collections::BTreeMap;
use tempfile::TempDir;

fn source_module(
    store: &TempDir,
    id: &str,
    enabled: bool,
    capability: &str,
) -> mfa_module_host::InstalledModule {
    let module_id = ModuleId::try_from(id).unwrap();
    let manifest: SourceManifest = serde_json::from_value(json!({
        "module_type": "source",
        "module_id": id,
        "module_version": "1.0.0",
        "package_format_version": "1.0.0",
        "source_api_version": "1.0.0",
        "mapping_version": "1.0.0",
        "compatible_app_versions": [">=0.1.0"],
        "provided_capabilities": [capability],
        "accepted_file_patterns": ["*.json"],
        "entrypoint_hash": "sha256:fixture",
        "localization_namespace": id
    }))
    .unwrap();
    mfa_module_host::InstalledModule {
        module_id,
        module_type: ModuleType::Source,
        module_version: ContractVersion::try_from("1.0.0").unwrap(),
        package_hash: format!("hash-{id}"),
        root: store.path().join(id),
        enabled,
        manifest: ModuleManifest::Source(manifest),
    }
}

fn settings(provider: Option<&str>) -> AppSettings {
    let mut active_providers = BTreeMap::new();
    if let Some(provider) = provider {
        active_providers.insert(
            CapabilityId::try_from("body.weight").unwrap(),
            ModuleId::try_from(provider).unwrap(),
        );
    }
    AppSettings {
        schema_version: mfa_config::CURRENT_SCHEMA_VERSION,
        locale: "en-US".to_owned(),
        active_providers,
    }
}

fn settings_for_two_providers() -> AppSettings {
    let mut settings = settings(Some("source-a"));
    settings.active_providers.insert(
        CapabilityId::try_from("nutrition.item").unwrap(),
        ModuleId::try_from("source-b").unwrap(),
    );
    settings
}

#[test]
fn explicit_provider_selection_is_single_and_deterministic() {
    let store = TempDir::new().unwrap();
    let modules = vec![
        source_module(&store, "source-b", true, "nutrition.item"),
        source_module(&store, "source-a", true, "body.weight"),
    ];
    let resolution = CapabilityRegistry::new()
        .resolve(&modules, &settings_for_two_providers())
        .unwrap();
    assert_eq!(resolution.active_providers.len(), 2);
    assert_eq!(
        resolution.active_providers.keys().next().unwrap().as_str(),
        "body.weight"
    );
    assert_eq!(
        resolution
            .active_providers
            .values()
            .next()
            .unwrap()
            .as_str(),
        "source-a"
    );
    assert_eq!(
        resolution.active_providers.keys().nth(1).unwrap().as_str(),
        "nutrition.item"
    );
}

#[test]
fn disabled_and_missing_selected_providers_are_rejected() {
    let store = TempDir::new().unwrap();
    let disabled = CapabilityRegistry::new()
        .resolve(
            &[source_module(&store, "source-a", false, "body.weight")],
            &settings(Some("source-a")),
        )
        .unwrap_err();
    assert_eq!(disabled.code(), "disabled_provider");

    let missing = CapabilityRegistry::new()
        .resolve(&[], &settings(Some("missing")))
        .unwrap_err();
    assert_eq!(missing.code(), "missing_provider");

    let not_offered = CapabilityRegistry::new()
        .resolve(
            &[source_module(&store, "source-a", true, "nutrition.item")],
            &settings(Some("source-a")),
        )
        .unwrap_err();
    assert_eq!(not_offered.code(), "capability_not_offered");
}

#[test]
fn unselected_offers_do_not_become_implicit_active_providers() {
    let store = TempDir::new().unwrap();
    let modules = vec![source_module(&store, "source-a", true, "body.weight")];
    let resolution = CapabilityRegistry::new()
        .resolve(&modules, &settings(None))
        .unwrap();
    assert!(resolution.active_providers.is_empty());
}
