# Follow-up task: XCUITest packaged-native acceptance

Status: **deferred planning-only task**. Do not implement in the current Plan 4
completion work. The current accepted criterion is the manual packaged-native
runbook at `docs/testing/plan4-packaged-native-manual.md`.

## Objective

Replace the manual interaction portion of Plan 4 with an auditable Apple-native
UI test that launches the exact packaged MyFitAnalytics application, drives only
synthetic isolated data, captures safe UI evidence, and verifies cleanup. The
future task must not change the meaning of the lower-level Rust, web, storage,
ingestion, or package gates.

The test must target this exact release artifact, not a dev server or a mock
transport:

- application bundle:
  `target/release/bundle/macos/MyFitAnalytics.app`;
- bundle identifier: `com.simarglok.myfitanalytics`;
- executable inside the bundle:
  `Contents/MacOS/myfitanalytics`;
- window contract: title `MyFitAnalytics`, initial size 1200 x 800, minimum
  size 900 x 600.

## Constraints

- Planning only in this task; no Xcode project, XCTest target, application
  code, accessibility identifiers, or packaging changes are to be added until
  the feasibility spike is approved.
- Use Apple XCTest/XCUITest APIs only for future UI control. Do not install or
  invoke CuaDriver, and do not replace it with another third-party foreground-
  control daemon.
- Do not use a persistent daemon, background controller, telemetry collector,
  browser automation service, localhost test server, or network dependency.
- The test must run with a fresh temporary `HOME`, temporary app-data root,
  temporary workspace, temporary source inboxes, and checked-in synthetic
  fixtures only. It must never select or inspect a real workspace, private
  export, health record, credential, secret, or normal user profile.
- The test must not grant or request Accessibility, Screen Recording, Apple
  Events, Developer Tools, or any other macOS permission. If a prompt appears,
  the test stops and reports a blocked feasibility result. Any permission
  change requires a separate explicit user approval point.
- Plan 5 tray/background/autostart/release distribution behavior is a
  non-goal. Notarization and publishing are non-goals.

## Proposed architecture

Create a small, separately reviewable Xcode test project or workspace with one
`XCUITest` target and one deterministic test entry point. The runner should:

1. receive the already-built `.app` path and acceptance-root path from the
   invoking shell;
2. validate the app bundle identifier, executable path, and package hashes
   before launching;
3. create a fresh synthetic root and copy only the allowlisted fixtures;
4. launch the exact `.app` through `XCUIApplication`/the approved Apple test
   launch mechanism, passing only test-root environment variables or a
   test-specific configuration seam reviewed in the feasibility spike;
5. interact through visible `XCUIElement` queries and stable accessibility
   identifiers/labels;
6. save screenshots and test attachments under the acceptance root with
   redaction/safety checks; and
7. quit, verify the exact app process is gone, verify the profile guard, and
   remove the temporary root.

The test must not silently fall back to `web/e2e`, a Vite dev URL, a mock
transport, or an un-packaged Rust binary. A launch-path mismatch is a hard
failure.

## First feasibility spike: Tauri and WKWebView

Before implementing the full flow, run a narrow spike with no permission
changes:

1. Build the exact `.app` using the existing package command and verify its
   identifier/hash.
2. Launch it from an isolated temporary `HOME` with no real workspace.
3. Determine whether the Tauri-hosted WKWebView exposes the Svelte semantic
   controls to XCUITest as a stable accessibility tree, including navigation,
   buttons, status text, tables, charts, and modal content.
4. Determine whether native file chooser and confirmation dialogs are exposed
   as `XCUIApplication` alerts/sheets with stable labels, and whether the test
   can cancel them without permission prompts.
5. Measure whether the app's existing `data-*`/ARIA semantics are sufficient.
   If not, propose a narrowly scoped accessibility-identifier contract for the
   web UI and native dialog bridge; do not add it during the spike.
6. Confirm that the test can observe the data-change refresh without restarting
   the app and can distinguish the packaged process from unrelated processes.

The spike output must be a short report containing `SUPPORTED`, `SUPPORTED WITH
IDENTIFIER WORK`, or `BLOCKED`, the exact observed query examples, and any
permission prompt. A permission prompt is `BLOCKED`; it is not a reason to ask
the test to grant permission automatically.

## Accessibility identifier contract

If the spike is approved, define stable identifiers before writing scenario
code. Identifiers must describe product semantics, not DOM structure or
volatile values. The proposed names are:

- `app-shell`, `navigation-analytics`, `status-banner`, `status-health`,
  `status-active-jobs`, `status-attention-items`;
- `nav-overview`, `nav-body`, `nav-nutrition`, `nav-activity`,
  `nav-strength`, `nav-sources`, `nav-phase-events`, `nav-settings`;
- `settings-workspace`, `settings-source-inbox-hevy`,
  `settings-source-inbox-mynetdiary`, `settings-provider-body-weight`,
  `settings-module-catalog`, `settings-module-update`;
- `dashboard-page`, `dashboard-availability`, `dashboard-coverage`,
  `dashboard-gap`, `dashboard-block-overview`, `dashboard-block-body`,
  `dashboard-block-nutrition`, `dashboard-block-activity`,
  `dashboard-block-strength`, `dashboard-block-sources`;
- `quality-retry-import`, `phase-event-form`, `phase-event-list`,
  `phase-event-delete`, `phase-event-delete-confirm`,
  `phase-event-delete-cancel`, and `phase-event-delete-confirmed`.

The identifier contract must preserve accessible labels and roles. It must not
include asset paths, UUIDs, timestamps, source rows, credentials, or personal
values. Charts need a text/role summary so the test does not depend on pixel
matching.

## Native dialogs and WKWebView boundaries

The future test must explicitly cover:

- workspace chooser: select only the synthetic workspace and assert the
  resulting settings state, then cancel a second chooser without changing it;
- source-inbox chooser: select only each synthetic inbox and verify the typed
  path-safe state, then cancel;
- module package chooser, if exercised: use only the checked-in package and
  cancel-path behavior;
- phase-event delete confirmation: assert event name/type/date, click Cancel,
  assert preservation, reopen, click Confirm delete, assert removal;
- any macOS alert/sheet emitted by the packaged app: query by role and stable
  label, never by screen coordinate.

If WKWebView does not expose a reliable native accessibility boundary for a
control, the spike must stop and propose an explicit application-side
identifier/role fix. It must not introduce a daemon or coordinate-based
fallback.

## Synthetic isolated roots and profile safety

Each test invocation must create a unique temporary root with at least:

```text
<acceptance-root>/home
<acceptance-root>/workspace/inbox/mynetdiary
<acceptance-root>/workspace/inbox/hevy
<acceptance-root>/artifacts
<acceptance-root>/screenshots
<acceptance-root>/profile-guard.tsv
```

Copy only:

- `modules/sources/mynetdiary/tests/fixtures/valid-full.xls`;
- `modules/sources/mynetdiary/tests/fixtures/missing-required-sheet.xls` for
  the failure/idempotence case;
- `modules/sources/hevy/tests/fixtures/measurement_data.csv`; and
- `modules/sources/hevy/tests/fixtures/workout_data.csv`.

Use the existing six-root hash-verified backup/restore protocol. Store labels,
digests, counts, and restoration status, but never print resolved home paths or
copy normal profile data into test attachments. Restoration must run in a
`defer`/`finally` path after normal quit and after every failure.

## Scenario coverage

The final XCUITest suite should cover the following without an app restart
between configuration, refresh, and the first dashboard observation:

1. Fresh packaged launch and initial non-ready state.
2. Settings workspace/inbox configuration and explicit provider selection.
3. One `Refresh data` action, progress/active-job state, data-change refresh,
   archive completion, inbox consumption, and current `Healthy`/zero-attention
   state.
4. Repeated Refresh with the checked-in failing fixture: one current attention
   item, deterministic failure-code count, privacy-safe reason, visible retry,
   and no duplicate count on the second refresh.
5. Explicit bundled-module update control when an update candidate is
   advertised; otherwise record deterministic `N/A` rather than claiming an
   update was exercised. Never trigger automatic updates.
6. Overview, Body, Nutrition, Activity, Strength, and Sources & quality pages;
   explicit coverage/gaps, freshness, typed module errors, null-preserving
   values, and no raw payloads.
7. Phase-event save, list, overlay/TDEE update, cancel-delete, and confirmed
   delete.
8. Clean quit, no exact packaged process, read-only DuckDB integrity/aggregate
   checks, profile hash equality, and temporary-root cleanup.

Screenshots must be limited to safe app-window content: navigation, status,
settings controls without paths, dashboard semantic states, and the phase-event
confirmation dialog. Do not capture file-picker path bars, profile directories,
raw exports, databases, logs, terminal output, credentials, or personal data.
Every attachment must have a stable name, test step, artifact hash, and privacy
review result.

## Deterministic gates and auditability

The future test is additive to, not a replacement for, these repository gates:

```bash
cargo test --workspace -- --test-threads=1
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
bash scripts/run-dashboard-gate.sh
pnpm --dir web test -- --run
pnpm --dir web exec svelte-check --tsconfig ./tsconfig.json
pnpm --dir web exec eslint .
pnpm --dir web build
pnpm --dir web exec prettier --check .
python3 scripts/verify_module_packages.py
node scripts/fixtures/verify_fixture_privacy.mjs
cargo tauri build --bundles app
cargo tauri build --bundles dmg -vv
codesign --verify --deep --strict --verbose=2 <packaged-app>
hdiutil verify <dmg>
```

The XCUITest runner must emit machine-readable step names, exit codes, exact
app/package hashes, fixture allowlist, test-root label, profile-guard result,
screenshot names, and cleanup result. It must never emit raw process logs or
resolved private paths. Test execution must be serial when the database actor
is involved.

## Acceptance criteria

The follow-up task is complete only when:

- the feasibility spike has an approved result and no unapproved permission
  change occurred;
- the test launches the exact packaged `.app` and fails closed on a wrong
  bundle/hash/path;
- all scenario coverage above passes against a fresh synthetic root;
- WKWebView controls and native dialogs are addressed with stable accessible
  roles/identifiers, not coordinates or a daemon;
- screenshots/attachments are safe, reproducible, and auditable;
- clean quit leaves no exact packaged process and the read-only database checks
  pass;
- the six profile-root digests are restored identically and the temporary root
  is removed; and
- all deterministic lower-level gates remain green.

## Risks and mitigations

| Risk                                                     | Mitigation                                                                                                                                      |
| -------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| WKWebView does not expose DOM controls to XCUITest       | Stop at the feasibility spike; add reviewed semantic identifiers/roles or keep the manual criterion. Do not add a controller daemon.            |
| File chooser or alert triggers a macOS permission prompt | Stop immediately and report `BLOCKED`; ask for explicit approval in a separate task.                                                            |
| Packaged resource/hash drift                             | Build and verify package hashes before launch; fail closed on mismatch.                                                                         |
| Test touches the normal profile                          | Use isolated `HOME`, six-root hash guard, exact before/after comparison, and cleanup trap.                                                      |
| Pixel rendering varies across macOS versions             | Assert accessible roles, labels, semantic text, and typed states; use screenshots as attachments, not pixel-only pass criteria.                 |
| Background timing makes Refresh flaky                    | Wait on explicit status/data-change conditions with bounded timeouts; keep the DB actor serial; never use an uncontrolled sleep-only heuristic. |
| Failure retry changes archive counts                     | Record aggregate counts read-only and compare semantic invariants rather than deleting or rewriting test data.                                  |

## Non-goals

- No XCUITest implementation in the current Plan 4 completion commit.
- No change to product analytics, ingestion, module-host, Tauri, Svelte, or
  packaging behavior unless a separately approved feasibility result identifies
  a concrete accessibility contract gap.
- No Plan 5 lifecycle/tray/background/release work.
- No notarization, publishing, remote mutation, telemetry, hosted service, or
  network access.
- No automatic permission request or grant.

## Explicit approval and stop points

The future implementer must stop and obtain explicit approval before:

1. adding any Xcode/XCTest target or product-side accessibility identifiers;
2. changing the packaged app's launch/configuration seam;
3. requesting or granting any macOS permission;
4. adding any new fixture, dependency, test daemon, network endpoint, or
   persistent service; or
5. reclassifying manual acceptance from `NOT RUN`/`BLOCKED` to `PASS`.
