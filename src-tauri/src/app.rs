use crate::commands;
use crate::events::spawn_event_forwarder;
use crate::state::AppState;
use std::error::Error;
use tauri::Manager;

const CORE_ENGLISH_CATALOG: &[u8] = include_bytes!("../../modules/locales/en/messages.json");

pub fn run() -> Result<(), Box<dyn Error>> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap_state,
            commands::list_modules,
            commands::set_workspace_root,
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
    let state =
        AppState::from_roots_with_core_catalog(config_root, module_root, CORE_ENGLISH_CATALOG)?;
    if let Some(workspace_root) = state.settings().workspace_root.clone() {
        tauri::async_runtime::block_on(state.configure_workspace(workspace_root))?;
    }
    if let Some(coordinator) = state
        .storage_lock()
        .map_err(|error| error.to_string())?
        .as_ref()
        .and_then(|storage| storage.coordinator.clone())
    {
        spawn_event_forwarder(app.handle(), coordinator);
    }
    app.manage(state);
    Ok(())
}
