# MyFitAnalytics Foundation Gate Evidence

Captured in the final Task 6 working tree before the Task 6 commit.

## Session and runtime

- Session ID: `20260825_181527_7834bb`
- Session title: `myfitanalytics-mvp-foundation`
- Model: `gpt-5.6-luna`
- Reasoning: `max` (required runtime)
- Repository: `/Users/simarglok/Git/MyFitAnalytics`
- Branch during verification: `feat/mvp-foundation`

## Toolchain

- `rustc --version`: `rustc 1.94.0 (4a4ef493e 2026-03-02)`
- `cargo --version`: `cargo 1.94.0 (85eff7c80 2026-01-15)`
- `node --version`: `v24.19.0`
- `pnpm --version`: `11.23.0`
- `cargo tauri --version`: `tauri-cli 2.11.0`
- Wasmtime dependency: exactly `47.0.3`

## Focused red/green verification

All focused red commands were observed failing before their corresponding implementation. The following final focused green commands exited 0:

- `cargo test -p mfa-contracts --test contract_roundtrip`
- `cargo test -p mfa-contracts --test schema_examples`
- `cargo test -p mfa-module-host --test package_lifecycle`
- `cargo test -p mfa-module-host --test package_security`
- `cargo clippy -p mfa-module-host --all-targets -- -D warnings`
- `cargo test -p mfa-module-host --test runtime_contract`
- `cargo test -p mfa-module-host --test runtime_limits`
- `cargo test -p mfa-config`
- `cargo test -p mfa-module-host --test capability_registry`
- `cargo test -p mfa-module-host --test locale_resolution`
- `cargo test -p myfitanalytics --test command_contract`
- `pnpm --dir web test -- --run`
- `pnpm --dir web exec svelte-check --tsconfig ./tsconfig.json`
- `pnpm --dir web build`

The final frontend suite reports 2 test files and 4 tests passed. The final command-contract suite reports 3 tests passed.

## Required Foundation gate

Every command below exited 0 in one final verification run. The complete stdout/stderr captured from that final run is included below.

| Command | Exit | Result |
|---|---:|---|
| `cargo test --workspace` | 0 | All workspace unit, integration, runtime, package, contract, settings, locale, capability, and command tests passed. |
| `cargo fmt --all --check` | 0 | Formatting clean. |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | No warnings. |
| `pnpm --dir web test -- --run` | 0 | 2 files / 4 tests passed. |
| `pnpm --dir web exec svelte-check --tsconfig ./tsconfig.json` | 0 | 0 errors, 0 warnings. |
| `pnpm --dir web build` | 0 | Vite production build passed. |
| `cargo tauri build --debug --no-bundle` | 0 | Debug binary built at `target/debug/myfitanalytics`. |

## Material deviations and compatibility adaptations

- The repository was already initialized, so Task 1 created and used `feat/mvp-foundation` instead of running the obsolete `git init` step.
- The pinned `zip 2.4.2` writer rejects duplicate names and its reader indexes names uniquely. Package inspection therefore performs a bounded central-directory duplicate scan before `ZipArchive`; this preserves duplicate-entry rejection without weakening production security.
- Task 4 fixture crates are standalone nested Cargo workspaces so they do not become production workspace members. Component fixtures are checked in and runtime tests load them directly; no runtime Cargo/network/tool invocation is involved.
- Wasmtime 47.0.3 marks `Config::async_support(true)` deprecated/no-op, but the approved contract requires the call. It remains present under a narrow deprecation allowance; async execution uses the pinned async component APIs.
- Task 5 uses a small new `mfa-config` crate for settings and atomic persistence. Locale catalogs validate declared locale/namespace against their manifest role and reject unmatched closing braces.
- Task 6 adds `src-tauri/src/lib.rs` so the required integration command test can exercise shared state/command helpers. The Tauri binary remains composed only in `main.rs`/`app.rs`.
- The production core English catalog is embedded with `include_bytes!` and passed through `LocaleResolver::from_core_json`; runtime setup does not depend on a checkout or build-machine path. The root-based constructor remains covered by tests.
- `core:default` was narrowed to an empty permission list for the `main` window because this shell uses only the application’s typed commands and no Tauri core/plugin APIs.
- `jsdom@27.0.6` was unavailable in the registry; pnpm reported `30.0.1` as the current published version, so `jsdom 30.0.1` and `@types/node 26.2.0` are used.
- Vitest required the browser Svelte export condition; `vite.config.ts` explicitly selects `browser` so component tests exercise client lifecycle behavior.
- `BootstrapState` includes `modules` in both Rust and TypeScript. `list_modules` remains a separate typed command because it is explicitly required by the approved plan.

## Security and boundary checks

- Source/dashboard Wasm guests receive no WASI linker or ambient authority. The runtime creates only typed component linkers and a host-owned read-only asset resource.
- Package hashes are checked before component compilation/cache lookup.
- Fuel, epoch timeout, memory, output-size, malformed-output, undeclared-capability, and fresh-store recovery tests pass.
- Tauri capability `default` targets only `main` and has no broad core/plugin permissions.
- The frontend import-boundary test scans production source files and fails if `@tauri-apps/api` appears outside `web/src/lib/tauri-transport.ts`.
- Rust command DTOs contain typed metadata only; paths, package hashes, and internal error details are not serialized.

## Known warning / risk

- `cargo tauri build` exits 0 but warns that the existing identifier `com.myfitanalytics.app` ends in `.app`, which conflicts with the macOS application bundle extension convention. No human action is required to close the Foundation gate; this identifier should be revisited before release packaging.

## Human action

None required for the Foundation gate. Stop here and wait for manager review before Storage/Ingestion work.


## Complete final gate output

### `cargo test --workspace`
Exit code: `0`

```text
   Compiling objc2-exception-helper v0.1.1
   Compiling wasmtime v47.0.3
   Compiling objc2 v0.6.4
   Compiling block2 v0.6.2
   Compiling objc2-core-foundation v0.3.2
   Compiling dispatch2 v0.3.1
   Compiling objc2-foundation v0.3.2
   Compiling objc2-app-kit v0.3.2
   Compiling mfa-module-host v0.1.0 (/Users/simarglok/Git/MyFitAnalytics/crates/mfa-module-host)
   Compiling objc2-web-kit v0.3.2
   Compiling tao v0.35.3
   Compiling muda v0.19.3
   Compiling window-vibrancy v0.6.0
   Compiling tauri-runtime v2.11.3
   Compiling wry v0.55.1
   Compiling tauri-runtime-wry v2.11.4
   Compiling tauri v2.11.5
   Compiling myfitanalytics v0.1.0 (/Users/simarglok/Git/MyFitAnalytics/src-tauri)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 9.28s
     Running unittests src/lib.rs (target/debug/deps/mfa_config-ef7f585d805dc986)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/settings_recovery.rs (target/debug/deps/settings_recovery-9402f23bea7e265a)

running 3 tests
test unsupported_schema_version_is_rejected_with_stable_code ... ok
test interrupted_temp_file_is_recovered_and_promoted ... ok
test settings_round_trip_uses_atomic_same_directory_save ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

     Running unittests src/lib.rs (target/debug/deps/mfa_contracts-dd1916ac9d6d58f7)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/contract_roundtrip.rs (target/debug/deps/contract_roundtrip-1e1c2476b69b8e00)

running 6 tests
test invalid_ids_and_versions_are_rejected_with_stable_codes ... ok
test temporal_newtypes_preserve_distinct_wire_semantics ... ok
test dashboard_manifest_round_trips_entrypoint_hash ... ok
test extension_records_have_the_required_versioned_shape ... ok
test canonical_observations_use_explicit_stable_tags ... ok
test source_batch_round_trips_without_variant_order_assumptions ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/schema_examples.rs (target/debug/deps/schema_examples-d725296895fe8f04)

running 4 tests
test source_manifest_rejects_missing_security_fields ... ok
test source_manifest_example_requires_the_complete_contract ... ok
test locale_manifest_rejects_executable_entries ... ok
test dashboard_manifest_declares_base_and_extension_dependencies ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

     Running unittests src/lib.rs (target/debug/deps/mfa_module_host-0100dbc045829690)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/capability_registry.rs (target/debug/deps/capability_registry-88775434a41191d4)

running 3 tests
test disabled_and_missing_selected_providers_are_rejected ... ok
test unselected_offers_do_not_become_implicit_active_providers ... ok
test explicit_provider_selection_is_single_and_deterministic ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/locale_resolution.rs (target/debug/deps/locale_resolution-14969e208063a2f5)

running 5 tests
test unmatched_closing_braces_are_rejected ... ok
test missing_keys_and_invalid_placeholders_are_visible_and_stable ... ok
test same_locale_namespace_from_different_module_ids_is_rejected ... ok
test locale_fallback_prefers_selected_locale_then_executable_then_core ... ok
test catalog_locale_and_namespace_must_match_their_manifest_role ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/package_lifecycle.rs (target/debug/deps/package_lifecycle-77cd5f340a8281cf)

running 5 tests
test registry_reconstructs_from_manifests_without_mutable_index ... ok
test install_is_content_addressed_atomic_and_idempotent ... ok
test checked_in_valid_source_fixture_is_installable ... ok
test failed_update_keeps_old_version_and_valid_update_adds_new_version ... ok
test disable_persists_without_deleting_bytes_and_uninstall_selects_latest_package ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s

     Running tests/package_security.rs (target/debug/deps/package_security-e5b15878deb81edb)

running 7 tests
test checked_in_traversal_fixture_is_rejected ... ok
test traversal_and_absolute_paths_are_rejected ... ok
test package_path_permissions_are_not_used_as_authority ... ok
test malformed_manifest_and_missing_entrypoint_are_rejected ... ok
test symlink_duplicate_manifest_and_hash_mismatch_have_distinct_codes ... ok
test extension_api_executable_locale_and_size_limits_are_rejected ... ok
test dashboard_entrypoint_hash_is_checked_before_installation ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

     Running tests/runtime_contract.rs (target/debug/deps/runtime_contract-c31ab0a6b12a4039)

running 3 tests
test runtime_api_accepts_shared_read_only_assets_without_path_authority ... ok
test source_component_transforms_asset_and_returns_validated_batch ... ok
test dashboard_component_returns_only_declarative_document_contract ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s

     Running tests/runtime_limits.rs (target/debug/deps/runtime_limits-5c8d96afd8c315c0)

running 8 tests
test mismatched_entrypoint_hash_is_rejected_before_compilation ... ok
test no_wasi_imports_are_linked_into_source_guests ... ok
test fuel_exhaustion_is_bounded_and_reported ... ok
test output_is_bounded_before_deserialization ... ok
test memory_growth_is_capped ... ok
test failed_invocation_does_not_poison_the_next_fresh_store ... ok
test epoch_timeout_is_bounded_under_two_seconds ... ok
test malformed_json_and_undeclared_capability_are_rejected ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.48s

     Running unittests src/lib.rs (target/debug/deps/myfitanalytics-6847180394959a5a)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/myfitanalytics-a9b9cc4c123f3e2d)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/command_contract.rs (target/debug/deps/command_contract-f249ef3aa09669ed)

running 3 tests
test embedded_core_catalog_does_not_require_a_repository_path ... ok
test list_modules_exposes_metadata_without_paths_or_internal_errors ... ok
test bootstrap_command_returns_typed_safe_state ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Doc-tests mfa_config

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests mfa_contracts

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests mfa_module_host

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests myfitanalytics

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


```

### `cargo fmt --all --check`
Exit code: `0`

```text

```

### `cargo clippy --workspace --all-targets -- -D warnings`
Exit code: `0`

```text
    Checking objc2-exception-helper v0.1.1
    Checking wasmtime v47.0.3
    Checking objc2 v0.6.4
    Checking block2 v0.6.2
    Checking objc2-core-foundation v0.3.2
    Checking dispatch2 v0.3.1
    Checking objc2-foundation v0.3.2
    Checking objc2-app-kit v0.3.2
    Checking mfa-module-host v0.1.0 (/Users/simarglok/Git/MyFitAnalytics/crates/mfa-module-host)
    Checking objc2-web-kit v0.3.2
    Checking tao v0.35.3
    Checking window-vibrancy v0.6.0
    Checking muda v0.19.3
    Checking tauri-runtime v2.11.3
    Checking wry v0.55.1
    Checking tauri-runtime-wry v2.11.4
    Checking tauri v2.11.5
    Checking myfitanalytics v0.1.0 (/Users/simarglok/Git/MyFitAnalytics/src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.51s

```

### `pnpm --dir web test -- --run`
Exit code: `0`

```text
$ vitest -- --run
7:58:02 PM [vite-plugin-svelte] !!! Support for rolldown-vite in vite-plugin-svelte is experimental (rolldown: 1.2.5, vite: 8.2.2) !!!
	See https://github.com/sveltejs/vite-plugin-svelte/issues/1143 for a list of known issues and to report feedback.
7:58:02 PM [vite-plugin-svelte] no Svelte config found at /Users/simarglok/Git/MyFitAnalytics/web - using default configuration.
`optimizeDeps.rollupOptions` / `ssr.optimizeDeps.rollupOptions` is deprecated. Use `optimizeDeps.rolldownOptions` instead. Note that this option may be set by a plugin. Set VITE_DEPRECATION_TRACE=1 to see where it is called.

 RUN  v4.1.10 /Users/simarglok/Git/MyFitAnalytics/web


 Test Files  2 passed (2)
      Tests  4 passed (4)
   Start at  19:58:02
   Duration  907ms (transform 227ms, setup 0ms, import 278ms, tests 170ms, environment 825ms)


```

### `pnpm --dir web exec svelte-check --tsconfig ./tsconfig.json`
Exit code: `0`

```text
Loading svelte-check in workspace: /Users/simarglok/Git/MyFitAnalytics/web
Getting Svelte diagnostics...

svelte-check found 0 errors and 0 warnings

```

### `pnpm --dir web build`
Exit code: `0`

```text
$ vite build
7:58:04 PM [vite-plugin-svelte] !!! Support for rolldown-vite in vite-plugin-svelte is experimental (rolldown: 1.2.5, vite: 8.2.2) !!!
	See https://github.com/sveltejs/vite-plugin-svelte/issues/1143 for a list of known issues and to report feedback.
7:58:04 PM [vite-plugin-svelte] no Svelte config found at /Users/simarglok/Git/MyFitAnalytics/web - using default configuration.
vite v8.2.2 building client environment for production...
transforming...
✓ 117 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                  0.44 kB │ gzip:  0.28 kB
dist/assets/index-CeEfwxqs.css   1.28 kB │ gzip:  0.69 kB
dist/assets/index-CDqEagTK.js   39.75 kB │ gzip: 15.61 kB

✓ built in 67ms

```

### `cargo tauri build --debug --no-bundle`
Exit code: `0`

```text
        Info Looking up installed tauri packages to check mismatched versions...
        Warn The bundle identifier "com.myfitanalytics.app" set in `"tauri.conf.json" identifier` ends with `.app`. This is not recommended because it conflicts with the application bundle extension on macOS.
     Running beforeBuildCommand `pnpm --dir web build`
$ vite build
7:58:05 PM [vite-plugin-svelte] !!! Support for rolldown-vite in vite-plugin-svelte is experimental (rolldown: 1.2.5, vite: 8.2.2) !!!
	See https://github.com/sveltejs/vite-plugin-svelte/issues/1143 for a list of known issues and to report feedback.
7:58:05 PM [vite-plugin-svelte] no Svelte config found at /Users/simarglok/Git/MyFitAnalytics/web - using default configuration.
vite v8.2.2 building client environment for production...
transforming...
✓ 117 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                  0.44 kB │ gzip:  0.28 kB
dist/assets/index-CeEfwxqs.css   1.28 kB │ gzip:  0.69 kB
dist/assets/index-CDqEagTK.js   39.75 kB │ gzip: 15.61 kB

✓ built in 51ms
   Compiling objc2-exception-helper v0.1.1
   Compiling wasmtime v47.0.3
   Compiling objc2 v0.6.4
   Compiling objc2-core-foundation v0.3.2
   Compiling block2 v0.6.2
   Compiling dispatch2 v0.3.1
   Compiling objc2-foundation v0.3.2
   Compiling objc2-app-kit v0.3.2
   Compiling mfa-module-host v0.1.0 (/Users/simarglok/Git/MyFitAnalytics/crates/mfa-module-host)
   Compiling objc2-web-kit v0.3.2
   Compiling tao v0.35.3
   Compiling muda v0.19.3
   Compiling window-vibrancy v0.6.0
   Compiling tauri-runtime v2.11.3
   Compiling wry v0.55.1
   Compiling tauri-runtime-wry v2.11.4
   Compiling tauri v2.11.5
   Compiling myfitanalytics v0.1.0 (/Users/simarglok/Git/MyFitAnalytics/src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.66s
       Built application at: /Users/simarglok/Git/MyFitAnalytics/target/debug/myfitanalytics

```
