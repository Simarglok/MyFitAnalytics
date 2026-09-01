use mfa_contracts::{
    AvailabilityState, CapabilityId, DashboardRequirement, ExtensionRequirement, ModuleId,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Fresh,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCapability {
    pub provider: Option<ModuleId>,
    pub contract_compatible: bool,
    pub has_successful_snapshot: bool,
    pub freshness: Freshness,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedExtension {
    pub available: bool,
    pub contract_compatible: bool,
    pub freshness: Freshness,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedCapabilities {
    pub capabilities: BTreeMap<CapabilityId, ResolvedCapability>,
    pub extensions: BTreeMap<String, ResolvedExtension>,
}

impl<const N: usize> From<[(CapabilityId, ResolvedCapability); N]> for ResolvedCapabilities {
    fn from(entries: [(CapabilityId, ResolvedCapability); N]) -> Self {
        Self {
            capabilities: BTreeMap::from(entries),
            extensions: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoverageCatalog {
    pub sufficient: BTreeMap<CapabilityId, bool>,
}

impl<const N: usize> From<[(CapabilityId, bool); N]> for CoverageCatalog {
    fn from(entries: [(CapabilityId, bool); N]) -> Self {
        Self {
            sufficient: BTreeMap::from(entries),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRegistryView {
    pub disabled_by_user: bool,
    pub missing_dependency: bool,
    pub incompatible_contract: bool,
    pub freshness: Freshness,
}

impl Default for ModuleRegistryView {
    fn default() -> Self {
        Self {
            disabled_by_user: false,
            missing_dependency: false,
            incompatible_contract: false,
            freshness: Freshness::Fresh,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Availability {
    pub state: AvailabilityState,
    pub freshness: Freshness,
    pub message_key: String,
    pub action: Option<String>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct AvailabilityResolver;

impl AvailabilityResolver {
    pub fn resolve(
        &self,
        requirement: &DashboardRequirement,
        registry: &ResolvedCapabilities,
        coverage: &CoverageCatalog,
        modules: &ModuleRegistryView,
    ) -> Availability {
        let freshness = capability_freshness(requirement, registry, modules);
        if modules.disabled_by_user {
            return Availability::new(
                AvailabilityState::DisabledByUser,
                freshness,
                "dashboard.disabled_by_user",
                Some("dashboard.action.enable"),
            );
        }
        if modules.missing_dependency {
            return Availability::new(
                AvailabilityState::MissingDependency,
                freshness,
                "dashboard.missing_dependency",
                Some("dashboard.action.configure_source"),
            );
        }
        if modules.incompatible_contract {
            return Availability::new(
                AvailabilityState::IncompatibleContract,
                freshness,
                "dashboard.incompatible_contract",
                Some("dashboard.action.update_module"),
            );
        }
        if let Some(extension) = &requirement.extension {
            match registry.extensions.get(&extension.namespace) {
                None => {
                    return Availability::new(
                        AvailabilityState::MissingDependency,
                        freshness,
                        "dashboard.missing_dependency",
                        Some("dashboard.action.configure_source"),
                    );
                }
                Some(status) if !status.available => {
                    return Availability::new(
                        AvailabilityState::WaitingForData,
                        freshness,
                        "dashboard.waiting_for_data",
                        Some("dashboard.action.import_data"),
                    );
                }
                Some(status) if !status.contract_compatible => {
                    return Availability::new(
                        AvailabilityState::IncompatibleContract,
                        freshness,
                        "dashboard.incompatible_contract",
                        Some("dashboard.action.update_module"),
                    );
                }
                Some(_) => {}
            }
        }
        let Some(capability) = registry.capabilities.get(&requirement.capability) else {
            return Availability::new(
                AvailabilityState::MissingCapability,
                freshness,
                "dashboard.missing_capability",
                Some("dashboard.action.configure_source"),
            );
        };
        if !capability.contract_compatible {
            return Availability::new(
                AvailabilityState::IncompatibleContract,
                freshness,
                "dashboard.incompatible_contract",
                Some("dashboard.action.update_module"),
            );
        }
        if capability.provider.is_none() {
            return Availability::new(
                AvailabilityState::MissingCapability,
                freshness,
                "dashboard.missing_capability",
                Some("dashboard.action.configure_source"),
            );
        }
        if !capability.has_successful_snapshot {
            return Availability::new(
                AvailabilityState::WaitingForData,
                freshness,
                "dashboard.waiting_for_data",
                Some("dashboard.action.import_data"),
            );
        }
        if !coverage
            .sufficient
            .get(&requirement.capability)
            .copied()
            .unwrap_or(true)
        {
            return Availability::new(
                AvailabilityState::InsufficientCoverage,
                freshness,
                "dashboard.insufficient_coverage",
                Some("dashboard.action.import_data"),
            );
        }
        Availability::new(AvailabilityState::Ready, freshness, "dashboard.ready", None)
    }
}

impl Availability {
    fn new(
        state: AvailabilityState,
        freshness: Freshness,
        message_key: &str,
        action: Option<&str>,
    ) -> Self {
        Self {
            state,
            freshness,
            message_key: message_key.to_owned(),
            action: action.map(str::to_owned),
        }
    }
}

fn capability_freshness(
    requirement: &DashboardRequirement,
    registry: &ResolvedCapabilities,
    modules: &ModuleRegistryView,
) -> Freshness {
    if modules.freshness == Freshness::Stale {
        return Freshness::Stale;
    }
    if registry
        .capabilities
        .get(&requirement.capability)
        .is_some_and(|status| status.freshness == Freshness::Stale)
    {
        return Freshness::Stale;
    }
    if requirement
        .extension
        .as_ref()
        .and_then(|ExtensionRequirement { namespace, .. }| registry.extensions.get(namespace))
        .is_some_and(|status| status.freshness == Freshness::Stale)
    {
        return Freshness::Stale;
    }
    Freshness::Fresh
}
