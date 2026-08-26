use mfa_contracts::{ContractVersion, ModuleId, ModuleManifest, ModuleType, SourceManifest};
use mfa_module_host::{LocaleResolver, ResolvedMessage};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn executable_module(
    store: &TempDir,
    id: &str,
    namespace: &str,
    message: &str,
) -> mfa_module_host::InstalledModule {
    let root = store.path().join(id);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("messages.json"),
        serde_json::to_vec(
            &json!({"locale":"en","namespace":namespace,"messages":{"hello":message}}),
        )
        .unwrap(),
    )
    .unwrap();
    let manifest: SourceManifest = serde_json::from_value(json!({
        "module_type": "source",
        "module_id": id,
        "module_version": "1.0.0",
        "package_format_version": "1.0.0",
        "source_api_version": "1.0.0",
        "mapping_version": "1.0.0",
        "compatible_app_versions": [">=0.1.0"],
        "provided_capabilities": ["body.weight"],
        "accepted_file_patterns": ["*.json"],
        "artifact_signatures": ["sha256:fixture"],
        "extension_contracts": [],
        "settings_schema": {},
        "entrypoint_hash": "sha256:fixture",
        "localization_namespace": namespace
    }))
    .unwrap();
    mfa_module_host::InstalledModule {
        module_id: ModuleId::try_from(id).unwrap(),
        module_type: ModuleType::Source,
        module_version: ContractVersion::try_from("1.0.0").unwrap(),
        package_hash: format!("hash-{id}"),
        root,
        enabled: true,
        manifest: ModuleManifest::Source(manifest),
    }
}

fn locale_module(
    store: &TempDir,
    id: &str,
    locale: &str,
    namespace: &str,
    message: &str,
) -> mfa_module_host::InstalledModule {
    let root = store.path().join(id);
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("messages.json"),
        serde_json::to_vec(
            &json!({"locale":locale,"namespace":namespace,"messages":{"hello":message}}),
        )
        .unwrap(),
    )
    .unwrap();
    let manifest = serde_json::from_value(json!({
        "module_type": "locale",
        "module_id": id,
        "locale": locale,
        "display_name": locale,
        "module_version": "1.0.0",
        "package_format_version": "1.0.0",
        "compatible_app_versions": [">=0.1.0"],
        "localization_namespace": namespace,
        "files": [{"path":"messages.json","sha256":"sha256:fixture","executable":false}]
    }))
    .unwrap();
    mfa_module_host::InstalledModule {
        module_id: ModuleId::try_from(id).unwrap(),
        module_type: ModuleType::Locale,
        module_version: ContractVersion::try_from("1.0.0").unwrap(),
        package_hash: format!("hash-{id}"),
        root,
        enabled: true,
        manifest: ModuleManifest::Locale(manifest),
    }
}

#[test]
fn locale_fallback_prefers_selected_locale_then_executable_then_core() {
    let store = TempDir::new().unwrap();
    let core = store.path().join("core");
    fs::create_dir_all(&core).unwrap();
    fs::write(
        core.join("messages.json"),
        br#"{"locale":"en","namespace":"core","messages":{"hello":"Core {name}"}}"#,
    )
    .unwrap();
    let executable = executable_module(&store, "source-a", "source", "Executable {name}");
    let locale = locale_module(&store, "locale-fr", "fr", "source", "Locale {name}");
    let resolver = LocaleResolver::new(&core, vec![executable, locale]).unwrap();
    let resolved = resolver.message("fr", "source", "hello", &json!({"name":"Ada"}));
    assert_eq!(resolved, ResolvedMessage::Found("Locale Ada".to_owned()));

    let fallback = resolver.message("de", "source", "hello", &json!({"name":"Ada"}));
    assert_eq!(
        fallback,
        ResolvedMessage::Found("Executable Ada".to_owned())
    );
    let core_fallback = resolver.message("de", "other", "hello", &json!({"name":"Ada"}));
    assert_eq!(core_fallback, ResolvedMessage::Found("Core Ada".to_owned()));
}

#[test]
fn missing_keys_and_invalid_placeholders_are_visible_and_stable() {
    let store = TempDir::new().unwrap();
    let core = store.path().join("core");
    fs::create_dir_all(&core).unwrap();
    fs::write(
        core.join("messages.json"),
        br#"{"locale":"en","namespace":"core","messages":{"hello":"Hello {name}"}}"#,
    )
    .unwrap();
    let resolver = LocaleResolver::new(&core, Vec::new()).unwrap();
    assert_eq!(
        resolver.message("en", "core", "missing", &json!({})),
        ResolvedMessage::Missing("⟦missing:core.missing⟧".to_owned())
    );
    assert_eq!(
        resolver.message("en", "core", "hello", &json!({})),
        ResolvedMessage::InvalidPlaceholders {
            missing: vec!["name".to_owned()]
        }
    );
    assert_eq!(
        resolver.message(
            "en",
            "core",
            "hello",
            &json!({"name":"Ada", "extra":"ignored"}),
        ),
        ResolvedMessage::Found("Hello Ada".to_owned())
    );

    fs::write(
        core.join("messages.json"),
        br#"{"locale":"en","namespace":"core","messages":{"bad":"Hello {bad-name}"}}"#,
    )
    .unwrap();
    let malformed = LocaleResolver::new(&core, Vec::new()).unwrap();
    assert_eq!(
        malformed.message("en", "core", "bad", &json!({"bad-name":"Ada"})),
        ResolvedMessage::InvalidPlaceholders {
            missing: vec!["bad-name".to_owned()]
        }
    );
}

#[test]
fn same_locale_namespace_from_different_module_ids_is_rejected() {
    let store = TempDir::new().unwrap();
    let core = store.path().join("core");
    fs::create_dir_all(&core).unwrap();
    fs::write(
        core.join("messages.json"),
        br#"{"locale":"en","namespace":"core","messages":{}}"#,
    )
    .unwrap();
    let first = locale_module(&store, "locale-a", "fr", "shared", "A");
    let second = locale_module(&store, "locale-b", "fr", "shared", "B");
    let error = LocaleResolver::new(&core, vec![first, second]).unwrap_err();
    assert_eq!(error.code(), "namespace_collision");
}

#[test]
fn catalog_locale_and_namespace_must_match_their_manifest_role() {
    let store = TempDir::new().unwrap();
    let core = store.path().join("core");
    fs::create_dir_all(&core).unwrap();
    fs::write(
        core.join("messages.json"),
        br#"{"locale":"fr","namespace":"core","messages":{}}"#,
    )
    .unwrap();
    let error = LocaleResolver::new(&core, Vec::new()).unwrap_err();
    assert_eq!(error.code(), "invalid_locale_catalog");

    let core = store.path().join("core-valid");
    fs::create_dir_all(&core).unwrap();
    fs::write(
        core.join("messages.json"),
        br#"{"locale":"en","namespace":"core","messages":{}}"#,
    )
    .unwrap();
    let executable = executable_module(&store, "source-bad", "source", "Hello");
    fs::write(
        executable.root.join("messages.json"),
        br#"{"locale":"fr","namespace":"source","messages":{}}"#,
    )
    .unwrap();
    let error = LocaleResolver::new(&core, vec![executable]).unwrap_err();
    assert_eq!(error.code(), "invalid_locale_catalog");

    let locale = locale_module(&store, "locale-bad", "fr", "source", "Bonjour");
    fs::write(
        locale.root.join("messages.json"),
        br#"{"locale":"fr","namespace":"other","messages":{}}"#,
    )
    .unwrap();
    let error = LocaleResolver::new(&core, vec![locale]).unwrap_err();
    assert_eq!(error.code(), "invalid_locale_catalog");
}

#[test]
fn unmatched_closing_braces_are_rejected() {
    let store = TempDir::new().unwrap();
    let core = store.path().join("core");
    fs::create_dir_all(&core).unwrap();
    fs::write(
        core.join("messages.json"),
        br#"{"locale":"en","namespace":"core","messages":{"bad":"Hello }"}}"#,
    )
    .unwrap();
    let resolver = LocaleResolver::new(&core, Vec::new()).unwrap();
    assert_eq!(
        resolver.message("en", "core", "bad", &json!({})),
        ResolvedMessage::InvalidPlaceholders {
            missing: vec!["<unmatched-close>".to_owned()]
        }
    );
}
