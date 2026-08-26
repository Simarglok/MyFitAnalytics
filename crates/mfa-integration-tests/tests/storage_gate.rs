use mfa_archive::{ArchiveCoordinator, ArchiveReconciler};
use mfa_config::WorkspacePaths;
use mfa_contracts::{
    CanonicalObservation, CapabilityId, ModuleId, ModuleManifest, ModuleType, NutritionItem,
    SourceBatch, SourceManifest, UtcInstant,
};
use mfa_db::{
    DatabaseFailurePoint, DatabaseService, LogicalSnapshotKey, QueryView, TestDatabaseFaultInjector,
};
use mfa_ingestion::{
    ArchiveRebuildConfig, ArchiveRebuildConfirmation, ArchiveRebuildService, BoxFuture, CoreEvent,
    IngestionCoordinator, IngestionDependencies, RecoveryGate, RecoveryService,
    RuntimeArchiveImporter, ScanReason, ScanRequest, SourceInvoker,
};
use mfa_module_host::{InstalledModule, RuntimeError, RuntimeLimits};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;
use tempfile::TempDir;

#[derive(Clone)]
struct FixtureRuntime {
    fail_next: Arc<AtomicBool>,
    invocations: Arc<AtomicUsize>,
    batch: SourceBatch,
}

impl SourceInvoker for FixtureRuntime {
    fn invoke_source<'a>(
        &'a self,
        _module: &'a InstalledModule,
        _asset: Arc<dyn mfa_contracts::ReadOnlyAsset>,
        _limits: RuntimeLimits,
    ) -> BoxFuture<'a, Result<SourceBatch, RuntimeError>> {
        Box::pin(async move {
            self.invocations.fetch_add(1, Ordering::SeqCst);
            if self.fail_next.swap(false, Ordering::SeqCst) {
                return Err(RuntimeError::new(
                    "module_guest_error",
                    "synthetic parse failure",
                ));
            }
            Ok(self.batch.clone())
        })
    }
}

fn fixture_module(temp: &TempDir, module_id: ModuleId) -> InstalledModule {
    let root = temp.path().join("fixture-module");
    std::fs::create_dir_all(&root).unwrap();
    InstalledModule {
        module_id: module_id.clone(),
        module_type: ModuleType::Source,
        module_version: "1.0.0".parse().unwrap(),
        package_hash: "sha256:fixture-package".to_owned(),
        root,
        enabled: true,
        manifest: ModuleManifest::Source(SourceManifest {
            module_type: ModuleType::Source,
            module_id,
            module_version: "1.0.0".parse().unwrap(),
            package_format_version: "1.0.0".parse().unwrap(),
            source_api_version: "1.0.0".parse().unwrap(),
            mapping_version: "1.0.0".parse().unwrap(),
            compatible_app_versions: vec![">=0.1.0".to_owned()],
            provided_capabilities: vec![CapabilityId::try_from("nutrition.items").unwrap()],
            accepted_file_patterns: vec!["*.fixture".to_owned()],
            entrypoint_hash: "sha256:fixture-entrypoint".to_owned(),
            localization_namespace: "source.fixture".to_owned(),
        }),
    }
}

fn fixture_batch() -> SourceBatch {
    SourceBatch {
        records: vec![CanonicalObservation::NutritionItem(NutritionItem {
            nutrition_item_id: uuid::Uuid::from_u128(701),
            occurred_local_at: None,
            local_date: "2026-01-01".parse().unwrap(),
            meal: "Lunch".to_owned(),
            food_source_id: "fixture-food".to_owned(),
            name: "Synthetic gate meal".to_owned(),
            amount_raw: "1 serving".to_owned(),
            calories_kcal: Some(500.0),
            protein_g: Some(20.0),
            fat_g: Some(10.0),
            carbs_g: Some(60.0),
            fiber_g: None,
            sugars_g: None,
            sodium_mg: None,
            source_record_id: Some("gate-source-1".to_owned()),
        })],
        extensions: Vec::new(),
        issues: Vec::new(),
    }
}

fn request() -> ScanRequest {
    ScanRequest::new(
        ScanReason::Manual,
        "2026-08-25T00:00:00Z".parse::<UtcInstant>().unwrap(),
    )
}

async fn scan_twice(coordinator: &IngestionCoordinator) {
    coordinator.request_scan(request()).await.unwrap();
    coordinator.request_scan(request()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn storage_gate_serializes_all_archive_ingestion_recovery_and_rebuild_paths() {
    let temp = TempDir::new().unwrap();
    let source = ModuleId::try_from("fixture-source").unwrap();
    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    workspace.enable_source(&source).unwrap();
    let database_path = temp.path().join("app-data/storage.duckdb");
    let database_fault = TestDatabaseFaultInjector::default();
    database_fault.fail_once(DatabaseFailurePoint::CanonicalInsert);
    let database =
        DatabaseService::start_with_fault_injector(&database_path, 16, Arc::new(database_fault))
            .await
            .unwrap();
    let runtime = Arc::new(FixtureRuntime {
        fail_next: Arc::new(AtomicBool::new(false)),
        invocations: Arc::new(AtomicUsize::new(0)),
        batch: fixture_batch(),
    });
    let dependencies = IngestionDependencies {
        workspace: workspace.clone(),
        source_module: fixture_module(&temp, source.clone()),
        archive: ArchiveCoordinator::new(workspace.clone(), source.clone()),
        database: database.clone(),
        runtime: runtime.clone(),
        limits: RuntimeLimits {
            max_memory_bytes: 8 * 1024 * 1024,
            fuel: 1_000_000,
            timeout: Duration::from_secs(1),
            max_output_bytes: 64 * 1024,
        },
        queue_capacity: 4,
    };
    let coordinator = IngestionCoordinator::start(dependencies).unwrap();

    let inbox = workspace.source_inbox(&source);
    std::fs::write(inbox.join("transaction.fixture"), b"transaction bytes").unwrap();
    scan_twice(&coordinator).await;
    assert!(!inbox.join("transaction.fixture").exists());
    let transaction_asset_id = ArchiveReconciler::new(workspace.clone(), source.clone())
        .scan()
        .unwrap()
        .assets
        .first()
        .unwrap()
        .asset_id;
    coordinator.retry_asset(transaction_asset_id).await.unwrap();

    let invocations_after_success = runtime.invocations.load(Ordering::SeqCst);
    std::fs::write(inbox.join("duplicate.fixture"), b"transaction bytes").unwrap();
    scan_twice(&coordinator).await;
    assert!(!inbox.join("duplicate.fixture").exists());
    assert_eq!(
        runtime.invocations.load(Ordering::SeqCst),
        invocations_after_success
    );

    runtime.fail_next.store(true, Ordering::SeqCst);
    std::fs::write(inbox.join("parse.fixture"), b"parse bytes").unwrap();
    scan_twice(&coordinator).await;
    assert!(!inbox.join("parse.fixture").exists());

    let recovery_gate = RecoveryGate::new();
    let recovery = RecoveryService::new(
        database.clone(),
        ArchiveReconciler::new(workspace.clone(), source.clone()),
        recovery_gate.clone(),
    );
    let report = recovery.startup().await.unwrap();
    assert_eq!(recovery_gate.mode(), mfa_ingestion::RecoveryMode::Normal);
    assert!(report.assets_to_ingest >= 1);

    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();

    let rebuild_database = DatabaseService::start(&database_path, 16).await.unwrap();
    let importer = RuntimeArchiveImporter::new(
        fixture_module(&temp, source.clone()),
        runtime,
        RuntimeLimits {
            max_memory_bytes: 8 * 1024 * 1024,
            fuel: 1_000_000,
            timeout: Duration::from_secs(1),
            max_output_bytes: 64 * 1024,
        },
    );
    let mut rebuild = ArchiveRebuildService::new(
        rebuild_database,
        ArchiveRebuildConfig {
            database_path: database_path.clone(),
            recovery_root: temp.path().join("recovery"),
            actor_capacity: 16,
        },
        ArchiveCoordinator::new(workspace.clone(), source.clone()).reconciler(),
        Arc::new(importer),
    );
    let plan = rebuild.preflight().await.unwrap();
    assert!(plan.assets.len() >= 2);
    let result = rebuild
        .rebuild(ArchiveRebuildConfirmation::confirmed())
        .await
        .unwrap();
    assert!(result.assets_rebuilt >= 1);
    assert!(result.recovery_copy.exists());
    let view = rebuild
        .database()
        .execute(QueryView::active_snapshot(
            LogicalSnapshotKey::new("fixture-source:default").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(view.counts.total, 1);
    rebuild.shutdown().await.unwrap();

    let _ = CoreEvent::DataChanged {
        capabilities: vec![CapabilityId::try_from("nutrition.items").unwrap()],
        dashboards: Vec::new(),
    };
}
