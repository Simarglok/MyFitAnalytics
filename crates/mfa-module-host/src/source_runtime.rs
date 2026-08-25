use crate::limits::RuntimeLimits;
use crate::package::InstalledModule;
use crate::runtime::RuntimeError;
use mfa_contracts::{ReadOnlyAsset, SourceBatch, SourceDescriptor, SourceValidation};
use std::sync::Arc;
use wasmtime::component::Resource;
use wasmtime::{Engine, Store, StoreLimits, StoreLimitsBuilder};

pub(crate) struct SourceStoreState {
    pub asset: Arc<dyn ReadOnlyAsset>,
    pub limits: StoreLimits,
}

impl bindings::myfitanalytics::source::host_asset::HostAssetReader for SourceStoreState {
    fn metadata(
        &mut self,
        _resource: Resource<bindings::AssetReader>,
    ) -> bindings::myfitanalytics::source::host_asset::AssetMetadata {
        let metadata = self.asset.metadata();
        bindings::myfitanalytics::source::host_asset::AssetMetadata {
            asset_id: metadata.asset_id.to_string(),
            file_name: metadata.file_name,
            media_type: metadata.media_type,
            byte_len: metadata.byte_len,
        }
    }

    fn read_at(
        &mut self,
        _resource: Resource<bindings::AssetReader>,
        offset: u64,
        max_bytes: u32,
    ) -> Result<Vec<u8>, String> {
        self.asset
            .read_at(offset, max_bytes)
            .map_err(|error| error.to_string())
    }

    fn drop(&mut self, _resource: Resource<bindings::AssetReader>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl bindings::myfitanalytics::source::host_asset::Host for SourceStoreState {}

pub(crate) async fn invoke(
    engine: &Engine,
    component: &wasmtime::component::Component,
    module: &InstalledModule,
    asset: Arc<dyn ReadOnlyAsset>,
    limits: RuntimeLimits,
) -> Result<SourceBatch, RuntimeError> {
    let store_limits = StoreLimitsBuilder::new()
        .memory_size(limits.max_memory_bytes)
        .trap_on_grow_failure(true)
        .instances(8)
        .tables(8)
        .build();
    let state = SourceStoreState {
        asset,
        limits: store_limits,
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

    let mut linker = wasmtime::component::Linker::new(engine);
    bindings::SourceModule::add_to_linker::<_, wasmtime::component::HasSelf<SourceStoreState>>(
        &mut linker,
        |state: &mut SourceStoreState| state,
    )
    .map_err(|error| RuntimeError::new("module_link_error", error.to_string()))?;
    let guest = bindings::SourceModule::instantiate_async(&mut store, component, &linker)
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_link_error", error))?;
    let (metadata,) = guest
        .func_metadata()
        .call_async(&mut store, ())
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_invoke_error", error))?;
    ensure_output_size(&metadata, limits.max_output_bytes)?;
    let descriptor: SourceDescriptor = serde_json::from_str(&metadata)
        .map_err(|error| RuntimeError::new("module_malformed_output", error.to_string()))?;
    validate_descriptor(module, &descriptor)?;

    let _detected = guest
        .func_detect()
        .call_async(&mut store, (Resource::new_own(0),))
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_invoke_error", error))?;
    let (validation,) = guest
        .func_validate()
        .call_async(&mut store, (Resource::new_own(0),))
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_invoke_error", error))?;
    let validation = validation.map_err(|error| RuntimeError::new("module_guest_error", error))?;
    ensure_output_size(&validation, limits.max_output_bytes)?;
    serde_json::from_str::<SourceValidation>(&validation)
        .map_err(|error| RuntimeError::new("module_malformed_output", error.to_string()))?;

    let (output,) = guest
        .func_parse()
        .call_async(&mut store, (Resource::new_own(0),))
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_invoke_error", error))?;
    let output = output.map_err(|error| RuntimeError::new("module_guest_error", error))?;
    ensure_output_size(&output, limits.max_output_bytes)?;
    let batch: SourceBatch = serde_json::from_str(&output)
        .map_err(|error| RuntimeError::new("module_malformed_output", error.to_string()))?;
    validate_batch_capabilities(module, &batch)?;
    Ok(batch)
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

fn validate_descriptor(
    module: &InstalledModule,
    descriptor: &SourceDescriptor,
) -> Result<(), RuntimeError> {
    if descriptor.module_id != module.module_id.as_str()
        || descriptor.module_version != module.module_version
    {
        return Err(RuntimeError::new(
            "module_descriptor_mismatch",
            "guest descriptor does not identify the installed module",
        ));
    }
    let allowed = match &module.manifest {
        mfa_contracts::ModuleManifest::Source(manifest) => &manifest.provided_capabilities,
        _ => {
            return Err(RuntimeError::new(
                "module_type_mismatch",
                "source invocation requires a source manifest",
            ));
        }
    };
    if descriptor
        .provided_capabilities
        .iter()
        .any(|capability| !allowed.contains(capability))
    {
        return Err(RuntimeError::new(
            "undeclared_capability_output",
            "guest descriptor reports an undeclared capability",
        ));
    }
    Ok(())
}

fn validate_batch_capabilities(
    module: &InstalledModule,
    batch: &SourceBatch,
) -> Result<(), RuntimeError> {
    let allowed = match &module.manifest {
        mfa_contracts::ModuleManifest::Source(manifest) => &manifest.provided_capabilities,
        _ => {
            return Err(RuntimeError::new(
                "module_type_mismatch",
                "source manifest required",
            ));
        }
    };
    if batch.extensions.iter().any(|extension| {
        !allowed
            .iter()
            .any(|capability| capability.as_str() == extension.namespace)
    }) {
        return Err(RuntimeError::new(
            "undeclared_capability_output",
            "guest batch reports an undeclared extension capability",
        ));
    }
    Ok(())
}

pub(crate) mod bindings {
    wasmtime::component::bindgen!({
        path: "../../modules/sdk/wit/source-api.wit",
        world: "source-module",
    });
}
