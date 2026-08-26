use crate::{ContractTestError, ExpectedResult};
use mfa_contracts::{SOURCE_API_VERSION, SourceBatch};

pub(crate) fn assert_expected(
    batch: &SourceBatch,
    expected: &ExpectedResult,
) -> Result<(), ContractTestError> {
    if batch.source_api_version.to_string() != SOURCE_API_VERSION
        || batch.records.len() != expected.records
        || batch.source_records.len() != expected.source_records
        || batch.lineage.len() != expected.lineage
        || batch.extensions.len() != expected.extensions
        || batch.issues.len() != expected.issues
        || batch.logical_snapshot_key != expected.logical_snapshot_key
    {
        return Err(ContractTestError::Mismatch(format!(
            "unexpected source batch counts or logical key: records={}, source_records={}, lineage={}, extensions={}, issues={}, key={}",
            batch.records.len(),
            batch.source_records.len(),
            batch.lineage.len(),
            batch.extensions.len(),
            batch.issues.len(),
            batch.logical_snapshot_key,
        )));
    }
    Ok(())
}
