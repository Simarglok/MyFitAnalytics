use mfa_contracts::{
    CapabilityId, ContractVersion, DashboardInput, DashboardManifest, DashboardRequirement,
    ExtensionRequirement, ModuleId, ModuleType,
};
use mfa_dashboard_host::{DashboardError, DatasetCatalog, DatasetResolver, ExtensionDataset};
use serde_json::json;
use std::collections::BTreeMap;
use std::str::FromStr;

fn version(value: &str) -> ContractVersion {
    ContractVersion::from_str(value).unwrap()
}

fn manifest(
    capabilities: Vec<DashboardRequirement>,
    extensions: Vec<ExtensionRequirement>,
) -> DashboardManifest {
    DashboardManifest {
        module_type: ModuleType::Dashboard,
        module_id: ModuleId::try_from("fixture-dashboard").unwrap(),
        module_version: version("1.0.0"),
        package_format_version: version("1.0.0"),
        dashboard_api_version: version("1.0.0"),
        entrypoint_hash: "a".repeat(64),
        compatible_app_versions: vec![">=0.1.0".to_owned()],
        required_capabilities: capabilities,
        required_extension_contracts: extensions,
        localization_namespace: "fixture.dashboard".to_owned(),
    }
}

fn catalog() -> DatasetCatalog {
    DatasetCatalog {
        capabilities: BTreeMap::from([
            (
                CapabilityId::try_from("body.weight").unwrap(),
                json!({"kg": 82.5}),
            ),
            (
                CapabilityId::try_from("nutrition.items").unwrap(),
                json!({"kcal": 2_000}),
            ),
            (
                CapabilityId::try_from("private.secret").unwrap(),
                json!({"token": "never"}),
            ),
        ]),
        extensions: BTreeMap::from([
            (
                "workouts.rpe".to_owned(),
                ExtensionDataset {
                    contract_version: version("1.0.0"),
                    value: json!({"rpe": 8}),
                },
            ),
            (
                "other.private".to_owned(),
                ExtensionDataset {
                    contract_version: version("1.0.0"),
                    value: json!({"private": true}),
                },
            ),
        ]),
    }
}

#[tokio::test]
async fn resolver_grants_only_declared_capabilities_and_extensions() {
    let manifest = manifest(
        vec![DashboardRequirement {
            capability: CapabilityId::try_from("body.weight").unwrap(),
            extension: Some(ExtensionRequirement {
                namespace: "workouts.rpe".to_owned(),
                contract_version: version("1.0.0"),
            }),
        }],
        vec![],
    );
    let input = DatasetResolver::new(catalog())
        .resolve(
            &manifest,
            mfa_analytics::DateRange::inclusive(
                "2026-01-01".parse().unwrap(),
                "2026-01-28".parse().unwrap(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(input.capabilities.len(), 1);
    assert!(
        input
            .capabilities
            .contains_key(&CapabilityId::try_from("body.weight").unwrap())
    );
    assert_eq!(input.extensions.len(), 1);
    assert!(input.extensions.contains_key("workouts.rpe"));
    assert!(
        !input
            .capabilities
            .contains_key(&CapabilityId::try_from("private.secret").unwrap())
    );
    assert!(!input.extensions.contains_key("other.private"));
}

#[tokio::test]
async fn resolver_requires_all_declared_namespaces_and_exact_compatible_versions() {
    let manifest = manifest(
        vec![
            DashboardRequirement {
                capability: CapabilityId::try_from("body.weight").unwrap(),
                extension: None,
            },
            DashboardRequirement {
                capability: CapabilityId::try_from("nutrition.items").unwrap(),
                extension: None,
            },
        ],
        vec![ExtensionRequirement {
            namespace: "workouts.rpe".to_owned(),
            contract_version: version("2.0.0"),
        }],
    );
    let error = DatasetResolver::new(catalog())
        .resolve(
            &manifest,
            mfa_analytics::DateRange::inclusive(
                "2026-01-01".parse().unwrap(),
                "2026-01-28".parse().unwrap(),
            ),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        DashboardError::IncompatibleExtension { .. }
    ));

    let _empty = DashboardInput {
        capabilities: BTreeMap::new(),
        extensions: BTreeMap::new(),
    };
}
