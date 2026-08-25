# Storage/Ingestion acceptance evidence

## Focused acceptance gate

Command:

```bash
./scripts/storage-ingestion-gate.sh
```

Result: exit 0.

- `storage_gate_serializes_all_archive_ingestion_recovery_and_rebuild_paths`: 1 passed.
- `cargo test -p myfitanalytics --test ingestion_commands -- --test-threads=1`: 4 passed.

The gate uses only temporary app-data/workspace roots and synthetic fixture bytes. It does not read personal exports, iCloud paths, credentials, or secrets.

## Scenario mapping

| Requirement | Automated evidence |
| --- | --- |
| Archive then parse/commit then inbox deletion | `mfa-ingestion/tests/end_to_end.rs::successful_asset_follows_archive_parse_commit_and_data_changed_order` |
| Content dedupe and repeated filenames | `exact_duplicate_records_receipt_removes_inbox_and_skips_guest`; gate duplicate phase |
| Parse failure retains input | `one_asset_failure_does_not_stop_next_asset_after_archive_cleanup`; gate parse phase |
| Archive/DB transaction failure is retryable | `canonical_transaction_failure_retries_a_registered_asset_before_inbox_delete`; gate transaction phase |
| Registration crash window retains archive and inbox | `registration_failure_keeps_archive_and_inbox_for_a_later_retry` |
| Startup crash reconciliation and recovery gate | `mfa-ingestion/tests/recovery.rs::startup_recovery_marks_interrupted_attempts_before_releasing_ingestion_gate`; gate recovery phase |
| Failed replacement leaves active snapshot unchanged | `mfa-db/tests/fault_injection.rs::active_snapshot_failure_keeps_the_previous_snapshot_visible`; `injected_rebuild_swap_failure_reopens_the_original_database` |
| Immutable archive rebuild | `mfa-ingestion/tests/rebuild.rs::rebuild_uses_a_temporary_actor_and_keeps_an_immutable_recovery_copy`; production importer test; gate rebuild phase |
| Manual retry creates a fresh attempt | `manual_retry_replays_an_archived_asset_with_a_new_attempt_identity` |
| Single actor ownership | `mfa-db/tests/actor_ownership.rs` (3 passed); no Tauri/archive code imports DuckDB |
| 32 concurrent desktop calls | `myfitanalytics/tests/ingestion_commands.rs::thirty_two_concurrent_query_and_refresh_commands_share_one_actor` |
| Stable DTOs and non-iCloud paths | `workspace_command_persists_settings_and_exposes_non_icloud_paths`; `refresh_status_and_quality_commands_return_safe_typed_dtos` |
| ID-only data change event | `data_changed_event_contains_only_refresh_identifiers`; `web/src/storage-boundary.test.ts` |

## RED/GREEN record

- Task 5 retry/recovery/rebuild tracers were observed failing before their production seams, then passed after implementation; the focused Rust suites are green.
- Task 6 `cargo test -p myfitanalytics --test ingestion_commands` first failed with missing storage command/runtime symbols, then passed with 4 tests.
- Task 6 `svelte-check` first failed because the new typed transport methods were absent, then passed after the transport/types/mock adapters were implemented.
- The frontend boundary suite initially failed under browser resolution for Node filesystem imports; the per-file Node environment annotation fixed that scoped test, and the exact frontend gate now passes (3 files, 5 tests).

## Architecture evidence

- `DatabaseService` remains the only DuckDB owner; Tauri state stores cloneable channel handles and never opens DuckDB.
- Archive scanning, hashing, source parsing, and archive filesystem operations stay outside the actor.
- Tauri commands validate/map DTOs and do not hold state locks over awaits.
- `data-changed` emits only capability/dashboard IDs; the Svelte app reloads view models through `AppTransport`.
- Settings expose configured workspace, inbox/archive, app-data, recovery, backup, and database paths without making iCloud a dependency.

## Fresh final verification

All commands below were run after the implementation commits and returned exit 0:

| Command | Result |
| --- | --- |
| `cargo test --workspace -- --test-threads=1` | All workspace unit, integration, and doc tests passed |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `pnpm --dir web test -- --run` | 3 files, 5 tests passed |
| `pnpm --dir web exec svelte-check --tsconfig ./tsconfig.json` | 0 errors, 0 warnings |
| `pnpm --dir web build` | Vite production build passed; 118 modules transformed |
| `./scripts/storage-ingestion-gate.sh` | 1 storage-gate test and 4 Tauri command tests passed |
| `git diff --check` | Passed |
| `git status --short --branch` | Clean; `feat/storage-ingestion...origin/main [ahead 6]` |
