use mfa_config::{AppSettings, SettingsStore};
use mfa_contracts::ModuleId;
use mfa_module_host::PackageInstaller;
use myfitanalytics::dialogs::DialogPort;
use myfitanalytics::{AppState, CommandError};
use semver::Version;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::TempDir;
use zip::CompressionMethod;
use zip::ZipArchive;
use zip::write::SimpleFileOptions;

struct MockDialogs {
    workspace: Mutex<Option<PathBuf>>,
    package: Mutex<Option<PathBuf>>,
    source_inboxes: Mutex<BTreeMap<String, Option<PathBuf>>>,
}

impl MockDialogs {
    fn none() -> Self {
        Self {
            workspace: Mutex::new(None),
            package: Mutex::new(None),
            source_inboxes: Mutex::new(BTreeMap::new()),
        }
    }

    fn with_package(path: PathBuf) -> Self {
        let dialogs = Self::none();
        *dialogs.package.lock().unwrap() = Some(path);
        dialogs
    }

    fn choose_workspace(&self, path: PathBuf) {
        *self.workspace.lock().unwrap() = Some(path);
    }

    fn choose_source_inbox(&self, module_id: &str, path: PathBuf) {
        self.source_inboxes
            .lock()
            .unwrap()
            .insert(module_id.to_owned(), Some(path));
    }
}

impl DialogPort for MockDialogs {
    fn pick_workspace_root(&self) -> Option<PathBuf> {
        self.workspace.lock().unwrap().take()
    }

    fn pick_module_package(&self) -> Option<PathBuf> {
        self.package.lock().unwrap().take()
    }

    fn pick_source_inbox(&self, module_id: &ModuleId) -> Option<PathBuf> {
        self.source_inboxes
            .lock()
            .unwrap()
            .get_mut(module_id.as_str())
            .and_then(Option::take)
    }
}

fn state() -> (AppState, TempDir) {
    let root = TempDir::new().unwrap();
    let config_root = root.path().join("config");
    let module_root = root.path().join("modules");
    fs::create_dir_all(&config_root).unwrap();
    SettingsStore::new(config_root.join("settings.json"))
        .save(&AppSettings::default())
        .unwrap();
    let state = AppState::from_roots_with_core_catalog(
        &config_root,
        &module_root,
        br#"{"locale":"en","namespace":"core","messages":{"app.title":"MyFitAnalytics"}}"#,
    )
    .unwrap();
    (state, root)
}

fn fixture_package() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/mfa-module-host/tests/fixtures/valid-source.mfasource")
}

fn bundled_package(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dist/modules")
        .join(name)
}

fn write_zip_package(path: &Path, files: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, bytes) in files {
        archive.start_file(*name, options).unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap();
}

fn dashboard_package(root: &Path) -> PathBuf {
    let wasm = b"synthetic-dashboard-wasm";
    let hash = format!("sha256:{:x}", Sha256::digest(wasm));
    let manifest = serde_json::json!({
        "module_type": "dashboard",
        "module_id": "fixture-dashboard",
        "module_version": "1.0.0",
        "package_format_version": "1.0.0",
        "dashboard_api_version": "1.0.0",
        "entrypoint_hash": hash,
        "compatible_app_versions": [">=0.1.0"],
        "required_capabilities": [],
        "required_extension_contracts": [],
        "localization_namespace": "dashboard.fixture"
    });
    let manifest = serde_json::to_vec(&manifest).unwrap();
    let path = root.join("fixture-dashboard.mfadashboard");
    write_zip_package(&path, &[("module.json", &manifest), ("module.wasm", wasm)]);
    path
}

fn locale_package(root: &Path) -> PathBuf {
    let messages = br#"{"locale":"fr","namespace":"locale.fixture","messages":{"hello":"Hello"}}"#;
    let hash = format!("sha256:{:x}", Sha256::digest(messages));
    let manifest = serde_json::json!({
        "module_type": "locale",
        "module_id": "fixture-locale",
        "locale": "fr",
        "display_name": "Français",
        "module_version": "1.0.0",
        "package_format_version": "1.0.0",
        "compatible_app_versions": [">=0.1.0"],
        "localization_namespace": "locale.fixture",
        "files": [{"path":"messages.json","sha256":hash,"executable":false}]
    });
    let manifest = serde_json::to_vec(&manifest).unwrap();
    let path = root.join("fixture-locale.mfalocale");
    write_zip_package(
        &path,
        &[("module.json", &manifest), ("messages.json", messages)],
    );
    path
}

fn incompatible_bundled_package(root: &Path) -> PathBuf {
    let file = fs::File::open(fixture_package()).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    let mut manifest_bytes = Vec::new();
    archive
        .by_name("module.json")
        .unwrap()
        .read_to_end(&mut manifest_bytes)
        .unwrap();
    let mut manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    manifest["module_id"] = serde_json::json!("incompatible-source");
    manifest["compatible_app_versions"] = serde_json::json!([">=99.0.0"]);
    let mut wasm = Vec::new();
    archive
        .by_name("module.wasm")
        .unwrap()
        .read_to_end(&mut wasm)
        .unwrap();
    let manifest = serde_json::to_vec(&manifest).unwrap();
    let path = root.join("incompatible-source.mfasource");
    write_zip_package(&path, &[("module.json", &manifest), ("module.wasm", &wasm)]);
    path
}

fn installed_during_previous_app_version_package(root: &Path) -> PathBuf {
    let file = fs::File::open(fixture_package()).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    let mut manifest_bytes = Vec::new();
    archive
        .by_name("module.json")
        .unwrap()
        .read_to_end(&mut manifest_bytes)
        .unwrap();
    let mut manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    manifest["module_id"] = serde_json::json!("upgraded-source");
    manifest["compatible_app_versions"] = serde_json::json!(["<2.0.0"]);
    let mut wasm = Vec::new();
    archive
        .by_name("module.wasm")
        .unwrap()
        .read_to_end(&mut wasm)
        .unwrap();
    let manifest = serde_json::to_vec(&manifest).unwrap();
    let path = root.join("upgraded-source.mfasource");
    write_zip_package(&path, &[("module.json", &manifest), ("module.wasm", &wasm)]);
    path
}

#[tokio::test]
async fn startup_and_refresh_keep_incompatible_installed_packages_inactive_but_visible() {
    let root = TempDir::new().unwrap();
    let config_root = root.path().join("config");
    let module_root = root.path().join("modules");
    let packages_root = root.path().join("packages");
    fs::create_dir_all(&config_root).unwrap();
    fs::create_dir_all(&packages_root).unwrap();
    SettingsStore::new(config_root.join("settings.json"))
        .save(&AppSettings::default())
        .unwrap();
    let package = installed_during_previous_app_version_package(&packages_root);
    PackageInstaller::with_app_version(&module_root, Version::parse("0.1.0").unwrap())
        .install(&package)
        .unwrap();

    let state = AppState::from_roots_with_core_catalog_at_app_version(
        &config_root,
        &module_root,
        br#"{"locale":"en","namespace":"core","messages":{"app.title":"MyFitAnalytics"}}"#,
        Version::parse("2.0.0").unwrap(),
    )
    .unwrap();
    let startup = myfitanalytics::commands::get_bootstrap_state_inner(&state)
        .await
        .unwrap();
    assert!(!startup.active_providers.contains_key("body.weight"));
    assert!(
        !startup
            .modules
            .iter()
            .find(|module| module.id == "upgraded-source")
            .unwrap()
            .enabled
    );
    let catalog = myfitanalytics::commands::list_module_catalog_inner(&state)
        .await
        .unwrap();
    let entry = catalog
        .iter()
        .find(|entry| entry.module.id == "upgraded-source")
        .unwrap();
    assert_eq!(entry.install_state, "incompatible");
    assert_eq!(
        entry.error_code.as_deref(),
        Some("incompatible_app_version")
    );

    myfitanalytics::commands::set_module_enabled_inner(&state, "upgraded-source".to_owned(), false)
        .await
        .unwrap();
    let refreshed = myfitanalytics::commands::list_module_catalog_inner(&state)
        .await
        .unwrap();
    let entry = refreshed
        .iter()
        .find(|entry| entry.module.id == "upgraded-source")
        .unwrap();
    assert_eq!(entry.install_state, "incompatible");
    assert_eq!(
        entry.error_code.as_deref(),
        Some("incompatible_app_version")
    );
}

#[tokio::test]
async fn finalization_failure_rolls_back_the_complete_uninstall_transaction() {
    let (state, root) = state();
    for package in [fixture_package(), bundled_package("hevy.mfasource")] {
        let install_dialog = MockDialogs::with_package(package);
        myfitanalytics::commands::choose_and_install_module_inner(&state, &install_dialog)
            .await
            .unwrap()
            .unwrap();
    }
    myfitanalytics::commands::select_module_provider_inner(
        &state,
        "body.weight".to_owned(),
        "fixture-source".to_owned(),
    )
    .await
    .unwrap();
    myfitanalytics::commands::set_module_enabled_inner(&state, "fixture-source".to_owned(), false)
        .await
        .unwrap();
    let workspace_dialog = MockDialogs::none();
    workspace_dialog.choose_workspace(root.path().join("workspace"));
    myfitanalytics::commands::choose_workspace_root_inner(&state, &workspace_dialog)
        .await
        .unwrap()
        .unwrap();
    let database_identity = state.storage_database_identity().unwrap();
    let unrelated_id = ModuleId::try_from("hevy").unwrap();
    let unrelated_identity = state.storage_coordinator_identity(&unrelated_id).unwrap();
    let providers_before = myfitanalytics::commands::get_bootstrap_state_inner(&state)
        .await
        .unwrap()
        .active_providers;
    let before_uninstall: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("modules/state.json")).unwrap()).unwrap();
    let package_hash = before_uninstall["active_packages"]["fixture-source"]["package_hash"]
        .as_str()
        .unwrap();
    let installed_root = root
        .path()
        .join("modules/fixture-source/1.0.0")
        .join(package_hash);

    state.set_uninstall_finalization_fault(Some(
        mfa_module_host::UninstallFinalizationFault::BeforeDelete,
    ));
    let error =
        myfitanalytics::commands::uninstall_module_inner(&state, "fixture-source".to_owned())
            .await
            .unwrap_err();
    assert_eq!(error.code, "atomic_uninstall_failed");
    assert!(installed_root.exists());
    assert_eq!(state.storage_database_identity(), Some(database_identity));
    assert_eq!(
        state.storage_coordinator_identity(&unrelated_id),
        Some(unrelated_identity)
    );
    let bootstrap = myfitanalytics::commands::get_bootstrap_state_inner(&state)
        .await
        .unwrap();
    assert_eq!(bootstrap.active_providers, providers_before);
    assert!(
        !bootstrap
            .modules
            .iter()
            .find(|module| module.id == "fixture-source")
            .unwrap()
            .enabled
    );
    let settings_json: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("config/settings.json")).unwrap())
            .unwrap();
    assert_eq!(
        settings_json["active_providers"]["body.weight"],
        "fixture-source"
    );
    let state_json: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("modules/state.json")).unwrap()).unwrap();
    assert!(state_json["active_packages"]["fixture-source"].is_object());
    assert!(
        state_json["uninstalled_modules"]
            .as_array()
            .is_none_or(|ids| !ids.iter().any(|id| id == "fixture-source"))
    );
}

#[tokio::test]
async fn source_inbox_change_reconfigures_the_coordinator_to_the_new_directory() {
    let (state, root) = state();
    for package in [fixture_package(), bundled_package("hevy.mfasource")] {
        let install_dialog = MockDialogs::with_package(package);
        myfitanalytics::commands::choose_and_install_module_inner(&state, &install_dialog)
            .await
            .unwrap()
            .unwrap();
    }
    for module_id in ["fixture-source", "hevy"] {
        myfitanalytics::commands::set_module_enabled_inner(&state, module_id.to_owned(), false)
            .await
            .unwrap();
    }
    let workspace_dialog = MockDialogs::none();
    workspace_dialog.choose_workspace(root.path().join("workspace"));
    myfitanalytics::commands::choose_workspace_root_inner(&state, &workspace_dialog)
        .await
        .unwrap()
        .unwrap();
    for module_id in ["fixture-source", "hevy"] {
        myfitanalytics::commands::set_module_enabled_inner(&state, module_id.to_owned(), true)
            .await
            .unwrap();
    }
    let database_identity = state.storage_database_identity().unwrap();
    let recovery_gate_identity = state.storage_recovery_gate_identity().unwrap();
    let unrelated_id = ModuleId::try_from("hevy").unwrap();
    let unrelated_identity = state.storage_coordinator_identity(&unrelated_id).unwrap();
    let new_inbox = root.path().join("selected-fixture-inbox");
    let inbox_dialog = MockDialogs::none();
    inbox_dialog.choose_source_inbox("fixture-source", new_inbox.clone());
    myfitanalytics::commands::choose_source_inbox_inner(
        &state,
        "fixture-source".to_owned(),
        &inbox_dialog,
    )
    .await
    .unwrap()
    .unwrap();

    std::fs::write(new_inbox.join("moved.fixture"), b"synthetic fixture bytes").unwrap();
    myfitanalytics::commands::refresh_now_inner(&state)
        .await
        .unwrap();
    myfitanalytics::commands::refresh_now_inner(&state)
        .await
        .unwrap();
    assert!(!new_inbox.join("moved.fixture").exists());
    assert_eq!(state.storage_database_identity(), Some(database_identity));
    assert_eq!(
        state.storage_recovery_gate_identity(),
        Some(recovery_gate_identity)
    );
    assert_eq!(
        state.storage_coordinator_identity(&unrelated_id),
        Some(unrelated_identity)
    );
}

#[tokio::test]
async fn uninstalled_bundled_incompatible_package_is_classified_by_backend_catalog() {
    let (state, root) = state();
    let package = incompatible_bundled_package(root.path());
    state.register_bundled_package(ModuleId::try_from("incompatible-source").unwrap(), package);
    let catalog = myfitanalytics::commands::list_module_catalog_inner(&state)
        .await
        .unwrap();
    let entry = catalog
        .iter()
        .find(|entry| entry.module.id == "incompatible-source")
        .unwrap();
    assert_eq!(entry.origin, "bundled");
    assert_eq!(entry.install_state, "incompatible");
    assert_eq!(
        entry.error_code.as_deref(),
        Some("incompatible_app_version")
    );
}

#[tokio::test]
async fn failed_uninstall_refresh_rolls_back_bytes_state_provider_and_registry() {
    let (state, root) = state();
    let package = fixture_package();
    let dialogs = MockDialogs::with_package(package);
    myfitanalytics::commands::choose_and_install_module_inner(&state, &dialogs)
        .await
        .unwrap()
        .unwrap();
    myfitanalytics::commands::select_module_provider_inner(
        &state,
        "body.weight".to_owned(),
        "fixture-source".to_owned(),
    )
    .await
    .unwrap();
    myfitanalytics::commands::set_module_enabled_inner(&state, "fixture-source".to_owned(), false)
        .await
        .unwrap();
    let before_uninstall: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("modules/state.json")).unwrap()).unwrap();
    let package_hash = before_uninstall["active_packages"]["fixture-source"]["package_hash"]
        .as_str()
        .unwrap();
    let installed_root = root
        .path()
        .join("modules/fixture-source/1.0.0")
        .join(package_hash);

    let corrupt_root = root.path().join("modules/corrupt-source/1.0.0/bad-package");
    std::fs::create_dir_all(&corrupt_root).unwrap();
    std::fs::write(corrupt_root.join("module.json"), b"not-json").unwrap();

    let error =
        myfitanalytics::commands::uninstall_module_inner(&state, "fixture-source".to_owned())
            .await
            .unwrap_err();
    assert_ne!(error.code, "ok");
    assert!(installed_root.exists());
    let bootstrap = myfitanalytics::commands::get_bootstrap_state_inner(&state)
        .await
        .unwrap();
    assert!(
        !bootstrap
            .modules
            .iter()
            .find(|module| module.id == "fixture-source")
            .unwrap()
            .enabled
    );
    let settings_json: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("config/settings.json")).unwrap())
            .unwrap();
    assert_eq!(
        settings_json["active_providers"]["body.weight"],
        "fixture-source"
    );
    let state_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.path().join("modules/state.json")).unwrap())
            .unwrap();
    assert!(state_json["active_packages"]["fixture-source"].is_object());
    assert!(
        state_json["uninstalled_modules"]
            .as_array()
            .is_none_or(|ids| !ids.iter().any(|id| id == "fixture-source"))
    );
}

#[tokio::test]
async fn source_lifecycle_reuses_the_shared_database_runtime() {
    let (state, root) = state();
    let install_dialog = MockDialogs::with_package(fixture_package());
    myfitanalytics::commands::choose_and_install_module_inner(&state, &install_dialog)
        .await
        .unwrap()
        .unwrap();

    myfitanalytics::commands::set_module_enabled_inner(&state, "fixture-source".to_owned(), false)
        .await
        .unwrap();
    let workspace_dialog = MockDialogs::none();
    workspace_dialog.choose_workspace(root.path().join("workspace"));
    myfitanalytics::commands::choose_workspace_root_inner(&state, &workspace_dialog)
        .await
        .unwrap()
        .unwrap();
    let database_identity = state.storage_database_identity().unwrap();
    assert!(
        myfitanalytics::commands::get_ingestion_status_inner(&state)
            .await
            .unwrap()
            .configured
    );

    myfitanalytics::commands::set_module_enabled_inner(&state, "fixture-source".to_owned(), true)
        .await
        .unwrap();
    assert_eq!(state.storage_database_identity(), Some(database_identity));

    let inbox_dialog = MockDialogs::none();
    inbox_dialog.choose_source_inbox("fixture-source", root.path().join("selected-inbox"));
    myfitanalytics::commands::choose_source_inbox_inner(
        &state,
        "fixture-source".to_owned(),
        &inbox_dialog,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(state.storage_database_identity(), Some(database_identity));

    myfitanalytics::commands::set_module_enabled_inner(&state, "fixture-source".to_owned(), false)
        .await
        .unwrap();
    assert_eq!(state.storage_database_identity(), Some(database_identity));
    myfitanalytics::commands::uninstall_module_inner(&state, "fixture-source".to_owned())
        .await
        .unwrap();
    assert_eq!(state.storage_database_identity(), Some(database_identity));
    assert!(
        myfitanalytics::commands::get_ingestion_status_inner(&state)
            .await
            .unwrap()
            .configured
    );
}

#[tokio::test]
async fn dashboard_and_locale_lifecycle_never_restarts_storage() {
    let (state, root) = state();
    let package_root = root.path().join("packages");
    fs::create_dir_all(&package_root).unwrap();
    let source_dialog = MockDialogs::with_package(fixture_package());
    myfitanalytics::commands::choose_and_install_module_inner(&state, &source_dialog)
        .await
        .unwrap()
        .unwrap();
    myfitanalytics::commands::set_module_enabled_inner(&state, "fixture-source".to_owned(), false)
        .await
        .unwrap();
    let workspace_dialog = MockDialogs::none();
    workspace_dialog.choose_workspace(root.path().join("workspace"));
    myfitanalytics::commands::choose_workspace_root_inner(&state, &workspace_dialog)
        .await
        .unwrap()
        .unwrap();
    myfitanalytics::commands::set_module_enabled_inner(&state, "fixture-source".to_owned(), true)
        .await
        .unwrap();
    let database_identity = state.storage_database_identity().unwrap();

    for package in [
        dashboard_package(&package_root),
        locale_package(&package_root),
    ] {
        let dialog = MockDialogs::with_package(package);
        myfitanalytics::commands::choose_and_install_module_inner(&state, &dialog)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state.storage_database_identity(), Some(database_identity));
    }
    for module_id in ["fixture-dashboard", "fixture-locale"] {
        myfitanalytics::commands::set_module_enabled_inner(&state, module_id.to_owned(), false)
            .await
            .unwrap();
        assert_eq!(state.storage_database_identity(), Some(database_identity));
        myfitanalytics::commands::set_module_enabled_inner(&state, module_id.to_owned(), true)
            .await
            .unwrap();
        assert_eq!(state.storage_database_identity(), Some(database_identity));
        myfitanalytics::commands::set_module_enabled_inner(&state, module_id.to_owned(), false)
            .await
            .unwrap();
        myfitanalytics::commands::uninstall_module_inner(&state, module_id.to_owned())
            .await
            .unwrap();
        assert_eq!(state.storage_database_identity(), Some(database_identity));
    }
}

#[tokio::test]
async fn source_lifecycle_preserves_unrelated_coordinator_continuity() {
    let (state, root) = state();
    for package in ["hevy.mfasource", "mynetdiary.mfasource"] {
        let install_dialog = MockDialogs::with_package(bundled_package(package));
        myfitanalytics::commands::choose_and_install_module_inner(&state, &install_dialog)
            .await
            .unwrap()
            .unwrap();
    }
    for module_id in ["hevy", "mynetdiary"] {
        myfitanalytics::commands::set_module_enabled_inner(&state, module_id.to_owned(), false)
            .await
            .unwrap();
    }
    let workspace_dialog = MockDialogs::none();
    workspace_dialog.choose_workspace(root.path().join("workspace"));
    myfitanalytics::commands::choose_workspace_root_inner(&state, &workspace_dialog)
        .await
        .unwrap()
        .unwrap();
    for module_id in ["hevy", "mynetdiary"] {
        myfitanalytics::commands::set_module_enabled_inner(&state, module_id.to_owned(), true)
            .await
            .unwrap();
    }
    let unrelated_id = ModuleId::try_from("mynetdiary").unwrap();
    let unrelated_identity = state.storage_coordinator_identity(&unrelated_id).unwrap();

    myfitanalytics::commands::set_module_enabled_inner(&state, "hevy".to_owned(), false)
        .await
        .unwrap();
    assert_eq!(
        state.storage_coordinator_identity(&unrelated_id),
        Some(unrelated_identity)
    );
    myfitanalytics::commands::set_module_enabled_inner(&state, "hevy".to_owned(), true)
        .await
        .unwrap();
    assert_eq!(
        state.storage_coordinator_identity(&unrelated_id),
        Some(unrelated_identity)
    );

    let inbox_dialog = MockDialogs::none();
    inbox_dialog.choose_source_inbox("hevy", root.path().join("hevy-inbox"));
    myfitanalytics::commands::choose_source_inbox_inner(&state, "hevy".to_owned(), &inbox_dialog)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        state.storage_coordinator_identity(&unrelated_id),
        Some(unrelated_identity)
    );
}

#[tokio::test]
async fn lifecycle_commands_are_cancellable_mutable_and_preserve_data_boundaries() {
    let (state, root) = state();
    state.register_bundled_package(
        ModuleId::try_from("fixture-source").unwrap(),
        fixture_package(),
    );
    let cancelled = MockDialogs::none();

    assert!(
        myfitanalytics::commands::choose_workspace_root_inner(&state, &cancelled)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        myfitanalytics::commands::choose_and_install_module_inner(&state, &cancelled)
            .await
            .unwrap()
            .is_none()
    );

    let install_dialog = MockDialogs::with_package(fixture_package());
    let installed =
        myfitanalytics::commands::choose_and_install_module_inner(&state, &install_dialog)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(installed.id, "fixture-source");
    assert!(
        myfitanalytics::commands::list_module_catalog_inner(&state)
            .await
            .unwrap()
            .iter()
            .any(|entry| entry.module.id == "fixture-source" && entry.install_state == "enabled")
    );

    myfitanalytics::commands::set_module_enabled_inner(&state, "fixture-source".to_owned(), false)
        .await
        .unwrap();
    let enable_error = myfitanalytics::commands::set_module_enabled_inner(
        &state,
        "fixture-source".to_owned(),
        true,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        enable_error,
        CommandError { code, .. } if code == "workspace_required"
    ));

    let workspace = root.path().join("workspace");
    let workspace_dialog = MockDialogs::none();
    workspace_dialog.choose_workspace(workspace.clone());
    let workspace_view =
        myfitanalytics::commands::choose_workspace_root_inner(&state, &workspace_dialog)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(workspace_view.workspace_root, workspace.to_string_lossy());
    myfitanalytics::commands::set_module_enabled_inner(&state, "fixture-source".to_owned(), true)
        .await
        .unwrap();

    let selected_inbox = root.path().join("selected-inbox");
    let inbox_dialog = MockDialogs::none();
    inbox_dialog.choose_source_inbox("fixture-source", selected_inbox.clone());
    let source_view = myfitanalytics::commands::choose_source_inbox_inner(
        &state,
        "fixture-source".to_owned(),
        &inbox_dialog,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        source_view
            .source_paths
            .iter()
            .find(|path| path.module_id == "fixture-source")
            .unwrap()
            .inbox_path,
        selected_inbox.to_string_lossy()
    );

    let provider = myfitanalytics::commands::select_module_provider_inner(
        &state,
        "body.weight".to_owned(),
        "fixture-source".to_owned(),
    )
    .await
    .unwrap();
    assert_eq!(provider.active_providers["body.weight"], "fixture-source");

    assert!(matches!(
        myfitanalytics::commands::uninstall_module_inner(&state, "fixture-source".to_owned())
            .await
            .unwrap_err(),
        CommandError { code, .. } if code == "module_must_be_disabled"
    ));

    let disabled = myfitanalytics::commands::set_module_enabled_inner(
        &state,
        "fixture-source".to_owned(),
        false,
    )
    .await
    .unwrap();
    assert!(!disabled.enabled);
    myfitanalytics::commands::uninstall_module_inner(&state, "fixture-source".to_owned())
        .await
        .unwrap();
    assert!(
        myfitanalytics::commands::list_module_catalog_inner(&state)
            .await
            .unwrap()
            .iter()
            .any(|entry| {
                entry.module.id == "fixture-source" && entry.install_state == "available"
            })
    );
}
