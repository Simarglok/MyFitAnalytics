use crate::commands;
use crate::state::AppState;
use std::error::Error;
use tauri::Manager;

const CORE_ENGLISH_CATALOG: &[u8] = include_bytes!("../../modules/locales/en/messages.json");

pub fn run() -> Result<(), Box<dyn Error>> {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_bootstrap_state,
            commands::list_modules
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
    app.manage(state);
    Ok(())
}
