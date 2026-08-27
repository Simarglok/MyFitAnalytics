# MyFitAnalytics Analytics and Dashboard UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan. Use `superpowers:test-driven-development` for every metric and UI behavior and `superpowers:verification-before-completion` before closing the plan.

**Goal:** Produce deterministic base analytics, resolve dashboard availability from capabilities and coverage, run the base dashboard as a sandboxed module, and render the complete local English MVP UI through typed Tauri queries.

**Architecture:** `mfa-analytics` transforms actor-returned canonical DTOs into versioned metrics and coverage evidence without opening DuckDB. `mfa-dashboard-host` grants each dashboard only declared datasets, invokes its Wasm component, and validates a non-executable `DashboardDocument`. Svelte renders a fixed set of safe card/table/chart primitives and gets all state through `AppTransport`.

**Tech Stack:** Rust 1.94.0, Wasmtime Component Model, Svelte 5.56, TypeScript, Vite 8.2, Apache ECharts 6.1, Vitest 4.1, Testing Library, Playwright, `insta` golden files.

**Spec:** [MVP-SPEC.md Sections 5, 12, 16–18, 19.2, 19.6–19.8, 20](</Users/simarglok/Library/Mobile Documents/iCloud~md~obsidian/Documents/Simarglok/MyFitAnalytics/MVP-SPEC.md>)

## Global Constraints

- Start only after the Bundled Source Modules gate passes.
- Analytics never replace missing values with zero and never read inactive-provider rows.
- Every derived value includes algorithm/mapping version, requested window, input coverage, and active snapshot references.
- Availability and freshness are separate: stale data may remain `ready` with a freshness modifier.
- A dashboard guest receives only datasets satisfying its declared dependencies and never receives raw filesystem paths, source bytes, SQL, or credentials.
- The frontend renders only host-supported declarative primitives. Module output cannot inject HTML, JavaScript, CSS, URLs, SQL, or Tauri commands.
- Source data such as food and exercise names remains untranslated. Only application/module UI strings use locale namespaces.
- Every non-ready graph remains visible with a stable state, English explanation, and relevant action.

---

### Task 1: Implement deterministic weight, nutrition, activity, and strength analytics

**Files:**

- Create: `crates/mfa-analytics/Cargo.toml`
- Create: `crates/mfa-analytics/src/lib.rs`
- Create: `crates/mfa-analytics/src/window.rs`
- Create: `crates/mfa-analytics/src/weight.rs`
- Create: `crates/mfa-analytics/src/nutrition.rs`
- Create: `crates/mfa-analytics/src/activity.rs`
- Create: `crates/mfa-analytics/src/strength.rs`
- Create: `crates/mfa-analytics/src/provenance.rs`
- Test: `crates/mfa-analytics/tests/weight_golden.rs`
- Test: `crates/mfa-analytics/tests/nutrition_golden.rs`
- Test: `crates/mfa-analytics/tests/activity_golden.rs`
- Test: `crates/mfa-analytics/tests/strength_golden.rs`
- Golden: `crates/mfa-analytics/tests/golden/base_metrics.json`

**Interfaces:**

```rust
pub struct MetricContext { pub requested: DateRange, pub as_of: LocalDate, pub snapshot_refs: Vec<SnapshotRef>, pub algorithm_version: AlgorithmVersion }
pub struct WeightAnalytics { pub observations: Vec<WeightPoint>, pub daily_median: Vec<WeightPoint>, pub trailing_7d_mean: Vec<NullablePoint>, pub slope_28d: Option<TheilSenEstimate>, pub provenance: DerivedProvenance }
pub struct NutritionDay { pub local_date: LocalDate, pub calories_kcal: Option<f64>, pub protein_g: Option<f64>, pub fat_g: Option<f64>, pub carbs_g: Option<f64>, pub fiber_g: Option<f64>, pub logged_item_count: u32, pub quality: NutritionQuality }
pub struct ActivityAnalytics { pub steps: Vec<NullablePoint>, pub mean_steps_7d: Vec<NullablePoint>, pub mean_steps_28d: Vec<NullablePoint>, pub events: Vec<ActivitySummary> }
pub struct StrengthAnalytics { pub session_counts: WindowCounts, pub session_durations: Vec<SessionDuration>, pub working_sets: Vec<WorkingSet>, pub weekly_best_e1rm: Vec<E1rmPoint> }
```

**Step 1: Write failing weight golden tests**

Use unsorted observations with two same-day values, missing dates, boundary dates, and outliers. Assert daily median, trailing seven-calendar-day mean over observed daily medians only, requested-range clipping, and 28-day Theil–Sen median pairwise slope with deterministic lower/upper bounds. A window with fewer than two distinct dates has no slope.

Run: `cargo test -p mfa-analytics --test weight_golden`

Expected: FAIL.

**Step 2: Implement weight analytics and rerun**

Sort by `LocalDate` then stable observation ID. Use exact day differences for pairwise slopes; never treat observation index as elapsed time. Record `weight.theil_sen@1` and active snapshot IDs in derived provenance.

Run: `cargo test -p mfa-analytics --test weight_golden`

Expected: PASS.

**Step 3: Write failing nutrition tests**

Prove all-calorie rows produce `complete`; any missing calorie produces `partial_fields` and null daily calories; no rows produce `missing`; user exclusion produces `excluded_by_user`; macro missing remains null; identical items retain count; rolling means include only complete, non-excluded values.

Run: `cargo test -p mfa-analytics --test nutrition_golden`

Expected: FAIL.

**Step 4: Implement nutrition aggregation and rerun**

Aggregate only over the requested date range. Represent every date explicitly in returned series so gaps render as gaps with quality state rather than zeros.

Run: `cargo test -p mfa-analytics --test nutrition_golden`

Expected: PASS.

**Step 5: Write and implement activity tests**

Assert 7/28-day step means use present daily counts only; steps and walking distance are never summed; unknown-mapping events are excluded; accepted event count, duration, distance by allowed type, and estimated calories remain separate; heart rate and water are optional independent series.

Run before implementation: `cargo test -p mfa-analytics --test activity_golden`

Expected: FAIL.

Run after implementation: same command; Expected: PASS.

**Step 6: Write and implement strength tests**

Assert session counts for trailing 7/14/28 calendar days, session duration, working sets by mapped exercise, and weekly best Epley e1RM `weight * (1 + reps / 30)`. Include only normal/failure, external load, positive weight/reps, reps 1–12, and exact governed exercise keys. Exclude warmup, bodyweight, assisted, duration, unknown load/type, and mechanically distinct variants.

Run before implementation: `cargo test -p mfa-analytics --test strength_golden`

Expected: FAIL.

Run after implementation: same command; Expected: PASS.

**Step 7: Run and commit**

```bash
cargo test -p mfa-analytics
cargo clippy -p mfa-analytics --all-targets -- -D warnings
git add crates/mfa-analytics
git commit -m "feat: compute deterministic base analytics"
```

---

### Task 2: Implement rolling TDEE, phase events, and coverage evidence

**Files:**

- Create: `crates/mfa-analytics/src/tdee.rs`
- Create: `crates/mfa-analytics/src/phase.rs`
- Create: `crates/mfa-analytics/src/coverage.rs`
- Test: `crates/mfa-analytics/tests/tdee_golden.rs`
- Test: `crates/mfa-analytics/tests/phase_events.rs`
- Golden: `crates/mfa-analytics/tests/golden/tdee_cases.json`
- Modify: `crates/mfa-db/migrations/0002_canonical.sql`
- Modify: `crates/mfa-db/src/snapshot.rs`

**Interfaces:**

```rust
pub struct TdeeCoverage { pub complete_nutrition_days: u32, pub weight_days: u32, pub first_7d_weight_days: u32, pub last_7d_weight_days: u32, pub excluded_days: u32, pub slope_available: bool }
pub enum TdeeResult { Ready(TdeeEstimate), InsufficientCoverage(TdeeCoverage) }
pub struct TdeeEstimate { pub kcal_per_day: f64, pub low: f64, pub high: f64, pub average_intake: f64, pub slope: TheilSenEstimate, pub coverage: TdeeCoverage, pub window: DateRange }
pub fn rolling_tdee(window: DateRange, nutrition: &[NutritionDay], weights: &[WeightPoint], phases: &[PhaseEvent]) -> TdeeResult;
```

**Step 1: Write one failing golden case per coverage threshold**

Create separate cases for fewer than 21 complete nutrition days, fewer than 8 weight dates, no weight in first seven days, no weight in last seven days, excluded days reducing thresholds, and unavailable slope. Each returns `InsufficientCoverage` and no numeric TDEE.

Run: `cargo test -p mfa-analytics --test tdee_golden coverage`

Expected: FAIL.

**Step 2: Write failing point/range sign tests**

For weight loss, stable weight, and gain, assert:

```text
point = average_intake - 7700 * slope
low   = average_intake - 7700 * slope_high
high  = average_intake - 7700 * slope_low
```

Assert exercise calories and any BMR fields have no input path into the function.

Run: `cargo test -p mfa-analytics --test tdee_golden estimates`

Expected: FAIL.

**Step 3: Implement TDEE and phase filtering**

The window is exactly 28 local dates ending at `as_of`. Exclude only dates covered by a phase event with `exclude_from_tdee=true`. Persist user-created phase events through actor commands with explicit event type, dates, description, and exclusion flag; no automatic phase inference exists.

**Step 4: Write and pass phase-event tests**

Test inclusive date bounds, overlapping events, non-excluding overlays, edit/delete through typed commands, and that exclusions affect only their dates.

Run: `cargo test -p mfa-analytics --test phase_events`

Expected: PASS after implementation.

**Step 5: Run and commit**

```bash
cargo test -p mfa-analytics --test tdee_golden
cargo test -p mfa-analytics --test phase_events
git add crates/mfa-analytics crates/mfa-db
git commit -m "feat: calculate gated rolling TDEE"
```

---

### Task 3: Resolve dashboard dependencies and constrain dashboard documents

**Files:**

- Create: `crates/mfa-dashboard-host/Cargo.toml`
- Create: `crates/mfa-dashboard-host/src/lib.rs`
- Create: `crates/mfa-dashboard-host/src/availability.rs`
- Create: `crates/mfa-dashboard-host/src/datasets.rs`
- Create: `crates/mfa-dashboard-host/src/document.rs`
- Create: `crates/mfa-dashboard-host/src/validator.rs`
- Test: `crates/mfa-dashboard-host/tests/availability.rs`
- Test: `crates/mfa-dashboard-host/tests/dependency_isolation.rs`
- Test: `crates/mfa-dashboard-host/tests/document_security.rs`

**Interfaces:**

```rust
pub struct AvailabilityResolver;
impl AvailabilityResolver { pub fn resolve(&self, requirement: &DashboardRequirement, registry: &ResolvedCapabilities, coverage: &CoverageCatalog, modules: &ModuleRegistryView) -> Availability; }
pub struct DatasetResolver;
impl DatasetResolver { pub async fn resolve(&self, manifest: &DashboardManifest, request: DateRange) -> Result<DashboardInput, DashboardError>; }
pub enum DashboardNode { Section(SectionNode), Card(CardNode), LineChart(LineChartNode), BarChart(BarChartNode), ScatterChart(ScatterChartNode), CalendarHeatmap(CalendarHeatmapNode), Table(TableNode), Status(StatusNode) }
```

**Step 1: Write failing availability tests**

Test deterministic precedence and payload for `disabled_by_user`, `missing_dependency`, `incompatible_contract`, `missing_capability`, `waiting_for_data`, `insufficient_coverage`, and `ready`. Freshness is an independent modifier. A provider with no successful snapshot is waiting, not missing.

Run: `cargo test -p mfa-dashboard-host --test availability`

Expected: FAIL.

**Step 2: Write failing dependency isolation tests**

Give a dashboard one required capability and one extension namespace. Assert it receives only those resolved datasets, never other installed-source or extension data. A cross-source dashboard receives multiple namespaces only when all are declared with compatible version ranges.

Run: `cargo test -p mfa-dashboard-host --test dependency_isolation`

Expected: FAIL.

**Step 3: Write failing document-security tests**

Reject unknown node/chart types, HTML, scripts, event-handler keys, CSS strings, URLs, SQL, non-finite numbers, oversized series, undeclared localization keys, and dataset references outside the input grant. Accept only fixed primitive unions and theme tokens.

Run: `cargo test -p mfa-dashboard-host --test document_security`

Expected: FAIL.

**Step 4: Implement resolver and validator**

Build `DashboardInput` from actor query DTOs and analytics outputs after dependency resolution. Invoke the Foundation dashboard runtime limits, deserialize, validate every node, and replace invalid output with a typed module error view. Dashboard installation/removal never changes source, canonical, or extension data.

**Step 5: Run and commit**

```bash
cargo test -p mfa-dashboard-host
cargo clippy -p mfa-dashboard-host --all-targets -- -D warnings
git add crates/mfa-dashboard-host
git commit -m "feat: resolve and constrain dashboard modules"
```

---

### Task 4: Implement the base dashboard as a bundled Wasm component

**Files:**

- Create: `modules/dashboards/base/Cargo.toml`
- Create: `modules/dashboards/base/module.json`
- Create: `modules/dashboards/base/locales/en.json`
- Create: `modules/dashboards/base/src/lib.rs`
- Create: `modules/dashboards/base/src/overview.rs`
- Create: `modules/dashboards/base/src/body.rs`
- Create: `modules/dashboards/base/src/nutrition.rs`
- Create: `modules/dashboards/base/src/activity.rs`
- Create: `modules/dashboards/base/src/strength.rs`
- Create: `modules/dashboards/base/src/sources.rs`
- Test: `modules/dashboards/base/tests/document_golden.rs`
- Test: `modules/dashboards/base/tests/conformance.rs`
- Golden: `modules/dashboards/base/tests/golden/*.json`

**Step 1: Write failing document golden tests**

For each page, provide ready and every relevant non-ready input. Assert:

- Overview cards resolve independently and include freshness/open-quality counts;
- Body contains raw weight, daily median, 7-day mean, 28-day trend, optional fat, and phase overlays;
- Nutrition contains calories/macros, 7-day mean, missing/excluded days, and TDEE inputs;
- Activity contains steps, accepted event aggregates, optional HR/water, and unknown-mapping warning;
- Strength contains workout calendar, 7/14/28 counts, duration, exercise sets, and weekly e1RM;
- Sources contains modules, provider selection, receipts/assets/hashes, active snapshots, attempts, errors, mappings, and last successful imports.

Run: `cargo test -p mfa-dashboard-base --test document_golden`

Expected: FAIL.

**Step 2: Implement pure page composition**

Each page function consumes only its typed slice of `DashboardInput` and returns supported nodes. Keep all user-facing strings as `base.*` keys in `locales/en.json`. Show disabled compact cards/graphs with state reason and action; do not hide base graphs.

**Step 3: Run conformance**

Prove base uses the same WIT API, package inspector, Wasmtime limits, dependency grant, and document validator as third-party dashboards.

Run:

```bash
cargo test -p mfa-dashboard-base
bash scripts/build-module-packages.sh
bash scripts/verify-module-packages.sh
```

Expected: PASS and `dist/modules/base.mfadashboard` is deterministic.

**Step 4: Commit**

```bash
git add modules/dashboards/base
git commit -m "feat: add sandboxed base dashboard module"
```

---

### Task 5: Expose analytics, provider selection, and phase events through typed application commands

**Files:**

- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/events.rs`
- Create: `src-tauri/src/view_models.rs`
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/lib/transport.ts`
- Modify: `web/src/lib/tauri-transport.ts`
- Modify: `web/src/lib/mock-transport.ts`
- Test: `src-tauri/tests/dashboard_commands.rs`
- Test: `src-tauri/tests/provider_selection_commands.rs`

**Interfaces:**

```rust
#[tauri::command] async fn get_navigation(state: State<'_, AppState>) -> Result<NavigationView, CommandError>;
#[tauri::command] async fn get_dashboard(module_id: String, page_id: String, range: DateRangeView, state: State<'_, AppState>) -> Result<DashboardPageView, CommandError>;
#[tauri::command] async fn select_provider(capability: String, module_id: String, state: State<'_, AppState>) -> Result<ProviderView, CommandError>;
#[tauri::command] async fn save_phase_event(input: PhaseEventInput, state: State<'_, AppState>) -> Result<PhaseEventView, CommandError>;
```

**Step 1: Write failing dashboard command tests**

Assert each command returns JSON-stable DTOs, queries only whole committed snapshots, includes availability/coverage/freshness, and never exposes raw SQL, archive paths in analytics pages, or unrestricted dashboard JSON.

Run: `cargo test -p myfitanalytics --test dashboard_commands`

Expected: FAIL.

**Step 2: Write failing provider-selection command tests**

Starting from packages installed through the lifecycle commands delivered by the Bundled Source Modules plan, assert provider conflicts require explicit selection, invalid providers are rejected, changing the provider atomically changes active canonical views and derived analytics, and navigation updates when dashboard availability changes. The earlier install/enable/disable/update/uninstall contract remains covered by `module_lifecycle_commands.rs` and is not reimplemented here.

Run: `cargo test -p myfitanalytics --test provider_selection_commands`

Expected: FAIL.

**Step 3: Implement query composition and command adapters**

Query canonical DTOs through `DatabaseService`, compute base analytics outside the actor, resolve dashboard input, invoke/validate the module, and return a typed page view. Cache only by active snapshot IDs, dashboard package hash, range, settings revision, and algorithm versions; invalidate on `DataChanged` or configuration change.

**Step 4: Run and commit**

```bash
cargo test -p myfitanalytics --test dashboard_commands
cargo test -p myfitanalytics --test provider_selection_commands
git add src-tauri web/src/lib
git commit -m "feat: expose analytics and provider selection"
```

---

### Task 6: Build the Svelte dashboard, Settings, quality, and localization UI

**Files:**

- Create: `web/src/lib/stores/app.svelte.ts`
- Create: `web/src/lib/stores/dashboard.svelte.ts`
- Create: `web/src/lib/components/AppShell.svelte`
- Create: `web/src/lib/components/Navigation.svelte`
- Create: `web/src/lib/components/StatusBanner.svelte`
- Create: `web/src/lib/components/DashboardRenderer.svelte`
- Create: `web/src/lib/components/AvailabilityPanel.svelte`
- Create: `web/src/lib/components/charts/LineChart.svelte`
- Create: `web/src/lib/components/charts/BarChart.svelte`
- Create: `web/src/lib/components/charts/ScatterChart.svelte`
- Create: `web/src/lib/components/charts/CalendarHeatmap.svelte`
- Create: `web/src/lib/pages/DashboardPage.svelte`
- Modify: `web/src/lib/pages/SettingsPage.svelte`
- Create: `web/src/lib/pages/SourcesQualityPage.svelte`
- Create: `web/src/lib/pages/PhaseEventsPage.svelte`
- Create: `web/src/lib/i18n/catalog.ts`
- Create: `web/src/lib/i18n/format.ts`
- Modify: `web/src/App.svelte`
- Modify: `web/src/styles.css`
- Test: `web/src/lib/components/DashboardRenderer.test.ts`
- Test: `web/src/lib/pages/DashboardPage.test.ts`
- Test: `web/src/lib/pages/SettingsPage.test.ts`
- Test: `web/src/lib/pages/SourcesQualityPage.test.ts`
- Test: `web/src/lib/i18n/catalog.test.ts`
- Test: `web/e2e/dashboard.spec.ts`

**Step 1: Write failing renderer tests**

Render every allowed `DashboardNode`, empty series, missing points, freshness modifiers, loading/error states, and all seven availability states. Assert unknown node input is rejected by TypeScript decoding and produces a safe error panel. Verify keyboard-focusable controls and accessible names.

Run: `pnpm --dir web test -- --run DashboardRenderer`

Expected: FAIL.

**Step 2: Implement safe ECharts wrappers**

Construct ECharts options only inside trusted wrappers from typed numeric/category arrays. Use explicit tree-shaken ECharts imports. Convert nulls to chart gaps, not zero. Escape all labels as text and dispose/recreate charts on component lifecycle changes.

**Step 3: Write failing page and Settings tests**

Assert navigation, range changes, refresh-after-`DataChanged`, Overview/Body/Nutrition/Activity/Strength rendering, provider choice, dashboard dependency/availability details, locale choice, manual rescan, phase-event edit, and confirmation before destructive rebuild. Assert the workspace and module lifecycle controls delivered by the Bundled Source Modules plan remain present and functional. Test aggregate states Healthy/Working/Attention/Blocked without modal focus stealing.

Run: `pnpm --dir web test -- --run`

Expected: FAIL until pages are implemented.

**Step 4: Implement pages and state stores**

Keep transport calls in stores/services, not chart components. Use request IDs or abort guards so late responses cannot replace a newer range. On a data-change event, refetch navigation and the visible page only. Base navigation remains visible when unavailable; installed additional dashboards appear after install.

**Step 5: Implement localization and tests**

Resolve selected locale → executable module English → core English → visible missing-key marker. Format dates/numbers through browser `Intl` using locale; do not translate source names. Add a test that enumerates all built-in keys used by Svelte and compares them with the English catalogs.

Run: `pnpm --dir web test -- --run`

Expected: PASS.

**Step 6: Add browser E2E through mock transport**

Playwright starts Vite with a deterministic mock transport and tests first-load, range navigation, disabled graph explanation, populated graph, Settings lifecycle, quality retry, phase event, locale fallback, and blocked recovery banner. This test does not start DuckDB.

Run: `pnpm --dir web exec playwright test web/e2e/dashboard.spec.ts`

Expected: PASS.

**Step 7: Commit**

```bash
git add web
git commit -m "feat: render local analytics dashboard"
```

---

### Task 7: Close the analytics and UI gate with real transport integration

**Files:**

- Create: `crates/mfa-integration-tests/tests/dashboard_gate.rs`
- Create: `web/e2e/fixtures/expected-dashboard.json`
- Create: `scripts/run-dashboard-gate.sh`
- Create: `docs/dashboard-module-authoring.md`

**Step 1: Write the failing integrated dashboard gate**

Import synthetic MyNetDiary/Hevy snapshots through production modules, request every base page through application command services, and compare reviewed semantic golden values and availability states. Do not snapshot volatile IDs, timestamps, or formatting; assert those separately by type/shape.

Run: `bash scripts/run-dashboard-gate.sh`

Expected: FAIL before final integration wiring.

**Step 2: Document dashboard authoring**

Include a complete minimal component, manifest, dependency/extension declarations, dataset grant shape, availability rules, locale namespace, supported document nodes, package/test commands, limits, and rejection cases. Include an explicit cross-source example requiring two dependencies without adding it as an MVP feature.

**Step 3: Run the full gate**

```bash
bash scripts/run-dashboard-gate.sh
cargo test --workspace -- --test-threads=1
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm --dir web exec prettier --check .
pnpm --dir web exec eslint .
pnpm --dir web exec svelte-check --tsconfig ./tsconfig.json
pnpm --dir web test -- --run
pnpm --dir web build
```

Expected: PASS.

**Step 4: Commit**

```bash
git add crates/mfa-integration-tests web/e2e scripts/run-dashboard-gate.sh docs/dashboard-module-authoring.md
git commit -m "test: verify analytics dashboard end to end"
```

## Plan Completion Evidence

Write `docs/superpowers/evidence/analytics-ui.md` with reviewed golden cases, all availability states, dependency isolation/security test names, frontend accessibility results, and the full gate output. Close the plan only when every graph returns either reviewed data or an exact non-ready state through the real application service composition.
