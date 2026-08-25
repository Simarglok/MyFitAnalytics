use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("database path is invalid: {detail}")]
    InvalidPath { detail: String },
    #[error("database could not be opened: {detail}")]
    Open { detail: String },
    #[error("database migration failed: {detail}")]
    Migration { detail: String },
    #[error("database migration checksum mismatch for version {version}")]
    MigrationChecksumMismatch { version: u32 },
    #[error("database schema version {version} is newer than supported version {supported}")]
    IncompatibleSchema { version: u32, supported: u32 },
    #[error("database command queue is closed")]
    ChannelClosed,
    #[error("database actor stopped before returning a result")]
    ActorStopped,
    #[error("database command failed: {detail}")]
    Command { detail: String },
    #[error("database validation failed ({code}): {detail}")]
    Validation { code: &'static str, detail: String },
    #[error("extension contract is not registered: {contract_id}@{contract_version}")]
    ExtensionContractMissing {
        contract_id: String,
        contract_version: String,
    },
    #[error("database shutdown failed: {detail}")]
    Shutdown { detail: String },
}

impl DatabaseError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidPath { .. } => "invalid_database_path",
            Self::Open { .. } => "database_open_failed",
            Self::Migration { .. } => "migration_failed",
            Self::MigrationChecksumMismatch { .. } => "migration_checksum_mismatch",
            Self::IncompatibleSchema { .. } => "incompatible_schema",
            Self::ChannelClosed => "database_channel_closed",
            Self::ActorStopped => "database_actor_stopped",
            Self::Command { .. } => "database_command_failed",
            Self::Validation { code, .. } => code,
            Self::ExtensionContractMissing { .. } => "extension_contract_missing",
            Self::Shutdown { .. } => "database_shutdown_failed",
        }
    }

    pub(crate) fn from_duckdb(error: duckdb::Error) -> Self {
        Self::Command {
            detail: error.to_string(),
        }
    }
}

impl From<crate::validation::ValidationError> for DatabaseError {
    fn from(error: crate::validation::ValidationError) -> Self {
        Self::Validation {
            code: error.code(),
            detail: error.to_string(),
        }
    }
}
