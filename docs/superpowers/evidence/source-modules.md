# Source Module Remediation Evidence

## Scope and disposition

This record covers the Terra High round-1 `CHANGES_REQUIRED` remediation in `/private/tmp/MyFitAnalytics-source-modules`. The implementation is **ready for Terra round 2 review**, subject to the two previously blocked external GUI gates remaining blocked by instruction. No push, PR mutation, merge, tag, release, or repository-settings change was performed.

- Repository: `https://github.com/Simarglok/MyFitAnalytics.git`
- Worktree: `/private/tmp/MyFitAnalytics-source-modules`
- Branch: `feat/source-modules`
- Baseline/merge-base: `5e6f3b17c25ee52905985155e442adb028fed84a`
- Initial verified branch/origin head: `ba019d84b9bf975cefebada9d3041a4fa3ee27cb`
- Implementation HEAD before this evidence commit: `7195f94`
- Existing PR: `#4`, `main <- feat/source-modules`, Draft; intentionally preserved.
- Terra status: round 1 was `CHANGES_REQUIRED`; this remediation is prepared for an independent round 2 review, which remains pending.

## Historical baseline (not fresh acceptance evidence)

The pre-remediation record at `ba019d8` identified:

- uninstall cleanup that could report failure after permanently deleting recoverable bytes;
- journal recovery that trusted serialized filesystem paths;
- wildcard resource packaging that could include stale fixture packages;
- missing sealed outer-app resources;
- native UI automation blocked by macOS Accessibility/Assistive Access;
- plain full Tauri app+DMG packaging blocked in the host’s Finder AppleEvent path.

Those results are retained as provenance only. The fresh results below are from the Terra remediation commits and do not retry the external GUI gates.

## Terra round-1 findings and fixes

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

### 4. Failure injection was scoped out of release API

Fix:

- Added the explicit `test-support` Cargo feature.
- The fault enum, installer fault field/builder/check method, crate re-export, and Tauri state setter are compiled only for `test-support`, tests, or debug assertions.
- Production release builds without `test-support` do not contain the fault field or public fault enum/re-export.
- Normal debug integration tests retain the existing real fault-injection coverage; no test was disabled or weakened.

Evidence:

- `cargo build -p mfa-module-host --release`: **PASS, exit 0** with test-support disabled.
- Full debug workspace tests and Tauri lifecycle tests continue to exercise all fault controls.

### 5. Remaining synthetic hash sentinel

Fix:

- `crates/mfa-module-host/tests/support/mod.rs` now derives the dashboard fixture entrypoint hash from the fixture bytes instead of declaring a fixed dummy sentinel.

Evidence:

- Exact former dashboard hash-sentinel scan scoped to implementation/test files (excluding this evidence and governing plans): 0 matches.
- The two broader marker matches are pre-existing prose in the governing plan documents, not implementation or test dummy values:
  - `docs/superpowers/plans/2026-08-25-myfitanalytics-desktop-release.md:142`
  - `docs/superpowers/plans/2026-08-25-myfitanalytics-foundation.md:444`

## Local remediation commits

- `9b4b49d fix: constrain uninstall journal paths`
- `b24e58e fix: keep fixture packages out of production bundles`
- `efead19 fix: defer committed uninstall cleanup failures`
- `ed87c2b test: confine uninstall fault injection`
- `7195f94 test: derive dashboard fixture hashes`

Earlier related local commits remain unchanged:

- `0b6324c fix: make module uninstall crash recoverable`
- `5474fb3 fix: align bundled resources with uninstall lifecycle`
- `54d76ed fix: seal macOS app resources with ad hoc signing`
- `e7245a7 test: derive synthetic manifest hashes`
- `bbf304d docs: record remediation evidence and blockers`

## Fresh focused RED/GREEN evidence

All focused regressions below were run from the remediation code. The earlier RED probes were intentional test-first failures; no diagnostic is invented where the old run output was not retained.

- Malicious external journal sentinel: RED `101`, GREEN `0`.
- Symlinked journal sentinel: GREEN `0` after the path-boundary guard; the test rejects the unsafe path before mutation.
- Post-backup-delete sync restart regression: RED `101`, GREEN `0`.
- Journal-clear restart regression: RED `101`, GREEN `0`.
- Full package lifecycle after both fixes: `26 passed, 0 failed`, exit `0`.
- Runtime contract support after fixture-hash correction: `5 passed, 0 failed`, exit `0`.
- Tauri lifecycle command suite after fault-scope correction: `10 passed, 0 failed`, exit `0`.

## Fresh non-GUI verification matrix

All commands in this section were run after the five Terra remediation commits and before this evidence commit.

### Full Rust/workspace gates

- `cargo test --workspace -- --test-threads=1` — **PASS, exit 0**; all workspace unit, integration, lifecycle, source-module, storage, security, runtime, and doc-test targets passed. The relevant final output included 26 package lifecycle tests, 7 package-security tests, 5 runtime-contract tests, 10 Tauri lifecycle-command tests, 4 MyNetDiary source integration tests, and 4 Hevy/MyNetDiary source-module gate tests.
- `cargo fmt --all --check` — **PASS, exit 0**.
- `cargo clippy --workspace --all-targets -- -D warnings` — **PASS, exit 0**.
- `cargo build -p mfa-module-host --release` — **PASS, exit 0**; release configuration excludes test fault support.

### Fixtures and package artifacts

- `pnpm --dir scripts/fixtures run verify` — **PASS, exit 0**; `verified 7 BIFF fixtures and 2 CSV fixtures; privacy scan passed`.
- `bash scripts/verify-module-packages.sh` — **PASS, exit 0**; two clean production builds, exact-layout assertion, deterministic byte comparison, manifest/hash validation, and forbidden guest-marker checks passed.
- `bash scripts/build-module-packages.sh` — **PASS, exit 0** during the contamination-sequence and app build.

Deterministic package SHA-256 values:

- MyNetDiary: `3871f17503080c308111741c0c2202ade91ef8e9e2584cc3cf2a00e7387fefa6`
- Hevy: `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379`

### Frontend gates

- `pnpm --dir web test -- --run SettingsPage` — **PASS, exit 0**; 4 files, 10 tests.
- `pnpm --dir web test -- --run` — **PASS, exit 0**; 4 files, 10 tests.
- `pnpm --dir web check` — **PASS, exit 0**; 0 errors, 0 warnings.
- `pnpm --dir web build` — **PASS, exit 0**.
- `pnpm run test && pnpm run check && pnpm run build` — **PASS, exit 0**.

The Vite build output includes existing rolldown-vite experimental/deprecation notices; the tests, Svelte check, and builds still exit successfully.

### Tauri app/resources/signing

- `cargo tauri build --bundles app` — **PASS, exit 0**; production package build ran first, then the app was built and ad-hoc signed with identity `"-"`.
- `codesign --verify --deep --strict --verbose=2 target/release/bundle/macos/MyFitAnalytics.app` — **PASS, exit 0**; valid on disk and satisfies its Designated Requirement.
- Exact app resource assertion — **PASS, exit 0**; `Contents/Resources/modules` contains only `hevy.mfasource` and `mynetdiary.mfasource`, with no guest package and no `_up_` directory.
- Resource `cmp` assertion — **PASS, exit 0**; packaged MyNetDiary and Hevy bytes match `dist/modules` exactly.

Fresh final artifact hashes:

- App executable `Contents/MacOS/myfitanalytics`: `3ebde9b84ef3075d9f1521c6cb5bf1cfcdc2ccc79371299c097c3123e13e0c19`
- Packaged MyNetDiary: `3871f17503080c308111741c0c2202ade91ef8e9e2584cc3cf2a00e7387fefa6`
- Packaged Hevy: `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379`

### Privacy, marker, and Git checks

- Tracked files scanned: `232`.
- Exact former dashboard hash-sentinel scan scoped to implementation/test files: **0 matches**.
- Broader marker scan: **2 pre-existing plan-document prose matches**, listed above; no implementation/test marker matches.
- Private-key/API-token pattern scan: **0 matches**.
- Credential-like tracked filenames: **0 matches**.
- `git diff --check`: **PASS, exit 0**.
- Initial remediation-session `git fetch origin --prune`: **PASS, exit 0**; canonical origin and remote branch provenance were unchanged.

No real exports, credentials, profiles, `.env` files, or derived personal data were opened or added. Only checked-in synthetic fixtures and isolated temporary test roots were used.

## External gates intentionally not retried

These are not claimed as fresh PASS results.

1. **Plain `cargo tauri build` app+DMG gate:** prior verified result was exit `1` during the Tauri DMG bundling path. It was not retried in this remediation because the external Finder/AppleEvent packaging gate was already blocked and the user explicitly prohibited retrying it. The app-only build, signing, resource exactness, and non-GUI artifact checks above are separate evidence.
2. **Native UI lifecycle gate:** prior exact macOS error was `System Events got an error: osascript is not allowed assistive access. (-1728)`. It was not retried. The packaged executable launch/quit smoke result does not substitute for picker-driven install/update/disable/re-enable/uninstall/import UI evidence.

## Implemented

- Canonical, symlink-rejecting uninstall journal reconstruction.
- Durable staged uninstall with pre-commit rollback and post-commit deferred cleanup.
- Restart-safe committed cleanup semantics and fault coverage.
- Clean production package staging and exact MyNetDiary/Hevy resource allowlist.
- Shared PackageInstaller validation and deterministic package/hash verification.
- Release-scoped test fault injection.
- Fixture-derived synthetic manifest hashes with no former fixed sentinel.
- Fresh full Rust, frontend, package, signing, resource, privacy, and non-GUI app verification.

## Not implemented / still blocked

- The plain full Tauri app+DMG gate remains externally blocked and was intentionally not retried.
- The native user-facing UI lifecycle remains externally blocked by missing macOS Accessibility/Assistive Access authority and was intentionally not retried.
- Independent Terra High round-2 review is pending.
- Remote push, PR update, merge, and all other GitHub mutations were not performed.

## Remaining work

1. Terra performs the independent round-2 review against this HEAD/evidence.
2. If Terra finds no further issues, the separately authorized remote handoff can be performed by the manager; this session did not push or mutate PR #4.
3. If external authority becomes available in a future authorized session, run the two blocked GUI gates; no product workaround is indicated by this remediation.

## Local cleanliness and handoff

The implementation commits are complete and the evidence update is the only remaining local documentation commit at handoff. Final exact `HEAD`, ahead count, and clean-worktree status must be taken from the post-evidence `git status`/`git rev-parse` output rather than inferred from this file.
