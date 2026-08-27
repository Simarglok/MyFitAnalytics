use duckdb::{Connection, params};
use mfa_db::DatabaseService;
use tempfile::TempDir;

#[tokio::test]
async fn fresh_database_reaches_schema_version_four_and_restart_is_idempotent() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("storage.duckdb");
    let service = DatabaseService::start(&path, 4).await.unwrap();
    assert_eq!(
        service
            .execute(mfa_db::HealthCheck)
            .await
            .unwrap()
            .schema_version,
        4
    );
    service.shutdown().await.unwrap();

    let restarted = DatabaseService::start(&path, 4).await.unwrap();
    assert_eq!(
        restarted
            .execute(mfa_db::HealthCheck)
            .await
            .unwrap()
            .schema_version,
        4
    );
    restarted.shutdown().await.unwrap();

    let connection = Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT name FROM schema_migration WHERE version = 4",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "user_phase_events"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'user_phase_event'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn tampered_migration_checksum_fails_closed() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("storage.duckdb");
    let service = DatabaseService::start(&path, 4).await.unwrap();
    service.shutdown().await.unwrap();

    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE schema_migration SET checksum = ? WHERE version = 1",
            params!["tampered"],
        )
        .unwrap();
    drop(connection);

    let error = DatabaseService::start(&path, 4).await.unwrap_err();
    assert_eq!(error.code(), "migration_checksum_mismatch");
}

#[tokio::test]
async fn unsupported_future_schema_is_stable_and_explicit() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("storage.duckdb");
    let service = DatabaseService::start(&path, 4).await.unwrap();
    service.shutdown().await.unwrap();

    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "INSERT INTO schema_migration(version, name, checksum, applied_at) VALUES (99, 'future', 'x', CURRENT_TIMESTAMP)",
            [],
        )
        .unwrap();
    drop(connection);

    let error = DatabaseService::start(&path, 4).await.unwrap_err();
    assert_eq!(error.code(), "incompatible_schema");
}

#[tokio::test]
async fn failed_migration_rolls_back_its_schema_and_version_row() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("storage.duckdb");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch("CREATE TABLE nutrition_item (bad INTEGER)")
        .unwrap();
    drop(connection);

    let _error = DatabaseService::start(&path, 4).await.unwrap_err();
    let connection = Connection::open(&path).unwrap();
    let applied: i64 = connection
        .query_row("SELECT MAX(version) FROM schema_migration", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(applied, 1);
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM schema_migration WHERE version = 2",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        0
    );
}

#[test]
fn provenance_schema_contains_immutable_asset_receipts_attempts_lineage_extensions_and_quality() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("storage.duckdb");
    let connection = Connection::open(&path).unwrap();
    mfa_db::migrations::apply_all_for_test(&connection).unwrap();
    for table in [
        "source_receipt",
        "source_asset",
        "ingestion_attempt",
        "source_record",
        "lineage",
        "extension_contract",
        "extension_record",
        "data_quality_item",
    ] {
        let exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = ?",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "missing table {table}");
    }
}
