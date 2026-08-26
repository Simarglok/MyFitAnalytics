CREATE TABLE IF NOT EXISTS source_asset (
    asset_id VARCHAR PRIMARY KEY,
    source_module_id VARCHAR NOT NULL,
    asset_type VARCHAR NOT NULL,
    original_filename VARCHAR NOT NULL,
    archive_path VARCHAR NOT NULL,
    byte_sha256 VARCHAR NOT NULL UNIQUE,
    file_size UBIGINT NOT NULL,
    received_at TIMESTAMP NOT NULL
);

CREATE TABLE IF NOT EXISTS source_receipt (
    receipt_id VARCHAR PRIMARY KEY,
    source_module_id VARCHAR NOT NULL,
    inbox_path VARCHAR NOT NULL,
    original_filename VARCHAR NOT NULL,
    discovered_at TIMESTAMP NOT NULL,
    asset_id VARCHAR,
    outcome VARCHAR NOT NULL CHECK (outcome IN ('accepted', 'duplicate', 'ignored', 'failed_before_archive')),
    detail VARCHAR,
    FOREIGN KEY (asset_id) REFERENCES source_asset(asset_id)
);

CREATE TABLE IF NOT EXISTS ingestion_attempt (
    attempt_id VARCHAR PRIMARY KEY,
    asset_id VARCHAR NOT NULL,
    source_module_id VARCHAR NOT NULL,
    source_module_version VARCHAR NOT NULL,
    source_module_package_sha256 VARCHAR NOT NULL,
    source_api_version VARCHAR NOT NULL,
    mapping_version VARCHAR NOT NULL,
    schema_fingerprint VARCHAR NOT NULL,
    logical_snapshot_key VARCHAR NOT NULL,
    started_at TIMESTAMP NOT NULL,
    finished_at TIMESTAMP,
    status VARCHAR NOT NULL CHECK (status IN ('running', 'succeeded', 'failed', 'interrupted', 'superseded')),
    error_code VARCHAR,
    error_message VARCHAR,
    record_count UBIGINT NOT NULL DEFAULT 0,
    FOREIGN KEY (asset_id) REFERENCES source_asset(asset_id)
);

CREATE TABLE IF NOT EXISTS source_record (
    source_record_id VARCHAR PRIMARY KEY,
    attempt_id VARCHAR NOT NULL,
    asset_id VARCHAR NOT NULL,
    sheet_name VARCHAR,
    source_row_number UINTEGER NOT NULL,
    source_record_key VARCHAR NOT NULL UNIQUE,
    raw_payload JSON NOT NULL,
    FOREIGN KEY (attempt_id) REFERENCES ingestion_attempt(attempt_id),
    FOREIGN KEY (asset_id) REFERENCES source_asset(asset_id)
);

CREATE TABLE IF NOT EXISTS lineage (
    snapshot_id VARCHAR NOT NULL,
    canonical_entity_type VARCHAR NOT NULL,
    canonical_entity_id VARCHAR NOT NULL,
    source_record_id VARCHAR NOT NULL,
    mapping_version VARCHAR NOT NULL,
    PRIMARY KEY (snapshot_id, canonical_entity_type, canonical_entity_id, source_record_id),
    FOREIGN KEY (source_record_id) REFERENCES source_record(source_record_id)
);

CREATE TABLE IF NOT EXISTS extension_contract (
    contract_id VARCHAR PRIMARY KEY,
    source_module_id VARCHAR NOT NULL,
    namespace VARCHAR NOT NULL,
    contract_version VARCHAR NOT NULL,
    payload_schema JSON NOT NULL,
    UNIQUE (source_module_id, namespace, contract_version)
);

CREATE TABLE IF NOT EXISTS extension_record (
    extension_record_id VARCHAR PRIMARY KEY,
    source_record_id VARCHAR NOT NULL,
    source_module_id VARCHAR NOT NULL,
    contract_id VARCHAR NOT NULL,
    contract_version VARCHAR NOT NULL,
    occurred_local_at TIMESTAMP,
    local_date DATE,
    payload JSON NOT NULL,
    FOREIGN KEY (source_record_id) REFERENCES source_record(source_record_id),
    FOREIGN KEY (contract_id) REFERENCES extension_contract(contract_id)
);

CREATE TABLE IF NOT EXISTS data_quality_item (
    data_quality_item_id VARCHAR PRIMARY KEY,
    item_type VARCHAR NOT NULL,
    source_asset_id VARCHAR,
    source_record_id VARCHAR,
    severity VARCHAR NOT NULL CHECK (severity IN ('info', 'warning', 'error', 'critical')),
    message VARCHAR NOT NULL,
    status VARCHAR NOT NULL CHECK (status IN ('open', 'resolved')),
    created_at TIMESTAMP NOT NULL,
    resolved_at TIMESTAMP,
    FOREIGN KEY (source_asset_id) REFERENCES source_asset(asset_id),
    FOREIGN KEY (source_record_id) REFERENCES source_record(source_record_id)
);

CREATE INDEX IF NOT EXISTS idx_source_receipt_asset ON source_receipt(asset_id);
CREATE INDEX IF NOT EXISTS idx_attempt_asset ON ingestion_attempt(asset_id);
CREATE INDEX IF NOT EXISTS idx_attempt_logical_key ON ingestion_attempt(logical_snapshot_key);
CREATE INDEX IF NOT EXISTS idx_source_record_attempt ON source_record(attempt_id);
CREATE INDEX IF NOT EXISTS idx_quality_status ON data_quality_item(status);
