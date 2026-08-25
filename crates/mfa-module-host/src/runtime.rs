use crate::dashboard_runtime;
use crate::limits::RuntimeLimits;
use crate::package::InstalledModule;
use crate::source_runtime;
use mfa_contracts::{
    DashboardDocument, DashboardInput, ModuleManifest, ReadOnlyAsset, SourceBatch,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wasmtime::component::Component;
use wasmtime::{Config, Engine};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    code: String,
    detail: String,
}

impl RuntimeError {
    pub(crate) fn new(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            detail: detail.into(),
        }
    }

    pub(crate) fn from_wasmtime(code: &str, error: wasmtime::Error) -> Self {
        let detail = error.to_string();
        let lower = detail.to_ascii_lowercase();
        let classified = if lower.contains("fuel") {
            "module_fuel_exhausted"
        } else if lower.contains("memory")
            || lower.contains("resource limit")
            || lower.contains("rust_oom")
            || lower.contains("alloc_error")
            || lower.contains("dlmalloc")
            || lower.contains("__rust_alloc")
        {
            "module_memory_limit"
        } else {
            code
        };
        Self::new(classified, detail)
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for RuntimeError {}

struct RuntimeInner {
    engine: Engine,
    source_cache: Mutex<HashMap<String, Component>>,
    dashboard_cache: Mutex<HashMap<String, Component>>,
}

#[derive(Clone)]
pub struct ComponentRuntime {
    inner: Arc<RuntimeInner>,
}

impl ComponentRuntime {
    pub fn new() -> Self {
        let mut config = Config::new();
        #[allow(deprecated)]
        config.async_support(true);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let engine = Engine::new(&config).expect("static Wasmtime runtime configuration");
        Self {
            inner: Arc::new(RuntimeInner {
                engine,
                source_cache: Mutex::new(HashMap::new()),
                dashboard_cache: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub async fn invoke_source(
        &self,
        module: &InstalledModule,
        asset: Arc<dyn ReadOnlyAsset>,
        limits: RuntimeLimits,
    ) -> Result<SourceBatch, RuntimeError> {
        let component = self.load_component(module, true)?;
        let engine = self.inner.engine.clone();
        let timeout = limits.timeout;
        with_timeout(
            engine,
            timeout,
            source_runtime::invoke(&self.inner.engine, &component, module, asset, limits),
        )
        .await
    }

    pub async fn invoke_dashboard(
        &self,
        module: &InstalledModule,
        input: DashboardInput,
        limits: RuntimeLimits,
    ) -> Result<DashboardDocument, RuntimeError> {
        let component = self.load_component(module, false)?;
        let engine = self.inner.engine.clone();
        let timeout = limits.timeout;
        with_timeout(
            engine,
            timeout,
            dashboard_runtime::invoke(&self.inner.engine, &component, module, input, limits),
        )
        .await
    }

    fn load_component(
        &self,
        module: &InstalledModule,
        source: bool,
    ) -> Result<Component, RuntimeError> {
        let bytes = std::fs::read(module.root.join("module.wasm"))
            .map_err(|error| RuntimeError::new("module_wasm_missing", error.to_string()))?;
        validate_entrypoint_hash(module, &bytes)?;
        let cache = if source {
            &self.inner.source_cache
        } else {
            &self.inner.dashboard_cache
        };
        let mut cache = cache
            .lock()
            .map_err(|_| RuntimeError::new("module_cache_poisoned", "component cache poisoned"))?;
        if let Some(component) = cache.get(&module.package_hash) {
            return Ok(component.clone());
        }
        let component = Component::new(&self.inner.engine, &bytes)
            .map_err(|error| RuntimeError::from_wasmtime("module_compile_error", error))?;
        cache.insert(module.package_hash.clone(), component.clone());
        Ok(component)
    }
}

impl Default for ComponentRuntime {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_entrypoint_hash(module: &InstalledModule, bytes: &[u8]) -> Result<(), RuntimeError> {
    let declared = match &module.manifest {
        ModuleManifest::Source(manifest) => &manifest.entrypoint_hash,
        ModuleManifest::Dashboard(manifest) => &manifest.entrypoint_hash,
        ModuleManifest::Locale(_) => {
            return Err(RuntimeError::new(
                "module_type_mismatch",
                "locale modules cannot be executed",
            ));
        }
    };
    let actual = format!("sha256:{:x}", Sha256::digest(bytes));
    if declared != &actual {
        return Err(RuntimeError::new(
            "module_hash_mismatch",
            format!("manifest declares {declared}, module.wasm is {actual}"),
        ));
    }
    Ok(())
}

async fn with_timeout<T, F>(engine: Engine, timeout: Duration, future: F) -> Result<T, RuntimeError>
where
    F: Future<Output = Result<T, RuntimeError>>,
{
    let deadline_fired = Arc::new(AtomicBool::new(false));
    let timer_deadline_fired = Arc::clone(&deadline_fired);
    let epoch_engine = engine.clone();
    let timer = tokio::spawn(async move {
        tokio::time::sleep(timeout).await;
        timer_deadline_fired.store(true, Ordering::Release);
        epoch_engine.increment_epoch();
    });
    let grace = timeout.saturating_add(Duration::from_millis(250));
    let result = tokio::time::timeout(grace, future).await;
    timer.abort();
    match result {
        Ok(Err(error)) if deadline_fired.load(Ordering::Acquire) => Err(RuntimeError::new(
            "module_timeout",
            error.detail().to_owned(),
        )),
        Ok(result) => result,
        Err(_) => Err(RuntimeError::new(
            "module_timeout",
            "component invocation exceeded its epoch deadline",
        )),
    }
}
