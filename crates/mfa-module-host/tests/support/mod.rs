#![allow(dead_code)]

use mfa_contracts::{
    AssetMetadata, AssetReadError, CapabilityId, DashboardInput, DashboardManifest, ModuleId,
    ModuleManifest, ModuleType, ReadOnlyAsset, SourceManifest,
};
use mfa_module_host::{InstalledModule, RuntimeLimits};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

pub struct MemoryAsset {
    bytes: Vec<u8>,
}

impl MemoryAsset {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }
}

impl ReadOnlyAsset for MemoryAsset {
    fn metadata(&self) -> AssetMetadata {
        AssetMetadata {
            asset_id: Uuid::from_u128(1),
            file_name: "fixture.bin".to_owned(),
            media_type: "application/octet-stream".to_owned(),
            byte_len: self.bytes.len() as u64,
        }
    }

    fn read_at(&self, offset: u64, max_bytes: u32) -> Result<Vec<u8>, AssetReadError> {
        let start = usize::try_from(offset)
            .map_err(|_| AssetReadError::InvalidRange { offset, max_bytes })?;
        if start > self.bytes.len() {
            return Err(AssetReadError::InvalidRange { offset, max_bytes });
        }
        let end = start
            .saturating_add(max_bytes as usize)
            .min(self.bytes.len());
        Ok(self.bytes[start..end].to_vec())
    }
}

pub fn asset(bytes: impl Into<Vec<u8>>) -> Arc<dyn ReadOnlyAsset> {
    Arc::new(MemoryAsset::new(bytes))
}

pub fn limits() -> RuntimeLimits {
    RuntimeLimits {
        max_memory_bytes: 8 * 1024 * 1024,
        fuel: 1_000_000,
        timeout: std::time::Duration::from_secs(1),
        max_output_bytes: 64 * 1024,
    }
}

pub fn source_module(store: &TempDir, fixture: &str, capabilities: &[&str]) -> InstalledModule {
    source_module_with_declared_hash(store, fixture, capabilities, None)
}

pub fn source_module_with_declared_hash(
    store: &TempDir,
    fixture: &str,
    capabilities: &[&str],
    declared_hash: Option<&str>,
) -> InstalledModule {
    installed_module(
        store,
        fixture,
        "guest-source",
        ModuleType::Source,
        json!({
            "module_type": "source",
            "module_id": "guest-source",
            "module_version": "1.0.0",
            "package_format_version": "1.0.0",
            "source_api_version": "1.0.0",
            "mapping_version": "1.0.0",
            "compatible_app_versions": [">=0.1.0"],
            "provided_capabilities": capabilities,
            "accepted_file_patterns": ["*.fixture"],
            "artifact_signatures": [],
            "extension_contracts": [],
            "settings_schema": {},
            "entrypoint_hash": declared_hash.unwrap_or_default(),
            "localization_namespace": "source.guest"
        }),
        declared_hash,
    )
}

pub fn dashboard_module(store: &TempDir, fixture: &str) -> InstalledModule {
    installed_module(
        store,
        fixture,
        "guest-dashboard",
        ModuleType::Dashboard,
        json!({
            "module_type": "dashboard",
            "module_id": "guest-dashboard",
            "module_version": "1.0.0",
            "package_format_version": "1.0.0",
            "dashboard_api_version": "1.0.0",
            "entrypoint_hash": "sha256:placeholder",
            "compatible_app_versions": [">=0.1.0"],
            "required_capabilities": [{"capability": "body.weight"}],
            "required_extension_contracts": [],
            "localization_namespace": "dashboard.guest"
        }),
        None,
    )
}

fn installed_module(
    store: &TempDir,
    fixture: &str,
    module_id: &str,
    module_type: ModuleType,
    manifest_value: serde_json::Value,
    declared_hash: Option<&str>,
) -> InstalledModule {
    let bytes = fs::read(fixture_path(fixture)).unwrap();
    let package_hash = format!("{:x}", Sha256::digest(&bytes));
    let mut manifest_value = manifest_value;
    if declared_hash.is_none() {
        if manifest_value.get("artifact_signatures").is_some() {
            manifest_value["artifact_signatures"] = json!([format!("sha256:{package_hash}")]);
        }
        if manifest_value.get("entrypoint_hash").is_some() {
            manifest_value["entrypoint_hash"] = json!(format!("sha256:{package_hash}"));
        }
    }
    let root = store.path().join(module_id);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("module.wasm"), &bytes).unwrap();
    let manifest = match module_type {
        ModuleType::Source => ModuleManifest::Source(
            serde_json::from_value::<SourceManifest>(manifest_value).unwrap(),
        ),
        ModuleType::Dashboard => ModuleManifest::Dashboard(
            serde_json::from_value::<DashboardManifest>(manifest_value).unwrap(),
        ),
        ModuleType::Locale => unreachable!(),
    };
    InstalledModule {
        module_id: ModuleId::try_from(module_id).unwrap(),
        module_type,
        module_version: "1.0.0".parse().unwrap(),
        package_hash,
        root,
        enabled: true,
        manifest,
    }
}

pub fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub fn dashboard_input() -> DashboardInput {
    let mut capabilities = BTreeMap::new();
    capabilities.insert(CapabilityId::try_from("body.weight").unwrap(), json!(82.5));
    DashboardInput {
        capabilities,
        extensions: BTreeMap::new(),
    }
}
