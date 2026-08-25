use mfa_config::{AppPaths, AppSettings, SettingsStore, WorkspacePaths};
use mfa_contracts::ModuleId;
use std::fs;
use tempfile::TempDir;

#[test]
fn settings_round_trip_preserves_explicit_workspace_and_app_data_roots() {
    let root = TempDir::new().unwrap();
    let settings_path = root.path().join("settings.json");
    let workspace = root.path().join("workspace");
    let app_data = root.path().join("app-data");
    let settings = AppSettings {
        workspace_root: Some(workspace.clone()),
        app_data_root: Some(app_data.clone()),
        ..AppSettings::default()
    };

    let store = SettingsStore::new(&settings_path);
    store.save(&settings).unwrap();

    assert_eq!(store.load().unwrap(), settings);
}

#[test]
fn enabling_a_source_creates_only_its_inbox_and_archive_directories() {
    let root = TempDir::new().unwrap();
    let workspace = WorkspacePaths::new(root.path().join("workspace"));
    let source = ModuleId::try_from("mynetdiary").unwrap();

    workspace.enable_source(&source).unwrap();

    assert!(workspace.source_inbox(&source).is_dir());
    assert!(workspace.source_archive(&source).is_dir());
    let entries: Vec<_> = fs::read_dir(root.path().join("workspace"))
        .unwrap()
        .collect();
    assert_eq!(entries.len(), 2);
    assert!(!root.path().join("workspace/mynetdiary").exists());
}

#[test]
fn path_policy_rejects_equal_or_nested_workspace_and_application_data_roots() {
    let root = TempDir::new().unwrap();
    let app_data = AppPaths::new(root.path().join("app-data"));

    assert!(WorkspacePaths::validate(root.path().join("app-data"), &app_data).is_err());
    assert!(WorkspacePaths::validate(root.path().join("app-data/workspace"), &app_data).is_err());
    assert!(WorkspacePaths::validate(root.path().join("workspace"), &app_data).is_ok());
    assert!(
        WorkspacePaths::validate(root.path(), &AppPaths::new(root.path().join("app-data")))
            .is_err()
    );
}
