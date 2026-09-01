use crate::limits::RuntimeLimits;
use crate::package::InstalledModule;
use crate::runtime::RuntimeError;
use mfa_contracts::{DashboardDocument, DashboardInput, ModuleManifest};
use mfa_dashboard_host::validate_raw_document_json;
use wasmtime::{Engine, Store, StoreLimits};

pub(crate) struct DashboardStoreState {
    pub limits: StoreLimits,
}

pub(crate) async fn invoke(
    engine: &Engine,
    component: &wasmtime::component::Component,
    module: &InstalledModule,
    input: DashboardInput,
    limits: RuntimeLimits,
) -> Result<DashboardDocument, RuntimeError> {
    let ModuleManifest::Dashboard(_) = &module.manifest else {
        return Err(RuntimeError::new(
            "module_type_mismatch",
            "dashboard invocation requires a dashboard manifest",
        ));
    };
    let state = DashboardStoreState {
        limits: wasmtime::StoreLimitsBuilder::new()
            .memory_size(limits.max_memory_bytes)
            .trap_on_grow_failure(true)
            .instances(8)
            .tables(8)
            .build(),
    };
    let mut store = Store::new(engine, state);
    store.limiter(|state| &mut state.limits);
    store
        .set_fuel(limits.fuel)
        .map_err(|error| RuntimeError::new("module_fuel_config", error.to_string()))?;
    store
        .fuel_async_yield_interval(Some(1_000))
        .map_err(|error| RuntimeError::new("module_fuel_config", error.to_string()))?;
    store.set_epoch_deadline(1);

    let linker = wasmtime::component::Linker::<DashboardStoreState>::new(engine);
    let guest = bindings::DashboardModule::instantiate_async(&mut store, component, &linker)
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_link_error", error))?;

    let (description,) = guest
        .func_describe()
        .call_async(&mut store, ())
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_invoke_error", error))?;
    ensure_output_size(&description, limits.max_output_bytes)?;
    let _: serde_json::Value = serde_json::from_str(&description)
        .map_err(|error| RuntimeError::new("module_malformed_output", error.to_string()))?;

    let input_json = serde_json::to_string(&input)
        .map_err(|error| RuntimeError::new("module_input_encode", error.to_string()))?;
    let (output,) = guest
        .func_compose()
        .call_async(&mut store, (&input_json,))
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_invoke_error", error))?;
    let output = output.map_err(|error| RuntimeError::new("module_guest_error", error))?;
    ensure_output_size(&output, limits.max_output_bytes)?;
    let raw: serde_json::Value = serde_json::from_str(&output)
        .map_err(|error| RuntimeError::new("module_malformed_output", error.to_string()))?;
    let document: DashboardDocument = validate_raw_document_json(&raw, &input)
        .map_err(|error| RuntimeError::new("module_malformed_output", error.code()))?;
    if !document.is_declarative() {
        return Err(RuntimeError::new(
            "module_non_declarative_output",
            "dashboard output is not a declarative document",
        ));
    }
    Ok(document)
}

fn ensure_output_size(output: &str, maximum: usize) -> Result<(), RuntimeError> {
    if output.len() > maximum {
        return Err(RuntimeError::new(
            "module_output_limit",
            format!(
                "guest output is {} bytes, maximum is {maximum}",
                output.len()
            ),
        ));
    }
    Ok(())
}

pub(crate) mod bindings {
    wasmtime::component::bindgen!({
        path: "../../modules/sdk/wit/dashboard-api.wit",
        world: "dashboard-module",
    });
}
