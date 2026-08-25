use mfa_config::AppSettings;
use mfa_contracts::{CapabilityId, ModuleId, ModuleManifest, ModuleType};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CapabilityError {
    #[error("provider {module_id} is missing")]
    MissingProvider { module_id: ModuleId },
    #[error("provider {module_id} is disabled")]
    DisabledProvider { module_id: ModuleId },
    #[error("provider {module_id} does not offer capability {capability}")]
    CapabilityNotOffered {
        module_id: ModuleId,
        capability: CapabilityId,
    },
    #[error("capability {capability} requires a source provider")]
    WrongModuleType { capability: CapabilityId },
}

impl CapabilityError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingProvider { .. } => "missing_provider",
            Self::DisabledProvider { .. } => "disabled_provider",
            Self::CapabilityNotOffered { .. } => "capability_not_offered",
            Self::WrongModuleType { .. } => "wrong_module_type",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResolution {
    pub active_providers: BTreeMap<CapabilityId, ModuleId>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CapabilityRegistry;

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn resolve(
        &self,
        modules: &[crate::InstalledModule],
        settings: &AppSettings,
    ) -> Result<ProviderResolution, CapabilityError> {
        let mut active_providers = BTreeMap::new();
        for (capability, module_id) in &settings.active_providers {
            let module = modules
                .iter()
                .find(|module| &module.module_id == module_id)
                .ok_or_else(|| CapabilityError::MissingProvider {
                    module_id: module_id.clone(),
                })?;
            if !module.enabled {
                return Err(CapabilityError::DisabledProvider {
                    module_id: module_id.clone(),
                });
            }
            if module.module_type != ModuleType::Source {
                return Err(CapabilityError::WrongModuleType {
                    capability: capability.clone(),
                });
            }
            let offered = match &module.manifest {
                ModuleManifest::Source(manifest) => &manifest.provided_capabilities,
                _ => unreachable!("module type was checked above"),
            };
            if !offered.contains(capability) {
                return Err(CapabilityError::CapabilityNotOffered {
                    module_id: module_id.clone(),
                    capability: capability.clone(),
                });
            }
            active_providers.insert(capability.clone(), module_id.clone());
        }
        Ok(ProviderResolution { active_providers })
    }
}
