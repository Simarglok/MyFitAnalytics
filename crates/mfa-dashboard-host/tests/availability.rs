use mfa_contracts::{
    AvailabilityState, CapabilityId, ContractVersion, DashboardRequirement, ModuleId,
};
use mfa_dashboard_host::{
    Availability, AvailabilityResolver, CoverageCatalog, Freshness, ModuleRegistryView,
    ResolvedCapabilities, ResolvedCapability,
};

fn requirement() -> DashboardRequirement {
    DashboardRequirement {
        capability: CapabilityId::try_from("body.weight").unwrap(),
        extension: None,
    }
}

fn provider_status(has_successful_snapshot: bool, freshness: Freshness) -> ResolvedCapability {
    ResolvedCapability {
        provider: Some(ModuleId::try_from("hevy").unwrap()),
        contract_compatible: true,
        has_successful_snapshot,
        freshness,
    }
}

fn registry(status: ResolvedCapability) -> ResolvedCapabilities {
    ResolvedCapabilities::from([(CapabilityId::try_from("body.weight").unwrap(), status)])
}

fn ready_modules() -> ModuleRegistryView {
    ModuleRegistryView::default()
}

fn assert_state(availability: Availability, state: AvailabilityState, message_key: &str) {
    assert_eq!(availability.state, state);
    assert_eq!(availability.message_key, message_key);
}

#[test]
fn availability_uses_stable_precedence_and_payload() {
    let resolver = AvailabilityResolver;
    let mut modules = ready_modules();
    modules.disabled_by_user = true;
    modules.missing_dependency = true;
    modules.incompatible_contract = true;
    assert_state(
        resolver.resolve(
            &requirement(),
            &registry(provider_status(false, Freshness::Fresh)),
            &CoverageCatalog::default(),
            &modules,
        ),
        AvailabilityState::DisabledByUser,
        "dashboard.disabled_by_user",
    );

    modules.disabled_by_user = false;
    assert_state(
        resolver.resolve(
            &requirement(),
            &registry(provider_status(false, Freshness::Fresh)),
            &CoverageCatalog::default(),
            &modules,
        ),
        AvailabilityState::MissingDependency,
        "dashboard.missing_dependency",
    );

    modules.missing_dependency = false;
    assert_state(
        resolver.resolve(
            &requirement(),
            &registry(ResolvedCapability {
                provider: Some(ModuleId::try_from("hevy").unwrap()),
                contract_compatible: false,
                has_successful_snapshot: true,
                freshness: Freshness::Fresh,
            }),
            &CoverageCatalog::default(),
            &modules,
        ),
        AvailabilityState::IncompatibleContract,
        "dashboard.incompatible_contract",
    );

    modules.incompatible_contract = false;
    assert_state(
        resolver.resolve(
            &requirement(),
            &ResolvedCapabilities::default(),
            &CoverageCatalog::default(),
            &modules,
        ),
        AvailabilityState::MissingCapability,
        "dashboard.missing_capability",
    );

    assert_state(
        resolver.resolve(
            &requirement(),
            &registry(provider_status(false, Freshness::Fresh)),
            &CoverageCatalog::default(),
            &modules,
        ),
        AvailabilityState::WaitingForData,
        "dashboard.waiting_for_data",
    );

    let mut coverage = CoverageCatalog::default();
    coverage
        .sufficient
        .insert(CapabilityId::try_from("body.weight").unwrap(), false);
    assert_state(
        resolver.resolve(
            &requirement(),
            &registry(provider_status(true, Freshness::Fresh)),
            &coverage,
            &modules,
        ),
        AvailabilityState::InsufficientCoverage,
        "dashboard.insufficient_coverage",
    );

    let ready = resolver.resolve(
        &requirement(),
        &registry(provider_status(true, Freshness::Stale)),
        &CoverageCatalog::from([(CapabilityId::try_from("body.weight").unwrap(), true)]),
        &modules,
    );
    assert_state(ready.clone(), AvailabilityState::Ready, "dashboard.ready");
    assert_eq!(ready.freshness, Freshness::Stale);
    assert!(ready.action.is_none());
}

#[test]
fn availability_payload_actions_are_state_specific() {
    let resolver = AvailabilityResolver;
    let mut modules = ModuleRegistryView {
        disabled_by_user: true,
        ..ModuleRegistryView::default()
    };
    let disabled = resolver.resolve(
        &requirement(),
        &ResolvedCapabilities::default(),
        &CoverageCatalog::default(),
        &modules,
    );
    assert_eq!(disabled.action.as_deref(), Some("dashboard.action.enable"));

    modules.disabled_by_user = false;
    modules.missing_dependency = true;
    let missing = resolver.resolve(
        &requirement(),
        &ResolvedCapabilities::default(),
        &CoverageCatalog::default(),
        &modules,
    );
    assert_eq!(
        missing.action.as_deref(),
        Some("dashboard.action.configure_source")
    );

    let _ = ContractVersion::try_from("1.0.0").unwrap();
}
