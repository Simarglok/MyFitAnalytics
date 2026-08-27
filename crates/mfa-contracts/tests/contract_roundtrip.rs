use chrono::{NaiveDate, NaiveDateTime, Utc};
use mfa_contracts::{
    AvailabilityState, BodyMeasurement, CanonicalObservation, ContractVersion, DashboardManifest,
    ExtensionRecord, LineageHook, LocalDate, LocalDateTime, ModuleId, SourceBatch, SourceRecord,
    UtcInstant,
};
use semver::Version;
use serde_json::json;
use std::str::FromStr;
use uuid::Uuid;

#[test]
fn canonical_observations_use_explicit_stable_tags() {
    let observation = CanonicalObservation::BodyMeasurement(BodyMeasurement {
        body_measurement_id: Uuid::nil(),
        local_date: LocalDate::from_str("2026-01-02").unwrap(),
        weight_kg: 80.5,
        body_fat_pct: None,
        source_record_id: Some("record-1".to_owned()),
    });

    let encoded = serde_json::to_value(&observation).unwrap();
    assert_eq!(encoded["type"], "body_measurement");
    assert!(encoded.get("value").is_some());
    assert_eq!(
        serde_json::from_value::<CanonicalObservation>(encoded).unwrap(),
        observation
    );

    let state = serde_json::to_value(AvailabilityState::Ready).unwrap();
    assert_eq!(state["type"], "ready");
}

#[test]
fn temporal_newtypes_preserve_distinct_wire_semantics() {
    let date = LocalDate::from(NaiveDate::from_ymd_opt(2026, 1, 2).unwrap());
    let local_datetime = LocalDateTime::from(
        NaiveDateTime::parse_from_str("2026-01-02 03:04:05", "%Y-%m-%d %H:%M:%S").unwrap(),
    );
    let instant =
        UtcInstant::from(chrono::DateTime::<Utc>::from_timestamp(1_767_323_045, 0).unwrap());

    assert_eq!(serde_json::to_value(date).unwrap(), json!("2026-01-02"));
    assert_eq!(
        serde_json::to_value(local_datetime).unwrap(),
        json!("2026-01-02T03:04:05")
    );
    assert_eq!(
        serde_json::to_value(instant).unwrap(),
        json!("2026-01-02T03:04:05Z")
    );
    assert_ne!(
        serde_json::to_value(date).unwrap(),
        serde_json::to_value(local_datetime).unwrap()
    );
}

#[test]
fn extension_records_have_the_required_versioned_shape() {
    let record = ExtensionRecord {
        namespace: "hevy.set-rpe".to_owned(),
        contract_version: ContractVersion::from_str("1.0.0").unwrap(),
        record_type: "set_metadata".to_owned(),
        source_record_key: "workout_data.csv:2".to_owned(),
        occurred_local_at: None,
        local_date: Some(LocalDate::from_str("2026-01-02").unwrap()),
        payload: json!({"rpe": 8}),
    };
    let encoded = serde_json::to_value(&record).unwrap();
    assert_eq!(encoded["namespace"], "hevy.set-rpe");
    assert_eq!(encoded["contract_version"], "1.0.0");
    assert_eq!(encoded["record_type"], "set_metadata");
    assert!(encoded.get("payload").is_some());
    let decoded = serde_json::from_value::<ExtensionRecord>(json!({
        "namespace": "hevy.set-rpe",
        "contract_version": "1.0.0",
        "record_type": "set_metadata",
        "source_record_key": "workout_data.csv:2",
        "occurred_local_at": null,
        "local_date": "2026-01-02",
        "payload": {"rpe": 8}
    }))
    .unwrap();
    assert_eq!(decoded, record);
}

#[test]
fn invalid_ids_and_versions_are_rejected_with_stable_codes() {
    let id = ModuleId::try_from("   ").unwrap_err();
    assert_eq!(id.code(), "blank_module_id");

    let version = ContractVersion::from_str("not-semver").unwrap_err();
    assert_eq!(version.code(), "invalid_contract_version");
}

#[test]
fn source_batch_round_trips_without_variant_order_assumptions() {
    let batch = SourceBatch {
        contract_version: ContractVersion::from_str("1.0.0").unwrap(),
        source_module_id: "hevy".to_owned(),
        source_api_version: ContractVersion::from_str("1.0.0").unwrap(),
        mapping_version: ContractVersion::from_str("1.0.0").unwrap(),
        schema_fingerprint:
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        logical_snapshot_key: "hevy:measurements".to_owned(),
        source_records: vec![SourceRecord {
            source_record_key: "measurement_data.csv:2".to_owned(),
            sheet_name: None,
            source_row_number: 2,
            raw_payload: json!({"date": "2026-01-02", "weight_kg": "81.0"}),
        }],
        lineage: vec![LineageHook {
            canonical_entity_type: "body_measurement".to_owned(),
            canonical_entity_id: Uuid::nil().to_string(),
            source_record_key: "measurement_data.csv:2".to_owned(),
            mapping_version: ContractVersion::from_str("1.0.0").unwrap(),
        }],
        records: vec![CanonicalObservation::BodyMeasurement(BodyMeasurement {
            body_measurement_id: Uuid::nil(),
            local_date: LocalDate::from_str("2026-01-02").unwrap(),
            weight_kg: 81.0,
            body_fat_pct: Some(18.2),
            source_record_id: Some("measurement_data.csv:2".to_owned()),
        })],
        extensions: vec![],
        issues: vec![],
    };
    let encoded = serde_json::to_string(&batch).unwrap();
    let decoded: SourceBatch = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, batch);
    assert_eq!(
        serde_json::from_str::<ContractVersion>("\"1.2.3\"").unwrap(),
        ContractVersion(Version::new(1, 2, 3))
    );
}

#[test]
fn dashboard_manifest_round_trips_entrypoint_hash() {
    let manifest: DashboardManifest = serde_json::from_value(json!({
        "module_type": "dashboard",
        "module_id": "base",
        "module_version": "1.0.0",
        "package_format_version": "1.0.0",
        "dashboard_api_version": "1.0.0",
        "entrypoint_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "compatible_app_versions": [">=0.1.0"],
        "required_capabilities": [{"capability": "body.weight"}],
        "required_extension_contracts": [],
        "localization_namespace": "dashboard.base"
    }))
    .unwrap();
    assert_eq!(
        manifest.entrypoint_hash,
        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
    );
    let encoded = serde_json::to_value(manifest).unwrap();
    assert!(encoded.get("entrypoint_hash").is_some());
}
