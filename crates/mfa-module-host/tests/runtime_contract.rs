mod support;

use mfa_contracts::{CanonicalObservation, DashboardBlock, ModuleType};
use mfa_module_host::ComponentRuntime;
use std::sync::Arc;
use support::{asset, dashboard_input, dashboard_module, limits, source_module};
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

#[test]
fn runtime_api_accepts_shared_read_only_assets_without_path_authority() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ComponentRuntime>();
    let _asset: Arc<dyn mfa_contracts::ReadOnlyAsset> = asset(b"ok");
}
