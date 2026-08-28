# Analytics dashboard evidence

## Scope

This file records the Plan 4 analytics/dashboard work, its non-foreground
production-path gate, historical packaged-native acceptance, and the current
HEAD verification status. The automated Rust/web gates, deterministic package
checks, and fresh app/DMG build checks are green at the current HEAD. The
approved current packaged-native criterion is the manual procedure in
`docs/testing/plan4-packaged-native-manual.md`; it is **NOT RUN** for the
authoritative current HEAD, so no current interactive assertions or screenshots
are claimed. The separate XCUITest contract is deferred in
`docs/superpowers/tasks/2026-08-29-xcuitest-native-acceptance.md`. No
third-party foreground-control daemon or macOS permission change is required by
the current criterion. Plan 5 (desktop lifecycle, tray/background behavior, and
release distribution) remains deferred.
Native acceptance must use a fresh isolated test profile and checked-in
synthetic fixtures only; no real export, private record, credential, or remote
operation is allowed.

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
| Packaged-native manual interaction and screenshots                                                                                                                    | `docs/testing/plan4-packaged-native-manual.md`; current result is **NOT RUN**                                                                                                       |

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

## Current HEAD verification status — manual acceptance NOT RUN

The product implementation/verification boundary is commit
`25c54806d06cb7c46b93cef1d7be5b4b93f9eec5`; this docs-only update is layered
on top and does not change product code. The implementation and automated
verification are complete within their stated boundaries. The approved manual
packaged-native criterion has not been executed on this HEAD, so its result is
**NOT RUN** and no native pass or screenshot is claimed.

### Implemented

- Focused ingestion RED/GREEN coverage proves that repeated refreshes of one
  failed asset remain one current attention item, and that successful
  processing clears the attention item and its failure-code count.
- The ingestion/UI boundary carries privacy-safe identity and reason data,
  deterministic failure-code counts, deterministic pending-module names, and
  localized Open Settings behavior. No raw paths or source content are
  surfaced, and no auto-update behavior is implemented.
- The dashboard/source gates passed. Deterministic package SHA-256 values are:

  | Package           | SHA-256                                                            |
  | ----------------- | ------------------------------------------------------------------ |
  | MyNetDiary source | `79a8c96594a95e508fc5cae95057323528d3f180af9e8f3c25bf472b635fc56c` |
  | Hevy source       | `b2f7963f09c392e96874a231cd54abdb694870929b3552c878f53a3fe8588379` |
  | Base dashboard    | `13a11f972e93c8bfd51b6e371fb8cef62f45a0887bb4339b1f0b93badf89d901` |

- The full mandatory Rust/web gates passed on this HEAD: workspace Rust tests,
  `mfa-ingestion` and `myfitanalytics` Clippy with `-D warnings`, rustfmt,
  frontend Vitest, Svelte check, frontend build, and diff checking. Svelte
  check reported 0 errors and the two already-known `AppShell` warnings.
  Playwright passed 5/5 using lockfile package `1.62.1` and an exact browser
  installed in a session-owned temporary cache.
- Fresh app and DMG packaging passed. The executable SHA-256 is
  `0b66ca53e055dc6101815e9b7516689c44f12a51fa7d492fa88735894812a611`; the
  DMG SHA-256 is
  `45a859450ebcb890c22bd91ac681955ad7363ccdd1d637f1370d048b9a64fdc4`.
  The production allowlist contained exactly three module files, and package
  hashes matched `dist`. The signing verifier, strict `codesign`, entitlements
  validation, and `hdiutil verify` all passed.
- The checked-in synthetic-fixture privacy verifier passed for 7 BIFF fixtures
  and 2 CSV fixtures. Secret/privacy scans reported no new-change findings.
- Residual `event_failures` belongs only to the test-only fault-injection path.
  Production `PipelineState` uses `NoFaultInjector`, whose `check` always
  returns `Ok`; it is not a source of repeated Refresh count.
- A prior fresh packaged launch showed a visible 1200x800 window and no crash in
  process or WebKit logs; that observation is historical and is not current
  manual acceptance evidence. The approved current procedure requires a fresh
  run, six-root hash guard, clean quit, and safe screenshot review.

### Not implemented

- The approved manual packaged-native acceptance has not been run on the current
  HEAD; interactive assertions and synthetic-only screenshots are therefore not
  claimed.
- XCUITest is deferred to the separate planning-only follow-up task.

### Remaining work

1. Execute `docs/testing/plan4-packaged-native-manual.md` against a fresh
   packaged artifact and record every pass/fail row, or record a truthful
   `BLOCKED`/`FAIL` result.
2. If native automation is desired later, obtain approval for and execute the
   separate XCUITest feasibility/task contract; it is not required for the
   current Plan 4 criterion.
3. Obtain review of the completed manual record before reclassifying it as
   passed.

### Blockers

No product or automated-gate blocker is recorded. The manual result is simply
not yet executed; any permission prompt or non-synthetic input during the
runbook is a stop condition requiring separate approval, not an invitation to
change permissions or add a foreground-control daemon. Plan 5 remains out of
scope, and remote push, PR update, and merge are not authorized.
