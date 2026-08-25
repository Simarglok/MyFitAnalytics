use crate::state::AppState;
use mfa_contracts::ModuleManifest;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapState {
    pub product_name: String,
    pub locale: String,
    pub active_providers: BTreeMap<String, String>,
    pub modules: Vec<ModuleView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleView {
    pub id: String,
    pub module_type: String,
    pub version: String,
    pub enabled: bool,
    pub localization_namespace: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

pub async fn get_bootstrap_state_inner(state: &AppState) -> Result<BootstrapState, CommandError> {
    Ok(BootstrapState {
        product_name: "MyFitAnalytics".to_owned(),
        locale: state.settings().locale.clone(),
        active_providers: state
            .providers()
            .active_providers
            .iter()
            .map(|(capability, module)| (capability.to_string(), module.to_string()))
            .collect(),
        modules: list_modules_inner(state).await?,
    })
}

pub async fn list_modules_inner(state: &AppState) -> Result<Vec<ModuleView>, CommandError> {
    Ok(state
        .modules()
        .iter()
        .map(|module| ModuleView {
            id: module.module_id.to_string(),
            module_type: match module.module_type {
                mfa_contracts::ModuleType::Source => "source",
                mfa_contracts::ModuleType::Dashboard => "dashboard",
                mfa_contracts::ModuleType::Locale => "locale",
            }
            .to_owned(),
            version: module.module_version.to_string(),
            enabled: module.enabled,
            localization_namespace: localization_namespace(&module.manifest),
        })
        .collect())
}

fn localization_namespace(manifest: &ModuleManifest) -> String {
    match manifest {
        ModuleManifest::Source(manifest) => manifest.localization_namespace.clone(),
        ModuleManifest::Dashboard(manifest) => manifest.localization_namespace.clone(),
        ModuleManifest::Locale(manifest) => manifest.localization_namespace.clone(),
    }
}

#[tauri::command]
pub async fn get_bootstrap_state(
    state: tauri::State<'_, AppState>,
) -> Result<BootstrapState, CommandError> {
    get_bootstrap_state_inner(&state).await
}

#[tauri::command]
pub async fn list_modules(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ModuleView>, CommandError> {
    list_modules_inner(&state).await
}
