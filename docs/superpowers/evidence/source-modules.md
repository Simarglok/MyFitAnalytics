# Source Module Remediation Evidence

## Scope and disposition

This document separates historical evidence from the fresh remediation run. The result is **Honest Draft**, not Ready: the project-local uninstall, resource, catalog, and app-signing defects were remediated and verified, but the plain Tauri app+DMG gate and the user-facing native UI lifecycle remain externally blocked. No push, PR mutation, merge, tag, release, or settings change was performed.

- Session title: `myfitanalytics-source-modules-remediation`
- Durable session ID: `20260826_230635_32b707`
- Model/effort: `gpt-5.6-luna max`
- Repository: `https://github.com/Simarglok/MyFitAnalytics.git`
- Worktree: `/private/tmp/MyFitAnalytics-source-modules`
- Branch: `feat/source-modules`
- Baseline/merge-base: `5e6f3b17c25ee52905985155e442adb028fed84a`
- Initial verified branch/origin head: `ba019d84b9bf975cefebada9d3041a4fa3ee27cb`
- Final implementation commit before this evidence-only commit: `e7245a7`
- Final branch head after the evidence commit is reported by the final `git rev-parse HEAD` handoff check.
- Existing PR: `#4`, `main <- feat/source-modules`, Draft; intentionally preserved.

## Historical baseline (not fresh acceptance evidence)

Before remediation, the recorded state at `ba019d8` reported:

- uninstall committed module state before filesystem cleanup and suppressed delete/read/sync failures;
- packaged modules landed under `Contents/Resources/_up_/dist/modules`, while runtime lookup used `resource_dir/modules`;
- the bundled catalog was empty in the packaged app;
- the outer app had no sealed resource signature;
- `cargo tauri build` failed during DMG creation;
- native install/disable/re-enable/uninstall UI could not be completed because Accessibility permission was unavailable.

The historical evidence also records a prior Terra High review as `CHANGES REQUIRED` after three rounds. That is provenance only. No new Terra review was run in this session; independent review remains pending.

## Root causes and implemented solutions

### 1. Atomic/restorable uninstall

Root cause: uninstall mixed durable state changes with destructive package cleanup and treated critical filesystem failures as ignorable. A second installer could also observe an incomplete operation after restart without a durable recovery decision.

Solution:

- added a durable uninstall journal and staged package transaction;
- moved package bytes into a recoverable staging location before applying state;
- made read, move, delete, state-write, and directory-sync failures typed failures;
- added rollback and startup/reopen reconstruction, including corrupt-journal error handling without panic;
- made registry refresh occur before finalization through a transaction-aware no-recovery listing path, so no fallible registry rebuild occurs after the irreversible commit boundary;
- preserved disable-before-uninstall and existing archive/provenance/history boundaries;
- removed the test-only synthetic manifest hash marker and derive helper signatures from fixture bytes.

Fresh focused failure-injection coverage is green for stage move, stage sync, package delete, post-delete read, post-read version removal, post-remove sync, backup read, backup delete, state sync, restore move, restore read, restart reconstruction, and corrupt journal recovery. The complete package lifecycle suite passed with 22 tests, and full module-host tests passed.

### 2. Tauri resource/runtime contract and catalog

Root cause: the resource list preserved the repository-relative source prefix in the bundle, producing `_up_/dist/modules`, while runtime looked under the stable resource root.

Solution:

- changed the Tauri resource configuration to map `../dist/modules/*.mfasource` to bundle-relative `modules/`;
- made bootstrap resolve `resource_dir/modules` only;
- require MyNetDiary and Hevy resources to exist and be regular files instead of silently filtering missing files;
- run bundled resources through the same `PackageInstaller` inspection/validation path as user-selected packages;
- verify the bundled catalog contains MyNetDiary and Hevy and preserve update/uninstall catalog behavior.

Fresh deterministic package output and packaged-resource byte comparisons are green. The signed app contains `Contents/Resources/modules/{mynetdiary,hevy,guest-source}.mfasource`; `_up_` is absent.

### 3. macOS app signing and DMG packaging

Root cause: no Tauri macOS signing identity was configured, so the linker-signed executable did not produce a sealed outer app resource signature. Separately, the standard create-dmg script invokes Finder AppleEvents for visual layout; this host is not authorized for those AppleEvents.

Solution:

- configured `bundle.macOS.signingIdentity` as `"-"`, which makes Tauri perform an ad-hoc outer-app signing pass;
- retained the standard Tauri/create-dmg path without weakening verification;
- produced a separate headless diagnostic DMG with the generated script's documented `--skip-jenkins --no-internet-enable` options only for artifact validation.

The app strict-signature gate is green. The exact plain full-build gate remains blocked at DMG layout customization, not at app compilation, resource assembly, hdiutil creation, or signing.

### 4. Production native lifecycle

The packaged executable launched under a fresh temporary HOME/profile with no debug flags, stayed alive without an immediate crash, installed the bundled MyNetDiary and Hevy package state, and exited via a normal AppleScript application quit with process exit 0.

The required native UI lifecycle could not be credibly automated. The exact macOS result was:

`System Events got an error: osascript is not allowed assistive access. (-1728)`

No retry was made. Therefore native workspace picker, package picker, live install/update/disable/re-enable/uninstall, production import, and user-facing archive/inbox lifecycle remain required-but-incomplete. The backend/native command tests and source integration tests are evidence for their scopes only and do not substitute for this UI gate.

## Focused RED/GREEN evidence

All listed RED runs were intentionally induced before the corresponding implementation or guard; exit `101` means the focused Cargo test failed as expected. Exact diagnostics for some early probes were not retained; no diagnostic text is inferred here.

- `backup_read_failure_restores_staged_package_and_previous_state`: RED `101`, restored guard, GREEN `0`.
- `backup_delete_failure_restores_package_and_previous_state`: RED `101`, restored guard, GREEN `0`.
- `stage_move_failure_preserves_package_and_previous_state`: RED `101`, restored guard, GREEN `0`.
- `post_stage_move_sync_failure_restores_package_and_previous_state`: RED `101`, restored guard, GREEN `0`.
- `state_sync_failure_preserves_package_and_previous_state`: RED `101`, restored guard, GREEN `0`.
- `recovery_restore_move_failure_keeps_the_transaction_for_a_clean_restart`: RED `101`, restored guard, GREEN `0`.
- `recovery_restore_read_failure_keeps_the_transaction_for_a_clean_restart`: RED `101`, restored guard, GREEN `0`.
- post-delete/read, post-read/remove, and post-remove/sync uninstall probes: RED `101`, restored boundary, GREEN `0`.
- `corrupt_uninstall_journal_is_reported_without_panicking_during_recovery`: RED before structured parsing, GREEN `0` after error propagation.
- `tauri_bundles_source_packages_under_the_runtime_modules_directory`: RED `101` against the old resource array, GREEN `0` after the source-to-`modules/` map.
- `bundled_package_paths_require_mynetdiary_and_hevy`: RED `101` before the helper existed, GREEN `0` after required-resource validation.
- native lifecycle command suite initially exposed three failures from refreshing the registry after finalization; moving refresh before finalization through a no-recovery transaction listing made the full serial suite GREEN.
- `registry_refresh_failure_rolls_back_before_uninstall_finalization`: RED `101` before the seam consumed the injected failure, GREEN `0` after rollback-safe ordering and the `BeforeRegistryRefresh` guard.
- strict app signing: RED `1` with `code has no resources but signature indicates they must be present`, GREEN `0` after configuring ad-hoc outer-app signing.
- the marker scan initially found one test-helper marker; after deriving synthetic manifest hashes from fixture bytes, the committed scan is GREEN with zero matches.

## Fresh mandatory verification matrix

Every command below was run in this remediation session. Exit codes are explicit.

### Fixtures and deterministic packages

- `pnpm --dir scripts/fixtures run verify` — **PASS, exit 0**; verified 7 BIFF fixtures and 2 CSV fixtures; fixture privacy scan passed.
- `bash scripts/build-module-packages.sh` — **PASS, exit 0**.
- `bash scripts/verify-module-packages.sh` — **PASS, exit 0**; deterministic MyNetDiary and Hevy package verification passed.

Deterministic package SHA-256 values:

- MyNetDiary: `3871f17503080c308111741c0c2202ade91ef8e9e2584cc3cf2a00e7387fefa6`
- Hevy: `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379`

### Rust source, integration, and lifecycle tests

- `cargo test -p mfa-source-contract-tests` — **PASS, exit 0**; 1 test.
- `cargo test -p mfa-source-mynetdiary` — **PASS, exit 0**; 6 tests.
- `cargo test -p mfa-source-hevy` — **PASS, exit 0**; 5 tests.
- `cargo test -p mfa-integration-tests --test source_modules_gate -- --test-threads=1` — **PASS, exit 0**; 4 tests.
- `cargo test -p mfa-integration-tests --test provider_selection -- --test-threads=1` — **PASS, exit 0**; 1 test.
- `cargo test -p myfitanalytics --test module_lifecycle_commands -- --test-threads=1` — **PASS, exit 0**; 10 tests.
- `cargo test -p mfa-module-host` — **PASS, exit 0**; unit, capability, locale, lifecycle, security, runtime-contract, and runtime-limit suites all passed, including 22 lifecycle tests.
- `cargo test --workspace -- --test-threads=1` — **PASS, exit 0**.
- `cargo fmt --all --check` — **PASS, exit 0**.
- `cargo clippy --workspace --all-targets -- -D warnings` — **PASS, exit 0**.

### Frontend and root commands

- `pnpm --dir web test -- --run SettingsPage` — **PASS, exit 0**; 4 files and 10 tests.
- `pnpm --dir web test -- --run` — **PASS, exit 0**; 4 files and 10 tests.
- `pnpm --dir web check` — **PASS, exit 0**; 0 errors and 0 warnings.
- `pnpm --dir web build` — **PASS, exit 0**.
- `pnpm run test && pnpm run check && pnpm run build` — **PASS, exit 0**; root scripts completed all three operations.

### Tauri app, resources, signing, and DMG

- `cargo tauri build --bundles app` after the signing configuration — **PASS, exit 0**; produced the app below and logged `Signing with identity "-"`.
- `cargo tauri build` after resource and signing fixes — **BLOCKED, exit 1**; Rust/app compilation and app signing completed, then Tauri failed while running `bundle_dmg.sh`.
- Default create-dmg reproduction with the generated script — **BLOCKED, exit 64**; the optional Finder AppleEvent customization returned `Finder got an error: AppleEvent timed out. (-1712)`. The standard hdiutil create/resize/attach/detach/convert stages succeed when the script is run with its headless skip option.
- `codesign --verify --deep --strict --verbose=2 target/release/bundle/macos/MyFitAnalytics.app` — **PASS, exit 0**; valid on disk and satisfies its designated requirement.
- Tauri bundle inspection — **PASS, exit 0**; `Contents/Resources/modules` contains `guest-source.mfasource`, `hevy.mfasource`, and `mynetdiary.mfasource`; no `Contents/Resources/_up_` path.
- pre/post package comparison using `cmp` for both packages — **PASS, exit 0**; bundled bytes are identical to `dist/modules` bytes.
- headless generated diagnostic command `bash target/release/bundle/dmg/bundle_dmg.sh --skip-jenkins --no-internet-enable target/release/bundle/dmg/MyFitAnalytics_0.1.0_aarch64.dmg target/release/bundle/macos/MyFitAnalytics.app` — **PASS, exit 0** as diagnostic artifact production only, not the plain Tauri DMG gate.
- `hdiutil verify` plus read-only attach, module-layout assertion, and detach — **PASS, exit 0** for the diagnostic DMG; checksum is valid and all three packaged module files are present.

Absolute diagnostic artifacts:

- App: `/private/tmp/MyFitAnalytics-source-modules/target/release/bundle/macos/MyFitAnalytics.app`
- DMG: `/private/tmp/MyFitAnalytics-source-modules/target/release/bundle/dmg/MyFitAnalytics_0.1.0_aarch64.dmg`

SHA-256 values from the final artifact validation:

- app executable `Contents/MacOS/myfitanalytics`: `6aaa71e5bbfb6c8edd50fd2f6a499ebd8a055cff858294889e348db91f30a395`
- app MyNetDiary package: `3871f17503080c308111741c0c2202ade91ef8e9e2584cc3cf2a00e7387fefa6`
- app Hevy package: `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379`
- diagnostic DMG: `76344fa9e91f74adff3cb25d57fb3de6ae8474b1abda223c940da37abdc7f9f6`

The package hashes before resource bundling and after extraction from the signed app match exactly. The diagnostic DMG is not represented as a pass for the mandatory plain `cargo tauri build` gate.

### Isolated production launch and native UI authority

- Fresh packaged executable launch with isolated temporary HOME/profile — **PASS, exit 0** for process startup; process remained alive without immediate crash.
- Fresh-profile backend state inspection — **PASS, exit 0**; active bundled module IDs were `hevy` and `mynetdiary`.
- `System Events` window/accessibility inspection — **BLOCKED, exit 1**; macOS returned `osascript is not allowed assistive access (-1728)`.
- normal `tell application "myfitanalytics" to quit` — **PASS, exit 0**.
- isolated process post-quit check — **PASS, exit 0**; process exited normally.
- Full user-facing native lifecycle — **REQUIRED BUT INCOMPLETE** because Accessibility/Screen Recording authority was unavailable. No picker, live lifecycle, or production UI import claim is made.

### Marker, privacy, and Git checks

- committed changed-file forbidden-marker scan — **PASS, exit 0**; 0 matches.
- full tracked-index privacy scan — **PASS, exit 0**; 232 tracked files, 0 suspicious secret filenames, 0 secret-token matches.
- added-diff privacy-pattern scan — **REVIEWED**; matches were policy wording about exports/credentials, not credentials or data.
- `git diff --check 5e6f3b17c25ee52905985155e442adb028fed84a...HEAD` — **PASS, exit 0**.
- `git fetch origin --prune` and provenance checks — **PASS, exit 0**; canonical origin unchanged and `origin/feat/source-modules` remained `ba019d84b9bf975cefebada9d3041a4fa3ee27cb`.
- No real exports, credentials, profiles, `.env` files, or derived personal data were opened or added. Only checked-in synthetic fixtures were used.

## Implemented

- Durable, rollback-safe, restart-reconstructable uninstall with real filesystem failure injection and regression coverage.
- Stable `Contents/Resources/modules` Tauri resource contract and required MyNetDiary/Hevy bundled catalog.
- Shared PackageInstaller validation for bundled and user-selected package paths.
- Ad-hoc outer-app signing with strict sealed-resource verification.
- Deterministic package, resource, integration, Rust, frontend, and native-command verification.
- Fresh isolated packaged launch and normal application quit evidence.
- This evidence document, committed separately after the verification run.

## Not implemented / required but incomplete

- The mandatory plain `cargo tauri build` app+DMG gate is not green because the host blocks Finder AppleEvents during create-dmg customization. The headless diagnostic DMG is explicitly not substituted for that gate.
- The mandatory user-facing native UI lifecycle is not green because macOS Accessibility/Screen Recording authority is unavailable. No unsafe bypass, debug protocol, or filesystem backdoor was added.
- A new independent Terra High review is pending; the historical CHANGES REQUIRED result must not be treated as approval.
- Push and PR update were not performed by instruction; PR #4 remains Draft.

## Remaining work

1. Run the plain `cargo tauri build` in an interactive macOS session with the required Finder AppleEvent/Accessibility authority, or otherwise provide an approved external headless packaging authority, and retain the mandatory command as the gate.
2. Grant the required macOS Accessibility/Screen Recording permissions in the authorized test session and run the real native picker/install/update/disable/re-enable/uninstall/import lifecycle with synthetic data.
3. Obtain the independent read-only Terra High review, address any findings with follow-up commits, then let the manager perform any separately authorized remote handoff.

## Local commits and cleanliness

- `0b6324c fix: make module uninstall crash recoverable`
- `5474fb3 fix: align bundled resources with uninstall lifecycle`
- `54d76ed fix: seal macOS app resources with ad hoc signing`
- `e7245a7 test: derive synthetic manifest hashes`
- Evidence-only follow-up commit: the commit containing this file, verified immediately after writing.

The final handoff reports the exact post-evidence `HEAD`, branch status, and confirms no push or PR mutation.
