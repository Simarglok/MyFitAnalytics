use mfa_contracts::{CapabilityId, DashboardBlock, DashboardInput, ModuleManifest};
use mfa_module_host::{ComponentRuntime, PackageInstaller, RuntimeLimits};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;
use tempfile::TempDir;

fn bundled_package_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/modules/base.mfadashboard")
}

#[test]
fn bundled_base_package_is_inspectable_by_the_production_installer() {
    let store = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let inspected = installer.inspect(&bundled_package_path()).unwrap();
    assert!(matches!(&inspected.manifest, ModuleManifest::Dashboard(_)));
    assert_eq!(
        inspected
            .entries
            .iter()
            .filter(|entry| !entry.is_dir)
            .count(),
        3
    );
}

#[tokio::test]
async fn bundled_base_component_invokes_through_the_production_runtime() {
    let store = TempDir::new().unwrap();
    let installer = PackageInstaller::new(store.path());
    let module = installer.install(&bundled_package_path()).unwrap();
    let input = DashboardInput {
        capabilities: [
            ("activity.days", json!({"steps": 9000})),
            ("body.fat_percentage", json!({"value": 18.0})),
            ("body.weight", json!([])),
            ("nutrition.items", json!({"calories": 2400})),
            ("strength.sessions", json!({"sessions": 4})),
            ("strength.sets", json!({"sets": 24})),
        ]
        .into_iter()
        .map(|(name, value)| (CapabilityId::try_from(name).unwrap(), value))
        .collect::<BTreeMap<_, _>>(),
        extensions: BTreeMap::new(),
    };
    let document = ComponentRuntime::new()
        .invoke_dashboard(&module, input, RuntimeLimits::default())
        .await
        .unwrap();
    assert_eq!(document.title_key, "base.overview.title");
    assert!(
        document
            .blocks
            .iter()
            .any(|block| matches!(block, DashboardBlock::Chart(_)))
    );
}
