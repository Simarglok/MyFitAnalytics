use mfa_archive::{ArchiveCoordinator, ArchiveReconciler};
use mfa_config::{AppSettings, WorkspacePaths};
use mfa_contracts::{
    AssetMetadata, AssetReadError, CapabilityId, ModuleId, ReadOnlyAsset, UtcInstant,
};
use mfa_db::{DatabaseService, LogicalSnapshotKey, QuerySnapshot, QueryView};
use mfa_ingestion::{IngestionCoordinator, IngestionDependencies, ScanReason, ScanRequest};
use mfa_module_host::{CapabilityRegistry, ComponentRuntime, PackageInstaller, RuntimeLimits};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use uuid::Uuid;

struct MemoryAsset {
    metadata: AssetMetadata,
    bytes: Vec<u8>,
}

impl ReadOnlyAsset for MemoryAsset {
    fn metadata(&self) -> AssetMetadata {
        self.metadata.clone()
    }

    fn read_at(&self, offset: u64, max_bytes: u32) -> Result<Vec<u8>, AssetReadError> {
        let start = usize::try_from(offset)
            .map_err(|_| AssetReadError::InvalidRange { offset, max_bytes })?;
        let end = start
            .saturating_add(max_bytes as usize)
            .min(self.bytes.len());
        if start > self.bytes.len() {
            return Err(AssetReadError::InvalidRange { offset, max_bytes });
        }
        Ok(self.bytes[start..end].to_vec())
    }
}

fn package(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../dist/modules")
        .join(name)
}

fn limits() -> RuntimeLimits {
    RuntimeLimits {
        max_memory_bytes: 32 * 1024 * 1024,
        fuel: 8_000_000,
        timeout: Duration::from_secs(10),
        max_output_bytes: 4 * 1024 * 1024,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bundled_hevy_package_runs_through_wasmtime_archive_and_database() {
    let temp = TempDir::new().unwrap();
    let installer = PackageInstaller::new(temp.path().join("module-store"));
    let hevy = installer.install(&package("hevy.mfasource")).unwrap();
    assert_eq!(hevy.module_id.as_str(), "hevy");
    assert!(hevy.enabled);
    assert!(hevy.root.join("module.wasm").exists());

    let mut settings = AppSettings::default();
    settings.active_providers.insert(
        CapabilityId::try_from("body.weight").unwrap(),
        ModuleId::try_from("hevy").unwrap(),
    );
    settings.active_providers.insert(
        CapabilityId::try_from("strength.sessions").unwrap(),
        ModuleId::try_from("hevy").unwrap(),
    );
    let providers = CapabilityRegistry::new()
        .resolve(std::slice::from_ref(&hevy), &settings)
        .unwrap();
    assert_eq!(providers.active_providers.len(), 2);

    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../modules/sources/hevy/tests/fixtures/measurement_data.csv"),
    )
    .unwrap();
    let asset = Arc::new(MemoryAsset {
        metadata: AssetMetadata {
            asset_id: Uuid::from_u128(0x1234),
            file_name: "measurement_data.csv".to_owned(),
            media_type: "text/csv".to_owned(),
            byte_len: bytes.len() as u64,
        },
        bytes: bytes.clone(),
    });
    let runtime = ComponentRuntime::new();
    let validation = runtime
        .validate_source(
            &hevy,
            Arc::clone(&asset) as Arc<dyn ReadOnlyAsset>,
            limits(),
        )
        .await
        .unwrap();
    assert!(validation.valid);
    assert_eq!(validation.logical_snapshot_key, "hevy:measurements:2026");
    let batch = runtime.invoke_source(&hevy, asset, limits()).await.unwrap();
    assert_eq!(batch.source_records.len(), 3);
    assert_eq!(batch.records.len(), 2);
    assert_eq!(batch.lineage.len(), 2);

    let workspace = WorkspacePaths::new(temp.path().join("workspace"));
    let hevy_id = ModuleId::try_from("hevy").unwrap();
    workspace.enable_source(&hevy_id).unwrap();
    let inbox = workspace.source_inbox(&hevy_id);
    std::fs::write(inbox.join("measurement_data.csv"), bytes).unwrap();
    let database_path = temp.path().join("app-data/storage.duckdb");
    let database = DatabaseService::start(&database_path, 16).await.unwrap();
    let coordinator = IngestionCoordinator::start(IngestionDependencies {
        workspace: workspace.clone(),
        source_module: hevy.clone(),
        archive: ArchiveCoordinator::new(workspace.clone(), hevy_id.clone()),
        database: database.clone(),
        runtime: Arc::new(ComponentRuntime::new()),
        limits: limits(),
        queue_capacity: 4,
    })
    .unwrap();
    let mut events = coordinator.subscribe();
    for _ in 0..2 {
        coordinator
            .request_scan(ScanRequest::new(
                ScanReason::Manual,
                "2026-08-26T00:00:00Z".parse::<UtcInstant>().unwrap(),
            ))
            .await
            .unwrap();
    }
    timeout(Duration::from_secs(20), async {
        loop {
            if matches!(
                events.recv().await.unwrap(),
                mfa_ingestion::CoreEvent::DataChanged { .. }
            ) {
                break;
            }
        }
    })
    .await
    .unwrap();
    assert!(!inbox.join("measurement_data.csv").exists());
    let archive = ArchiveReconciler::new(workspace.clone(), hevy_id.clone())
        .scan()
        .unwrap();
    assert_eq!(archive.assets.len(), 1);
    let view = database
        .execute(QueryView::active_snapshot(
            LogicalSnapshotKey::new("hevy:measurements:2026").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(view.counts.total, 2);
    let details = database
        .execute(QuerySnapshot::active(
            LogicalSnapshotKey::new("hevy:measurements:2026").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(details.logical_snapshot_key, "hevy:measurements:2026");
    assert_eq!(details.canonical_records.len(), 2);
    assert_eq!(details.source_records.len(), 3);
    assert_eq!(details.lineage.len(), 2);
    assert_eq!(details.extensions.len(), 2);
    assert!(details.issues.is_empty());
    assert!(
        details
            .canonical_records
            .iter()
            .any(|record| record["value"]["weight_kg"] == 81.4)
    );
    assert!(
        details
            .source_records
            .iter()
            .any(|record| record.raw_payload["date"] == "2026-02-01 00:00:00")
    );
    assert!(details.lineage.iter().all(|lineage| {
        details
            .source_records
            .iter()
            .any(|record| record.source_record_id == lineage.source_record_id)
    }));
    while events.try_recv().is_ok() {}
    std::fs::write(
        inbox.join("measurement_data.csv"),
        b"date,weight_kg,fat_percent,waist_cm,neck_cm,hip_cm\n2026-02-01 00:00:00,81.4,18.2,86.0,38.0,98.0\n",
    )
    .unwrap();
    for _ in 0..2 {
        coordinator
            .request_scan(ScanRequest::new(
                ScanReason::Manual,
                "2026-08-26T00:01:00Z".parse::<UtcInstant>().unwrap(),
            ))
            .await
            .unwrap();
    }
    timeout(Duration::from_secs(20), async {
        loop {
            if matches!(
                events.recv().await.unwrap(),
                mfa_ingestion::CoreEvent::DataChanged { .. }
            ) {
                break;
            }
        }
    })
    .await
    .unwrap();
    let replacement = database
        .execute(QuerySnapshot::active(
            LogicalSnapshotKey::new("hevy:measurements:2026").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(replacement.counts.total, 1);
    assert_eq!(replacement.canonical_records.len(), 1);
    assert_eq!(replacement.source_records.len(), 1);
    assert_eq!(replacement.lineage.len(), 1);
    assert_eq!(replacement.extensions.len(), 1);
    assert!(replacement.issues.is_empty());
    assert!(
        replacement
            .historical_source_records
            .iter()
            .any(|record| record.raw_payload["weight_kg"] == "81.1")
    );
    assert!(
        !replacement
            .canonical_records
            .iter()
            .any(|record| record["value"]["weight_kg"] == 81.1)
    );
    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bundled_hevy_workout_fixture_runs_through_refresh_archive_and_database() {
    let temp = TempDir::new().unwrap();
    let installer = PackageInstaller::new(temp.path().join("module-store"));
    let hevy = installer.install(&package("hevy.mfasource")).unwrap();
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../modules/sources/hevy/tests/fixtures/workout_data.csv"),
    )
    .unwrap();
    let workspace = mfa_config::WorkspacePaths::new(temp.path().join("workspace"));
    workspace.enable_source(&hevy.module_id).unwrap();
    let inbox = workspace.source_inbox(&hevy.module_id);
    std::fs::write(inbox.join("workout_data.csv"), &bytes).unwrap();
    let database = DatabaseService::start(&temp.path().join("storage.duckdb"), 16)
        .await
        .unwrap();
    let coordinator = IngestionCoordinator::start(IngestionDependencies {
        workspace: workspace.clone(),
        source_module: hevy.clone(),
        archive: ArchiveCoordinator::new(workspace.clone(), hevy.module_id.clone()),
        database: database.clone(),
        runtime: Arc::new(ComponentRuntime::new()),
        limits: limits(),
        queue_capacity: 4,
    })
    .unwrap();
    let mut events = coordinator.subscribe();
    for _ in 0..2 {
        coordinator
            .request_scan(ScanRequest::new(
                ScanReason::Manual,
                "2026-08-26T00:02:00Z".parse::<UtcInstant>().unwrap(),
            ))
            .await
            .unwrap();
    }
    timeout(Duration::from_secs(20), async {
        loop {
            let event = events.recv().await.unwrap();
            if matches!(event, mfa_ingestion::CoreEvent::DataChanged { .. }) {
                break;
            }
            if matches!(event, mfa_ingestion::CoreEvent::QualityChanged) {
                break;
            }
        }
    })
    .await
    .unwrap();
    let details = database
        .execute(QuerySnapshot::active(
            LogicalSnapshotKey::new("hevy:workouts:2026").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(details.logical_snapshot_key, "hevy:workouts:2026");
    assert_eq!(details.counts.total, 5);
    assert_eq!(details.counts.workout_sessions, 1);
    assert_eq!(details.counts.exercise_sets, 4);
    assert_eq!(details.canonical_records.len(), 5);
    assert_eq!(details.source_records.len(), 4);
    assert_eq!(details.historical_source_records.len(), 4);
    assert_eq!(details.lineage.len(), 5);
    assert!(details.extensions.is_empty());
    assert!(details.issues.is_empty());
    assert!(details.canonical_records.iter().any(|record| {
        record["type"] == "workout_session"
            && record["value"]["title"] == "Synthetic Push"
            && record["value"]["duration_seconds"] == 2520
    }));
    assert!(details.canonical_records.iter().any(|record| {
        record["type"] == "exercise_set"
            && record["value"]["load_type"] == "duration"
            && record["value"]["duration_seconds"] == 45
    }));
    assert!(
        details
            .source_records
            .iter()
            .any(|record| record.raw_payload["notes"] == "fictional set")
    );
    assert!(
        details
            .lineage
            .iter()
            .all(|lineage| lineage.mapping_version == "1.0.0")
    );
    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bundled_mynetdiary_fixture_runs_through_refresh_archive_and_database() {
    let temp = TempDir::new().unwrap();
    let installer = PackageInstaller::new(temp.path().join("module-store"));
    let mynetdiary = installer.install(&package("mynetdiary.mfasource")).unwrap();
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../modules/sources/mynetdiary/tests/fixtures/valid-full.xls"),
    )
    .unwrap();
    let workspace = mfa_config::WorkspacePaths::new(temp.path().join("workspace"));
    workspace.enable_source(&mynetdiary.module_id).unwrap();
    let inbox = workspace.source_inbox(&mynetdiary.module_id);
    std::fs::write(inbox.join("valid-full.xls"), &bytes).unwrap();
    let database = DatabaseService::start(&temp.path().join("storage.duckdb"), 16)
        .await
        .unwrap();
    let coordinator = IngestionCoordinator::start(IngestionDependencies {
        workspace: workspace.clone(),
        source_module: mynetdiary.clone(),
        archive: ArchiveCoordinator::new(workspace.clone(), mynetdiary.module_id.clone()),
        database: database.clone(),
        runtime: Arc::new(ComponentRuntime::new()),
        limits: limits(),
        queue_capacity: 4,
    })
    .unwrap();
    let mut events = coordinator.subscribe();
    coordinator
        .request_scan(ScanRequest::new(
            ScanReason::Startup,
            "2026-08-26T00:03:00Z".parse::<UtcInstant>().unwrap(),
        ))
        .await
        .unwrap();
    timeout(Duration::from_secs(20), async {
        loop {
            if matches!(
                events.recv().await.unwrap(),
                mfa_ingestion::CoreEvent::DataChanged { .. }
            ) {
                break;
            }
        }
    })
    .await
    .unwrap();
    let details = database
        .execute(QuerySnapshot::active(
            LogicalSnapshotKey::new("mynetdiary:2026").unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(details.logical_snapshot_key, "mynetdiary:2026");
    assert_eq!(details.counts.total, 5);
    assert_eq!(details.canonical_records.len(), 5);
    assert_eq!(details.source_records.len(), 8);
    assert_eq!(details.historical_source_records.len(), 8);
    assert_eq!(details.lineage.len(), 6);
    assert_eq!(details.extensions.len(), 1);
    assert!(details.issues.is_empty());
    assert!(
        details
            .canonical_records
            .iter()
            .any(|record| { record["type"] == "activity_day" && record["value"]["steps"] == 6400 })
    );
    assert!(details.canonical_records.iter().any(|record| {
        record["type"] == "activity_day" && record["value"]["water_ml"] == 500.0
    }));
    assert!(details.canonical_records.iter().any(|record| {
        record["type"] == "nutrition_item"
            && record["value"]["local_date"] == "2026-01-04"
            && record["value"]["amount_raw"] == "1 serving"
    }));
    assert!(
        details
            .source_records
            .iter()
            .all(|record| record.raw_payload.is_object())
    );
    assert!(
        details
            .lineage
            .iter()
            .all(|lineage| lineage.mapping_version == "1.0.0")
    );
    coordinator.shutdown().await.unwrap();
    database.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bundled_mynetdiary_package_runs_through_wasmtime_biff_contract() {
    let temp = TempDir::new().unwrap();
    let installer = PackageInstaller::new(temp.path().join("module-store"));
    let module = installer.install(&package("mynetdiary.mfasource")).unwrap();
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../modules/sources/mynetdiary/tests/fixtures/valid-full.xls"),
    )
    .unwrap();
    let asset = Arc::new(MemoryAsset {
        metadata: AssetMetadata {
            asset_id: Uuid::from_u128(0x5678),
            file_name: "renamed-export.xls".to_owned(),
            media_type: "application/vnd.ms-excel".to_owned(),
            byte_len: bytes.len() as u64,
        },
        bytes,
    });
    let runtime = ComponentRuntime::new();
    let validation = runtime
        .validate_source(
            &module,
            Arc::clone(&asset) as Arc<dyn ReadOnlyAsset>,
            limits(),
        )
        .await
        .unwrap();
    assert!(validation.valid);
    assert_eq!(validation.logical_snapshot_key, "mynetdiary:2026");
    let batch = runtime
        .invoke_source(&module, asset, limits())
        .await
        .unwrap();
    assert_eq!(batch.source_module_id, "mynetdiary");
    assert_eq!(batch.records.len(), 6);
    assert_eq!(batch.extensions.len(), 1);
    assert_eq!(batch.source_records.len(), 8);
}
