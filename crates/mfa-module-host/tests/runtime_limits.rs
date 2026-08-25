mod support;

use mfa_module_host::ComponentRuntime;
use std::time::Duration;
use support::{asset, limits, source_module, source_module_with_declared_hash};
use tempfile::TempDir;

fn assert_code(error: mfa_module_host::RuntimeError, expected: &str) {
    assert_eq!(error.code(), expected, "expected {expected}, got {error:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mismatched_entrypoint_hash_is_rejected_before_compilation() {
    let store = TempDir::new().unwrap();
    let module = source_module_with_declared_hash(
        &store,
        "guest-source.wasm",
        &["body.weight"],
        Some("sha256:0000000000000000000000000000000000000000000000000000000000000000"),
    );
    assert_code(
        ComponentRuntime::new()
            .invoke_source(&module, asset(b"ok"), limits())
            .await
            .unwrap_err(),
        "module_hash_mismatch",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_wasi_imports_are_linked_into_source_guests() {
    let store = TempDir::new().unwrap();
    let module = source_module(&store, "guest-source-wasi.wasm", &["body.weight"]);
    let error = ComponentRuntime::new()
        .invoke_source(&module, asset(b"ok"), limits())
        .await
        .unwrap_err();
    assert_code(error, "module_link_error");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fuel_exhaustion_is_bounded_and_reported() {
    let store = TempDir::new().unwrap();
    let module = source_module(&store, "guest-source.wasm", &["body.weight"]);
    let mut bounded = limits();
    bounded.fuel = 100;
    assert_code(
        ComponentRuntime::new()
            .invoke_source(&module, asset(b"loop"), bounded)
            .await
            .unwrap_err(),
        "module_fuel_exhausted",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn epoch_timeout_is_bounded_under_two_seconds() {
    let store = TempDir::new().unwrap();
    let module = source_module(&store, "guest-source.wasm", &["body.weight"]);
    let mut bounded = limits();
    bounded.fuel = u64::MAX;
    bounded.timeout = Duration::from_millis(50);
    assert_code(
        ComponentRuntime::new()
            .invoke_source(&module, asset(b"loop"), bounded)
            .await
            .unwrap_err(),
        "module_timeout",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_growth_is_capped() {
    let store = TempDir::new().unwrap();
    let module = source_module(&store, "guest-source.wasm", &["body.weight"]);
    let mut bounded = limits();
    bounded.max_memory_bytes = 2 * 1024 * 1024;
    assert_code(
        ComponentRuntime::new()
            .invoke_source(&module, asset(b"memory"), bounded)
            .await
            .unwrap_err(),
        "module_memory_limit",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_is_bounded_before_deserialization() {
    let store = TempDir::new().unwrap();
    let module = source_module(&store, "guest-source.wasm", &["body.weight"]);
    let mut bounded = limits();
    bounded.max_output_bytes = 64;
    assert_code(
        ComponentRuntime::new()
            .invoke_source(&module, asset(b"huge"), bounded)
            .await
            .unwrap_err(),
        "module_output_limit",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_json_and_undeclared_capability_are_rejected() {
    let store = TempDir::new().unwrap();
    let module = source_module(&store, "guest-source.wasm", &["body.weight"]);
    let runtime = ComponentRuntime::new();
    assert_code(
        runtime
            .invoke_source(&module, asset(b"malformed"), limits())
            .await
            .unwrap_err(),
        "module_malformed_output",
    );
    let undeclared_module = source_module(&store, "guest-source-undeclared.wasm", &["body.weight"]);
    assert_code(
        runtime
            .invoke_source(&undeclared_module, asset(b"ok"), limits())
            .await
            .unwrap_err(),
        "undeclared_capability_output",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_invocation_does_not_poison_the_next_fresh_store() {
    let store = TempDir::new().unwrap();
    let module = source_module(&store, "guest-source.wasm", &["body.weight"]);
    let runtime = ComponentRuntime::new();
    assert!(
        runtime
            .invoke_source(&module, asset(b"malformed"), limits())
            .await
            .is_err()
    );
    assert!(
        runtime
            .invoke_source(&module, asset(b"ok"), limits())
            .await
            .is_ok()
    );
}
