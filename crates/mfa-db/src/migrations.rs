use crate::error::DatabaseError;
use chrono::Utc;
use duckdb::{Connection, params};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const CURRENT_SCHEMA_VERSION: u32 = 4;

struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
}

fn migrations() -> [Migration; 4] {
    [
        Migration {
            version: 1,
            name: "provenance",
            sql: include_str!("../migrations/0001_provenance.sql"),
        },
        Migration {
            version: 2,
            name: "canonical",
            sql: include_str!("../migrations/0002_canonical.sql"),
        },
        Migration {
            version: 3,
            name: "active_snapshots",
            sql: include_str!("../migrations/0003_active_snapshots.sql"),
        },
        Migration {
            version: 4,
            name: "user_phase_events",
            sql: include_str!("../migrations/0004_user_phase_events.sql"),
        },
    ]
}

pub fn apply_all_for_test(connection: &Connection) -> Result<(), DatabaseError> {
    apply_all(connection)
}

pub(crate) fn apply_all(connection: &Connection) -> Result<(), DatabaseError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migration (
                version UINTEGER PRIMARY KEY,
                name VARCHAR NOT NULL,
                checksum VARCHAR NOT NULL,
                applied_at TIMESTAMP NOT NULL
            )",
        )
        .map_err(|error| DatabaseError::Migration {
            detail: error.to_string(),
        })?;

    let mut applied = BTreeMap::new();
    let mut statement = connection
        .prepare("SELECT version, name, checksum FROM schema_migration ORDER BY version")
        .map_err(|error| DatabaseError::Migration {
            detail: error.to_string(),
        })?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| DatabaseError::Migration {
            detail: error.to_string(),
        })?;
    for row in rows {
        let (version, name, checksum) = row.map_err(|error| DatabaseError::Migration {
            detail: error.to_string(),
        })?;
        applied.insert(version, (name, checksum));
    }

    if let Some((&version, _)) = applied.iter().next_back()
        && version > CURRENT_SCHEMA_VERSION
    {
        return Err(DatabaseError::IncompatibleSchema {
            version,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }

    for migration in migrations() {
        let checksum = checksum(migration.sql);
        if let Some((name, recorded_checksum)) = applied.get(&migration.version) {
            if name != migration.name || recorded_checksum != &checksum {
                return Err(DatabaseError::MigrationChecksumMismatch {
                    version: migration.version,
                });
            }
            continue;
        }

        let transaction =
            connection
                .unchecked_transaction()
                .map_err(|error| DatabaseError::Migration {
                    detail: error.to_string(),
                })?;
        transaction
            .execute_batch(migration.sql)
            .map_err(|error| DatabaseError::Migration {
                detail: error.to_string(),
            })?;
        transaction
            .execute(
                "INSERT INTO schema_migration(version, name, checksum, applied_at) VALUES (?, ?, ?, ?)",
                params![migration.version, migration.name, checksum, Utc::now()],
            )
            .map_err(|error| DatabaseError::Migration {
                detail: error.to_string(),
            })?;
        transaction
            .commit()
            .map_err(|error| DatabaseError::Migration {
                detail: error.to_string(),
            })?;
    }

    Ok(())
}

pub(crate) fn checksum(sql: &str) -> String {
    format!("{:x}", Sha256::digest(sql.as_bytes()))
}
