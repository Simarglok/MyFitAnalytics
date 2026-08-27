use mfa_contracts::{
    CanonicalObservation, CapabilityId, LineageHook, ModuleId, ModuleManifest, ModuleType,
    NutritionItem, SourceBatch, SourceExtensionContract, SourceManifest, SourceRecord,
    SourceValidation,
};
use mfa_ingestion::{BoxFuture, SourceInvoker};
use mfa_module_host::{InstalledModule, RuntimeError, RuntimeLimits};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
pub struct FakeRuntime {
    batch: SourceBatch,
    invocations: Arc<Mutex<usize>>,
    fail_next: Arc<Mutex<bool>>,
}

impl SourceInvoker for FakeRuntime {
    fn validate_source<'a>(
        &'a self,
        _module: &'a InstalledModule,
        _asset: Arc<dyn mfa_contracts::ReadOnlyAsset>,
        _limits: RuntimeLimits,
    ) -> BoxFuture<'a, Result<SourceValidation, RuntimeError>> {
        Box::pin(async move {
            Ok(SourceValidation {
                valid: true,
                issues: Vec::new(),
                source_module_id: self.batch.source_module_id.clone(),
                source_api_version: self.batch.source_api_version.clone(),
                logical_snapshot_key: self.batch.logical_snapshot_key.clone(),
                schema_fingerprint: self.batch.schema_fingerprint.clone(),
                mapping_version: self.batch.mapping_version.clone(),
            })
        })
    }

    fn invoke_source<'a>(
        &'a self,
        _module: &'a InstalledModule,
        _asset: Arc<dyn mfa_contracts::ReadOnlyAsset>,
        _limits: RuntimeLimits,
    ) -> BoxFuture<'a, Result<SourceBatch, RuntimeError>> {
        Box::pin(async move {
            let mut invocations = self.invocations.lock().unwrap();
            *invocations += 1;
            let mut fail_next = self.fail_next.lock().unwrap();
            if *fail_next {
                *fail_next = false;
                return Err(RuntimeError::new(
                    "module_guest_error",
                    "synthetic parse failure",
                ));
            }
            Ok(self.batch.clone())
        })
    }
}

pub fn fake_runtime(batch: SourceBatch) -> Arc<FakeRuntime> {
    Arc::new(FakeRuntime {
        batch,
        invocations: Arc::new(Mutex::new(0)),
        fail_next: Arc::new(Mutex::new(false)),
    })
}

impl FakeRuntime {
    #[allow(dead_code)]
    pub fn fail_next(&self) {
        *self.fail_next.lock().unwrap() = true;
    }
}

pub fn nutrition_batch() -> SourceBatch {
    SourceBatch {
        contract_version: "1.0.0".parse().unwrap(),
        source_module_id: "fixture-source".to_owned(),
        source_api_version: "1.0.0".parse().unwrap(),
        mapping_version: "1.0.0".parse().unwrap(),
        schema_fingerprint:
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        logical_snapshot_key: "fixture:2026".to_owned(),
        source_records: vec![SourceRecord {
            source_record_key: "source-1".to_owned(),
            sheet_name: None,
            source_row_number: 1,
            raw_payload: serde_json::json!({"fixture": true}),
        }],
        lineage: vec![LineageHook {
            canonical_entity_type: "nutrition_item".to_owned(),
            canonical_entity_id: "00000000-0000-0000-0000-0000000002bc".to_owned(),
            source_record_key: "source-1".to_owned(),
            mapping_version: "1.0.0".parse().unwrap(),
        }],
        records: vec![CanonicalObservation::NutritionItem(NutritionItem {
            nutrition_item_id: Uuid::from_u128(700),
            occurred_local_at: None,
            local_date: "2026-01-01".parse().unwrap(),
            meal: "Lunch".to_owned(),
            food_source_id: "fixture-food".to_owned(),
            name: "Synthetic meal".to_owned(),
            amount_raw: "1 serving".to_owned(),
            calories_kcal: Some(500.0),
            protein_g: Some(20.0),
            fat_g: Some(10.0),
            carbs_g: Some(60.0),
            fiber_g: None,
            sugars_g: None,
            sodium_mg: None,
            source_record_id: Some("source-1".to_owned()),
        })],
        extensions: Vec::new(),
        issues: Vec::new(),
    }
}

pub fn fake_module(temp: &tempfile::TempDir, id: ModuleId) -> InstalledModule {
    let root = temp.path().join("module");
    std::fs::create_dir_all(&root).unwrap();
    InstalledModule {
        module_id: id.clone(),
        module_type: ModuleType::Source,
        module_version: "1.0.0".parse().unwrap(),
        package_hash: "sha256:fixture-package".to_owned(),
        root,
        enabled: true,
        manifest: ModuleManifest::Source(SourceManifest {
            module_type: ModuleType::Source,
            module_id: id,
            module_version: "1.0.0".parse().unwrap(),
            package_format_version: "1.0.0".parse().unwrap(),
            source_api_version: "1.0.0".parse().unwrap(),
            mapping_version: "1.0.0".parse().unwrap(),
            compatible_app_versions: vec![">=0.1.0".to_owned()],
            provided_capabilities: vec![CapabilityId::try_from("nutrition.items").unwrap()],
            accepted_file_patterns: vec!["*.fixture".to_owned()],
            artifact_signatures: vec!["sha256:fixture-entrypoint".to_owned()],
            extension_contracts: vec![SourceExtensionContract {
                namespace: "fixture.extension".to_owned(),
                contract_version: "1.0.0".parse().unwrap(),
                payload_schema: serde_json::json!({"type": "object"}),
            }],
            settings_schema: serde_json::json!({}),
            entrypoint_hash: "sha256:fixture-entrypoint".to_owned(),
            localization_namespace: "source.fixture".to_owned(),
        }),
    }
}
