use mfa_config::{AppSettings, CURRENT_SCHEMA_VERSION, SettingsStore};
use mfa_contracts::{CapabilityId, ModuleId};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use tempfile::TempDir;

fn settings() -> AppSettings {
    let mut active_providers = BTreeMap::new();
    active_providers.insert(
        CapabilityId::try_from("body.weight").unwrap(),
        ModuleId::try_from("source-a").unwrap(),
    );
    AppSettings {
        schema_version: CURRENT_SCHEMA_VERSION,
        locale: "en-US".to_owned(),
        workspace_root: None,
        app_data_root: None,
        active_providers,
    }
}

#[test]
fn settings_round_trip_uses_atomic_same_directory_save() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("settings.json");
    let store = SettingsStore::new(&path);
    store.save(&settings()).unwrap();
    assert_eq!(store.load().unwrap(), settings());
    assert!(!path.with_extension("json.tmp").exists());
}

#[test]
fn interrupted_temp_file_is_recovered_and_promoted() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("settings.json");
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec(&settings()).unwrap()).unwrap();
    let recovered = SettingsStore::new(&path).load().unwrap();
    assert_eq!(recovered, settings());
    assert!(path.exists());
    assert!(!temporary.exists());

    fs::write(
        &temporary,
        br#"{"schema_version":1,"locale":"stale","active_providers":{}}"#,
    )
    .unwrap();
    assert_eq!(SettingsStore::new(&path).load().unwrap(), settings());
    assert!(!temporary.exists());
}

#[test]
fn unsupported_schema_version_is_rejected_with_stable_code() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("settings.json");
    fs::write(
        &path,
        serde_json::to_vec(&json!({
            "schema_version": CURRENT_SCHEMA_VERSION + 1,
            "locale": "en-US",
            "active_providers": {}
        }))
        .unwrap(),
    )
    .unwrap();
    let error = SettingsStore::new(&path).load().unwrap_err();
    assert_eq!(error.code(), "unsupported_schema_version");
}
