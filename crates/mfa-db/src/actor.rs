use crate::command::{
    DatabaseCommand, FailAttempt, FailAttemptResult, HealthCheckResult, MarkInterruptedResult,
    RegisterAsset, RegisterAssetResult, RegisterReceipt, RegisterReceiptResult, StartAttempt,
    StartAttemptResult, ViewResponse,
};
use crate::error::DatabaseError;
use crate::migrations;
use duckdb::{Connection, params};
use std::path::PathBuf;
use tokio::sync::mpsc;

pub(crate) fn run_actor(
    path: PathBuf,
    mut receiver: mpsc::Receiver<DatabaseCommand>,
    ready: std::sync::mpsc::SyncSender<Result<(), DatabaseError>>,
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
        if process_command(&connection, command) {
            break;
        }
    }
}

fn process_command(connection: &Connection, command: DatabaseCommand) -> bool {
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
        DatabaseCommand::QueryView(_, response) => {
            let _ = response.send(Ok(ViewResponse { rows: Vec::new() }));
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
