# MyFitAnalytics Bundled Source Modules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan. Use `superpowers:test-driven-development` for every parser and mapping behavior and `superpowers:verification-before-completion` before closing the plan.

**Goal:** Implement installable MyNetDiary and Hevy source components, then expose the minimum Settings workflow required to choose a workspace and install, enable, disable, update, and uninstall module packages without rebuilding or restarting the desktop application.

**Architecture:** Each source is a separate Rust guest component under `modules/sources`. Guests accept archive bytes and metadata only, detect content rather than trusting filenames, and emit source records, canonical observations, extensions, and mapping issues. A shared host conformance harness builds, packages, installs, and invokes each component without granting ambient authority. The Tauri host owns native workspace/package pickers and the mutable module registry; a minimal Svelte Settings page invokes only typed commands and reflects registry changes in the same application session.

**Tech Stack:** Rust 1.94.0, WebAssembly Component Model, `cargo-component`, Calamine 0.36.1 with `chrono` for BIFF `.xls`, `csv`, Serde, Chrono, Tauri 2 native dialogs, Svelte 5, Vitest, synthetic BIFF8/CSV fixtures, `insta` snapshots.

**Spec:** [MVP-SPEC.md Sections 4–8, 10, 12–13, 14.5–14.9, 19.1–19.4, 20](</Users/simarglok/Library/Mobile Documents/iCloud~md~obsidian/Documents/Simarglok/MyFitAnalytics/MVP-SPEC.md>)

## Global Constraints

- Start only after the Storage and Ingestion gate passes.
- Never commit the user’s actual exports or any derived personally identifying values.
- Fixtures must be synthetic and must preserve format signatures, column spelling, empty cells, duplicate rows, decimal comma/NBSP, reverse ordering, and optional-sheet/column behavior.
- File extension and filename are hints only. Probe BIFF CDFV2 signature and workbook structure; probe CSV header and UTF-8 validity.
- Guests emit raw source values before normalization and stable row numbers for lineage.
- Missing numeric input becomes `None`; it never becomes zero.
- Unknown governed values become explicit quality issues and remain traceable.
- MyNetDiary strength rows produce no canonical activity; Hevy owns strength capabilities.
- MyNetDiary body-weight/body-fat rows produce no base body capability; Hevy owns them in the MVP default provider configuration.
- By this plan's acceptance gate, a user can choose the workspace and manage local `.mfasource`, `.mfadashboard`, and `.mfalocale` packages from Settings without rebuilding or restarting the application.
- Native pickers return only a user-approved directory or package path. The frontend never receives unrestricted filesystem access.

---

### Task 1: Create the source SDK, conformance harness, fixture factory, and package builder

**Files:**

- Create: `modules/sdk/rust/Cargo.toml`
- Create: `modules/sdk/rust/src/lib.rs`
- Create: `crates/mfa-source-contract-tests/Cargo.toml`
- Create: `crates/mfa-source-contract-tests/src/lib.rs`
- Create: `crates/mfa-source-contract-tests/src/assertions.rs`
- Create: `crates/mfa-source-contract-tests/src/harness.rs`
- Create: `scripts/build-module-packages.sh`
- Create: `scripts/fixtures/package.json`
- Create: `scripts/fixtures/build_mynetdiary_fixtures.mjs`
- Create: `scripts/fixtures/verify_fixture_privacy.mjs`
- Create: `modules/sources/mynetdiary/tests/fixtures/fixture-manifest.json`
- Create: `modules/sources/hevy/tests/fixtures/measurement_data.csv`
- Create: `modules/sources/hevy/tests/fixtures/workout_data.csv`
- Test: `crates/mfa-source-contract-tests/tests/harness_self_test.rs`

**Interfaces:**

```rust
pub trait SourceGuest {
    fn descriptor() -> SourceDescriptor;
    fn detect(asset: &mut GuestAssetReader) -> u8;
    fn validate(asset: &mut GuestAssetReader) -> Result<SourceValidation, GuestError>;
    fn parse(asset: &mut GuestAssetReader) -> Result<SourceBatch, GuestError>;
}
pub struct ContractHarness { runtime: ComponentRuntime, package_installer: PackageInstaller }
impl ContractHarness {
    pub async fn assert_conforms(&self, package: &Path, cases: &[SourceCase]) -> Result<ConformanceReport, ContractTestError>;
}
pub struct SourceCase { pub fixture: PathBuf, pub expected_probe: ProbeExpectation, pub expected_result: ExpectedResult }
```

**Step 1: Write a failing harness self-test**

Use the Foundation fake guest to prove the harness verifies manifest/API compatibility, package hash, probe determinism, parse determinism, declared capabilities, logical snapshot key, source-record uniqueness, canonical lineage, extension contracts, stable issue codes, output size, and absence of forbidden imports.

Run: `cargo test -p mfa-source-contract-tests --test harness_self_test`

Expected: FAIL because the harness does not exist.

**Step 2: Add fixture construction and privacy checks**

Pin `xlsx@0.18.5` inside the isolated `scripts/fixtures` package and write BIFF8 workbooks with `bookType: "biff8"`. Generate these committed fixtures:

```text
valid-full.xls
missing-required-sheet.xls
optional-sheets-absent.xls
schema-drift.xls
mixed-year.xls
unknown-activity.xls
decimal-comma-nbsp.xls
```

The generator uses fictional dates, foods, activities, notes, and numeric values. `fixture-manifest.json` records each file SHA-256 and scenario, not personal provenance. `verify_fixture_privacy.mjs` scans extracted workbook strings and CSV text against a denylist of real names, emails, IDs, and source filenames supplied in the script, then verifies every digest.

Run:

```bash
pnpm --dir scripts/fixtures install
pnpm --dir scripts/fixtures run build
pnpm --dir scripts/fixtures run verify
```

Expected: PASS and no user export is read by the generator.

**Step 3: Implement SDK helpers and package build**

SDK helpers must canonicalize JSON, validate finite numbers, construct source-record keys from asset/sheet/row, and attach lineage. `scripts/build-module-packages.sh` runs `cargo component build --release`, assembles `module.json`, component, and `locales/en.json`, hashes entries, creates deterministic zip ordering/timestamps, and writes `.mfasource` packages to `dist/modules/`.

**Step 4: Run the self-test and commit**

```bash
cargo test -p mfa-source-contract-tests
bash scripts/build-module-packages.sh --fixture-only
pnpm --dir scripts/fixtures run verify
git add modules/sdk crates/mfa-source-contract-tests scripts modules/sources/*/tests/fixtures
git commit -m "test: add source module conformance harness"
```

---

### Task 2: Detect and validate MyNetDiary BIFF yearly snapshots

**Files:**

- Create: `modules/sources/mynetdiary/Cargo.toml`
- Create: `modules/sources/mynetdiary/module.json`
- Create: `modules/sources/mynetdiary/locales/en.json`
- Create: `modules/sources/mynetdiary/src/lib.rs`
- Create: `modules/sources/mynetdiary/src/workbook.rs`
- Create: `modules/sources/mynetdiary/src/schema.rs`
- Create: `modules/sources/mynetdiary/src/cells.rs`
- Create: `modules/sources/mynetdiary/src/error.rs`
- Test: `modules/sources/mynetdiary/tests/workbook_contract.rs`

**Interfaces:**

```rust
pub struct WorkbookSchema { pub sheets: BTreeMap<SheetKind, ValidatedSheet> }
pub enum SheetKind { Food, Measurements, Exercise, Trackers, WaterGlasses }
pub fn detect_mynetdiary(asset: &mut GuestAssetReader) -> u8;
pub fn validate_workbook(bytes: &[u8]) -> Result<WorkbookSchema, MappingError>;
pub fn infer_calendar_year(schema: &WorkbookSchema) -> Result<i32, MappingError>;
```

**Step 1: Write failing detection tests**

Assert the module accepts CDFV2/BIFF with required MyNetDiary sheets regardless of filename and rejects renamed CSV, XLSX zip, corrupt CDFV2, and arbitrary BIFF workbooks. Probe confidence must be deterministic and must not parse canonical output.

Run: `cargo test -p mfa-source-mynetdiary --test workbook_contract probe`

Expected: FAIL.

**Step 2: Write failing schema tests**

Assert `Food`, `Measurements`, and `Exercise` are required; `Trackers` and `Water Glasses` are optional; `Notes` and `Vitamins` are ignored. Match headers after trimming only, preserving exact diagnostic text. Reject missing/duplicate required columns and dates spanning more than one calendar year. The logical key is `mynetdiary:<content-year>` and never comes from the filename.

Run: `cargo test -p mfa-source-mynetdiary --test workbook_contract schema`

Expected: FAIL.

**Step 3: Implement byte-backed Calamine reading**

Use `calamine::Xls<std::io::Cursor<Vec<u8>>>` so the guest never needs a path. Normalize cell access into a typed adapter that returns the original display value and a parsed value. Map workbook errors to stable source codes such as `mynetdiary.invalid_biff`, `mynetdiary.missing_sheet`, `mynetdiary.missing_column`, and `mynetdiary.mixed_calendar_year`.

**Step 4: Run tests and commit**

```bash
cargo test -p mfa-source-mynetdiary --test workbook_contract
cargo clippy -p mfa-source-mynetdiary --all-targets -- -D warnings
git add modules/sources/mynetdiary
git commit -m "feat: validate MyNetDiary yearly xls exports"
```

---

### Task 3: Map all approved MyNetDiary sheets to canonical observations

**Files:**

- Create: `modules/sources/mynetdiary/src/number.rs`
- Create: `modules/sources/mynetdiary/src/datetime.rs`
- Create: `modules/sources/mynetdiary/src/food.rs`
- Create: `modules/sources/mynetdiary/src/measurements.rs`
- Create: `modules/sources/mynetdiary/src/activity.rs`
- Create: `modules/sources/mynetdiary/src/activity_mapping.json`
- Create: `modules/sources/mynetdiary/src/trackers.rs`
- Create: `modules/sources/mynetdiary/src/water.rs`
- Modify: `modules/sources/mynetdiary/src/lib.rs`
- Test: `modules/sources/mynetdiary/tests/food.rs`
- Test: `modules/sources/mynetdiary/tests/measurements.rs`
- Test: `modules/sources/mynetdiary/tests/activity.rs`
- Test: `modules/sources/mynetdiary/tests/optional_sheets.rs`
- Test: `modules/sources/mynetdiary/tests/conformance.rs`

**Interfaces:**

```rust
pub fn parse_number(raw: &str) -> Result<Option<f64>, ValueError>;
pub fn map_food(sheet: &ValidatedSheet, ctx: &MappingContext) -> Result<MappedRows, MappingError>;
pub fn map_measurements(sheet: &ValidatedSheet, ctx: &MappingContext) -> Result<MappedRows, MappingError>;
pub fn map_activity(sheet: &ValidatedSheet, mapping: &ActivityMapping, ctx: &MappingContext) -> Result<MappedRows, MappingError>;
pub fn map_trackers(sheet: Option<&ValidatedSheet>, ctx: &MappingContext) -> Result<MappedRows, MappingError>;
pub fn map_water(sheet: Option<&ValidatedSheet>, ctx: &MappingContext) -> Result<MappedRows, MappingError>;
```

**Step 1: Write failing Food tests**

Prove one row becomes one `NutritionItem`; two identical rows remain two records; `Food ID` is not entry identity; amount stays raw; optional blank nutrients are null; typed nutrients parse dot/comma/NBSP forms; unknown columns remain in source-record raw payload; local datetime and local date remain distinct.

Run: `cargo test -p mfa-source-mynetdiary --test food`

Expected: FAIL.

**Step 2: Implement Food mapping and rerun**

Iterate workbook rows with 1-based source row numbers including the header offset. Construct deterministic entity IDs from attempt-local row identity, never from food content. Reject non-finite or negative nutrients with a row-level schema error.

Run: `cargo test -p mfa-source-mynetdiary --test food`

Expected: PASS.

**Step 3: Write failing Measurements tests, implement, and rerun**

Assert only `Daily Steps Count` emits `ActivityDay` input; other measurement types remain only in raw source records. Date is `LocalDate`; non-negative integral steps are required.

Run before implementation: `cargo test -p mfa-source-mynetdiary --test measurements`

Expected: FAIL.

Run after implementation: same command; Expected: PASS.

**Step 4: Write failing Exercise tests**

Cover walking, running, hiking, cycling, elliptical, both treadmill names, and stretching. Assert `Traditional Strength Training` emits no canonical activity and no unknown-mapping warning. Unknown names emit `activity_type=unknown`, `quality_status=unknown_mapping`, and a quality issue. Parse duration, distance, calories, decimal comma, and NBSP while preserving raw amount/notes. `apple_health` is allowed only when provided as user `origin_hint`; it is never inferred from the file.

Run: `cargo test -p mfa-source-mynetdiary --test activity`

Expected: FAIL.

**Step 5: Implement governed activity mapping**

Deserialize the checked-in `activity_mapping.json` into exact-name rules. Keep parser mechanics independent of mapping values. Unknown mapping is non-fatal and excluded from accepted aggregates by `quality_status`.

Run: `cargo test -p mfa-source-mynetdiary --test activity`

Expected: PASS.

**Step 6: Write and implement optional-sheet tests**

For `Trackers`, import only exact `Heart Rate`, use bpm only for that governed mapping, preserve optional unit/notes/labels in raw payload, and keep local timestamp. For `Water Glasses`, use `Water, ml` canonically and emit glass count as `mynetdiary.water-glasses@1` extension. Missing optional sheets yield empty output without failure.

Run before implementation: `cargo test -p mfa-source-mynetdiary --test optional_sheets`

Expected: FAIL.

Run after implementation: same command; Expected: PASS.

**Step 7: Run conformance and commit**

```bash
cargo test -p mfa-source-mynetdiary
cargo test -p mfa-source-mynetdiary --test conformance
git add modules/sources/mynetdiary
git commit -m "feat: map MyNetDiary observations"
```

---

### Task 4: Implement the Hevy measurements source package

**Files:**

- Create: `modules/sources/hevy/Cargo.toml`
- Create: `modules/sources/hevy/module.json`
- Create: `modules/sources/hevy/locales/en.json`
- Create: `modules/sources/hevy/src/lib.rs`
- Create: `modules/sources/hevy/src/csv_input.rs`
- Create: `modules/sources/hevy/src/measurements.rs`
- Create: `modules/sources/hevy/src/error.rs`
- Test: `modules/sources/hevy/tests/measurements.rs`

**Interfaces:**

```rust
pub enum HevyArtifact { Measurements, Workouts }
pub fn detect_hevy(asset: &mut GuestAssetReader) -> ProbeResult<HevyArtifact>;
pub fn parse_measurements(input: CsvInput, ctx: &MappingContext) -> Result<SourceBatch, MappingError>;
```

**Step 1: Write failing probe and schema tests**

Detect UTF-8 comma CSV by required header sets, not filename. Measurements require `date` and `weight_kg`; `fat_percent` and circumference columns are optional. Reject invalid UTF-8, duplicate headers, absent required fields, and a file matching neither Hevy contract.

Run: `cargo test -p mfa-source-hevy --test measurements probe`

Expected: FAIL.

**Step 2: Write failing mapping tests**

Assert each row creates one `BodyMeasurement`; date including textual midnight becomes only `LocalDate`; weight must be positive; blank fat percentage is null; multiple same-date measurements remain separate; circumference fields remain in raw payload and optional extension records, not base canonical columns.

Run: `cargo test -p mfa-source-hevy --test measurements mapping`

Expected: FAIL.

**Step 3: Implement measurement mapping**

Use strict header-index mapping and flexible blank-field deserialization. Emit capabilities `body.weight` and `body.fat_percentage`; capability availability later depends on actual non-null coverage. Put recognized circumference values into `hevy.body-circumference@1` extension records with typed centimeters.

**Step 4: Run tests and commit**

```bash
cargo test -p mfa-source-hevy --test measurements
cargo clippy -p mfa-source-hevy --all-targets -- -D warnings
git add modules/sources/hevy
git commit -m "feat: import Hevy body measurements"
```

---

### Task 5: Implement Hevy workout grouping and set semantics

**Files:**

- Create: `modules/sources/hevy/src/workouts.rs`
- Create: `modules/sources/hevy/src/exercise_mapping.json`
- Modify: `modules/sources/hevy/src/lib.rs`
- Test: `modules/sources/hevy/tests/workouts.rs`
- Test: `modules/sources/hevy/tests/conformance.rs`

**Interfaces:**

```rust
pub fn parse_workouts(input: CsvInput, mapping: &ExerciseMapping, ctx: &MappingContext) -> Result<SourceBatch, MappingError>;
pub fn group_sessions(rows: Vec<WorkoutRow>) -> Result<Vec<WorkoutGroup>, MappingError>;
pub fn assign_exercise_blocks(rows: &[WorkoutRow]) -> Vec<u32>;
```

**Step 1: Write failing session tests**

Require `title,start_time,end_time,exercise_title,set_index,set_type`. Group by title plus start/end local timestamps even when input is reverse chronological. Preserve source row order inside the normalized session. Compute duration from local timestamps and reject end before start.

Run: `cargo test -p mfa-source-hevy --test workouts sessions`

Expected: FAIL.

**Step 2: Write failing set tests**

Assert consecutive exercise-title groups receive increasing block ordinals; repeated exercise titles separated by another exercise remain separate blocks. Set identity includes session, block ordinal, set index, and source row number. Accept warmup/normal/failure; preserve unknown type and mark it excluded from e1RM. Support blank weight/reps/RPE and duration-based sets.

Run: `cargo test -p mfa-source-hevy --test workouts sets`

Expected: FAIL.

**Step 3: Implement governed load-type mapping**

Map exact normalized exercise names to `external`, `bodyweight`, `assisted`, or `duration`. Unknown exercises receive `load_type=unknown` and a visible mapping issue. Do not infer an external load from a non-null number when the exercise mapping says assisted/bodyweight.

**Step 4: Run Hevy conformance and commit**

```bash
cargo test -p mfa-source-hevy
cargo test -p mfa-source-hevy --test conformance
git add modules/sources/hevy
git commit -m "feat: import Hevy workouts and sets"
```

---

### Task 6: Package bundled modules and prove production-path ingestion

**Files:**

- Modify: `scripts/build-module-packages.sh`
- Create: `scripts/verify-module-packages.sh`
- Modify: `src-tauri/src/app.rs`
- Create: `src-tauri/resources/modules/.gitkeep`
- Modify: `src-tauri/tauri.conf.json`
- Create: `crates/mfa-integration-tests/tests/source_modules_gate.rs`
- Create: `crates/mfa-integration-tests/tests/provider_selection.rs`
- Create: `docs/source-module-authoring.md`

**Step 1: Write failing package verification tests**

Build both `.mfasource` packages twice and assert byte-identical output. Inspect manifests, entry hashes, English namespaces, accepted artifact declarations, WIT API `1.0.0`, mapping versions, and absence of forbidden imports.

Run: `bash scripts/verify-module-packages.sh`

Expected: FAIL before bundled packaging is complete.

**Step 2: Write the failing integration gate**

Install built packages through `PackageInstaller`, copy synthetic assets to real inbox paths, invoke `Refresh Now`, and query through `DatabaseService`. Assert exact canonical counts/values, source rows, lineage, extension contracts, issues, and logical keys for all fixtures. Then import a replacement snapshot and prove removed rows disappear while provenance remains.

Run: `cargo test -p mfa-integration-tests --test source_modules_gate -- --test-threads=1`

Expected: FAIL.

**Step 3: Write failing provider-selection tests**

Install a second fixture provider for `body.weight`. Assert ingestion retains both sources but active views require one configured provider, never merge values, and atomically change after provider selection. Confirm the bundled defaults choose MyNetDiary for nutrition/activity and Hevy for body/strength.

Run: `cargo test -p mfa-integration-tests --test provider_selection -- --test-threads=1`

Expected: FAIL.

**Step 4: Embed and install bundled packages**

Build packages before Tauri resource assembly and embed them as an immutable available-module catalog. On the first profile launch, install the approved defaults through the same inspector and store used for local packages. A later bundled version appears as an available update but does not silently reinstall an uninstalled module or overwrite provider choices.

**Step 5: Document the authoring contract**

`docs/source-module-authoring.md` must show repository layout, manifest fields, WIT exports, canonical/extension output rules, English namespace, build/package/test commands, resource constraints, prohibited imports, versioning rules, and compatibility failure behavior using a complete minimal guest example.

**Step 6: Run the source-module gate**

```bash
pnpm --dir scripts/fixtures run verify
bash scripts/build-module-packages.sh
bash scripts/verify-module-packages.sh
cargo test --workspace -- --test-threads=1
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

**Step 7: Commit**

```bash
git add scripts src-tauri crates/mfa-integration-tests docs/source-module-authoring.md
git commit -m "feat: bundle verified MyNetDiary and Hevy modules"
```

---

### Task 7: Make workspace and module lifecycle operable from Settings

**Files:**

- Modify: `Cargo.lock`
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/dialogs.rs`
- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `modules/locales/en/messages.json`
- Modify: `web/src/App.svelte`
- Modify: `web/src/styles.css`
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/lib/transport.ts`
- Modify: `web/src/lib/tauri-transport.ts`
- Modify: `web/src/lib/mock-transport.ts`
- Create: `web/src/lib/pages/SettingsPage.svelte`
- Test: `src-tauri/tests/module_lifecycle_commands.rs`
- Test: `web/src/lib/pages/SettingsPage.test.ts`

**Interfaces:**

```rust
pub struct ModuleCatalogEntryView {
    pub module: ModuleView,
    pub origin: String,
    pub install_state: String,
    pub available_version: Option<String>,
}

#[tauri::command]
async fn choose_workspace_root(
    window: tauri::Window,
    state: tauri::State<'_, AppState>,
) -> Result<Option<WorkspaceView>, CommandError>;

#[tauri::command]
async fn list_module_catalog(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ModuleCatalogEntryView>, CommandError>;

#[tauri::command]
async fn choose_and_install_module(
    window: tauri::Window,
    state: tauri::State<'_, AppState>,
) -> Result<Option<ModuleView>, CommandError>;

#[tauri::command]
async fn set_module_enabled(
    module_id: String,
    enabled: bool,
    state: tauri::State<'_, AppState>,
) -> Result<ModuleView, CommandError>;

#[tauri::command]
async fn uninstall_module(
    module_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), CommandError>;
```

**Step 1: Write failing native lifecycle tests**

Use a mock dialog port. Assert the workspace picker accepts directories only; the package picker filters `.mfasource`, `.mfadashboard`, and `.mfalocale`; cancellation returns `Ok(None)`; incompatible or malformed packages return stable `PackageError` codes; and a successful install is visible through `list_module_catalog` without restart. Installing a compatible newer version must expose the update in the catalog, preserve the current package after failed inspection, and switch only after successful installation. Assert enabling a source requires a configured workspace, creates its inbox/archive subtree, refreshes capability/provider resolution, and starts its ingestion coordinator. Assert disable stops new scans without deleting archive, provenance, or canonical history. Assert uninstall requires the module to be disabled, removes only executable package bytes, and leaves bundled catalog entries available for reinstall.

Run: `cargo test -p myfitanalytics --test module_lifecycle_commands -- --test-threads=1`

Expected: FAIL because the application state is immutable after startup and native picker commands do not exist.

**Step 2: Write failing Settings UI tests**

Render configured and unconfigured states through `MockTransport`. Assert Settings offers `Choose Workspace...` and `Install Module Package...`, groups catalog entries into Sources/Dashboards/Language, shows installed/available/enabled/disabled/incompatible/update states, confirms uninstall, reports stable localized errors, and refreshes the list after every successful action. A source cannot be enabled before workspace selection, and the UI explains why. Cancellation changes no state and is not shown as an error.

Run: `pnpm --dir web test -- --run SettingsPage`

Expected: FAIL because the minimal Settings page and lifecycle transport methods do not exist.

**Step 3: Implement native pickers and mutable registry refresh**

Wrap Tauri dialogs behind a mockable `DialogPort`. Keep the selected package path inside the Rust command; return the configured workspace path only as part of `WorkspaceView` for display. Pass an approved package path directly to `PackageInstaller`, then rebuild installed-module, capability-provider, and locale state atomically before returning a view model. Reconfigure only affected source coordinators after enable/disable; do not open another DuckDB connection. The package picker grants access only to the selected file and the inspector remains authoritative for type, extension, compatibility, hashes, archive traversal, and executable-content validation.

**Step 4: Implement the minimal Settings page**

Keep lifecycle calls in `AppTransport`; Settings receives typed catalog entries and never submits manually entered filesystem paths. Preserve the existing storage-health and installed-module summary on the application shell, add navigation to Settings, display the configured workspace and each enabled source inbox path from `WorkspaceView`, and use localization keys for every new user-facing string. Bundled MyNetDiary and Hevy packages appear as available or installed entries and use the same lifecycle commands as user-selected packages.

**Step 5: Run lifecycle and regression gates**

```bash
cargo test -p myfitanalytics --test module_lifecycle_commands -- --test-threads=1
cargo test --workspace -- --test-threads=1
pnpm --dir web test -- --run SettingsPage
pnpm --dir web check
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

**Step 6: Commit**

```bash
git add Cargo.lock src-tauri modules/locales/en web
git commit -m "feat: manage module packages from settings"
```

## Plan Completion Evidence

Write `docs/superpowers/evidence/source-modules.md` with fixture digests, package digests, conformance reports, canonical count/value assertions, provider-selection evidence, native lifecycle test output, Settings UI test output, and a macOS smoke showing workspace selection plus package install/disable/re-enable without rebuilding or restarting the application. The user’s external exports may be used for a local non-committed validation run; record only redacted schema fingerprints and pass/fail outcomes, never raw values or paths.
