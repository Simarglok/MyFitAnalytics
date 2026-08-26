use crate::commands;
use crate::events::tauri_event_sink;
use crate::state::AppState;
use mfa_config::SettingsStore;
use mfa_contracts::ModuleManifest;
use mfa_module_host::{CapabilityRegistry, ModuleRegistry, PackageInstaller};
use std::error::Error;
use std::path::Path;
use tauri::Manager;

const CORE_ENGLISH_CATALOG: &[u8] = include_bytes!("../../modules/locales/en/messages.json");

pub fn run() -> Result<(), Box<dyn Error>> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap_state,
            commands::list_modules,
            commands::list_module_catalog,
            commands::choose_workspace_root,
            commands::get_workspace_view,
            commands::choose_and_install_module,
            commands::choose_source_inbox,
            commands::set_module_enabled,
            commands::update_module,
            commands::uninstall_module,
            commands::select_module_provider,
            commands::refresh_now,
            commands::get_ingestion_status,
            commands::list_quality_items,
            commands::retry_asset
        ])
        .setup(setup)
        .run(tauri::generate_context!())?;
    Ok(())
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn Error>> {
    let config_root = app.path().app_config_dir()?;
    let module_root = app.path().app_data_dir()?.join("modules");
    let bundled_packages = install_bundled_modules(app, &module_root)?;
    apply_bundled_provider_defaults(&config_root, &module_root)?;
    let state =
        AppState::from_roots_with_core_catalog(config_root, module_root, CORE_ENGLISH_CATALOG)?;
    for (module_id, package) in bundled_packages {
        state.register_bundled_package(module_id, package);
    }
    state.set_event_sink(tauri_event_sink(app.handle()));
    if let Some(workspace_root) = state.settings().workspace_root.clone() {
        tauri::async_runtime::block_on(state.configure_workspace(workspace_root))?;
    }
    app.manage(state);
    Ok(())
}

fn install_bundled_modules(
    app: &tauri::App,
    module_root: &Path,
) -> Result<Vec<(mfa_contracts::ModuleId, std::path::PathBuf)>, Box<dyn Error>> {
    let installer = PackageInstaller::new(module_root);
    let resource_root = app.path().resource_dir()?.join("modules");
    let packages = ["mynetdiary", "hevy"]
        .into_iter()
        .map(|module| resource_root.join(format!("{module}.mfasource")))
        .filter(|package| package.exists())
        .collect::<Vec<_>>();
    installer.install_bundled_defaults(&packages)?;
    let mut registered = Vec::new();
    for package in packages {
        let inspected = installer.inspect(&package)?;
        let module_id = match inspected.manifest {
            ModuleManifest::Source(manifest) => manifest.module_id,
            ModuleManifest::Dashboard(manifest) => manifest.module_id,
            ModuleManifest::Locale(manifest) => manifest.module_id,
        };
        registered.push((module_id, package));
    }
    Ok(registered)
}

fn apply_bundled_provider_defaults(
    config_root: &Path,
    module_root: &Path,
) -> Result<(), Box<dyn Error>> {
    let settings_store = SettingsStore::new(config_root.join("settings.json"));
    let mut settings = settings_store.load()?;
    let modules = PackageInstaller::new(module_root).list()?;
    CapabilityRegistry::new().apply_bundled_defaults(&modules, &mut settings)?;
    settings_store.save(&settings)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::apply_bundled_provider_defaults;
    use mfa_config::{CURRENT_SCHEMA_VERSION, SettingsStore};
    use mfa_contracts::{CapabilityId, ModuleId};
    use mfa_module_host::PackageInstaller;
    use std::path::Path;
    use tempfile::TempDir;

    fn package(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dist/modules")
            .join(name)
    }

    #[test]
    fn bundled_defaults_are_written_without_overwriting_existing_choices() {
        let temp = TempDir::new().unwrap();
        let config_root = temp.path().join("config");
        let module_root = temp.path().join("modules");
        let installer = PackageInstaller::new(&module_root);
        installer.install(&package("hevy.mfasource")).unwrap();
        installer.install(&package("mynetdiary.mfasource")).unwrap();

        apply_bundled_provider_defaults(&config_root, &module_root).unwrap();
        let store = SettingsStore::new(config_root.join("settings.json"));
        let mut settings = store.load().unwrap();
        assert_eq!(
            settings
                .active_providers
                .get(&CapabilityId::try_from("body.weight").unwrap())
                .unwrap(),
            &ModuleId::try_from("hevy").unwrap()
        );

        settings.active_providers.insert(
            CapabilityId::try_from("body.weight").unwrap(),
            ModuleId::try_from("hevy").unwrap(),
        );
        assert_eq!(settings.schema_version, CURRENT_SCHEMA_VERSION);
        store.save(&settings).unwrap();
        apply_bundled_provider_defaults(&config_root, &module_root).unwrap();
        let retained = store.load().unwrap();
        assert_eq!(retained, settings);
    }
}
