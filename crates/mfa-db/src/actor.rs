use crate::command::{
    DatabaseCommand, FailAttempt, FailAttemptResult, HealthCheckResult, MarkInterruptedResult,
    ReconcileArchiveInventory, ReconcileArchiveInventoryResult, RegisterAsset, RegisterAssetResult,
    RegisterReceipt, RegisterReceiptResult, StartAttempt, StartAttemptResult, ViewResponse,
};
use crate::error::DatabaseError;
use crate::fault::{DatabaseFailurePoint, DatabaseFaultInjector};
use crate::migrations;
use crate::provenance::{
    ExtensionContractRegistration, ExtensionContractRegistrationResult, RecordCounts,
    SnapshotCommitResult, ValidatedSnapshotBatch, canonical_entity_key, canonical_identity,
};
use crate::validation::{self, ValidationError};
use crate::views::{QueryView, ViewRequest};
use chrono::Utc;
use duckdb::{Connection, params};
use mfa_contracts::{CanonicalObservation, CapabilityId};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;

pub(crate) fn run_actor(
    path: PathBuf,
    mut receiver: mpsc::Receiver<DatabaseCommand>,
    ready: std::sync::mpsc::SyncSender<Result<(), DatabaseError>>,
    fault_injector: std::sync::Arc<dyn DatabaseFaultInjector>,
) {
    let connection = match Connection::open(&path) {
        Ok(connection) => connection,
        Err(error) => {
            let _ = ready.send(Err(DatabaseError::Open {
                detail: error.to_string(),
            }));
            return;
        }
    };
    if let Err(error) = migrations::apply_all(&connection) {
        let _ = ready.send(Err(error));
        return;
    }
    if ready.send(Ok(())).is_err() {
        return;
    }

    while let Some(command) = receiver.blocking_recv() {
        if process_command(&connection, command, fault_injector.as_ref()) {
            break;
        }
    }
}

fn process_command(
    connection: &Connection,
    command: DatabaseCommand,
    fault_injector: &dyn DatabaseFaultInjector,
) -> bool {
    match command {
        DatabaseCommand::HealthCheck(response) => {
            let _ = response.send(health_check(connection));
        }
        DatabaseCommand::RegisterAsset(command, response) => {
            let _ = response.send(register_asset(connection, command));
        }
        DatabaseCommand::RegisterReceipt(command, response) => {
            let _ = response.send(register_receipt(connection, command));
        }
        DatabaseCommand::StartAttempt(command, response) => {
            let _ = response.send(start_attempt(connection, command));
        }
        DatabaseCommand::FailAttempt(command, response) => {
            let _ = response.send(fail_attempt(connection, command));
        }
        DatabaseCommand::MarkInterrupted(response) => {
            let _ = response.send(mark_interrupted(connection));
        }
        DatabaseCommand::ReconcileArchive(_, response) => {
            let _ = response.send(Err(DatabaseError::Command {
                detail: "archive reconciliation is not initialized".to_owned(),
            }));
        }
        DatabaseCommand::ReconcileArchiveInventory(command, response) => {
            let _ = response.send(reconcile_archive_inventory(connection, command));
        }
        DatabaseCommand::QueryView(command, response) => {
            let _ = response.send(query_view(connection, command));
        }
        DatabaseCommand::CommitSnapshot(command, response) => {
            let _ = response.send(commit_snapshot(connection, command.0, fault_injector));
        }
        DatabaseCommand::RegisterExtensionContract(command, response) => {
            let _ = response.send(register_extension_contract(connection, command));
        }
        DatabaseCommand::Shutdown(response) => {
            let _ = response.send(Ok(()));
            return true;
        }
    }
    false
}

fn health_check(connection: &Connection) -> Result<HealthCheckResult, DatabaseError> {
    let schema_version = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migration",
            [],
            |row| row.get::<_, u32>(0),
        )
        .map_err(DatabaseError::from_duckdb)?;
    Ok(HealthCheckResult {
        actor_thread_id: format!("{:?}", std::thread::current().id()),
        schema_version,
    })
}

fn register_asset(
    connection: &Connection,
    command: RegisterAsset,
) -> Result<RegisterAssetResult, DatabaseError> {
    let asset = command.asset;
    let inserted = connection
        .execute(
            "INSERT INTO source_asset(
                asset_id, source_module_id, asset_type, original_filename, archive_path,
                byte_sha256, file_size, received_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (byte_sha256) DO NOTHING",
            params![
                asset.asset_id.to_string(),
                asset.source_module_id.to_string(),
                asset.asset_type,
                asset.original_filename,
                asset.archive_path,
                asset.byte_sha256,
                asset.file_size,
                asset.received_at.as_datetime(),
            ],
        )
        .map_err(DatabaseError::from_duckdb)?;
    let asset_id = connection
        .query_row(
            "SELECT asset_id FROM source_asset WHERE byte_sha256 = ?",
            params![asset.byte_sha256],
            |row| row.get::<_, String>(0),
        )
        .map_err(DatabaseError::from_duckdb)?;
    let asset_id = asset_id.parse().map_err(|error| DatabaseError::Command {
        detail: format!("stored asset identity is invalid: {error}"),
    })?;
    Ok(RegisterAssetResult {
        asset_id,
        inserted: inserted == 1,
    })
}

fn register_receipt(
    connection: &Connection,
    command: RegisterReceipt,
) -> Result<RegisterReceiptResult, DatabaseError> {
    connection
        .execute(
            "INSERT INTO source_receipt(
                receipt_id, source_module_id, inbox_path, original_filename,
                discovered_at, asset_id, outcome
             ) VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (receipt_id) DO NOTHING",
            params![
                command.receipt_id.to_string(),
                command.source_module_id.to_string(),
                command.inbox_path,
                command.original_filename,
                command.discovered_at.as_datetime(),
                command.asset_id.map(|id| id.to_string()),
                command.outcome,
            ],
        )
        .map_err(DatabaseError::from_duckdb)?;
    Ok(RegisterReceiptResult {
        receipt_id: command.receipt_id,
        asset_id: command.asset_id,
        outcome: command.outcome,
    })
}

fn reconcile_archive_inventory(
    connection: &Connection,
    command: ReconcileArchiveInventory,
) -> Result<ReconcileArchiveInventoryResult, DatabaseError> {
    let source_module_id = command.source_module_id.to_string();
    let mut registered_assets = 0;
    let mut assets_to_ingest = Vec::new();
    let mut archive_paths = BTreeSet::new();

    for asset in command.assets {
        if asset.source_module_id != command.source_module_id {
            return Err(DatabaseError::Command {
                detail: "archive inventory source module does not match command".to_owned(),
            });
        }
        archive_paths.insert(asset.archive_path.clone());
        let inserted = connection
            .execute(
                "INSERT INTO source_asset(
                    asset_id, source_module_id, asset_type, original_filename, archive_path,
                    byte_sha256, file_size, received_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT (byte_sha256) DO NOTHING",
                params![
                    asset.asset_id.to_string(),
                    asset.source_module_id.to_string(),
                    asset.asset_type,
                    asset.original_filename,
                    asset.archive_path,
                    asset.byte_sha256,
                    asset.file_size,
                    asset.received_at.as_datetime(),
                ],
            )
            .map_err(DatabaseError::from_duckdb)?;
        registered_assets += u64::from(inserted == 1);

        let stored_asset_id: String = connection
            .query_row(
                "SELECT asset_id FROM source_asset WHERE byte_sha256 = ?",
                params![asset.byte_sha256],
                |row| row.get(0),
            )
            .map_err(DatabaseError::from_duckdb)?;
        let stored_asset_id: Uuid =
            stored_asset_id
                .parse()
                .map_err(|error| DatabaseError::Command {
                    detail: format!("stored asset identity is invalid: {error}"),
                })?;
        let has_attempt = connection
            .query_row(
                "SELECT COUNT(*) FROM ingestion_attempt WHERE asset_id = ?",
                params![stored_asset_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .map_err(DatabaseError::from_duckdb)?;
        if has_attempt == 0 {
            let mut pending = asset;
            pending.asset_id = stored_asset_id;
            assets_to_ingest.push(pending);
        }
    }

    let mut missing_asset_ids = Vec::new();
    let mut statement = connection
        .prepare(
            "SELECT asset_id, archive_path FROM source_asset
             WHERE source_module_id = ? ORDER BY asset_id",
        )
        .map_err(DatabaseError::from_duckdb)?;
    let rows = statement
        .query_map(params![source_module_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(DatabaseError::from_duckdb)?;
    for row in rows {
        let (asset_id, archive_path) = row.map_err(DatabaseError::from_duckdb)?;
        if archive_paths.contains(&archive_path) {
            continue;
        }
        let asset_uuid = asset_id.parse().map_err(|error| DatabaseError::Command {
            detail: format!("stored asset identity is invalid: {error}"),
        })?;
        connection
            .execute(
                "INSERT INTO data_quality_item(
                    data_quality_item_id, item_type, source_asset_id, source_record_id,
                    severity, message, status, created_at, resolved_at
                 ) VALUES (?, ?, ?, NULL, 'critical', ?, 'open', CURRENT_TIMESTAMP, NULL)
                 ON CONFLICT (data_quality_item_id) DO NOTHING",
                params![
                    format!("archive_missing:{asset_id}"),
                    "missing_archive_asset",
                    asset_id,
                    format!("immutable archive asset is missing: {archive_path}"),
                ],
            )
            .map_err(DatabaseError::from_duckdb)?;
        missing_asset_ids.push(asset_uuid);
    }

    Ok(ReconcileArchiveInventoryResult {
        registered_assets,
        missing_assets: missing_asset_ids.len() as u64,
        missing_asset_ids,
        assets_to_ingest,
    })
}

fn start_attempt(
    connection: &Connection,
    command: StartAttempt,
) -> Result<StartAttemptResult, DatabaseError> {
    connection
        .execute(
            "INSERT INTO ingestion_attempt(
                attempt_id, asset_id, source_module_id, source_module_version,
                source_module_package_sha256, source_api_version, mapping_version,
                schema_fingerprint, logical_snapshot_key, started_at, status
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'running')",
            params![
                command.attempt_id.to_string(),
                command.asset_id.to_string(),
                command.source_module_id.to_string(),
                command.source_module_version,
                command.source_module_package_sha256,
                command.source_api_version,
                command.mapping_version,
                command.schema_fingerprint,
                command.logical_snapshot_key,
                command.started_at.as_datetime(),
            ],
        )
        .map_err(DatabaseError::from_duckdb)?;
    Ok(StartAttemptResult {
        attempt_id: command.attempt_id,
    })
}

fn fail_attempt(
    connection: &Connection,
    command: FailAttempt,
) -> Result<FailAttemptResult, DatabaseError> {
    connection
        .execute(
            "UPDATE ingestion_attempt
             SET finished_at = ?, status = ?, error_code = ?, error_message = ?, record_count = ?
             WHERE attempt_id = ?",
            params![
                command.finished_at.as_datetime(),
                command.status,
                command.error_code,
                command.error_message,
                command.record_count,
                command.attempt_id.to_string(),
            ],
        )
        .map_err(DatabaseError::from_duckdb)?;
    Ok(FailAttemptResult {
        attempt_id: command.attempt_id,
    })
}

fn mark_interrupted(connection: &Connection) -> Result<MarkInterruptedResult, DatabaseError> {
    let count = connection
        .execute(
            "UPDATE ingestion_attempt
             SET status = 'interrupted', finished_at = CURRENT_TIMESTAMP
             WHERE status = 'running'",
            [],
        )
        .map_err(DatabaseError::from_duckdb)?;
    Ok(MarkInterruptedResult {
        count: count as u64,
    })
}

fn register_extension_contract(
    connection: &Connection,
    command: ExtensionContractRegistration,
) -> Result<ExtensionContractRegistrationResult, DatabaseError> {
    connection
        .execute(
            "INSERT INTO extension_contract(
                contract_id, source_module_id, namespace, contract_version, payload_schema
             ) VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (contract_id) DO NOTHING",
            params![
                command.contract_id,
                command.source_module_id.to_string(),
                command.namespace,
                command.contract_version,
                json_string(&command.payload_schema)?,
            ],
        )
        .map_err(DatabaseError::from_duckdb)?;
    Ok(ExtensionContractRegistrationResult {
        contract_id: command.contract_id,
    })
}

fn database_fault(fault: crate::fault::DatabaseFault) -> DatabaseError {
    DatabaseError::FaultInjected {
        point: format!("{:?}", fault.point),
    }
}

fn commit_snapshot(
    connection: &Connection,
    batch: std::sync::Arc<ValidatedSnapshotBatch>,
    fault_injector: &dyn DatabaseFaultInjector,
) -> Result<SnapshotCommitResult, DatabaseError> {
    validation::validate_batch(&batch).map_err(DatabaseError::from)?;
    ensure_attempt_is_ready(connection, &batch)?;
    ensure_extension_contracts(connection, &batch)?;

    let snapshot_id = Uuid::new_v4();
    let snapshot_id_string = snapshot_id.to_string();
    let counts = validation::record_counts(&batch);
    let changed_capabilities = changed_capabilities(&batch.observations)?;
    fault_injector
        .check(DatabaseFailurePoint::TransactionStart)
        .map_err(database_fault)?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(DatabaseError::from_duckdb)?;

    transaction
        .execute(
            "INSERT INTO logical_snapshot(
                snapshot_id, logical_snapshot_key, attempt_id, created_at, status
             ) VALUES (?, ?, ?, ?, 'committed')",
            params![
                &snapshot_id_string,
                batch.logical_key.to_string(),
                batch.attempt.attempt_id.to_string(),
                Utc::now(),
            ],
        )
        .map_err(DatabaseError::from_duckdb)?;

    for source_record in &batch.source_records {
        transaction
            .execute(
                "INSERT INTO source_record(
                    source_record_id, attempt_id, asset_id, sheet_name,
                    source_row_number, source_record_key, raw_payload
                 ) VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![
                    &source_record.source_record_id,
                    batch.attempt.attempt_id.to_string(),
                    batch.attempt.asset_id.to_string(),
                    &source_record.sheet_name,
                    source_record.source_row_number,
                    &source_record.source_record_key,
                    json_string(&source_record.raw_payload)?,
                ],
            )
            .map_err(DatabaseError::from_duckdb)?;
    }

    for extension in &batch.extensions {
        transaction
            .execute(
                "INSERT INTO extension_record(
                    extension_record_id, source_record_id, source_module_id,
                    contract_id, contract_version, occurred_local_at, local_date, payload
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    &extension.extension_record_id,
                    &extension.source_record_id,
                    extension.source_module_id.to_string(),
                    &extension.contract_id,
                    &extension.contract_version,
                    extension.occurred_local_at.map(|value| value.as_naive()),
                    extension.local_date.map(|value| value.as_naive()),
                    json_string(&extension.payload)?,
                ],
            )
            .map_err(DatabaseError::from_duckdb)?;
    }

    for issue in &batch.issues {
        transaction
            .execute(
                "INSERT INTO data_quality_item(
                    data_quality_item_id, item_type, source_asset_id, source_record_id,
                    severity, message, status, created_at, resolved_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    &issue.data_quality_item_id,
                    &issue.item_type,
                    issue.source_asset_id.map(|value| value.to_string()),
                    &issue.source_record_id,
                    &issue.severity,
                    &issue.message,
                    &issue.status,
                    issue.created_at.as_datetime(),
                    issue.resolved_at.as_ref().map(|value| value.as_datetime()),
                ],
            )
            .map_err(DatabaseError::from_duckdb)?;
    }

    for lineage in &batch.lineage {
        transaction
            .execute(
                "INSERT INTO lineage(
                    snapshot_id, canonical_entity_type, canonical_entity_id,
                    source_record_id, mapping_version
                 ) VALUES (?, ?, ?, ?, ?)",
                params![
                    &snapshot_id_string,
                    &lineage.canonical_entity_type,
                    &lineage.canonical_entity_id,
                    &lineage.source_record_id,
                    &lineage.mapping_version,
                ],
            )
            .map_err(DatabaseError::from_duckdb)?;
    }

    for observation in &batch.observations {
        if !matches!(observation, CanonicalObservation::WorkoutSession(_)) {
            insert_canonical(
                &transaction,
                &snapshot_id_string,
                batch.logical_key.as_str(),
                &batch,
                observation,
                fault_injector,
            )?;
        }
    }
    for observation in &batch.observations {
        if matches!(observation, CanonicalObservation::WorkoutSession(_)) {
            insert_canonical(
                &transaction,
                &snapshot_id_string,
                batch.logical_key.as_str(),
                &batch,
                observation,
                fault_injector,
            )?;
        }
    }

    fault_injector
        .check(DatabaseFailurePoint::ActiveSwitch)
        .map_err(database_fault)?;

    transaction
        .execute(
            "UPDATE logical_snapshot
             SET status = 'superseded'
             WHERE logical_snapshot_key = ? AND snapshot_id <> ?",
            params![batch.logical_key.as_str(), &snapshot_id_string],
        )
        .map_err(DatabaseError::from_duckdb)?;
    transaction
        .execute(
            "UPDATE ingestion_attempt
             SET status = 'superseded'
             WHERE logical_snapshot_key = ?
               AND status = 'succeeded'
               AND attempt_id <> ?",
            params![
                batch.logical_key.as_str(),
                batch.attempt.attempt_id.to_string()
            ],
        )
        .map_err(DatabaseError::from_duckdb)?;
    transaction
        .execute(
            "UPDATE ingestion_attempt
             SET status = 'succeeded', finished_at = CURRENT_TIMESTAMP, record_count = ?
             WHERE attempt_id = ?",
            params![counts.total, batch.attempt.attempt_id.to_string()],
        )
        .map_err(DatabaseError::from_duckdb)?;
    transaction
        .execute(
            "INSERT INTO active_snapshot(
                logical_snapshot_key, snapshot_id, attempt_id, committed_at,
                changed_capabilities, record_count
             ) VALUES (?, ?, ?, CURRENT_TIMESTAMP, ?, ?)
             ON CONFLICT (logical_snapshot_key) DO UPDATE SET
                snapshot_id = excluded.snapshot_id,
                attempt_id = excluded.attempt_id,
                committed_at = excluded.committed_at,
                changed_capabilities = excluded.changed_capabilities,
                record_count = excluded.record_count",
            params![
                batch.logical_key.as_str(),
                &snapshot_id_string,
                batch.attempt.attempt_id.to_string(),
                serde_json::to_string(&changed_capabilities).map_err(|error| {
                    DatabaseError::Command {
                        detail: error.to_string(),
                    }
                })?,
                counts.total,
            ],
        )
        .map_err(DatabaseError::from_duckdb)?;
    transaction.commit().map_err(DatabaseError::from_duckdb)?;

    Ok(SnapshotCommitResult {
        snapshot_id,
        changed_capabilities,
        counts,
    })
}

fn ensure_attempt_is_ready(
    connection: &Connection,
    batch: &ValidatedSnapshotBatch,
) -> Result<(), DatabaseError> {
    let count = connection
        .query_row(
            "SELECT COUNT(*) FROM ingestion_attempt WHERE attempt_id = ?",
            params![batch.attempt.attempt_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .map_err(DatabaseError::from_duckdb)?;
    if count != 1 {
        return Err(DatabaseError::Command {
            detail: "snapshot attempt is not registered".to_owned(),
        });
    }
    let (status, logical_key, asset_id): (String, String, String) = connection
        .query_row(
            "SELECT status, logical_snapshot_key, asset_id
             FROM ingestion_attempt WHERE attempt_id = ?",
            params![batch.attempt.attempt_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(DatabaseError::from_duckdb)?;
    if status != "running" {
        return Err(DatabaseError::Command {
            detail: format!("snapshot attempt is not running: {status}"),
        });
    }
    if logical_key != batch.logical_key.as_str() || asset_id != batch.attempt.asset_id.to_string() {
        return Err(DatabaseError::Command {
            detail: "snapshot attempt provenance does not match batch".to_owned(),
        });
    }
    Ok(())
}

fn ensure_extension_contracts(
    connection: &Connection,
    batch: &ValidatedSnapshotBatch,
) -> Result<(), DatabaseError> {
    for extension in &batch.extensions {
        let count = connection
            .query_row(
                "SELECT COUNT(*) FROM extension_contract
                 WHERE contract_id = ? AND source_module_id = ? AND contract_version = ?",
                params![
                    &extension.contract_id,
                    extension.source_module_id.to_string(),
                    &extension.contract_version,
                ],
                |row| row.get::<_, i64>(0),
            )
            .map_err(DatabaseError::from_duckdb)?;
        if count != 1 {
            return Err(DatabaseError::ExtensionContractMissing {
                contract_id: extension.contract_id.clone(),
                contract_version: extension.contract_version.clone(),
            });
        }
    }
    Ok(())
}

fn insert_canonical(
    transaction: &duckdb::Transaction<'_>,
    snapshot_id: &str,
    logical_key: &str,
    batch: &ValidatedSnapshotBatch,
    observation: &CanonicalObservation,
    fault_injector: &dyn DatabaseFaultInjector,
) -> Result<(), DatabaseError> {
    fault_injector
        .check(DatabaseFailurePoint::CanonicalInsert)
        .map_err(database_fault)?;
    let source_record_id = source_record_id(batch, observation)?;
    match observation {
        CanonicalObservation::NutritionItem(value) => transaction.execute(
            "INSERT INTO nutrition_item(
                    nutrition_item_id, snapshot_id, logical_snapshot_key, occurred_local_at,
                    local_date, meal, food_source_id, name, amount_raw, calories_kcal,
                    protein_g, fat_g, carbs_g, fiber_g, sugars_g, sodium_mg, source_record_id
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                value.nutrition_item_id.to_string(),
                snapshot_id,
                logical_key,
                value.occurred_local_at.map(|value| value.as_naive()),
                value.local_date.as_naive(),
                &value.meal,
                &value.food_source_id,
                &value.name,
                &value.amount_raw,
                value.calories_kcal,
                value.protein_g,
                value.fat_g,
                value.carbs_g,
                value.fiber_g,
                value.sugars_g,
                value.sodium_mg,
                source_record_id,
            ],
        ),
        CanonicalObservation::BodyMeasurement(value) => transaction.execute(
            "INSERT INTO body_measurement(
                body_measurement_id, snapshot_id, logical_snapshot_key, local_date,
                weight_kg, body_fat_pct, source_record_id
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                value.body_measurement_id.to_string(),
                snapshot_id,
                logical_key,
                value.local_date.as_naive(),
                value.weight_kg,
                value.body_fat_pct,
                source_record_id,
            ],
        ),
        CanonicalObservation::ActivityEvent(value) => transaction.execute(
            "INSERT INTO activity_event(
                activity_event_id, snapshot_id, logical_snapshot_key, occurred_local_at,
                local_date, activity_type, source_name, duration_seconds, distance_km,
                estimated_calories_kcal, origin_hint, quality_status, source_record_id
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                value.activity_event_id.to_string(),
                snapshot_id,
                logical_key,
                value.occurred_local_at.as_naive(),
                value.local_date.as_naive(),
                &value.activity_type,
                &value.source_name,
                value.duration_seconds,
                value.distance_km,
                value.estimated_calories_kcal,
                &value.origin_hint,
                &value.quality_status,
                source_record_id,
            ],
        ),
        CanonicalObservation::ActivityDay(value) => transaction.execute(
            "INSERT INTO activity_day(
                activity_day_id, snapshot_id, logical_snapshot_key, local_date, steps,
                water_ml, heart_rate_observation_count, activity_duration_seconds,
                activity_distance_km, estimated_activity_calories_kcal, source_record_id
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                value.local_date.to_string(),
                snapshot_id,
                logical_key,
                value.local_date.as_naive(),
                value.steps.map(|value| value as i64),
                value.water_ml,
                value.heart_rate_observation_count,
                value.activity_duration_seconds as i64,
                value.activity_distance_km,
                value.estimated_activity_calories_kcal,
                source_record_id,
            ],
        ),
        CanonicalObservation::HeartRate(value) => transaction.execute(
            "INSERT INTO heart_rate_observation(
                heart_rate_observation_id, snapshot_id, logical_snapshot_key,
                observed_local_at, heart_rate_bpm, source_record_id
             ) VALUES (?, ?, ?, ?, ?, ?)",
            params![
                value.heart_rate_observation_id.to_string(),
                snapshot_id,
                logical_key,
                value.observed_local_at.as_naive(),
                value.heart_rate_bpm,
                source_record_id,
            ],
        ),
        CanonicalObservation::WorkoutSession(value) => transaction.execute(
            "INSERT INTO workout_session(
                workout_session_id, snapshot_id, logical_snapshot_key, title,
                started_local_at, ended_local_at, duration_seconds, source_record_group_key,
                source_record_id
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                value.workout_session_id.to_string(),
                snapshot_id,
                logical_key,
                &value.title,
                value.started_local_at.as_naive(),
                value.ended_local_at.as_naive(),
                value.duration_seconds,
                &value.source_record_group_key,
                source_record_id,
            ],
        ),
        CanonicalObservation::ExerciseSet(value) => transaction.execute(
            "INSERT INTO exercise_set(
                exercise_set_id, snapshot_id, logical_snapshot_key, workout_session_id,
                exercise_title_raw, exercise_key, exercise_block_ordinal, set_index,
                set_type, load_type, weight_kg, reps, duration_seconds, rpe, source_record_id
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                value.exercise_set_id.to_string(),
                snapshot_id,
                logical_key,
                value.workout_session_id.to_string(),
                &value.exercise_title_raw,
                &value.exercise_key,
                value.exercise_block_ordinal,
                value.set_index,
                &value.set_type,
                &value.load_type,
                value.weight_kg,
                value.reps,
                value.duration_seconds,
                value.rpe,
                source_record_id,
            ],
        ),
        CanonicalObservation::PhaseEvent(value) => transaction.execute(
            "INSERT INTO phase_event(
                phase_event_id, snapshot_id, logical_snapshot_key, event_type,
                start_date, end_date, description, exclude_from_tdee, source_record_id
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                value.phase_event_id.to_string(),
                snapshot_id,
                logical_key,
                &value.event_type,
                value.start_date.as_naive(),
                value.end_date.as_naive(),
                &value.description,
                value.exclude_from_tdee,
                source_record_id,
            ],
        ),
    }
    .map_err(DatabaseError::from_duckdb)?;
    Ok(())
}

fn source_record_id(
    batch: &ValidatedSnapshotBatch,
    observation: &CanonicalObservation,
) -> Result<String, DatabaseError> {
    if let Ok((_, _, source_record_id)) = canonical_identity(observation) {
        return Ok(source_record_id);
    }
    let (entity_type, entity_id) = canonical_entity_key(observation);
    batch
        .lineage
        .iter()
        .find(|lineage| {
            lineage.canonical_entity_type == entity_type && lineage.canonical_entity_id == entity_id
        })
        .map(|lineage| lineage.source_record_id.clone())
        .ok_or_else(|| DatabaseError::Validation {
            code: "lineage_missing",
            detail: ValidationError::MissingLineage {
                entity_type,
                entity_id,
            }
            .to_string(),
        })
}

fn changed_capabilities(
    observations: &[CanonicalObservation],
) -> Result<Vec<CapabilityId>, DatabaseError> {
    let mut capabilities = BTreeSet::new();
    for observation in observations {
        let names: &[&str] = match observation {
            CanonicalObservation::NutritionItem(_) => &["nutrition.items"],
            CanonicalObservation::BodyMeasurement(_) => &["body.weight", "body.composition"],
            CanonicalObservation::ActivityEvent(_) => &["activity.events"],
            CanonicalObservation::ActivityDay(_) => &["activity.days"],
            CanonicalObservation::HeartRate(_) => &["heart_rate.observations"],
            CanonicalObservation::WorkoutSession(_) => &["workouts.sessions"],
            CanonicalObservation::ExerciseSet(_) => &["workouts.sets"],
            CanonicalObservation::PhaseEvent(_) => &["cycle.phase_events"],
        };
        for name in names {
            capabilities.insert(CapabilityId::try_from(*name).map_err(|error| {
                DatabaseError::Command {
                    detail: error.to_string(),
                }
            })?);
        }
    }
    Ok(capabilities.into_iter().collect())
}

fn query_view(connection: &Connection, command: QueryView) -> Result<ViewResponse, DatabaseError> {
    let ViewRequest::ActiveSnapshot {
        logical_snapshot_key,
    } = command.request;
    let snapshot_id = connection
        .query_row(
            "SELECT COUNT(*) FROM active_snapshot WHERE logical_snapshot_key = ?",
            params![&logical_snapshot_key],
            |row| row.get::<_, i64>(0),
        )
        .map_err(DatabaseError::from_duckdb)?;
    if snapshot_id == 0 {
        return Ok(ViewResponse {
            logical_snapshot_key,
            snapshot_id: None,
            counts: RecordCounts::zero(),
        });
    }
    let snapshot_id: String = connection
        .query_row(
            "SELECT snapshot_id FROM active_snapshot WHERE logical_snapshot_key = ?",
            params![&logical_snapshot_key],
            |row| row.get(0),
        )
        .map_err(DatabaseError::from_duckdb)?;
    let snapshot_uuid = snapshot_id
        .parse()
        .map_err(|error| DatabaseError::Command {
            detail: format!("stored snapshot identity is invalid: {error}"),
        })?;
    let mut counts = RecordCounts::zero();
    counts.nutrition_items =
        active_count(connection, "active_nutrition_items", &logical_snapshot_key)?;
    counts.body_measurements = active_count(
        connection,
        "active_body_measurements",
        &logical_snapshot_key,
    )?;
    counts.activity_events =
        active_count(connection, "active_activity_events", &logical_snapshot_key)?;
    counts.activity_days = active_count(connection, "active_activity_days", &logical_snapshot_key)?;
    counts.heart_rate_observations = active_count(
        connection,
        "active_heart_rate_observations",
        &logical_snapshot_key,
    )?;
    counts.workout_sessions =
        active_count(connection, "active_workout_sessions", &logical_snapshot_key)?;
    counts.exercise_sets = active_count(connection, "active_exercise_sets", &logical_snapshot_key)?;
    counts.phase_events = active_count(connection, "active_phase_events", &logical_snapshot_key)?;
    counts.total = counts.nutrition_items
        + counts.body_measurements
        + counts.activity_events
        + counts.activity_days
        + counts.heart_rate_observations
        + counts.workout_sessions
        + counts.exercise_sets
        + counts.phase_events;
    Ok(ViewResponse {
        logical_snapshot_key,
        snapshot_id: Some(snapshot_uuid),
        counts,
    })
}

fn active_count(
    connection: &Connection,
    view: &str,
    logical_snapshot_key: &str,
) -> Result<u64, DatabaseError> {
    let query = format!("SELECT COUNT(*) FROM {view} WHERE logical_snapshot_key = ?");
    connection
        .query_row(&query, params![logical_snapshot_key], |row| {
            row.get::<_, i64>(0)
        })
        .map(|count| count as u64)
        .map_err(DatabaseError::from_duckdb)
}

fn json_string(value: &Value) -> Result<String, DatabaseError> {
    serde_json::to_string(value).map_err(|error| DatabaseError::Command {
        detail: error.to_string(),
    })
}
