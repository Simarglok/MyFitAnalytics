use mfa_contracts::{CapabilityId, DashboardInput};
use mfa_dashboard_base::{compose_json, describe_module};
use mfa_dashboard_host::{DocumentValidationError, validate_document_json};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn grant() -> DashboardInput {
    DashboardInput {
        page_id: None,
        capabilities: BTreeMap::from([
            (CapabilityId::try_from("body.weight").unwrap(), json!([])),
            (
                CapabilityId::try_from("body.fat_percentage").unwrap(),
                json!({}),
            ),
            (
                CapabilityId::try_from("nutrition.items").unwrap(),
                json!({}),
            ),
            (CapabilityId::try_from("activity.days").unwrap(), json!({})),
            (
                CapabilityId::try_from("strength.sessions").unwrap(),
                json!({}),
            ),
            (CapabilityId::try_from("strength.sets").unwrap(), json!({})),
        ]),
        extensions: BTreeMap::new(),
    }
}

fn localization_keys() -> BTreeSet<String> {
    let locale: Value = serde_json::from_str(include_str!("../locales/en.json")).unwrap();
    locale["keys"]
        .as_object()
        .unwrap()
        .keys()
        .map(|key| format!("base.{key}"))
        .collect()
}

#[test]
fn base_output_passes_the_same_document_validator_as_guest_dashboards() {
    let input = serde_json::json!({"capabilities": {}, "extensions": {}, "page_id": "overview"});
    let output = compose_json(&input.to_string()).unwrap();
    let raw: Value = serde_json::from_str(&output).unwrap();
    let document = validate_document_json(&raw, &grant(), &localization_keys()).unwrap();
    assert_eq!(document.title_key, "base.overview.title");
}

#[test]
fn base_descriptor_has_the_dashboard_contract_fields() {
    let descriptor: Value = serde_json::from_str(&describe_module()).unwrap();
    for field in [
        "module_id",
        "module_version",
        "dashboard_api_version",
        "required_capabilities",
        "localization_namespace",
    ] {
        assert!(
            descriptor.get(field).is_some(),
            "missing descriptor field {field}"
        );
    }
    assert_eq!(descriptor["module_id"], "base");
    assert!(
        matches!(compose_json("{\"page_id\":\"unknown\"}"), Err(error) if error == "unknown_page")
    );
    let _ = DocumentValidationError::MalformedDocument;
}
