# Analytics dashboard evidence

## Scope

This file records the Plan 4 analytics/dashboard work, its non-foreground
production-path gate, historical packaged-native acceptance, and the current
Draft verification status. The previously approved Settings hydration and
fail-closed profile guard/runbook corrections are historical records from source
revision `41bbf5f686bb996000c424064913a123c6d19987`. The earlier provider
correction at `892e1c8ea6ce1ce751a56df4558d11656f5e1f2c` and its
full/package/DMG/hygiene evidence are historical and labeled separately below.
The current bounded UI correction was verified against baseline
`8ea3f0774a46dc24a3a90bc1e3cc6183ddde6261` and parent
`bed42a76179efbc9cebe1600eac8d58cf2a1ac3e`; no future amended hash is used as
evidence. Historical product/package evidence remains separately labeled.

**Authorization note (2026-08-31):** For the exact proposed replacement, the
human replied `Разрешаю`. This replaces only the old native “Update clears
incompatibility” expectation; all other acceptance requirements and the
genuine-incompatibility automated tests remain unchanged.

## Current bounded UI correction checkpoint — 2026-08-31

The bounded UI correction was verified against baseline
`8ea3f0774a46dc24a3a90bc1e3cc6183ddde6261` (parent
`bed42a76179efbc9cebe1600eac8d58cf2a1ac3e`). The product/test scope is limited
to two confirmed defects:

- successful Settings mutations now refresh the AppShell/dashboard snapshot
  through the existing typed transport, preserve local Settings/Phase/Source
  page selection and explicit provider consent, and reject older in-flight
  bootstrap/catalog/workspace responses;
- a rejected Sources & quality retry now keeps the last-good failure rows and
  Retry action visible, reports an operation-specific retry error, and keeps
  list-load errors separate while retaining last-good rows.

The genuine manager RED artifact for the stale shell is
`/private/tmp/mfa-plan4-ui-manager.GjHxmr/settings-red.log`; it records the
real mounted AppShell/Settings/Navigation regression before the production
patch. The implementer RED artifact for rejected Retry is
`/private/tmp/mfa-plan4-ui-fix.Sy7B6h/sources-quality-red.log` (exit 1: the
expected failure row rendered zero rows). Post-fix generation-guard mutation
proofs are retained at
`/private/tmp/mfa-plan4-ui-fix.Sy7B6h/app-shell-generation-mutation-red.log`
and
`/private/tmp/mfa-plan4-ui-fix.Sy7B6h/settings-generation-mutation-red.log`;
both failed with the older response visible when the relevant guard was
temporarily removed, and the guards were restored before subsequent checks.

Focused GREEN evidence is 22/22 tests across the three affected files. The
interim complete web run is 63/63 tests in 13 files; Svelte check reports 0
errors and the 2 existing `AppShell.svelte` warnings. The fresh logs are under
`/private/tmp/mfa-plan4-ui-fix.Sy7B6h/`; the manager also retained interim
copies under `/private/tmp/mfa-plan4-ui-manager.GjHxmr/`. Final manager
automatic source/web verification passed all 33 intended rows across two
ledgers: `gates.tsv` passed 21 rows through `workspace`, while
`continuation-gates.tsv` passed 12 rows from `rustfmt` through
`diff-unchanged`. The original `disk-before-rustfmt` exit 70 remains preserved
as an interrupted historical attempt; `head-unchanged` and byte-for-byte
`diff-unchanged` passed. Separate app/DMG package-gate, DMG audit,
correction-round hygiene, and corrected native retest remain **PENDING**/
**NOT RUN**. Scoped cleanup is retained in
`/private/tmp/mfa-plan4-ui-manager.GjHxmr/passed-test-binaries-cleanup.log` and
`/private/tmp/mfa-plan4-ui-manager.GjHxmr/debug-build-cache-cleanup.log`: first
only 19 validated rebuildable Mach-O test executables, then only the isolated
worktree's regenerable `target/debug/build` cache. No source, dependency,
profile, backup, release artifact, or evidence log was removed.

The manager-recorded native run remains **FAIL** for this correction boundary:
after explicit module updates, the module cards showed `Enabled`/no Update,
but the shell still showed `Not configured` and pending module updates; after
explicit `Use for activity.days`, Overview showed `WaitingForData` while
navigation remained `MissingCapability`, with a later explicit Refresh clearing
the stale shell state. The rejected retry of synthetic `invalid.xls` (the
missing-required-sheet fixture) produced `retry_failed` and replaced the
single failure row/action with `Unable to load quality items`. Successful
native observations were fixture import, Overview coverage `4/31`, stable
`Attention` equal to 1 after repeated Refresh, one visible quality failure row,
normal application Quit/process absence, retained synthetic aggregate checks,
and exact six-root guarded restoration. The full native page walkthrough and
Phase events CRUD were not completed. The fixed-source native retest staging
root `/private/tmp/mfa-plan4-ui-retest.aWTdix` was not run and remains
**NOT RUN/PENDING**, not PASS. The detailed native record is
`/private/tmp/mfa-plan4-native-approved.P6u51Q/manager-evidence.md`.

Historical verification for source revision `41bbf5f686bb996000c424064913a123c6d19987`:

- The full-gate run passed all 33 rows with exit 0. The workspace reported 266
  tests and 0 ignored; Vitest reported 56 tests in 13 files; Playwright reported
  5 passed tests; the profile guard reported 20 passed tests and the independent
  guard probe reported 2 passed tests; Svelte check reported 0 errors and 2
  pre-existing warnings.
- The package-gate run passed all 9 rows with exit 0: fresh app and full DMG,
  signing with the 2 required entitlements, strict code-signature verification,
  `hdiutil` verification, exact byte equality for the 3 packaged module
  resources against `dist`, and unchanged source HEAD.
- The following artifact hashes are historical records from that revision:

  | Artifact          | SHA-256                                                            |
  | ----------------- | ------------------------------------------------------------------ |
  | `.app` executable | `a569639e5e529ffca9a0e658b2965e74be5cd46c1da9f732c861e6c6b792b74b` |
  | DMG               | `da55e1e49e379520ddc093bccc86b3b6bfd14686c62f005b2527daefbc579f76` |
  | Base dashboard    | `13a11f972e93c8bfd51b6e371fb8cef62f45a0887bb4339b1f0b93badf89d901` |
  | Hevy source       | `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379` |
  | MyNetDiary source | `79a8c96594a95e508fc5cae95057323528d3f180af9e8f3c25bf472b635fc56c` |

The recorded automated/package verification involved no app launch or
real-profile read/mutation. No new dependency or OS privilege was added, and no
remote write was performed. Secret filename/key scans had no hits; the marker
scan matched only existing documentation and a `pnpm` lock-integrity false
positive.

The approved current packaged-native criterion is the manual procedure in
`docs/testing/plan4-packaged-native-manual.md`; it remains **NOT RUN/PENDING**
for the current product, so no packaged-native acceptance, interactive
assertions, screenshot, or app launch is claimed. The separate app/DMG
package-gate, DMG audit, and correction-round hygiene also remain **PENDING**.
A final external Terra High review remains
**PENDING** and requires new informed authorization; the product is not
classified Ready.

Historical pre-correction package evidence anchored at
`bed42a76179efbc9cebe1600eac8d58cf2a1ac3e` remains separate. Its earlier
packaging attempts are not current evidence: an initial `cargo tauri build
--bundles app` exited 0; the first full packaging attempt,
`CI=true cargo tauri build -vv`, exited 1 during sandbox dependency resolution;
an approved `pnpm install --frozen-lockfile` using the local dependency store
exited 0 without changing the lockfile or package manifests; the next
`cargo tauri build --ci -vv` attempt exited 1 during sandbox DMG image creation;
and an approved cached-dependency packaging rerun exited 0 and produced both
app and DMG. Its historical app binary SHA-256 was
`41a90db23fc22755ff13394ddc9940adefe6eb1a9d20764b6ffcebb5cc3f96ed`, and its
historical DMG SHA-256 was
`f6e56b75e856a56760b01380fcf1f24919817497b7977e9a45649ed0c44ac97e`.
Historical offline strict signing/entitlements, bundle-resource equality,
embedded-app verification, and DMG checksum verification remain historical;
signing was ad hoc and not notarized.

A prior manual run at `273997a` is historical only: it passed workspace/import
visibility and a stable one-attention observation but failed Sources & quality
because the page was empty despite the attention item. That native run remains
**FAIL** and is never retroactively **PASS**; the old test runner remains
disabled. The separate XCUITest contract is deferred in
`docs/superpowers/tasks/2026-08-29-xcuitest-native-acceptance.md`. No
third-party foreground-control daemon or macOS permission change is required by
the current criterion. Plan 5 (desktop lifecycle, tray/background behavior, and
release distribution) remains deferred.
Native acceptance must use the existing OS user with only the exact six roots
isolated to a fresh empty surface and checked-in synthetic fixtures only; no
HOME override, real export, private record, credential, or remote operation is
allowed.

Historical pre-recovery note (2026-08-31): before the approved recovery, the old
runner was guarded-aborted with exit 1. The audit recorded Application Support
tree hash mismatching, with 13 files present and no file mtime newer than the
before-manifest, Preferences matching, Caches and WebKit mismatching, Saved
Application State and HTTPStorages absent, and the cause unknown. This records
the pre-recovery state only; it is not native acceptance evidence.

As of 2026-08-31, the app is stopped and the explicitly approved copy-based
recovery is complete within its authorized scope. The available Application
Support copy was restored; its normalized tree hash matches the chosen
available source, **not** the original baseline. Preferences were restored
hash-identically to the original baseline and passed syntax validation. Live
Caches and WebKit were first copied and verified; the live originals were then
moved into private quarantine, while the pre-restore copies remain separately
retained. Prior live Preferences were preserved the same way before restoration.
New empty live directories were created. Originally absent Saved Application
State and HTTPStorages remain absent. All original
backups, prior recovery variants, the failed-run snapshot, and the original
before-manifest were preserved unchanged. Independent recheck verified the
restoration, preserved copies, empty directories, absent roots, and unchanged
baseline. No deletion or app launch occurred; the cause of the original
mismatch remains unknown. This recovery does not satisfy the historical
hash-identical acceptance gate: full original-baseline six-root equivalence
remains unproven.

All Task 7 runs use checked-in synthetic MyNetDiary and Hevy fixtures, a fresh
`tempfile` app-data/workspace root, the bundled `.mfasource`/`.mfadashboard`
packages, and one serial database actor.

## Plan 4 task record

| Plan 4 task                                      | Implementation evidence                                                                      | Verification boundary                                                                                                         |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| 1. Deterministic base analytics                  | `c7b29ba697c20c7a5d09f1fcb980910c25a15ca` (`feat: compute deterministic base analytics`)     | Existing `mfa-analytics` weight, nutrition, activity, and strength golden suites                                              |
| 2. Rolling TDEE, phase events, coverage          | `c6d65970b1dcf9fd58d86b27c688b518e1a0b982` (`feat: calculate gated rolling TDEE`)            | Existing TDEE and phase-event suites; Task 7 re-queries after a persisted phase event                                         |
| 3. Sandboxed base dashboard module               | `c2e1e91b9054e2dd0fe9391a2a8778bf90f23378` (`feat: add sandboxed base dashboard module`)     | Base dashboard document/security golden suites and packaged invocation                                                        |
| 4. Dashboard host/capability and locale boundary | Foundation/source-module implementation commits                                              | Host package, dataset, validator, locale, and runtime-limit suites                                                            |
| 5. Command analytics/provider integration        | `a4727858ff627d28b2750e3b67dc782bd071933c` (`feat: expose analytics and provider selection`) | Tauri command contract and Task 7 provider-selection path                                                                     |
| 6. Local Svelte dashboard UI                     | `ad1835583f39f31e211a03dbdc400508dae5836e` (`feat: render local analytics dashboard`)        | Accepted frontend verification: 8 Vitest files / 25 tests, ESLint, Svelte check, Vite build, Prettier, and 5 Playwright tests |
| 7. Production-path end-to-end gate               | This change                                                                                  | `bash scripts/run-dashboard-gate.sh`, plus the focused commands below                                                         |

## Current Plan 4 requirement-to-evidence checklist

| Requirement                                                                                                                                                           | Current evidence boundary                                                                                                                                                           |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Deterministic weight, nutrition, activity, and strength analytics                                                                                                     | `crates/mfa-analytics/tests/*_golden.rs`; semantic assertions in `crates/mfa-integration-tests/tests/dashboard_gate.rs`                                                             |
| Rolling TDEE, coverage, phase exclusion, and persisted phase events                                                                                                   | `crates/mfa-analytics/tests/tdee_golden.rs`; `crates/mfa-analytics/tests/phase_events.rs`; `src-tauri/tests/dashboard_commands.rs`; production gate re-query                        |
| Sandboxed declarative dashboard and safe document validation                                                                                                          | `crates/mfa-dashboard-host/tests/{document_security,dependency_isolation}.rs`; `modules/dashboards/base/tests/{conformance,document_golden}.rs`; packaged invocation in Task 7 gate |
| Availability precedence, dependency isolation, capability/provider selection, and locale boundary                                                                     | `crates/mfa-dashboard-host/tests/availability.rs`; module-host capability/locale/runtime suites; Tauri provider and command-contract tests                                          |
| Typed Tauri-to-Svelte dashboard shell, settings, source quality, refresh, and phase-event UI                                                                          | `web/src/**/*.test.ts`; `src-tauri/tests/{dashboard_commands,ingestion_commands,module_lifecycle_commands}.rs`; 5 Playwright scenarios                                              |
| Serial production-path import, archive, active snapshots, all six pages, semantic goldens, and DB lifecycle                                                           | `bash scripts/run-dashboard-gate.sh`; `crates/mfa-integration-tests/tests/dashboard_gate.rs`                                                                                        |
| Ingestion/actionability correction: stable current failures, idempotent Refresh, success clearing, deterministic counts, safe retry/Open Settings, and no auto-update | Focused `mfa-ingestion` and `myfitanalytics` correction tests; typed status/quality transport tests                                                                                 |
| Packaged-native manual interaction and screenshots                                                                                                                    | `docs/testing/plan4-packaged-native-manual.md`; current result is **NOT RUN/PENDING**                                                                                               |

The exact historical RED stdout for Tasks 1–6 was not retained in the current
session database. It is not reconstructed here. The table identifies the
accepted implementation commits and the fresh Task 7 RED/GREEN evidence below.

## Task 7 RED evidence

The gate was written before the corresponding wiring and was run serially. The
following failures were observed for the expected missing boundary:

| Boundary                 | Command/result                                                                             | Observed failure                                                                                                                                                                                                          |
| ------------------------ | ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Application dependency   | `cargo test -p mfa-integration-tests --test dashboard_gate -- --test-threads=1` / exit 101 | The new gate could not resolve the application command crate from `mfa-integration-tests`.                                                                                                                                |
| Snapshot discovery       | Same focused command / exit 101                                                            | Archive/ingestion completed, but dashboard availability reported `waiting_for_data`, `observed_days: 0`, `expected_days: 31`; the production command queried only the module ID rather than active logical snapshot keys. |
| Fresh package dispatch   | Same focused command / exit 101                                                            | The stale packaged base module returned the overview title for a non-overview page.                                                                                                                                       |
| Localization allowlist   | Same focused command / exit 101                                                            | The freshly dispatched base pages emitted keys missing from the command validator allowlist.                                                                                                                              |
| Structured analytics     | Same focused command / exit 101                                                            | The command-returned nutrition card had no structured `days` analytics for the semantic golden.                                                                                                                           |
| Wasmtime safety boundary | Same focused command / exit 101                                                            | Passing all full 59-day activity series through repeated declarative cards reached `module_memory_limit`; the fix retained the 64 MiB limit and compacted non-null activity points at the command boundary.               |
| Phase overlay            | Same focused command / exit 101                                                            | The post-save re-query had no persisted `phaseEvents` payload before the command wiring was restored.                                                                                                                     |

A diagnostic run independently verified the ingestion boundary before the
snapshot fix: MyNetDiary archive inventory was 1, Hevy archive inventory was 2,
all three inbox files had been deleted after successful commit, and ingestion
health was normal. These facts prevented the gate from masking a parser or
archive failure as a dashboard failure.

## Prior Task 7 GREEN evidence

The focused gate passed after each prior Task 7 RED boundary was fixed. This
section is historical; correction-round-2 results are recorded below.

```text
cargo test -p mfa-integration-tests --test dashboard_gate -- --test-threads=1
running 1 test
test production_dashboard_gate_imports_fixtures_and_queries_every_base_page ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The gate asserts, through the real application command services:

- MyNetDiary and Hevy package discovery, source-module invocation, immutable
  archive inventory, canonical ingestion, active logical snapshot discovery,
  and post-success inbox deletion;
- all six base page IDs, title keys, declarative block keys, coverage state, and
  freshness metadata;
- semantic golden values for weight observations/daily medians/trailing mean/
  slope, nutrition quality and trailing calories, activity steps/event
  aggregates/means/heart rate/water, strength session windows/working sets/e1RM,
  and TDEE coverage/readiness;
- exact provider selection (`body.weight` → `hevy`);
- all seven `AvailabilityResolver` states with stable precedence;
- phase-event save/list behavior, persisted phase overlay after a fresh page
  command, and TDEE `excluded_days` changing from the fixture's pre-phase value
  to its post-phase value;
- volatile phase IDs and generated timestamps only as shape checks, never as
  semantic golden values.

The reviewed semantic fixture is
`web/e2e/fixtures/expected-dashboard.json`. It contains the activity heart-rate
128 bpm, water 500 ml, unknown-event count, navigation IDs, provider choice,
phase fields, and TDEE pre/post exclusion values consumed by the assertions.

## Exact Task 7 checks

The runner rebuilds the checked-in module packages and then runs the serial
production-path gate:

```bash
bash scripts/run-dashboard-gate.sh
```

The focused Cargo command is:

```bash
cargo test -p mfa-integration-tests --test dashboard_gate -- --test-threads=1
```

Prior runner result: `bash scripts/run-dashboard-gate.sh` exited `0`; package
builds completed and `production_dashboard_gate_imports_fixtures_and_queries_every_base_page`
reported 1 passed, 0 failed. This is not the correction-round-2 result.

The relevant frontend commands are:

```bash
pnpm --dir web test -- --run
pnpm --dir web exec svelte-check --tsconfig ./tsconfig.json
pnpm --dir web exec eslint .
pnpm --dir web exec prettier --check .
pnpm --dir web build
PLAYWRIGHT_BROWSERS_PATH=<session-owned-temp-cache>/playwright pnpm --dir web exec playwright test web/e2e/dashboard.spec.ts
```

Fresh results: Vitest exited `0` with 8 files/25 tests passed; Svelte check
exited `0` with 0 errors and 2 existing warnings in `AppShell.svelte`; ESLint
exited `0`; Prettier exited `0`; Vite build exited `0`; and the isolated-cache
Playwright command exited `0` with 5 tests passed. The no-cache Playwright
attempt exited `1` because this checkout intentionally uses the session-owned
browser cache; it did not indicate a test failure. Task 7 does not claim native foreground acceptance; the historical packaged
native acceptance is recorded in the historical Correction round 4 section
below.

The exact serialized full-workspace command was first RED after the Plan 4
phase-event migration was present: the old schema-version assertions in
`mfa-db/tests/migrations.rs` and `mfa-ingestion/tests/end_to_end.rs` still
expected 3 while the production migration path correctly reported 4. The
focused corrections updated those expectations and added version-4
`user_phase_event` schema coverage. The focused migration test then passed 5/5,
and the duplicate-ingestion regression passed 1/1 (9 filtered).

The exact serialized full-workspace command was then rerun:

```bash
cargo test --workspace -- --test-threads=1
```

It exited `0`: all workspace unit, integration, and doctests passed, including
the corrected migration and ingestion suites and the production dashboard gate.

## Correction round 2 evidence

Terra re-review identified six scoped findings. The following RED/GREEN records
were run against checked-in synthetic fixtures and packaged components only:

| Finding                               | RED evidence                                                                                                                                                                                                           | Scoped correction / GREEN evidence                                                                                                                                                              |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Packaged activity invoke failure      | `bash scripts/run-dashboard-gate.sh` reached the activity page and returned the typed `dashboard.module_error.module_invoke_error`; the direct packaged test also exposed the missing `heart_rate.observations` input. | Activity cards now consume separate declared datasets, the direct fixture supplies heart rate, and the gate progressed through all page/analytics assertions without the Wasmtime invoke error. |
| Rust/Tauri dashboard JSON mismatch    | Frontend transport tests lacked Rust snake_case document and typed module-error coverage.                                                                                                                              | Vitest: `pnpm --dir web test -- --run` exited `0` (8 files, 27 tests), including actual Rust-shaped Document and ModuleError cases; malformed content still renders the safe unavailable panel. |
| `activity.days` capability contract   | The production gate reported an incompatible activity provider contract after the activity capability split.                                                                                                           | Source manifest, component metadata, bundled defaults, dashboard requirements, and emitted datasets now agree; the gate reached all six page documents.                                         |
| Optional body-fat analytics           | Base dashboard RED: `body_page_renders_optional_body_fat_series_without_weight_substitution` failed because no body-fat chart existed.                                                                                 | Analytics body-fat suite exited `0` (2 tests); dashboard-base body-fat regression exited `0`; command assembly uses `body_fat_pct` and preserves nulls rather than weight values.               |
| Governed exercise keys                | Strength RED: an unmapped `external` exercise entered working sets.                                                                                                                                                    | Strength focused regression exited `0`; only the checked-in governed external mapping is accepted and source exercise names remain unchanged.                                                   |
| DashboardBlock wrapper unknown fields | Validator RED: a wrapper-level `onClick` field was accepted.                                                                                                                                                           | Dashboard-host focused validator regression exited `0`; wrapper fields are rejected before document deserialization.                                                                            |

The final correction-round-2 gate was run with `bash scripts/run-dashboard-gate.sh`
after the scoped fixture/contract updates. It reached the TDEE coverage
assertion with the stale expectation (`complete_nutrition_days`: actual `0`,
expected `1`); correction round 3 updated that fixture expectation to the exact
February window semantics recorded below.

Focused Rust suites already run in this correction round include
`cargo test -p mfa-analytics` (exit `0`), the dashboard-base focused suite
(body-fat regression exit `0`), the dashboard-host wrapper regression (exit
`0`), and the packaged gate invocations described above. No native foreground
acceptance or production app launch had been done at the time of this
correction-round-2 record; the later packaged acceptance is recorded below.
Plan 5 work and remote mutation were not done.

## Historical correction round 3 evidence — fixture corrections

Terra confirmed three stale test/golden fixtures. No product logic was changed
in this round. RED evidence was captured before the fixture edits:

| Boundary                        | RED result                                                                                                                                                            |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Packaged base runtime fixture   | `cargo test -p mfa-module-host --test base_dashboard` — exit `101`; `missing_capability_input: missing capability activity.events`.                                   |
| Base dashboard document goldens | `cargo test -p mfa-dashboard-base --test document_golden` — exit `101`; shared ready input/golden still expected the old nutrition object shape.                      |
| Production gate                 | Prior round’s exact-window TDEE expectation was stale: February 1–28 contains the two February weights, no January 4 nutrition day, and no January 15–16 phase dates. |

The minimal GREEN corrections were:

- added the declared `activity.events` input to the packaged component fixture;
- updated the shared dashboard ready input and its dependent overview,
  nutrition, and sources goldens to distinct `days`, trailing-mean, and TDEE
  fields;
- updated all affected exact-window TDEE expectations consistently: zero
  complete nutrition days, two weight days, two first-seven-day weight days,
  zero last-seven-day weight days, slope available, and zero pre/post phase
  exclusions.

Fresh final results:

```text
cargo test -p mfa-module-host --test base_dashboard
test result: ok. 2 passed; 0 failed

cargo test -p mfa-dashboard-base --test document_golden
test result: ok. 6 passed; 0 failed

bash scripts/run-dashboard-gate.sh
test production_dashboard_gate_imports_fixtures_and_queries_every_base_page ... ok
test result: ok. 1 passed; 0 failed

cargo fmt --all --check                         # exit 0
git diff --check                                # exit 0
```

Round 3 touched only test/golden fixtures and this evidence file; at that time
it did not launch the native app. The subsequent packaged acceptance is
recorded below and used only a fresh test profile and synthetic workspace; no
personal profile or real workspace was accessed, no Plan 5 work was run, and
no remote was mutated.

## Historical correction round 4 evidence — Settings, bundled refresh, and packaged native acceptance

The correction was implemented in the pre-review Plan 4 feature commit: a top-level Settings route
parallel to Phase events, Sources & quality limited to quality/retry behavior,
and hash-aware bundled package refresh. The packaged native acceptance below
was completed on that pre-review commit before the evidence-only amend.

The Settings correction:

- adds visible top-level Settings navigation parallel to Phase events;
- routes Settings to `SettingsPage`, which owns the workspace and source-inbox
  choosers;
- removes the embedded Settings controls from Sources & quality; and
- keeps the typed chooser backend commands unchanged.

The bundled package refresh correction:

- retains the previous bundled catalog before writing the new catalog;
- automatically rotates a same-version hash only when the active package
  identity exactly matched the previous bundled catalog entry and the module
  was not disabled or uninstalled;
- preserves custom active packages, disabled modules, uninstalled modules, and
  unrelated provider state;
- surfaces an ambiguous same-version mismatch as an explicit `Update`; and
- makes an explicit update activate the exact selected catalog hash.

The focused package lifecycle test was run RED before implementation:
`cargo test -p mfa-module-host --test package_lifecycle -- --test-threads=1`
exited `101`. The GREEN package lifecycle suite then passed 30 tests, and
`cargo test -p myfitanalytics --test module_lifecycle_commands --
--test-threads=1` passed 12 tests.

Fresh independent gates for the pre-review Plan 4 feature commit:

| Check                                                         | Result                                                       |
| ------------------------------------------------------------- | ------------------------------------------------------------ |
| `cargo test --workspace -- --test-threads=1`                  | exit `0`                                                     |
| `pnpm --dir web test -- --run`                                | exit `0`; 8 files/33 tests passed                            |
| `pnpm --dir web exec prettier --check .`                      | exit `0`                                                     |
| `pnpm --dir web exec eslint .`                                | exit `0`                                                     |
| `pnpm --dir web exec svelte-check --tsconfig ./tsconfig.json` | exit `0`; 0 errors and 2 existing `AppShell.svelte` warnings |
| `cargo fmt --all --check`                                     | exit `0`                                                     |
| `cargo clippy --workspace --all-targets -- -D warnings`       | exit `0`                                                     |
| `bash scripts/run-dashboard-gate.sh`                          | exit `0`                                                     |
| `git diff --check`                                            | exit `0`                                                     |

Native packaging and launch acceptance:

- `cargo tauri build --bundles app` succeeded and was ad-hoc signed; it was
  not notarized.
- `cargo tauri build --bundles dmg -vv` succeeded at
  `target/release/bundle/dmg/MyFitAnalytics_0.1.0_aarch64.dmg`.
- The app was rebuilt afterward at
  `target/release/bundle/macos/MyFitAnalytics.app`.
- The native run used only the checked-in synthetic `valid-full.xls`,
  `measurement_data.csv`, and `workout_data.csv`, with workspace
  `$ACCEPTANCE_ROOT/workspace`.
- The fresh profile initially showed `Healthy` / `Not configured`, three
  bundled modules `Enabled`, and Overview `Waiting for data`; it showed no
  `[[object Object]]`, raw JSON, or `Incompatible Contract`.

The fresh packaged accessibility tree showed Settings in the top-level
Analytics navigation. The Settings page showed Choose Workspace, Choose inbox
for Hevy and MyNetDiary, provider controls, language, and module lifecycle
controls. The top-level Settings chooser selected only the synthetic workspace,
and the derived Hevy and MyNetDiary inbox paths were shown.

Refreshing archived all three fixture files and emptied the inboxes. The UI
auto-refreshed to Healthy, 0 active jobs, 0 attention items, Latest observation
`Feb 3 2026`, Overview Coverage `1/31 Limited`, Body weight and Nutrition
`Available`, and `Data is ready`. Body, Nutrition, Activity, and Strength each
rendered typed cards and readiness messages, with no raw payloads.

The Phase events route rendered its form. A synthetic event was saved with
type `synthetic phase`, start `2026-01-01`, end `2026-01-01`, and
`exclude_from_tdee true`; the UI listed it, and returning to Overview
succeeded.

After a clean quit, an independent DuckDB+WAL query returned:

| Table                    | Count |
| ------------------------ | ----: |
| `source_asset`           |     3 |
| `source_receipt`         |     3 |
| `ingestion_attempt`      |     3 |
| `logical_snapshot`       |     3 |
| `active_snapshot`        |     3 |
| `nutrition_item`         |     2 |
| `body_measurement`       |     2 |
| `activity_day`           |     1 |
| `heart_rate_observation` |     1 |
| `workout_session`        |     1 |
| `exercise_set`           |     4 |
| `user_phase_event`       |     1 |

The latest phase row was
`synthetic phase|2026-01-01|2026-01-01|true`.

The hash guard protected six real roots. Baseline and after matched exactly and
stably:

| Root                                     | Digest                                                                                 |
| ---------------------------------------- | -------------------------------------------------------------------------------------- |
| Application Support                      | `053f6b2ae908b90a27de7f548155f52ae59ab474b5c7b61141dad5729572ed4f`                     |
| Caches                                   | `a750c3dca81b7dbbbf63e0ca798f1ef3224b125804ae07a528dc7975bbf8db24`                     |
| WebKit                                   | `97cbbe2844d594d61118ee750b34ef0f5e86fe50a6676c43`                                     |
| Preferences                              | `f1e7c74da98f6a565968c4e481b3943d4ca19ec4a3d06731b8e15705261e3a0a`                     |
| Saved Application State and HTTPStorages | both absent; digest `47e9c196252b5866d81bcf29627f6fe50b54428ec30ed736998e139d60e10a9a` |

The test profile was removed and the Russian input source was restored. No
personal exports or real workspace were read, and no remote mutation occurred.

## Post-native review correction round 1

This round addresses the Terra High findings discovered after the packaged
acceptance above. It changes runtime and UI behavior, so the packaged/native
results in Correction round 4 remain historical pre-correction evidence. The
current HEAD packaged-native acceptance is recorded in the current status
section below and remains Draft/INCOMPLETE. This file does not reinterpret the
historical Correction round 4 results.

In particular, the earlier `Coverage 1/31 Limited` and `Data is ready` pairing
is retained only as a historical pre-correction capture; it is not the
corrected availability behavior. The current HEAD packaged-native acceptance
is not Ready; its exact blocker is recorded in the current status section below.

The scoped corrections are:

- production dashboard invocation now validates the raw JSON document with the
  shared strict validator before returning a typed document; unknown top-level
  and block-wrapper fields are rejected;
- host-computed availability state is carried into base dashboard composition,
  so insufficient coverage cannot produce a contradictory ready status; the
  typed availability action is preserved through transport and rendered as a
  localized Settings-routed control for known safe actions, or explicit
  guidance for unknown actions;
- navigation owns the initial dashboard range, ending at the latest available
  observation or using the local-calendar current date when no observation
  exists; the frontend consumes that range and has no production fixture-date
  fallback; and
- the base dashboard routes only from `DashboardInput.page_id`; the obsolete
  `capabilities["dashboard.page"]` fallback is removed; and the dashboard
  authoring guide no longer describes `dashboard.page` as a grantable
  capability.

Focused RED/GREEN evidence for this round:

| Boundary                            | RED                                                                                                                                                                                                                               | GREEN                                                                                                                                                                    |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Production raw dashboard validation | `cargo test -p mfa-module-host --test runtime_contract dashboard_component_rejects_unknown_raw_document_and_block_fields -- --test-threads=1` exited `101` because wrapper fields were accepted by the direct typed deserializer. | The focused runtime contract suite and dashboard-host security suite passed after the runtime called the shared raw validator.                                           |
| Base availability state             | `cargo test -p mfa-dashboard-base --test document_golden non_ready_availability_is_not_reported_as_ready_by_any_page -- --test-threads=1` exited `101` before `DashboardInput.availability_state` existed.                        | The focused base regression passed for Overview, Body, Nutrition, Activity, Strength, and Sources.                                                                       |
| Availability action UI              | `pnpm --dir web test -- --run src/lib/components/AvailabilityPanel.test.ts` exited `1` because the panel rendered no action control.                                                                                              | The focused component regression passed with the localized Import data control and callback propagation.                                                                 |
| Backend initial range               | `cargo test -p myfitanalytics --test dashboard_commands navigation_uses_the_injected_local_date_when_no_observation_exists -- --test-threads=1` was RED during removal of the synthetic navigation seam.                          | The no-observation command regression, two injected-clock helper cases, and the production gate's latest-observation window assertion passed.                            |
| Frontend initial range              | `pnpm --dir web test -- --run src/lib/stores/dashboard.svelte.test.ts` exited `1` because the store still initialized Jan 2026.                                                                                                   | The focused store initialization regression passed using the backend-provided range.                                                                                     |
| Base page routing                   | `cargo test -p mfa-dashboard-base --test document_golden compose_json_ignores_the_ungrantable_dashboard_page_capability -- --test-threads=1` exited `101` because the obsolete capability fallback routed to Body.                | The same regression passed after `page_id` became the sole production routing input; `git grep` found no `dashboard.page` reference under `modules/dashboards/base/src`. |

The production-boundary latest-observation assertion was added after the
backend range implementation was already present in the correction tree. Its
serial command was:

```text
cargo test -p mfa-integration-tests --test dashboard_gate -- --test-threads=1
test production_dashboard_gate_imports_fixtures_and_queries_every_base_page ... ok
test result: ok. 1 passed, 0 failed
```

With the checked-in fixtures, the latest observation is `2026-02-03`, so the
backend returned the inclusive 31-day initial range `2026-01-04` through
`2026-02-03`. The no-observation RED/GREEN record above remains the evidence
for the local-date fallback branch.

The correction-round focused tests use checked-in synthetic fixtures or
injected dates only. They do not constitute the current packaged/native
acceptance recorded below.

## Safety and truthfulness boundary

- No dashboard guest receives filesystem paths, source bytes, SQL, credentials,
  or undeclared capability/extension data.
- Dashboard runtime defaults remain 64 MiB memory, 10,000,000 fuel, a 2-second
  epoch timeout, 1 MiB output, and eight store instances/tables.
- `entrypoint_hash` is a SHA-256 integrity/digest check, not publisher signing.
  Release signing and notarization are separate packaging controls. The
  pre-correction Plan 4 packaged native acceptance is historical as recorded
  above. The current approved manual packaged-native criterion is not yet run;
  the fresh app was ad-hoc signed, notarization was not performed, and Plan 5
  remains deferred.
- The gate is serial and uses one actor-owned database connection.
- README status, the manual runbook, and the XCUITest task intentionally
  separate completed Plan 4 automation from the not-yet-run manual criterion,
  deferred XCUITest work, and deferred Plan 5.

## Historical post-correction packaged native acceptance — prior artifact baseline

Packaged-native acceptance was previously recorded against the code/artifact
baseline `74697bc0b851b7ef75d5369c189174e246e735b6`, which was the code branch
tip at that time. This section is historical and is not the current HEAD
acceptance status. No product code, tests, configuration, or packaged artifact
claims in this historical section are reclassified by the current Draft status.
Plan 5 and notarization remain out of scope.

### Packaging and artifact verification

- Fresh `cargo tauri build` succeeded and produced
  `target/release/bundle/macos/MyFitAnalytics.app` and
  `target/release/bundle/dmg/MyFitAnalytics_0.1.0_aarch64.dmg`.
- `codesign --verify --deep --strict --verbose=2` passed. Signing was ad hoc,
  not notarized, as expected and out of scope.
- DMG SHA-256:
  `cd9b9009c1cc99a0306418fec276beb5a257deb7993849b006a7438815637c4b`.
- `hdiutil verify` passed.
- Acceptance root:
  `$ACCEPTANCE_ROOT` (ephemeral evidence; no
  personal data).

### Synthetic workspace acceptance

Only the checked-in synthetic fixtures were copied: `valid-full.xls`,
`measurement_data.csv`, and `workout_data.csv`. The exact selected workspace UI
was
`$ACCEPTANCE_ROOT/workspace`; the exact selected
source inboxes were under that workspace root.

Refresh consumed all three inbox files. The UI became `Healthy`, with `0`
active jobs and `0` attention items; Latest observation was `Feb 3, 2026`;
the dashboard reported `More data is needed` / `insufficient coverage`; and
Body weight and Nutrition showed `Available`.

The synthetic phase event was created with:

```text
type: round3 persistence
dates: 2026-08-28 through 2026-08-28
description: packaged native cancel-confirm acceptance
Exclude from TDEE: checked
```

A clean restart loaded the event automatically without Refresh. Delete opened
an accessibility-visible `AXApplicationDialog` described `Confirm phase event
deletion`, named the event, and exposed explicit `Confirm delete` and `Cancel`
controls. Cancel closed the dialog and preserved the event. Reopening the
dialog and choosing `Confirm delete` removed it; a subsequent clean restart
showed `No phase events recorded.` The app cleanly quit and was not running
afterward.

### Profile guard and restoration

The hash-verified profile guard covered six roots. Baseline and restored hashes
matched exactly:

| Root                                     | Digest / state                                                                    |
| ---------------------------------------- | --------------------------------------------------------------------------------- |
| Application Support                      | `053f6b2ae908b90a27de7f548155f52ae59ab474b5c7b61141dad5729572ed4f` (13 files)     |
| Caches                                   | `a750c3dca81b7dbbbf63e0ca798f1ef3224b125804ae07a528dc7975bbf8db24` (2 files)      |
| WebKit                                   | `97cbbe2844d594d611f85922d011c5b61118ee750b34ef0f5e86fe50a6676c43` (8 files)      |
| Preferences                              | `f1e7c74da98f6a565968c4e481b3943d4ca19ec4a3d06731b8e15705261e3a0a` (1 file)       |
| Saved Application State and HTTPStorages | absent; digest `47e9c196252b5866d81bcf29627f6fe50b54428ec30ed736998e139d60e10a9a` |

The guard manifest reports `restored=true` and exact `before==after`. The test
profile was preserved under the acceptance root, and the original user
profile was restored.

### Independent verification on the acceptance target baseline

- Targeted frontend verification: 2 files / 11 tests passed.
- Full web verification: 12 files / 44 tests passed.
- Svelte check: 0 errors and 2 pre-existing `AppShell` warnings.
- Prettier and `git diff --check` were clean.
- The same Rust tree had already passed full `cargo test`, Clippy, rustfmt,
  and the production dashboard gate. This final correction was frontend-only.

## Current product verification status — Draft / INCOMPLETE

The persisted Settings hydration and fail-closed profile guard corrections are
recorded in source revision `41bbf5f686bb996000c424064913a123c6d19987`; the
full-gate, package-gate, and artifact-hash records at the top of this file are
historical for that earlier revision. The earlier narrow provider correction at
`892e1c8ea6ce1ce751a56df4558d11656f5e1f2c` and its fresh full/package/DMG/
hygiene results are also historical. The current bounded UI correction was
verified against baseline `8ea3f0774a46dc24a3a90bc1e3cc6183ddde6261` (parent
`bed42a76179efbc9cebe1600eac8d58cf2a1ac3e`). Final manager automatic source/web
verification passed all 33 intended rows across `gates.tsv` and
`continuation-gates.tsv`; the original `disk-before-rustfmt` exit 70 remains a
separate interrupted attempt, while `head-unchanged` and byte-for-byte
`diff-unchanged` passed. Separate app/DMG package-gate, DMG audit,
correction-round hygiene, and corrected native retest remain **PENDING**/
**NOT RUN**. This status remains **Draft/INCOMPLETE**, not Ready. The recorded
automated/package verification for `892e1c` involved no app launch, interactive
assertions, or screenshots. As
of 2026-08-31,
the explicitly approved copy-based
recovery is complete within its authorized scope: the available Application
Support copy matches its chosen available source after normalization, **not**
the original baseline; Preferences match the original baseline hash and pass
syntax validation; live Caches and WebKit were first copied and verified, the
live originals were then moved into private quarantine, and the pre-restore
copies remain separately retained; prior live Preferences were preserved the
same way before restoration; and new empty live directories were created;
the originally absent roots remain absent; and all recovery variants and
baselines were preserved unchanged. The independent recheck verified this state.
The old runner remains disabled, the earlier native run remains **FAIL** and is
never retroactively **PASS**, and full original-baseline six-root equivalence
remains unproven. The manager-recorded native run for the pre-fix UI behavior is
also **FAIL** for stale shell state and rejected Retry hiding the quality row;
The fixed-source native retest remains **NOT RUN/PENDING**, not PASS. A final
read-only Terra High review remains **PENDING** pending new informed
authorization.

### Implemented

- The dashboard date-range correction is recorded at `273997a`; the current
  product correction adds typed current-failure/quality/retry diagnostics,
  stable source-module namespaces, immediate success clearing, and an empty
  privacy-safe refresh event. Current failure views expose no raw payload,
  path, or error detail, and valid `asset_id` values remain independently
  constrained to archived `asset:<uuid>` identities.
- Absent required-provider selection now reports `MissingCapability` with the
  `Configure source` action in navigation and dashboard commands. After an
  explicit provider selection, including `activity.days`, a provider with no
  successful snapshot reports `WaitingForData` with `Import data`. No automatic
  provider selection or settings rewrite was added; an actually incompatible
  selected provider remains `IncompatibleContract` and retains its precedence
  over another missing capability.
- Focused RED/GREEN coverage proves that repeated Refresh operations retain one
  current attention item, successful Retry clears current attention without a
  follow-up scan, retry failure preserves a safe diagnostic, identical failure
  identities from different sources remain distinct and deterministic, and the
  existing data-changed boundary refreshes visible shell health.
- Historical pre-correction mandatory automated Rust checks passed on product
  commit `bed42a7`:

  ```text
  bash scripts/run-dashboard-gate.sh                                  # exit 0
  cargo test -p mfa-integration-tests --test source_modules_gate -- --test-threads=1  # exit 0
  cargo test --workspace -- --test-threads=1                          # exit 0
  cargo fmt --all -- --check                                           # exit 0
  cargo clippy --workspace --all-targets -- -D warnings               # exit 0
  cargo build --workspace --release                                    # exit 0; 1m27s
  ```

- Historical post-install web rerun passed Prettier, ESLint, Svelte check, Vitest,
  Playwright, and the Vite build: Vitest reported 53 passed tests, Playwright
  reported 5 passed tests, and Svelte check reported 0 errors with the two known
  `AppShell.svelte` warnings.
- Playwright `1.62.1` using Chromium revision `1234` from the session cache
  passed 5/5. An initial sandbox `EPERM` on the local port was resolved by an
  approved local-only rerun; this is automated browser evidence, not packaged
  native acceptance.
- Historical pre-correction packaging attempts and approved reruns were:
  - Initial `cargo tauri build --bundles app`: exit 0.
  - Initial full `CI=true cargo tauri build -vv`: exit 1 during sandbox
    dependency resolution.
  - Approved `pnpm install --frozen-lockfile` using the local dependency store:
    exit 0 with runtime pnpm `11.19.0`; lockfile and package manifests
    unchanged, with no package-manager upgrade.
  - `cargo tauri build --ci -vv`: exit 1 during sandbox DMG image creation.
  - Approved `CI=true npm_config_store_dir=SESSION_PNPM_STORE cargo tauri build
-vv`: exit 0; both app and DMG produced. `SESSION_PNPM_STORE` is a
    placeholder for the private session dependency store. The DMG used
    `--skip-jenkins`, so no Finder-prettifying AppleScript ran.
- Offline artifact verification passed: strict signing and both required
  entitlements, `codesign --verify --deep --strict`, DMG `hdiutil verify`, exact
  three-file bundle-resource allowlist and byte equality to `dist/modules`,
  read-only DMG attachment, embedded-app signing verification, and app/embedded
  app byte comparison. The DMG was detached successfully, and no app launch or
  notarization occurred. Signing is ad hoc. The packaging run made no profile
  changes.
- Historical pre-correction packaging hashes retained from the `bed42a7` evidence
  are:

  | Artifact          | SHA-256                                                            |
  | ----------------- | ------------------------------------------------------------------ |
  | `.app` executable | `41a90db23fc22755ff13394ddc9940adefe6eb1a9d20764b6ffcebb5cc3f96ed` |
  | DMG               | `f6e56b75e856a56760b01380fcf1f24919817497b7977e9a45649ed0c44ac97e` |

- Approved copy-based recovery completed on 2026-08-31 without app launch or
  deletion. The available Application Support copy was restored and its
  normalized tree hash matches the chosen available source, **not** the
  original baseline. Preferences were restored hash-identically to the original
  baseline and passed syntax validation. Live Caches and WebKit were first
  copied and verified; the live originals were then moved into private
  quarantine, while the pre-restore copies remain separately retained. Prior
  live Preferences were preserved the same way before restoration. New empty
  live directories were created. Originally absent Saved Application State and
  HTTPStorages remain absent. All original backups, prior recovery variants, the
  failed-run snapshot, and the original before-manifest were preserved unchanged.
  Independent recheck verified the restoration, all copies, empty directories,
  absent roots, and unchanged baseline. The cause of the original mismatch
  remains unknown. This does not satisfy the historical hash-identical
  acceptance gate; full original-baseline six-root equivalence remains unproven.

- `python3 scripts/verify_module_packages.py` passed two-build determinism and
  the exact three-production-package allowlist. The existing module-package
  SHA-256 table is unchanged:

  | Package           | SHA-256                                                            |
  | ----------------- | ------------------------------------------------------------------ |
  | MyNetDiary source | `79a8c96594a95e508fc5cae95057323528d3f180af9e8f3c25bf472b635fc56c` |
  | Hevy source       | `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379` |
  | Base dashboard    | `13a11f972e93c8bfd51b6e371fb8cef62f45a0887bb4339b1f0b93badf89d901` |

- The checked-in synthetic-fixture privacy verifier passed for 7 BIFF fixtures
  and 2 CSV fixtures. The tracked token/private-key pattern scan had no
  matches (`git grep` exit 1 for the expected no-match result); no new env,
  key, or database files were added. Product diff checking was clean.
- Residual `event_failures` belongs only to the test-only fault-injection path.
  Production `PipelineState` uses `NoFaultInjector`, whose `check` always
  returns `Ok`; it is not a source of repeated Refresh count.
- Repository provenance for this verification: `origin/main` and mergebase were
  refreshed to `4e5e2acfa19920678e99af34d03387d389ddd6d8`; PR `#5` remains Draft
  and OPEN at remote head `314d451c841517717c70a1c8377f4ae8c539519a`. The
  local product branch was ahead by 10 before this docs-only update. No remote
  mutation was performed.

## Current correction round — persisted Settings hydration and fail-closed profile guard

This revision records two approved fixes. It does not alter the historical
recovery records above and does not claim a native run.

### Settings persisted-provider hydration

- `SettingsPage.reload()` now awaits the existing typed `getBootstrapState()`
  transport call and applies its persisted `activeProviders` map on initial
  mount and every remount/reload.
- The real Tauri path is unchanged: `getBootstrapState` reads the persisted
  application state, and provider selection persists before returning. The
  mock transport now retains the complete selected mapping to model that real
  side effect; it does not infer or auto-select providers from module
  capabilities.
- Focused RED against the unchanged component:
  `cd web && pnpm exec vitest run src/lib/pages/SettingsPage.test.ts` exited
  `1` with `3 failed, 5 passed`.
- Focused GREEN after the minimal component/mock correction:
  the same command exited `0` with `8 passed`.
- The regression covers initial saved mapping, remount after explicit
  selection, and privacy-safe bootstrap failure handling. No native app launch
  or profile operation was used.

### Checked-in fail-closed profile guard

`scripts/profile_guard.py` and its executable synthetic suite replace the old
operational guard procedure. The helper never launches an app and never
deletes a tree. Its phases are capture of independent masters, pre-launch
isolation of originals into a retained original-holding tree while leaving all
six live roots absent, and post-quit variant isolation plus copy-restore into a
separate retained variant-holding tree. The manifest/hash contract includes
canonical relative names, empty directories, mode/size/mtime metadata, and
regular-file bytes; `preferences` is explicitly a file root. The native adapter
uses the fixed bundle-ID roots and real OS-home lookup; the fake-home native
builder/lifecycle test injects both home and process context and does not access
the host profile. The process identity is the actual lowercase executable
`myfitanalytics`.

Clarified RED against the prior seeded two-phase helper:
`python3 scripts/test_profile_guard.py` exited `1` with `7` assertion failures.
Clarified GREEN after the three-phase/native correction:
the same command exited `0` with `20` synthetic tests passed. The tests cover
rename independence, byte corruption, all-master verification before mutation,
partial move/copy and interruption/retry, absent roots, malformed manifests,
destination conflicts, symlinks, unsupported objects, exact process absence,
and injected fake-home/process contexts, including exact root-parent durability
and subprocess failure/empty/malformed process output. No real profile or private recovery
tree was read or modified.
An independent manager synthetic probe also passed its two checks after the
lowercase executable correction: exact process identity and the fake-home
capture/isolate/variant/restore round trip. This is not native acceptance.

### Current absent-provider correction evidence

The current correction was developed with strict command-level TDD. The
pre-correction missing-provider command regression was observed RED:

```text
cargo test -p myfitanalytics --test dashboard_commands missing_provider_selection_is_reported_in_navigation_and_dashboard -- --exact --nocapture
exit 101
```

The failure was the expected absent-provider classification: the command path
returned `IncompatibleContract` instead of `MissingCapability`.

Fresh focused GREEN checks after the one-line adapter correction are:

```text
cargo test -p myfitanalytics --test dashboard_commands -- --nocapture
exit 0 — 9 passed, 0 failed

cargo test -p myfitanalytics --lib commands::tests::incompatible_selected_provider_remains_incompatible_ahead_of_missing_capability -- --exact --nocapture
exit 0 — 1 passed, 0 failed

cargo test -p myfitanalytics --test provider_selection_commands -- --nocapture
exit 0 — 1 passed, 0 failed

cargo fmt --all -- --check
exit 0

git diff --check
exit 0
```

The command regressions use temporary roots and bundled synthetic packages. The
manager-provided focused Clippy log also exited `0`. The manager-provided
historical-update probe exited `0`: it showed `MissingCapability` before and
after both explicit module updates, then `WaitingForData` only after explicit
new-capability selection; provider state was unchanged by updates and changed
only after explicit selection. This records the human-approved current criterion for
the historical packages: initially advertised `base` and `MyNetDiary` pending
updates clear after both explicit updates, while unselected `activity.days`
remains `MissingCapability` with `Configure source` before and after. The
genuine `IncompatibleContract` behavior remains a mandatory automatic-test
boundary; this scenario does not prove native clearing of a real
incompatibility.

### Fresh final verification — frozen source revision `892e1c8`

The manager independently verified the frozen source revision
`892e1c8ea6ce1ce751a56df4558d11656f5e1f2c` with these evidence scripts:

```text
bash /private/tmp/mfa-plan4-provider-state.oKTcqN/full-gates.sh     # runner exit 0; 33/33 rows exit 0
bash /private/tmp/mfa-plan4-provider-state.oKTcqN/package-gates.sh  # runner exit 0; 9/9 rows exit 0
bash /private/tmp/mfa-plan4-provider-state.oKTcqN/audit-dmg.sh      # exit 0
bash /private/tmp/mfa-plan4-provider-state.oKTcqN/hygiene.sh        # exit 0
```

- The full-gate runner used `CARGO_NET_OFFLINE=true` and
  `CARGO_BUILD_JOBS=2`. All 33 `gates.tsv` rows exited `0`, and the runner
  exited `0`: Rust reported 269 passed, 0 ignored, and 90 result groups; Vitest
  reported 56 passed tests in 13 files; Playwright reported 5 passed tests with
  locked Playwright `1.62.1` and Chromium revision `1234` from the existing
  cache; profile guard checks reported 20 passed synthetic tests plus 2
  independent checks; Svelte reported 0 errors and 2 pre-existing `AppShell`
  warnings. Full formatting, Clippy, release, source, dashboard, provider,
  lifecycle, ingestion, web, determinism, privacy, and unchanged-source gates
  all passed.
- The package-gate runner used `CI=true`, `CARGO_NET_OFFLINE=true`, and
  `CARGO_BUILD_JOBS=2`; it built the app with `cargo tauri build --bundles app`
  and the full DMG with `cargo tauri build`. All 9 `package-gates.tsv` rows
  exited `0`, and the runner exited `0`: both required entitlements, strict
  codesign, `hdiutil verify`, exact byte equality for the 3 packaged module
  resources against `dist`, and unchanged source HEAD passed. Signing was ad
  hoc and not notarized.
- `audit-dmg.sh` exited `0`: the DMG was attached read-only without Finder, the
  embedded app passed strict signature verification, the complete app tree was
  byte-identical to the current bundle, and the image detached successfully.
- `hygiene.sh` exited `0`: secret/key/token and sensitive-filename scans had no
  matches; exactly 7 checked-in XLS fixtures and 2 CSV fixtures were checked
  and privacy-verified as synthetic fixtures; marker hits were limited to the
  old scan description and a lock-integrity substring.
- Fresh artifact hashes from `final-artifacts.sha256` are:

  | Artifact          | SHA-256                                                            |
  | ----------------- | ------------------------------------------------------------------ |
  | `.app` executable | `d0d14b89871c739aa7d001d9981db6cd25744ba420929f3b2c7fac9141eb9f0e` |
  | DMG               | `27b7d4182680f7e3a8142cb27e03a3b3bf9e94b7e66b6210277fef086af3b313` |
  | Base dashboard    | `13a11f972e93c8bfd51b6e371fb8cef62f45a0887bb4339b1f0b93badf89d901` |
  | Hevy source       | `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379` |
  | MyNetDiary source | `79a8c96594a95e508fc5cae95057323528d3f180af9e8f3c25bf472b635fc56c` |

- Read-only `gh pr view 5 --repo Simarglok/MyFitAnalytics --json number,state,isDraft,headRefOid,headRefName,baseRefName,url` recorded PR `#5`
  as OPEN and Draft at remote head `314d451c841517717c70a1c8377f4ae8c539519a`.
  A separate read-only `git ls-remote` check recorded unchanged `origin/main`
  at `4e5e2acfa19920678e99af34d03387d389ddd6d8`. No remote write was performed.
- During the recorded automated/package verification, no app was launched, no
  native GUI or profile operation occurred, and no publication or notarization
  occurred. These fresh automated/package results do not substitute for native
  acceptance.

### Current acceptance boundary

The manager-recorded P6 native UI/packaged-native run on the pre-fix package is
**FAIL** for stale shell state after Settings mutations and for rejected Retry
hiding the quality row. Its normal Quit, storage, and six-root restoration
observations are separate successful results. The fixed-source native retest
remains **NOT RUN/PENDING**, not PASS; the historical app/DMG and module hashes
recorded above are package/audit evidence only and must not be treated as a
fixed-source native result. The separate current app/DMG package-gate, DMG
audit, and correction-round hygiene remain **PENDING**. The mandatory
prior-production-package → current-package normal-install scenario must first
show pending updates named `base` and `MyNetDiary`; after explicit `Update` on
both modules those notices must disappear. For unselected `activity.days`, the
expected `MissingCapability`/`Configure source` state, visible as `Required data
is not available yet` with `Configure source`, must remain the same before and
after both updates. No automatic package update or provider selection is
allowed. Only explicit `Use` for `activity.days` may change selection and yield
`WaitingForData` with `Import data` before data arrives. This criterion remains
**PENDING**, never `N/A`; see the manager-only preflight and Russian manual
guide. It does not prove clearing a genuine `IncompatibleContract`; the
mandatory automatic incompatible-provider tests remain separate. The historical
native run remains **FAIL** and is never retroactively changed.

### Not implemented / pending

- The fixed-source native retest remains **NOT RUN/PENDING**; the fresh staging root
  `/private/tmp/mfa-plan4-ui-retest.aWTdix` was not run. The manager-recorded
  pre-fix native run is **FAIL** for stale shell state and rejected Retry hiding
  the single quality row. Its successful fixture import, `4/31` coverage,
  stable `Attention` equal to 1, one visible failure row, normal Quit/process
  absence, retained synthetic aggregate checks, and exact six-root restoration
  do not turn the failed run into a PASS. The remaining analytics-page
  walkthrough and Phase events CRUD were not completed. Historical package-gate
  and DMG audit evidence above does not substitute for the fixed-source
  manual/native retest; the separate current package-gate, DMG audit, and
  correction-round hygiene remain **PENDING**. Historical pre-correction
  packaging had ad-hoc signing and no notarization.
- A final read-only Terra High review remains **PENDING** and requires new
  informed authorization; it is not assumed approved for `892e1c8`.
- Full/package/DMG/hygiene runs for source revision `892e1c` passed as recorded
  above. For the current bounded UI correction, all 33 intended automatic
  source/web rows passed across the initial and continuation ledgers; the
  original `disk-before-rustfmt` exit 70 remains a separate interrupted attempt,
  and `head-unchanged`/`diff-unchanged` passed. The separate current
  package-gate, DMG audit, and correction-round hygiene remain **PENDING**. The
  remaining fixed-source native work is the historical-package
  scenario: pending updates must initially name `base` and `MyNetDiary`, both
  explicit module Updates must clear those notices, and unselected
  `activity.days` must remain `MissingCapability`/`Configure source` before and
  after. Only explicit `Use` may yield `WaitingForData`/`Import data`. This
  does not replace the mandatory automatic incompatible-provider tests or prove
  clearing a genuine `IncompatibleContract`.
- XCUITest is deferred to the separate planning-only follow-up task.

### Remaining work

1. Prepare a safe fresh native acceptance run without reusing the disabled old
   runner, using the checked-in capture/isolate/restore helper and a fresh
   existing OS user’s exact six roots. Retain all original and variant copies;
   do not seed the fresh live roots with recovered profile data.
2. Execute `docs/testing/plan4-packaged-native-manual.md` against the freshly
   verified current app/DMG artifacts and record every pass/fail row, or record a
   truthful `BLOCKED`/`FAIL` result.
   The acceptance checklist must retain the full goal:
   visible import without restart; bounded failed Refresh with actionable
   Sources & quality and Retry; pending updates/Open Settings initially naming
   base and MyNetDiary; explicit Update for both modules clearing those notices
   while unselected `activity.days` remains
   `MissingCapability`/`Configure source` before and after; explicit `Use` for
   `activity.days` yielding `WaitingForData`/`Import data`; every analytics page,
   gaps, providers, phase CRUD, and Plan 3 Settings; clean quit/process
   absence; aggregate DB integrity; and hash-identical six-root restore.
3. Use the proven normal `PackageInstaller`/`CapabilityRegistry` preparation,
   then run pending-update/Open Settings, verify that the initial pending names
   are `base` and `MyNetDiary`, and perform explicit `Update` for both modules.
   Verify that both pending notices disappear while unselected `activity.days`
   remains `MissingCapability` with `Configure source` before and after the
   updates. Explicitly use `activity.days` and verify the transition to
   `WaitingForData` with `Import data`; this remains **PENDING**, never `N/A`.
   The genuine `IncompatibleContract` behavior remains covered by mandatory
   automatic-provider tests and is not claimed cleared by this scenario.
4. Keep the historical hash-identical six-root acceptance gate separate from
   the approved copy-based recovery; full original-baseline six-root equivalence
   remains unproven and must not be inferred from the available-source match.
5. If native automation is desired later, obtain approval for and execute the
   separate XCUITest feasibility/task contract; it is not required for the
   current Plan 4 criterion.
6. Obtain the final read-only Terra High review of the completed manual record
   before reclassifying the product as Ready.

### Blockers

The mandatory native UI checklist and final read-only review prevent a Ready
classification. The prior manual run at `273997a` is not reusable evidence for
`bed42a7`: it passed workspace/import visibility and a stable one-attention
observation but failed Sources & quality because the page was empty despite the
attention item. That native run remains **FAIL** and is never retroactively
**PASS**; the old runner remains disabled. Approved copy-based recovery is
complete within scope, with the available Application Support copy matching its
chosen available source after normalization but **not** the original baseline,
Preferences matching the original baseline hash with valid syntax, live Caches
and WebKit first copied and verified before the live originals were moved into
private quarantine, with the pre-restore copies separately retained, prior live
Preferences preserved the same way before restoration, new empty live
directories created, and originally absent roots still absent. All
backups and recovery variants were preserved unchanged; no deletion or app
launch occurred, and the cause of the original mismatch remains unknown. The
independent recheck verified restoration, copies, empty directories, absent
roots, and unchanged baseline. Full original-baseline six-root equivalence
remains unproven; the historical hash-identical acceptance gate is unsatisfied.
No new native acceptance has occurred, and the final Terra High review remains
pending.

The current automatic source/web results are recorded above and do not substitute
for native acceptance; the separate current package/DMG/hygiene checks remain
pending. Any permission prompt or
non-synthetic input during the runbook is a stop condition requiring separate
approval, not an invitation to change permissions or add a foreground-control
daemon. Residual `event_failures` remains test-injector-only. Plan 5 remains out
of scope, XCUITest is deferred, and remote push, PR update, and merge are not
authorized.
