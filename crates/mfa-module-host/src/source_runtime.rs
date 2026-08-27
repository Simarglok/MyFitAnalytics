use crate::limits::RuntimeLimits;
use crate::package::InstalledModule;
use crate::runtime::RuntimeError;
use mfa_contracts::{
    ModuleManifest, ReadOnlyAsset, SOURCE_BATCH_CONTRACT_VERSION, SourceBatch, SourceDescriptor,
    SourceValidation,
};
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

pub(crate) async fn probe(
    engine: &Engine,
    component: &wasmtime::component::Component,
    module: &InstalledModule,
    asset: Arc<dyn ReadOnlyAsset>,
    limits: RuntimeLimits,
) -> Result<u8, RuntimeError> {
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
    let (contract_version,) = guest
        .func_contract_version()
        .call_async(&mut store, ())
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_invoke_error", error))?;
    if contract_version != SOURCE_BATCH_CONTRACT_VERSION {
        return Err(RuntimeError::new(
            "module_contract_version_mismatch",
            format!(
                "guest reports source batch contract {contract_version}, host requires {SOURCE_BATCH_CONTRACT_VERSION}"
            ),
        ));
    }
    let (detected,) = guest
        .func_detect()
        .call_async(&mut store, (Resource::new_own(0),))
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_invoke_error", error))?;
    Ok(detected)
}

pub(crate) async fn validate(
    engine: &Engine,
    component: &wasmtime::component::Component,
    module: &InstalledModule,
    asset: Arc<dyn ReadOnlyAsset>,
    limits: RuntimeLimits,
) -> Result<SourceValidation, RuntimeError> {
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
    let (contract_version,) = guest
        .func_contract_version()
        .call_async(&mut store, ())
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_invoke_error", error))?;
    if contract_version != SOURCE_BATCH_CONTRACT_VERSION {
        return Err(RuntimeError::new(
            "module_contract_version_mismatch",
            format!(
                "guest reports source batch contract {contract_version}, host requires {SOURCE_BATCH_CONTRACT_VERSION}"
            ),
        ));
    }
    let (validation,) = guest
        .func_validate()
        .call_async(&mut store, (Resource::new_own(0),))
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_invoke_error", error))?;
    let validation = validation.map_err(|error| RuntimeError::new("module_guest_error", error))?;
    ensure_output_size(&validation, limits.max_output_bytes)?;
    let validation: SourceValidation = serde_json::from_str(&validation)
        .map_err(|error| RuntimeError::new("module_malformed_output", error.to_string()))?;
    let manifest = match &module.manifest {
        ModuleManifest::Source(manifest) => manifest,
        _ => {
            return Err(RuntimeError::new(
                "module_type_mismatch",
                "source validation requires a source manifest",
            ));
        }
    };
    if !validation.valid {
        return Err(RuntimeError::new(
            "source_validation_failed",
            serde_json::to_string(&validation.issues)
                .unwrap_or_else(|_| "source validation reported issues".to_owned()),
        ));
    }
    if validation.source_module_id != module.module_id.as_str()
        || validation.source_api_version != manifest.source_api_version
        || validation.mapping_version != manifest.mapping_version
        || validation.logical_snapshot_key.trim().is_empty()
        || !is_sha256_fingerprint(&validation.schema_fingerprint)
    {
        return Err(RuntimeError::new(
            "source_validation_metadata_mismatch",
            "guest validation metadata does not match the installed source module",
        ));
    }
    Ok(validation)
}

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

    let (contract_version,) = guest
        .func_contract_version()
        .call_async(&mut store, ())
        .await
        .map_err(|error| RuntimeError::from_wasmtime("module_invoke_error", error))?;
    if contract_version != SOURCE_BATCH_CONTRACT_VERSION {
        return Err(RuntimeError::new(
            "module_contract_version_mismatch",
            format!(
                "guest reports source batch contract {contract_version}, host requires {SOURCE_BATCH_CONTRACT_VERSION}"
            ),
        ));
    }

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
    validate_batch(module, &batch)?;
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
    let manifest = match &module.manifest {
        ModuleManifest::Source(manifest) => manifest,
        _ => {
            return Err(RuntimeError::new(
                "module_type_mismatch",
                "source invocation requires a source manifest",
            ));
        }
    };
    if descriptor.module_id != module.module_id.as_str()
        || descriptor.module_version != module.module_version
        || descriptor.source_api_version != manifest.source_api_version
        || descriptor.mapping_version != manifest.mapping_version
    {
        return Err(RuntimeError::new(
            "module_descriptor_mismatch",
            "guest descriptor does not identify the installed module",
        ));
    }
    let allowed = &manifest.provided_capabilities;
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
    if descriptor.extension_contracts.iter().any(|declared| {
        !manifest.extension_contracts.iter().any(|allowed| {
            allowed.namespace == declared.namespace
                && allowed.contract_version == declared.contract_version
        })
    }) {
        return Err(RuntimeError::new(
            "undeclared_extension_contract",
            "guest descriptor reports an undeclared extension contract",
        ));
    }
    Ok(())
}

fn validate_batch(module: &InstalledModule, batch: &SourceBatch) -> Result<(), RuntimeError> {
    let manifest = match &module.manifest {
        mfa_contracts::ModuleManifest::Source(manifest) => manifest,
        _ => {
            return Err(RuntimeError::new(
                "module_type_mismatch",
                "source manifest required",
            ));
        }
    };
    if batch.contract_version.to_string() != SOURCE_BATCH_CONTRACT_VERSION
        || batch.source_module_id != module.module_id.as_str()
        || batch.source_api_version != manifest.source_api_version
        || batch.mapping_version != manifest.mapping_version
    {
        return Err(RuntimeError::new(
            "source_batch_metadata_mismatch",
            "guest batch metadata does not match the installed source module",
        ));
    }
    if batch.logical_snapshot_key.trim().is_empty()
        || !is_sha256_fingerprint(&batch.schema_fingerprint)
    {
        return Err(RuntimeError::new(
            "source_batch_metadata_invalid",
            "guest batch logical key or schema fingerprint is invalid",
        ));
    }
    let mut source_keys = std::collections::BTreeSet::new();
    for source_record in &batch.source_records {
        if source_record.source_record_key.trim().is_empty()
            || source_record.source_row_number == 0
            || !source_keys.insert(source_record.source_record_key.clone())
        {
            return Err(RuntimeError::new(
                "source_record_identity_invalid",
                "guest source records must have unique non-empty keys and positive rows",
            ));
        }
    }
    for observation in &batch.records {
        if let Some(source_record_key) = observation_source_key(observation)
            && !source_keys.contains(source_record_key)
        {
            return Err(RuntimeError::new(
                "source_record_reference_invalid",
                "canonical observation references an unknown source record",
            ));
        }
    }
    for hook in &batch.lineage {
        if hook.canonical_entity_type.trim().is_empty()
            || hook.canonical_entity_id.trim().is_empty()
            || !source_keys.contains(&hook.source_record_key)
            || hook.mapping_version != batch.mapping_version
        {
            return Err(RuntimeError::new(
                "lineage_hook_invalid",
                "guest lineage hook does not reference a declared source record",
            ));
        }
    }
    if batch.extensions.iter().any(|extension| {
        !source_keys.contains(&extension.source_record_key)
            || !manifest.extension_contracts.iter().any(|contract| {
                contract.namespace == extension.namespace
                    && contract.contract_version == extension.contract_version
            })
    }) {
        return Err(RuntimeError::new(
            "undeclared_extension_contract",
            "guest batch reports an undeclared extension contract or source record",
        ));
    }
    Ok(())
}

fn observation_source_key(observation: &mfa_contracts::CanonicalObservation) -> Option<&str> {
    match observation {
        mfa_contracts::CanonicalObservation::NutritionItem(value) => {
            value.source_record_id.as_deref()
        }
        mfa_contracts::CanonicalObservation::BodyMeasurement(value) => {
            value.source_record_id.as_deref()
        }
        mfa_contracts::CanonicalObservation::ActivityEvent(value) => {
            value.source_record_id.as_deref()
        }
        mfa_contracts::CanonicalObservation::HeartRate(value) => value.source_record_id.as_deref(),
        mfa_contracts::CanonicalObservation::ExerciseSet(value) => {
            value.source_record_id.as_deref()
        }
        mfa_contracts::CanonicalObservation::ActivityDay(_)
        | mfa_contracts::CanonicalObservation::WorkoutSession(_)
        | mfa_contracts::CanonicalObservation::PhaseEvent(_) => None,
    }
}

fn is_sha256_fingerprint(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) mod bindings {
    wasmtime::component::bindgen!({
        path: "../../modules/sdk/wit/source-api.wit",
        world: "source-module",
    });
}
