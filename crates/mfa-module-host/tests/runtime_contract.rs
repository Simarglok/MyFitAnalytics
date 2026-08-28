mod support;

use mfa_contracts::{CanonicalObservation, DashboardBlock, ModuleType};
use mfa_module_host::ComponentRuntime;
use std::sync::Arc;
use support::{
    asset, dashboard_input, dashboard_input_for_page, dashboard_module, limits, source_module,
};
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn source_component_transforms_asset_and_returns_validated_batch() {
    let store = TempDir::new().unwrap();
    let module = source_module(&store, "guest-source.wasm", &["body.weight"]);
    assert_eq!(module.module_type, ModuleType::Source);

    let batch = ComponentRuntime::new()
        .invoke_source(&module, asset(b"ok"), limits())
        .await
        .unwrap();

    assert_eq!(batch.extensions.len(), 0);
    assert!(batch.issues.is_empty());
    assert_eq!(batch.records.len(), 1);
    match &batch.records[0] {
        CanonicalObservation::BodyMeasurement(measurement) => {
            assert_eq!(measurement.weight_kg, 82.5);
        }
        other => panic!("unexpected source record: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn source_component_validation_returns_guest_snapshot_metadata() {
    let store = TempDir::new().unwrap();
    let module = source_module(&store, "guest-source.wasm", &["body.weight"]);
    let validation = ComponentRuntime::new()
        .validate_source(&module, asset(b"ok"), limits())
        .await
        .unwrap();

    assert!(validation.valid);
    assert_eq!(validation.source_module_id, "guest-source");
    assert_eq!(validation.source_api_version.to_string(), "1.0.0");
    assert_eq!(validation.mapping_version.to_string(), "1.0.0");
    assert_eq!(validation.logical_snapshot_key, "fixture:2026");
    assert!(validation.schema_fingerprint.starts_with("sha256:"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn source_component_rejects_duplicate_guest_source_record_keys() {
    let store = TempDir::new().unwrap();
    let module = source_module(&store, "guest-source.wasm", &["body.weight"]);
    let error = ComponentRuntime::new()
        .invoke_source(&module, asset(b"invalid-record"), limits())
        .await
        .unwrap_err();

    assert_eq!(error.code(), "source_record_identity_invalid");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dashboard_component_returns_only_declarative_document_contract() {
    let store = TempDir::new().unwrap();
    let module = dashboard_module(&store, "guest-dashboard.wasm");
    let document = ComponentRuntime::new()
        .invoke_dashboard(&module, dashboard_input(), limits())
        .await
        .unwrap();

    assert!(document.is_declarative());
    assert_eq!(document.title_key, "dashboard.guest.title");
    assert!(matches!(
        document.blocks.first(),
        Some(DashboardBlock::Card(_))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dashboard_component_rejects_unknown_raw_document_and_block_fields() {
    let store = TempDir::new().unwrap();
    let module = dashboard_module(&store, "guest-dashboard.wasm");
    let runtime = ComponentRuntime::new();

    let top_level = runtime
        .invoke_dashboard(
            &module,
            dashboard_input_for_page(Some("raw-top-level-unknown")),
            limits(),
        )
        .await
        .unwrap_err();
    assert_eq!(top_level.code(), "module_malformed_output");

    for page_id in ["raw-block-on-click", "raw-block-html", "raw-block-url"] {
        let error = runtime
            .invoke_dashboard(&module, dashboard_input_for_page(Some(page_id)), limits())
            .await
            .unwrap_err();
        assert_eq!(
            error.code(),
            "module_malformed_output",
            "field case: {page_id}"
        );
    }
}

#[test]
fn runtime_api_accepts_shared_read_only_assets_without_path_authority() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ComponentRuntime>();
    let _asset: Arc<dyn mfa_contracts::ReadOnlyAsset> = asset(b"ok");
}
