# Source Module Remediation Evidence

## Scope and disposition

This record covers the Terra High round-1 and round-2 `CHANGES_REQUIRED` remediations, the manager-verification corrections, the production-ingestion correction, the macOS signing remediations, and Terra correction round 1 in `/private/tmp/MyFitAnalytics-source-modules`. Final packaged native acceptance is green on implementation commit `f352efd`; Terra final review round 1 returned `CHANGES REQUIRED` for a README status contradiction, and final review round 2 returned `APPROVED` after the correction. The final artifact, signing, lifecycle, and post-quit database evidence is recorded below. No push, PR mutation, merge, tag, release, or repository-settings change was performed.

- Repository: `https://github.com/Simarglok/MyFitAnalytics.git`
- Worktree: `/private/tmp/MyFitAnalytics-source-modules`
- Branch: `feat/source-modules`
- Baseline/merge-base: `5e6f3b17c25ee52905985155e442adb028fed84a`
- Initial verified branch/origin head: `ba019d84b9bf975cefebada9d3041a4fa3ee27cb`
- Implementation HEAD before this documentation-only follow-up: `f352efdc18d2fc10c5d4dda0b28ff4182530bdf5` (`f352efd`)
- Existing PR: `#4`, `main <- feat/source-modules`, Draft; intentionally preserved.
- Terra status: rounds 1 and 2 were `CHANGES_REQUIRED`; Terra round 3 approval was followed by manager discovery that the exact Tauri integration command failed at `7555b58`, then manager reproduction of production-ingestion and macOS signing failures. Terra final review round 1 returned `CHANGES REQUIRED` for the exact README blocker recorded below; correction round 1 remediation updates README Status without changing product code. Terra final review round 2 returned `APPROVED` after that correction. Final native acceptance remains green on `f352efd`.

## Terra review correction round 1

- Reviewer title: `myfitanalytics-source-modules-terra-review`
- Durable review session: `20260827_003626_cfc12a`
- Model: `gpt-5.6-terra`, high
- New goal: `review round 1`
- Verdict: `CHANGES REQUIRED`
- Verified blocker: `README.md:9` still said Storage/Ingestion and bundled source modules were planned/incomplete, contradicting the implemented scope and final evidence.
- Correction round 1 remediation: README Status now states that Storage/Ingestion and the bundled Hevy/MyNetDiary source modules are implemented; keeps analytics/dashboard UI, tray/background product behavior, and release distribution explicitly incomplete/future; and identifies local macOS app/DMG bundles as ad hoc-signed development-validation artifacts with strict signature/entitlement verification and no notarization/publishing claim. No product code was changed.

## Terra final review round 2

- Reviewer title: `myfitanalytics-source-modules-terra-review`
- Durable review session: `20260827_003626_cfc12a`
- Model: `gpt-5.6-terra`, high
- New goal: `review round 2`
- Verdict: `APPROVED`
- Correction rounds used: `1`
- The round-1 README blocker is resolved. The round-1 correction changed only `README.md` plus this evidence document; no product code was changed.
- Non-blocking residual risk 1: app/DMG are ad hoc-signed development-validation artifacts; notarization and publishing are explicitly unclaimed.
- Non-blocking residual risk 2: `com.apple.security.cs.disable-library-validation` is intentionally enabled for DuckDB dynamic extension loading and is a release-hardening trade-off.

## Historical baseline (not fresh acceptance evidence)

The pre-remediation record at `ba019d8` identified:

- uninstall cleanup that could report failure after permanently deleting recoverable bytes;
- journal recovery that trusted serialized filesystem paths;
- wildcard resource packaging that could include stale fixture packages;
- missing sealed outer-app resources;
- native UI automation blocked by macOS Accessibility/Assistive Access;
- plain full Tauri app+DMG packaging blocked in the host’s Finder AppleEvent path.

Those results are retained as provenance only. The final artifact and native acceptance evidence below supersedes the provisional pending/blocker statements from the earlier runs.

## Terra round-1 and round-2 findings and fixes

### 1. Journal-controlled paths could escape the store

Finding validated: the old recovery path parsed only `module_id`, then trusted serialized `original_root`, `staged_root`, `backup_path`, and `version_root` before calling `path_exists`, moving directories, restoring backups, or clearing state. A valid-shape journal could therefore point recovery at an external directory.

Fix:

- `UninstallTransaction::from_journal` now receives the store root before recovery filesystem inspection.
- The module ID must be one safe path component.
- The journal’s previous active-package snapshot must contain a valid semver version and exactly 64 hexadecimal package-hash characters.
- Staging and backup filenames must contain valid, matching transaction UUIDs.
- Every serialized path is compared with the recomputed store-root/module/version/hash transaction layout.
- Existing ancestors are canonicalized and must remain under the canonical store root.
- Symlink components at the store root or below it are rejected; only host path-prefix symlinks such as macOS `/var -> /private/var` are tolerated.
- Recovery uses the recomputed paths, never the journal’s path values.

Regression evidence:

- `malicious_uninstall_journal_cannot_modify_external_sentinel`: valid-shape external-path journal is rejected; external staged sentinel remains present and byte-identical.
- `symlinked_uninstall_journal_path_cannot_modify_external_sentinel`: store-relative path through a symlink is rejected; external sentinel remains present and byte-identical.
- Existing corrupt-journal recovery remains non-panicking.

### 2. Fixture packages could leak into production resources

Finding validated: `build-module-packages.sh` previously left old `dist/modules` files in place, and the Tauri wildcard could copy a `guest-source.mfasource` produced by `--fixture-only`.

Fix:

- Every package build removes and recreates `dist/modules` before writing output.
- Tauri resources are an exact allowlist:
  - `../dist/modules/mynetdiary.mfasource -> modules/mynetdiary.mfasource`
  - `../dist/modules/hevy.mfasource -> modules/hevy.mfasource`
- The package verifier asserts that production output contains exactly those two files after both deterministic builds.
- Bootstrap still requires MyNetDiary and Hevy through the shared `PackageInstaller` path.

Regression evidence:

- `tauri_bundles_only_the_production_source_packages`: exact config assertion is green.
- Former contamination sequence (`--fixture-only`, then production build): final package set is exactly `hevy.mfasource mynetdiary.mfasource`.
- Release app resource assertion: exact packaged set is `['hevy.mfasource', 'mynetdiary.mfasource']`; `guest-source.mfasource` and `Contents/Resources/_up_` are absent.

### 3. Post-commit cleanup failures could report permanent uninstall

Finding validated: after `Committed` was journaled, backup deletion, store sync, and journal clear were still fallible and propagated into rollback. If the backup had already been deleted, rollback could not restore the package.

Fix:

- The second durable `Committed` journal write is the explicit point of no return.
- Failures while deleting the rollback backup, syncing the store, or clearing the journal are cleanup failures after commit and return success with the committed journal retained for deferred cleanup.
- Committed-phase recovery never restores package bytes from a remaining backup. It removes deferred cleanup artifacts and retries journal cleanup; cleanup retry failure leaves the committed transaction for a later restart.
- All pre-commit failures continue to return typed errors and restore package/state through rollback.

Regression evidence:

- `post_backup_delete_sync_failure_keeps_committed_uninstall_for_restart_cleanup`: injected post-backup-delete sync failure returns success, leaves the committed journal, and a fresh installer removes the journal without restoring the package.
- `journal_clear_failure_keeps_committed_uninstall_for_restart_cleanup`: injected journal-clear failure returns success, leaves the committed journal, and restart cleanup removes it without reactivating the package.
- Existing pre-commit delete/read/move/state/sync/backup/recovery fault tests remain green.

### 4. Failure injection was scoped out of release API and test targets

Fix:

- Added the explicit `test-support` Cargo feature.
- The fault enum, installer fault field/builder/check method, crate re-export, and Tauri state setter are compiled only for `test-support` or tests; no `debug_assertions` gate remains.
- Production release builds without `test-support` do not contain the fault field or public fault enum/re-export.
- Host and Tauri packages use test-only self `dev-dependencies` requesting `test-support`, so Cargo enables it for integration-test builds without changing production defaults.
- No test was disabled or weakened; the mandatory commands run without feature flags.

Evidence:

- `cargo build -p mfa-module-host --release`: **PASS, exit 0** with test-support disabled.
- `cargo test -p mfa-module-host --test package_lifecycle -- --test-threads=1`: **PASS, exit 0**; 26 tests, without feature flags.
- `cargo test -p myfitanalytics --test module_lifecycle_commands -- --test-threads=1`: **PASS, exit 0**; 10 tests, without feature flags.
- A default-feature release consumer compile probe fails as expected when importing the fault enum; the production API is absent.

### 5. Remaining synthetic hash sentinel

Fix:

- `crates/mfa-module-host/tests/support/mod.rs` now derives the dashboard fixture entrypoint hash from the fixture bytes instead of declaring a fixed dummy sentinel.

Evidence:

- Exact former dashboard hash-sentinel scan scoped to implementation/test files (excluding this evidence and governing plans): 0 matches.
- The two broader marker matches are pre-existing prose in the governing plan documents, not implementation or test dummy values:
  - `docs/superpowers/plans/2026-08-25-myfitanalytics-desktop-release.md:142`
  - `docs/superpowers/plans/2026-08-25-myfitanalytics-foundation.md:444`

### 6. One startup scan did not revisit a stable preexisting inbox file

Manager reproduction after the Terra round-3 approval showed that a real MyNetDiary workbook could remain in the configured inbox indefinitely after enabling the source. The file was unchanged and valid, but the archive remained empty after the condition-based wait.

Root cause:

- `StableScanner` intentionally requires two consecutive equal, readable fingerprint observations before returning a candidate.
- `start_source_coordinator` issued exactly one `Startup` request after creating the coordinator.
- The production coordinator had no watcher, periodic request, or UI refresh action; after the first observation, no second scan could occur.
- `StableScanner::pending_paths` was not involved in the production path and remains outside this correction; no pending-path behavior is claimed here.

Fix:

- `ScanQueue::start_periodic` keeps the existing bounded queue/executor path and adds one Tokio interval to the queue loop; it does not create a second database owner or a direct ingestion path.
- `IngestionCoordinator` uses a one-second periodic interval. The first periodic tick is delayed by one full interval, preserving the startup observation as the first stability observation.
- Missed ticks use `MissedTickBehavior::Skip`, preventing a burst or busy loop after a long scan.
- Periodic requests use the same `QueueCommand`/`ScanRequest` flow and remain coalesced with manual requests by `run_pending`.
- Queue shutdown remains the stop boundary; the periodic loop is owned by the queue task and terminates with its acknowledged shutdown path rather than leaving a detached scheduler task.

Final manager-provided native acceptance evidence on implementation commit `f352efd`:

- The packaged native lifecycle used isolated HOME `/private/tmp/mfa-native-acceptance-final2-20260827/home` and only the checked-in synthetic `valid-full.xls` fixture.
- The real user profile and iCloud were untouched.
- UI proved initial bundled install, MyNetDiary disable, re-enable, disable plus confirmed uninstall, catalog reinstall, enable, and custom inbox selection.
- The synthetic fixture autonomously moved from `/private/tmp/mfa-native-acceptance-20260827/mynetdiary-inbox` to the dated workspace archive.
- The app remained alive as PID `84078`; UI Storage was healthy.
- A clean menu quit returned exit `0`, with no matching background process remaining.

Additional final native lifecycle evidence from a second packaged run on the same isolated HOME `/private/tmp/mfa-native-acceptance-final2-20260827/home`:

- The native workspace picker reselected the same temporary workspace.
- The native package picker selected `/private/tmp/MyFitAnalytics-source-modules/dist/modules/mynetdiary.mfasource` with the input source explicitly set to Russian -> ABC via Command+Space; ABC was verified, then Russian was restored and verified.
- After an initial successful import, MyNetDiary was disabled. A checked-in duplicate fixture remained in the custom inbox after more than 3 seconds, proving that disabled state stops new imports; the previously confirmed archive asset remained.
- Confirmed uninstall removed the installed MyNetDiary package bytes while the archive remained.
- The native package picker reinstall appeared immediately without restart in Disabled state. Enabling MyNetDiary then autonomously disposed of the held fixture; the inbox was empty.
- A second clean menu quit returned exit `0`, with no matching process remaining.

Post-quit copied DB+WAL query:

- snapshot: `Some(008f3fa0-927d-41f1-8bec-b09ba467b04d)`
- total/canonical: `5`
- nutrition: `2`
- activity event: `1`
- activity day: `1`
- heart rate: `1`
- source records: `8`
- historical source records: `8`
- lineage: `6`
- extensions: `1`
- issues: `0`
- steps `6400`: `true`
- water `500`: `true`
- synthetic nutrition `2026-01-04`, amount `1 serving`: `true`
- lineage mapping `1.0.0`: `true`
- The post-lifecycle query remained unchanged from the first run: snapshot `Some(008f3fa0-927d-41f1-8bec-b09ba467b04d)`, canonical `5`, source `8`, historical `8`, lineage `6`, extensions `1`, issues `0`; all semantic assertions were `true`. This proves uninstall/reinstall did not delete history or provenance.

Regression evidence:

- `startup_scan_autonomously_ingests_preexisting_stable_file`: RED at the unfixed implementation, exit `101`; the exact failure was `one startup scan should converge a stable preexisting file: Elapsed(())` after `3.10s`.
- The same test is GREEN after the correction, `1 passed, 0 failed`, exit `0`; it issues exactly one `Startup` request and asserts inbox removal, one archived asset, and one committed active snapshot.
- `queue_semantics`: GREEN, `7 passed, 0 failed`, exit `0`; delayed periodic work, Manual-over-Periodic coalescing, shutdown acknowledgement, and related queue invariants passed.
- Real `bundled_mynetdiary_fixture_runs_through_refresh_archive_and_database` now issues one `Startup` request and passes through the actual package, archive, Wasmtime, and DuckDB path.

### 7. macOS signing sequence

The diagnosed native sequence was:

- The old packaged build terminated during real Wasmtime ingestion with `EXC_BAD_ACCESS`, `SIGKILL (Code Signature Invalid)`, namespace `CODESIGNING`, indicator `Invalid Page`, and a `tokio-rt-worker` faulting thread. The Wasmtime executable-memory path was fixed with the narrow `com.apple.security.cs.allow-unsigned-executable-memory=true` entitlement.
- The next clean isolated-HOME startup exposed DuckDB’s official dynamically loaded `json.duckdb_extension` being rejected because the mapping process and mapped file had different Team IDs. The narrow hardened-runtime remediation was `com.apple.security.cs.disable-library-validation=true`.
- `scripts/verify-macos-signing.sh` now asserts both signed entitlements on the packaged app; no DuckDB schema or extension-loading workaround was introduced.
- Notarization was explicitly skipped because no Apple credentials were configured; no notarization claim is made.

### 8. Real-profile restoration

Profile restoration was completed after native acceptance and before this evidence amendment; the app was not launched again:

- Verified that no MyFitAnalytics app process was running; only the Hermes process title contained the string.
- The real Application Support profile directory was empty: `0 KiB`, no entries.
- The original backup at `/private/tmp/mfa-native-acceptance-backup-20260827/original-profile` was `1156 KiB` with `10` files.
- SHA-256 was recorded for all 10 backup files; private settings contents and values are intentionally not included here.
- Removed only the verified empty real profile directory.
- Moved `original-profile` back to the exact `/Users/simarglok/Library/Application Support/com.simarglok.myfitanalytics` path.
- Verified the restored profile at `1156 KiB`; every one of the 10 file hashes exactly matched its pre-move SHA-256 value.
- The backup parent remains. Restoration is evidenced only as exact hash-equivalent restoration.

## Local remediation commits

- `9b4b49d fix: constrain uninstall journal paths`
- `b24e58e fix: keep fixture packages out of production bundles`
- `efead19 fix: defer committed uninstall cleanup failures`
- `ed87c2b test: confine uninstall fault injection`
- `7195f94 test: derive dashboard fixture hashes`
- `f2e4116 fix: gate fault injection behind test feature`
- `9c8b8ea test: enable fault support for integration targets`
- `42cc3d2 chore: lock test-only feature dependencies`
- `9f9da5d fix: rescan inbox for stable source files`
- `f52c731 fix: preserve queued scans during shutdown`
- `95237d8 test: cover queue scheduler invariants`
- `475c11b fix: permit Wasmtime executable memory`
- `f352efd fix: allow DuckDB extension loading`

Earlier related local commits remain unchanged:

- `0b6324c fix: make module uninstall crash recoverable`
- `5474fb3 fix: align bundled resources with uninstall lifecycle`
- `54d76ed fix: seal macOS app resources with ad hoc signing`
- `e7245a7 test: derive synthetic manifest hashes`
- `bbf304d docs: record remediation evidence and blockers`

## Fresh focused RED/GREEN evidence

All focused regressions below were run from the remediation code. The earlier RED probes were intentional test-first failures; the final packaged/native evidence is recorded on implementation commit `f352efd`.

- Malicious external journal sentinel: RED `101`, GREEN `0`.
- Symlinked journal sentinel: GREEN `0` after the path-boundary guard; the test rejects the unsafe path before mutation.
- Post-backup-delete sync restart regression: RED `101`, GREEN `0`.
- Journal-clear restart regression: RED `101`, GREEN `0`.
- Full package lifecycle after both fixes: `26 passed, 0 failed`, exit `0`.
- Runtime contract support after fixture-hash correction: `5 passed, 0 failed`, exit `0`.
- Tauri lifecycle command suite after fault-scope correction: `10 passed, 0 failed`, exit `0`.
- Manager-required exact Tauri integration command at `7555b58`: RED `101` because `cfg(test)` did not propagate to library dependencies.
- Self-dev-dependency correction: the exact unchanged Tauri command GREEN `0`; host lifecycle command GREEN `0` without feature flags.
- Final correction release-surface probe: default-feature consumer compile failed as expected because `UninstallFinalizationFault` is not exported; workspace release build GREEN `0`.
- Final packaged/native acceptance on `f352efd`: GREEN; the native lifecycle, autonomous archive move, healthy Storage state, clean quit, and post-quit DB+WAL query all passed.
- Autonomous startup rescan correction: the exact end-to-end regression was RED `101`, then GREEN `0` after the periodic queue scheduler.
- Packaged Wasmtime signing verifier: RED before `allow-unsigned-executable-memory`; GREEN after the entitlement was embedded and asserted.
- Packaged DuckDB signing verifier: RED with `allow-unsigned-executable-memory=true` but missing `disable-library-validation`; GREEN after the entitlement was embedded and asserted.

## Final verification matrix on f352efd

The final artifact/build evidence below is from the acceptance run on implementation commit `f352efd`. Final native acceptance is green; the native lifecycle and post-quit DB+WAL details are recorded above.

### Rust/workspace gates

- `cargo fmt --all --check`: **PASS, exit 0**.
- `cargo clippy --workspace --all-targets -- -D warnings`: **PASS, exit 0**.
- `cargo test --workspace -- --test-threads=1`: **PASS, exit 0**; `queue_semantics` 7, `package_lifecycle` 26, `module_lifecycle_commands` 10, and `source_modules_gate` 4 all green.
- `cargo build --workspace --release`: **PASS, exit 0**; sequential rerun after the verifier.

### Final mandatory command results

Each command below exited `0`:

- `cargo test -p mfa-source-contract-tests` — **PASS, exit 0**.
- `cargo test -p mfa-source-mynetdiary` — **PASS, exit 0**.
- `cargo test -p mfa-source-hevy` — **PASS, exit 0**.
- `cargo test -p mfa-integration-tests --test source_modules_gate -- --test-threads=1` — **PASS, exit 0**; 4 passed.
- `cargo test -p mfa-integration-tests --test provider_selection -- --test-threads=1` — **PASS, exit 0**; 1 passed.
- `cargo test -p myfitanalytics --test module_lifecycle_commands -- --test-threads=1` — **PASS, exit 0**; 10 passed.
- `cargo test -p mfa-ingestion -- --test-threads=1` — **PASS, exit 0**; queue 7, end-to-end 10, all green.
- `pnpm --dir web test -- --run SettingsPage` — **PASS, exit 0**; 4 files / 10 tests.
- `cargo tauri build --bundles app` — **PASS, exit 0**; app bundle green, ad-hoc signing, notarization skipped truthfully.
- After the app-only rebuild, strict `codesign` verification and the both-entitlement verifier exited `0`; the packaged resource layout contained exactly the two modules, resource hashes equaled the `dist` package hashes, and executable/package hashes were unchanged.

### Frontend, fixtures, and packages

- `pnpm --dir web test --run`: **PASS**; 4 files / 10 tests green.
- `pnpm --dir web check`: **PASS**; 0 errors, 0 warnings.
- `pnpm --dir web build`: **PASS, exit 0**.
- Fixture privacy: **PASS**; 7 BIFF + 2 CSV green.
- Deterministic packages: **PASS**; package bytes and hashes verified.

### Tauri app, DMG, and signing

- Full `cargo tauri build`: **PASS, exit 0**; app + DMG green.
- Strict signing verifier: **PASS**; valid on disk and satisfies its Designated Requirement; `com.apple.security.cs.allow-unsigned-executable-memory=true`; `com.apple.security.cs.disable-library-validation=true`.
- `hdiutil verify` on the final DMG: **VALID**.

Final artifact SHA-256 values:

- App executable `Contents/MacOS/myfitanalytics`: `ecea38dbb965124a4577a1a9f71a80e4aa70d54305b58529f426f285205dbcf7`
- DMG: `834d70603627201385790d07ab16d57a836055b226cbcccfd746f256617ebf46`
- MyNetDiary package: `3871f17503080c308111741c0c2202ade91ef8e9e2584cc3cf2a00e7387fefa6`
- Hevy package: `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379`

### Repository hygiene

- `git diff --check`: **PASS, exit 0**.
- `git diff --cached --check`: **PASS, exit 0**.
- Standard TODO/TBD/FIXME/PLACEHOLDER marker scan: one false-positive substring only inside `pnpm-lock.yaml` integrity hash; no actionable marker.
- Tracked-file name scan: no exports, profiles, credentials, secrets, tokens, `.env`, or temporary acceptance artifacts; the only name matches were legitimate `pnpm-workspace.yaml` and `scripts/check-workspace.sh`.
- Privacy scan: **PASS**.
- Acceptance baseline merge-base: exact `5e6f3b17c25ee52905985155e442adb028fed84a`.
- Before this evidence amendment, `origin/main...HEAD` divergence at implementation commit `f352efd` was `0` behind / `31` ahead.
- No real exports, credentials, profiles, `.env` files, or derived personal data were opened or added. Only checked-in synthetic fixtures and isolated temporary test roots were used.

## Final native acceptance and explicit exclusions

Final packaged native acceptance is green on implementation commit `f352efd`. The first and second isolated packaged runs, including picker-driven lifecycle, disabled-import protection, uninstall/reinstall, clean quit, and unchanged post-quit DB+WAL history/provenance, are documented in the native acceptance section above.

- The final native lifecycle is no longer pending manager rerun.
- Notarization was explicitly skipped because no Apple credentials were configured; no notarization claim is made.
- Remote push, PR update, merge, and all other GitHub mutations were not performed.

## Implemented

- Canonical, symlink-rejecting uninstall journal reconstruction.
- Durable staged uninstall with pre-commit rollback and post-commit deferred cleanup.
- Restart-safe committed cleanup semantics and fault coverage.
- Clean production package staging and exact MyNetDiary/Hevy resource allowlist.
- Shared PackageInstaller validation and deterministic package/hash verification.
- Release-scoped test fault injection.
- Fixture-derived synthetic manifest hashes with no former fixed sentinel.
- Autonomous periodic rescans that preserve two readable stability observations and terminate with queue shutdown.
- Narrow hardened-runtime entitlements for Wasmtime executable memory and DuckDB dynamic extension loading, enforced by the packaged signing verifier.
- Fresh full Rust, frontend, package, signing, DMG, resource, privacy, and native lifecycle verification.

## Remaining work

1. No product remediation remains for this scope after final native acceptance on `f352efd`.
2. Any remote handoff requires separate explicit authorization; this session did not push or mutate PR #4.

## Local cleanliness and handoff

This is a documentation-only follow-up to implementation commit `f352efdc18d2fc10c5d4dda0b28ff4182530bdf5` (`f352efd`). This amendment contains only the scoped documentation files `README.md` and `docs/superpowers/evidence/source-modules.md`; its exact SHA and final clean-worktree status are taken from post-commit Git output rather than inferred here.
