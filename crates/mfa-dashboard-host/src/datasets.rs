use mfa_analytics::DateRange;
use mfa_contracts::{
    CapabilityId, ContractVersion, DashboardInput, DashboardManifest, ExtensionRequirement,
};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionDataset {
    pub contract_version: ContractVersion,
    pub value: Value,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DatasetCatalog {
    pub capabilities: BTreeMap<CapabilityId, Value>,
    pub extensions: BTreeMap<String, ExtensionDataset>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DashboardError {
    #[error("required capability is not granted: {capability}")]
    MissingCapability { capability: CapabilityId },
    #[error("required extension is not granted: {namespace}")]
    MissingExtension { namespace: String },
    #[error("extension contract is incompatible: {namespace}@{expected}, got {actual}")]
    IncompatibleExtension {
        namespace: String,
        expected: ContractVersion,
        actual: ContractVersion,
    },
}

#[derive(Debug, Clone)]
pub struct DatasetResolver {
    catalog: DatasetCatalog,
}

impl DatasetResolver {
    pub fn new(catalog: DatasetCatalog) -> Self {
        Self { catalog }
    }

    pub async fn resolve(
        &self,
        manifest: &DashboardManifest,
        _request: DateRange,
    ) -> Result<DashboardInput, DashboardError> {
        let mut input = DashboardInput {
            page_id: None,
            capabilities: BTreeMap::new(),
            extensions: BTreeMap::new(),
        };
        for requirement in &manifest.required_capabilities {
            let value = self
                .catalog
                .capabilities
                .get(&requirement.capability)
                .cloned()
                .ok_or_else(|| DashboardError::MissingCapability {
                    capability: requirement.capability.clone(),
                })?;
            input
                .capabilities
                .insert(requirement.capability.clone(), value);
            if let Some(extension) = &requirement.extension {
                grant_extension(&self.catalog, &mut input, extension)?;
            }
        }
        for extension in &manifest.required_extension_contracts {
            grant_extension(&self.catalog, &mut input, extension)?;
        }
        Ok(input)
    }
}

fn grant_extension(
    catalog: &DatasetCatalog,
    input: &mut DashboardInput,
    requirement: &ExtensionRequirement,
) -> Result<(), DashboardError> {
    let Some(dataset) = catalog.extensions.get(&requirement.namespace) else {
        return Err(DashboardError::MissingExtension {
            namespace: requirement.namespace.clone(),
        });
    };
    if dataset.contract_version != requirement.contract_version {
        return Err(DashboardError::IncompatibleExtension {
            namespace: requirement.namespace.clone(),
            expected: requirement.contract_version.clone(),
            actual: dataset.contract_version.clone(),
        });
    }
    input
        .extensions
        .insert(requirement.namespace.clone(), dataset.value.clone());
    Ok(())
}
