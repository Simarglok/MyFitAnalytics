# MyFitAnalytics Desktop Lifecycle and Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan. Use `superpowers:test-driven-development` for every lifecycle behavior and `superpowers:verification-before-completion` before claiming the MVP is complete.

**Goal:** Complete the desktop experience, background refresh lifecycle, recovery/diagnostics, platform hardening, cross-platform verification, and a launchable macOS MVP bundle with traceable acceptance evidence.

**Architecture:** The Tauri composition root starts one service graph and one actor-owned DuckDB connection. Tray, window, watcher, timer, and second-instance events enqueue commands into those services. Platform-specific adapters live only in `src-tauri`; core crates remain portable. The shipped WebView loads bundled assets and opens no listening port.

**Tech Stack:** Rust 1.94.0, Tauri 2.11, Tauri single-instance/autostart/dialog plugins, Tokio, `notify`, `tracing`, Svelte 5, GitHub Actions macOS/Windows/Linux, macOS `.app`/DMG packaging.

**Spec:** [MVP-SPEC.md Sections 9, 14.1–14.4, 15, 17.9, 18–20](</Users/simarglok/Library/Mobile Documents/iCloud~md~obsidian/Documents/Simarglok/MyFitAnalytics/MVP-SPEC.md>)

## Global Constraints

- Start only after the Analytics and Dashboard UI gate passes.
- One installed application instance may own the writable profile. Only its `DatabaseService` opens DuckDB.
- Closing the main window hides it and leaves background services running; explicit Quit performs orderly shutdown.
- The application opens the dashboard and requests a refresh on normal launch.
- Watcher events are hints only; startup, watcher, periodic, and tray refresh all enter the same coalescing scan queue.
- Launch at login is optional and disabled by default.
- Platform integrations remain in `src-tauri` or narrow adapter crates; `mfa-*` core crates must build on macOS, Windows, and Linux.
- Logs redact food names, exercise notes, raw rows, credentials, and personal paths by default.
- No completion claim is allowed without fresh full-suite, production-build, and macOS bundle smoke evidence.

---

### Task 1: Implement tray, close-to-background, single instance, and optional autostart

**Files:**

- Create: `src-tauri/src/tray.rs`
- Create: `src-tauri/src/lifecycle.rs`
- Create: `src-tauri/src/single_instance.rs`
- Create: `src-tauri/src/autostart.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/capabilities/default.json`
- Create: `src-tauri/tests/lifecycle.rs`
- Create: `src-tauri/tests/tray.rs`
- Create: `web/src/lib/components/WindowControls.svelte`
- Test: `web/src/lib/components/WindowControls.test.ts`

**Interfaces:**

```rust
pub enum TrayAction { OpenDashboard, RefreshNow, OpenSettings, Quit }
pub enum AggregateState { Healthy, Working, Attention, Blocked }
pub struct LifecycleController;
impl LifecycleController {
    pub async fn handle_window_close(&self, label: &str) -> LifecycleDecision;
    pub async fn handle_tray(&self, action: TrayAction) -> Result<(), LifecycleError>;
    pub async fn handle_second_instance(&self, args: Vec<String>) -> Result<(), LifecycleError>;
    pub async fn quit(&self) -> Result<(), LifecycleError>;
}
#[tauri::command] async fn get_launch_at_login(state: State<'_, AppState>) -> Result<bool, CommandError>;
#[tauri::command] async fn set_launch_at_login(enabled: bool, state: State<'_, AppState>) -> Result<bool, CommandError>;
```

**Step 1: Write failing lifecycle tests**

Through injected window/tray/service ports, assert normal launch shows/focuses the main window and queues one startup scan; main-window close prevents process exit and hides the window; Open Dashboard and Open Settings show/focus the correct route; Refresh Now queues the standard manual scan; Quit stops watcher/timer, drains ingestion, shuts down `DatabaseService`, then exits.

Run: `cargo test -p myfitanalytics --test lifecycle`

Expected: FAIL.

**Step 2: Write failing tray-state tests**

Assert menu IDs and labels are exactly Open Dashboard, Refresh Now, Settings, Quit. Map Healthy/Working/Attention/Blocked to accessible tooltip text and bundled monochrome template icons. Background errors update state without showing a modal or focusing the app.

Run: `cargo test -p myfitanalytics --test tray`

Expected: FAIL.

**Step 3: Implement lifecycle adapters**

Use `tauri-plugin-single-instance` before service setup so a second launch signals the primary and exits without opening DuckDB. Use `tauri-plugin-autostart` behind the Settings commands; initialize disabled and never enable implicitly. Use stable tray menu IDs rather than localized display text for dispatch.

**Step 4: Implement and test frontend controls**

Settings reflects actual OS autostart state and reports permission errors inline. Window controls call typed transport actions only.

Run:

```bash
cargo test -p myfitanalytics --test lifecycle
cargo test -p myfitanalytics --test tray
pnpm --dir web test -- --run WindowControls
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri web
git commit -m "feat: add desktop tray and lifecycle"
```

---

### Task 2: Run watcher and periodic refresh as one coalescing background service

**Files:**

- Create: `src-tauri/src/background.rs`
- Create: `src-tauri/src/watcher.rs`
- Create: `src-tauri/src/scheduler.rs`
- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/lifecycle.rs`
- Modify: `crates/mfa-config/src/settings.rs`
- Test: `src-tauri/tests/background_refresh.rs`

**Interfaces:**

```rust
pub struct BackgroundRefresh { cancellation: tokio_util::sync::CancellationToken }
impl BackgroundRefresh {
    pub async fn start(config: RefreshConfig, ingestion: IngestionCoordinator, watcher: Box<dyn WorkspaceWatcher>, clock: Arc<dyn Clock>) -> Result<Self, BackgroundError>;
    pub async fn reconfigure(&self, config: RefreshConfig) -> Result<(), BackgroundError>;
    pub async fn shutdown(self) -> Result<(), BackgroundError>;
}
pub struct RefreshConfig { pub periodic_interval: Duration, pub watcher_enabled: bool }
```

**Step 1: Write failing virtual-time tests**

Pause Tokio time. Assert startup queues once, watcher bursts debounce into one watcher scan, timer ticks enqueue periodic scans, tray/manual scan uses the same coordinator, overlapping reasons coalesce according to queue semantics, workspace changes restart only the watcher, and shutdown cancels pending timer/debounce tasks.

Run: `cargo test -p myfitanalytics --test background_refresh -- --test-threads=1`

Expected: FAIL.

**Step 2: Implement scheduler and watcher ports**

Wrap `notify` behind `WorkspaceWatcher`; callbacks send pathless wake signals because the authoritative scan enumerates enabled inboxes. Default periodic interval is 15 minutes and is persisted as a bounded setting. Never parse or archive in the watcher callback.

**Step 3: Handle unavailable synchronized files**

Map placeholder/not-local/read-sharing errors to `waiting`; do not create a failed attempt or notification. Periodic scan checks them again. A disappeared candidate is removed from stability tracking without quality error.

**Step 4: Run and commit**

```bash
cargo test -p myfitanalytics --test background_refresh -- --test-threads=1
git add src-tauri crates/mfa-config
git commit -m "feat: refresh inboxes in the background"
```

---

### Task 3: Add recovery mode, rebuild confirmation, and diagnostics

**Files:**

- Create: `src-tauri/src/diagnostics.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/capabilities/default.json`
- Create: `web/src/lib/pages/RecoveryPage.svelte`
- Create: `web/src/lib/pages/DiagnosticsPage.svelte`
- Modify: `web/src/lib/pages/SettingsPage.svelte`
- Test: `src-tauri/tests/recovery_commands.rs`
- Test: `web/src/lib/pages/RecoveryPage.test.ts`
- Test: `web/src/lib/pages/DiagnosticsPage.test.ts`

**Interfaces:**

```rust
#[tauri::command] async fn get_diagnostics(state: State<'_, AppState>) -> Result<DiagnosticsView, CommandError>;
#[tauri::command] async fn preflight_rebuild(state: State<'_, AppState>) -> Result<RebuildPlanView, CommandError>;
#[tauri::command] async fn rebuild_database(confirmation_token: String, state: State<'_, AppState>) -> Result<RebuildResultView, CommandError>;
```

**Step 1: Write failing native-command tests**

Reuse the workspace and module picker commands delivered by the Bundled Source Modules plan. Assert recovery mode rejects refresh/import but permits Settings, diagnostics, archive inspection metadata, and rebuild preflight; the existing pickers remain reachable and preserve their cancellation and package-filter behavior.

Run: `cargo test -p myfitanalytics --test recovery_commands`

Expected: FAIL.

**Step 2: Write failing UI tests**

Assert blocked state routes to Recovery, shows stable error code and health checks, lists missing module dependencies, requires the user to type `REBUILD` after preflight, displays recovery-copy destination and progress, and never claims success after a failed rebuild. Diagnostics copy excludes raw rows, names, notes, credentials, and absolute personal paths.

Run: `pnpm --dir web test -- --run RecoveryPage DiagnosticsPage`

Expected: FAIL.

**Step 3: Implement recovery and diagnostics UI**

Bind confirmation tokens to the current rebuild plan digest and expire them after one use or configuration change. Stream progress as typed stage/percentage events without row data. Extend the existing Settings page and dialog abstraction rather than introducing a second filesystem-selection path.

**Step 4: Run and commit**

```bash
cargo test -p myfitanalytics --test recovery_commands
pnpm --dir web test -- --run RecoveryPage DiagnosticsPage
git add src-tauri web
git commit -m "feat: add recovery and diagnostics settings"
```

---

### Task 4: Harden logs, permissions, package contents, and privacy boundaries

**Files:**

- Create: `crates/mfa-telemetry/Cargo.toml`
- Create: `crates/mfa-telemetry/src/lib.rs`
- Create: `crates/mfa-telemetry/src/redaction.rs`
- Create: `crates/mfa-telemetry/src/rotation.rs`
- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/tauri.conf.json`
- Create: `scripts/audit-package.sh`
- Create: `scripts/audit-core-portability.sh`
- Test: `crates/mfa-telemetry/tests/redaction.rs`
- Test: `src-tauri/tests/capabilities.rs`

**Interfaces:**

```rust
pub struct PrivacyFields<'a> { pub event: &'a str, pub code: Option<&'a str>, pub source_module_id: Option<&'a str>, pub asset_id: Option<AssetId>, pub attempt_id: Option<AttemptId>, pub path: Option<&'a Path> }
pub fn redact_path(path: &Path, roots: &KnownRoots) -> RedactedPath;
pub fn init_logging(log_dir: &Path, level: LogLevel) -> Result<LoggingGuard, TelemetryError>;
```

**Step 1: Write failing redaction tests**

Feed food names, exercise notes, raw CSV/XLS strings, emails, home paths, workspace paths, credentials, and URLs through logging/diagnostic adapters. Assert output contains stable IDs, source module, error code, and path category/hash only. Add property tests for control characters and Unicode.

Run: `cargo test -p mfa-telemetry --test redaction`

Expected: FAIL.

**Step 2: Implement structured rotating logs**

Write JSON lines under local app data with size/count bounds, local restrictive permissions where supported, and an RAII flush guard. Do not add telemetry or network exporters. Treat raw values as opt-in `Sensitive<T>` types that do not implement `Display` or `Debug`.

**Step 3: Audit Tauri permissions and package contents**

`default.json` grants only main-window core/event/dialog/autostart permissions needed by implemented commands. `audit-package.sh` fails if the bundle contains test fixtures, user exports, source maps, development URLs, private keys, unexpected executables, or a localhost binding string. It verifies bundled module hashes against manifests.

Run:

```bash
cargo test -p myfitanalytics --test capabilities
cargo test -p mfa-telemetry
bash scripts/audit-package.sh --development-tree
```

Expected: PASS.

**Step 4: Audit portable core boundaries**

`audit-core-portability.sh` fails if any `crates/mfa-*` manifest depends on Tauri/AppKit/Win32/GTK desktop APIs or if provider-specific cloud APIs/paths occur outside tests. Allowed OS-specific filesystem durability code must sit behind a tested portable trait.

Run: `bash scripts/audit-core-portability.sh`

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/mfa-telemetry src-tauri scripts
git commit -m "chore: harden privacy and desktop permissions"
```

---

### Task 5: Add cross-platform CI and deterministic release builds

**Files:**

- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/macos-package.yml`
- Create: `scripts/ci/rust-gate.sh`
- Create: `scripts/ci/frontend-gate.sh`
- Create: `scripts/ci/module-gate.sh`
- Create: `scripts/ci/check-lockfiles.sh`
- Create: `docs/building.md`
- Modify: `src-tauri/tauri.conf.json`

**Step 1: Add the CI definition and validate syntax locally**

`ci.yml` has these jobs:

```text
lockfiles-and-contracts  ubuntu-latest
rust-core                macos-latest, windows-latest, ubuntu-latest
source-dashboard-modules macos-latest, windows-latest, ubuntu-latest
frontend                 ubuntu-latest
tauri-compile             macos-latest, windows-latest, ubuntu-latest
```

Every job uses the pinned Rust toolchain, frozen Cargo/pnpm lockfiles, explicit `cargo component` version, dependency caches keyed by lockfiles, and uploaded test reports. Cross-platform jobs do not access personal/cloud paths.

Run:

```bash
actionlint .github/workflows/ci.yml .github/workflows/macos-package.yml
bash scripts/ci/check-lockfiles.sh
```

Expected: FAIL until scripts and complete workflow are present, then PASS.

**Step 2: Implement exact gate scripts**

`rust-gate.sh` runs format, Clippy all workspace targets with warnings denied, then full workspace tests. `frontend-gate.sh` runs Prettier, ESLint, Svelte typecheck, Vitest, and production build. `module-gate.sh` regenerates fixtures, builds packages twice, compares hashes, inspects imports, and runs conformance suites.

**Step 3: Configure macOS package workflow**

Build the production `.app` and DMG from a clean checkout, audit contents, verify code signature structure with `codesign --verify --deep --strict`, upload artifacts and checksums, then run the package smoke script from Task 6. Signing identity/notarization credentials are optional release inputs and not required for the personal MVP gate; the workflow must make that distinction explicit.

**Step 4: Document reproducible local commands**

`docs/building.md` lists prerequisites for macOS, Windows, and Linux; exact toolchain installation; frozen build commands; local app-data override used by tests; module packaging; platform limitations; and macOS unsigned local bundle behavior.

**Step 5: Run local CI scripts and commit**

```bash
bash scripts/ci/check-lockfiles.sh
bash scripts/ci/rust-gate.sh
bash scripts/ci/frontend-gate.sh
bash scripts/ci/module-gate.sh
git add .github scripts/ci docs/building.md src-tauri/tauri.conf.json
git commit -m "ci: verify core modules frontend and macOS package"
```

---

### Task 6: Add macOS bundle smoke and close every MVP acceptance criterion

**Files:**

- Create: `scripts/smoke/macos-package.sh`
- Create: `scripts/smoke/verify-smoke-profile.sh`
- Create: `src-tauri/src/smoke_profile.rs`
- Test: `src-tauri/tests/smoke_profile.rs`
- Create: `docs/acceptance-matrix.md`
- Create: `docs/superpowers/evidence/mvp-final.md`
- Test: `crates/mfa-integration-tests/tests/acceptance_matrix.rs`

**Interfaces:**

```rust
pub struct ProfileOverride(PathBuf);
impl ProfileOverride { pub fn from_cli(args: &[String]) -> Result<Option<Self>, ProfileError>; }
pub struct SmokeMarker { pub schema_version: u32, pub app_version: String, pub stage: SmokeStage, pub profile_hash: String }
```

**Step 1: Write failing profile-override tests**

Accept `--profile-root <absolute-empty-or-existing-directory>` only before service startup. Reject relative paths, root/home directories, workspace roots, symlinks escaping the chosen parent, and concurrent ownership. The override changes local app data only; workspace remains chosen through Settings. Never include the raw override path in the marker.

Run: `cargo test -p myfitanalytics --test smoke_profile`

Expected: FAIL.

**Step 2: Implement the launchable-bundle smoke script**

The script must:

1. create explicit temporary profile and workspace roots with `mktemp -d`;
2. copy synthetic source fixtures into the workspace inboxes;
3. launch `MyFitAnalytics.app/Contents/MacOS/MyFitAnalytics --profile-root <profile> --workspace-root-for-smoke <workspace>`;
4. wait with a bounded timeout for structured `app_ready` and `initial_scan_complete` markers;
5. launch the bundle a second time and verify the primary receives focus without a second database owner;
6. invoke the app’s test-safe CLI control socket substitute by sending `SIGUSR1` for Refresh Now and verify a new coalesced scan marker; on non-Unix platforms this signal path is not compiled;
7. send `SIGTERM`, verify orderly actor shutdown marker and zero/non-crash exit;
8. run `verify-smoke-profile.sh` to confirm database, archive, inbox cleanup, package store, logs, and no listening TCP sockets;
9. retain redacted logs only on failure and remove temporary roots on success.

The signal handler calls the same typed lifecycle actions as tray commands and contains no direct database/filesystem work. The smoke-only workspace CLI argument is accepted only together with an explicit profile override and never persisted into default user configuration.

Run: `bash scripts/smoke/macos-package.sh target/release/bundle/macos/MyFitAnalytics.app`

Expected: FAIL until production package and marker wiring exist, then PASS.

**Step 3: Perform the visible macOS interaction smoke**

From a clean profile, record the date/operator/result for: first-launch directory picker, dashboard opens, initial refresh progress, Settings opens, module list/provider defaults, window close leaves tray active, tray Open Dashboard restores/focuses, tray Refresh Now works, autostart default is off and can be toggled, and tray Quit removes the process. This is a platform smoke record, not a substitute for automated core behavior tests.

**Step 4: Build the acceptance matrix**

Create one row for each numbered criterion in MVP-SPEC Section 20 with requirement text, owning plan/task, automated test or smoke step, latest evidence path, and status. The matrix validator fails on a missing criterion, duplicate number, non-existent test path, or status other than `pass` at final gate.

Run: `cargo test -p mfa-integration-tests --test acceptance_matrix`

Expected: PASS after all 30 criteria are linked.

**Step 5: Run the fresh final gate**

Run from a clean checkout state:

```bash
bash scripts/ci/check-lockfiles.sh
bash scripts/ci/rust-gate.sh
bash scripts/ci/frontend-gate.sh
bash scripts/ci/module-gate.sh
cargo tauri build
bash scripts/audit-package.sh target/release/bundle/macos/MyFitAnalytics.app
bash scripts/smoke/macos-package.sh target/release/bundle/macos/MyFitAnalytics.app
cargo test -p mfa-integration-tests --test acceptance_matrix
git status --short
```

Expected: every command exits 0 and `git status --short` is empty after final evidence is committed. If the evidence file is updated by the gate, commit it and rerun the status and evidence validators before claiming completion.

**Step 6: Commit**

```bash
git add scripts/smoke src-tauri docs/acceptance-matrix.md docs/superpowers/evidence/mvp-final.md
git commit -m "test: close MyFitAnalytics MVP acceptance gate"
```

## Plan Completion Evidence

`docs/superpowers/evidence/mvp-final.md` records exact versions, commit SHA, clean status, full gate commands and exit codes, bundle path/hash, automated smoke result, visible macOS smoke checklist, CI run links, and the acceptance-matrix validator result. Do not mark the MVP complete if any platform/core/module/frontend/package requirement is skipped or stale.
