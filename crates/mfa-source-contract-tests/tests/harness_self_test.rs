use mfa_source_contract_tests::{ContractHarness, ExpectedResult, ProbeExpectation, SourceCase};
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn harness_self_test_proves_fake_source_contract() {
    let store = TempDir::new().unwrap();
    let package =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/guest-source.mfasource");
    let harness = ContractHarness::new(store.path());
    let cases = [SourceCase {
        fixture: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ok.fixture"),
        expected_probe: ProbeExpectation { confidence: 1 },
        expected_result: ExpectedResult {
            records: 1,
            source_records: 1,
            lineage: 1,
            extensions: 0,
            issues: 0,
            logical_snapshot_key: "fixture:2026".to_owned(),
        },
    }];

    let report = harness.assert_conforms(&package, &cases).await.unwrap();
    assert_eq!(report.cases, 1);
    assert!(report.deterministic_probe);
    assert!(report.deterministic_parse);
    assert!(report.forbidden_imports_absent);
}
