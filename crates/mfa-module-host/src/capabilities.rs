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

    pub fn resolve_runtime(
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
                continue;
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

    pub fn apply_bundled_defaults(
        &self,
        modules: &[crate::InstalledModule],
        settings: &mut AppSettings,
    ) -> Result<ProviderResolution, CapabilityError> {
        let mut candidate = settings.clone();
        for (capability, module_id) in bundled_defaults() {
            if candidate.active_providers.contains_key(&capability) {
                continue;
            }
            let available = modules.iter().any(|module| {
                module.enabled
                    && module.module_id == module_id
                    && matches!(
                        &module.manifest,
                        ModuleManifest::Source(manifest)
                            if manifest.provided_capabilities.contains(&capability)
                    )
            });
            if available {
                candidate.active_providers.insert(capability, module_id);
            }
        }
        let resolution = self.resolve_runtime(modules, &candidate)?;
        *settings = candidate;
        Ok(resolution)
    }

    pub fn select_provider(
        &self,
        modules: &[crate::InstalledModule],
        settings: &mut AppSettings,
        capability: &CapabilityId,
        module_id: &ModuleId,
    ) -> Result<ProviderResolution, CapabilityError> {
        let mut candidate = settings.clone();
        candidate
            .active_providers
            .insert(capability.clone(), module_id.clone());
        let resolution = self.resolve(modules, &candidate)?;
        *settings = candidate;
        Ok(resolution)
    }
}

pub fn bundled_defaults() -> BTreeMap<CapabilityId, ModuleId> {
    [
        ("nutrition.items", "mynetdiary"),
        ("activity.events", "mynetdiary"),
        ("body.weight", "hevy"),
        ("body.fat_percentage", "hevy"),
        ("strength.sessions", "hevy"),
        ("strength.sets", "hevy"),
    ]
    .into_iter()
    .map(|(capability, module_id)| {
        (
            CapabilityId::try_from(capability).expect("valid bundled capability"),
            ModuleId::try_from(module_id).expect("valid bundled module id"),
        )
    })
    .collect()
}
