# MyFitAnalytics Foundation and Module Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan. Use `superpowers:test-driven-development` for every behavior and `superpowers:verification-before-completion` before closing the plan.

**Goal:** Establish the buildable application skeleton, versioned contracts, installable module packages, sandboxed Wasmtime host, capability selection, locale fallback, and typed Tauri-to-Svelte bridge.

**Architecture:** The repository is a Rust workspace with a pnpm frontend workspace. `mfa-contracts` is dependency-leaf shared vocabulary; `mfa-module-host` validates and installs packages and is the only crate that instantiates Wasm components. The Tauri binary composes services but does not contain source-specific mapping logic. A minimal UI proves that typed state crosses the native bridge without a localhost server.

**Tech Stack:** Rust 1.94.0/edition 2024, Cargo, Tauri 2.11, Wasmtime 47 Component Model, `wit-bindgen`, Serde, JSON Schema, Node.js 24.19 LTS, pnpm 11.23, Svelte 5.56, TypeScript, Vite 8.2, Vitest 4.

**Spec:** [MVP-SPEC.md Sections 5, 14.1–14.9, 17.1, 18, 19.2, 19.7–19.9](</Users/simarglok/Library/Mobile Documents/iCloud~md~obsidian/Documents/Simarglok/MyFitAnalytics/MVP-SPEC.md>)

## Global Constraints

- Initialize Git in the existing empty workspace; do not overwrite files that appear after plan creation.
- Keep `mfa-contracts` free of Tauri, DuckDB, Wasmtime, filesystem, and UI dependencies.
- Component packages are zip containers with canonical relative paths and a manifest hash. Reject traversal, symlinks, duplicate entries, and unexpected executable files.
- Do not add WASI to source or dashboard linkers. Guests get no ambient authority.
- Use explicit semantic versions for host API, mapping contract, package format, and extension contracts.
- Store user-installed modules outside the repository. Tests use temporary module stores.

---

### Task 1: Initialize the repository and reproducible workspaces

**Files:**

- Create: `.gitignore`
- Create: `.node-version`
- Create: `rust-toolchain.toml`
- Create: `Cargo.toml`
- Create: `Cargo.lock`
- Create: `package.json`
- Create: `pnpm-workspace.yaml`
- Create: `pnpm-lock.yaml`
- Create: `web/package.json`
- Create: `web/tsconfig.json`
- Create: `web/vite.config.ts`
- Create: `web/index.html`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/main.rs`
- Test: `scripts/check-workspace.sh`

**Step 1: Initialize Git and add the failing workspace check**

Run:

```bash
git init
mkdir -p scripts web/src src-tauri/src src-tauri/capabilities
```

Create `scripts/check-workspace.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
test "$(rustc --version | awk '{print $2}')" = "1.94.0"
test "$(node --version)" = "v24.19.0"
test "$(pnpm --version)" = "11.23.0"
cargo metadata --no-deps --format-version 1 >/dev/null
pnpm --dir web exec vite --version >/dev/null
cargo tauri --version >/dev/null
```

Run: `bash scripts/check-workspace.sh`

Expected: FAIL because the toolchain and manifests do not exist.

**Step 2: Create the root manifests**

Use this workspace shape in `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/*", "modules/sources/*", "modules/dashboards/*", "src-tauri"]

[workspace.package]
edition = "2024"
rust-version = "1.94.0"
license = "MIT"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
semver = { version = "1", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync", "time"] }
wasmtime = { version = "=47.0.3", default-features = false, features = ["async", "component-model", "component-model-async", "cranelift", "runtime"] }
```

Pin `rust-toolchain.toml` to `1.94.0` with `rustfmt` and `clippy`; this satisfies Wasmtime 47 while remaining compatible with `duckdb-rs`. Pin `.node-version` and `engines.node` to `24.19.0`, and set root `packageManager` to `pnpm@11.23.0`. The root `package.json` exposes `check`, `test`, and `build` scripts that delegate to `web`. The frontend manifest pins Svelte `5.56.10`, Vite `8.2.2`, Vitest `4.1.10`, TypeScript, `svelte-check`, and `@sveltejs/vite-plugin-svelte` to lockfile-resolved versions. `.gitignore` excludes `target/`, `node_modules/`, `web/dist/`, and generated `dist/modules/` packages.

Configure `tauri.conf.json` with `devUrl: http://localhost:1420` only for the Vite development process and `frontendDist: ../web/dist` for the shipped app. No production HTTP server is created.

**Step 3: Generate lockfiles and prove the skeleton builds**

Run:

```bash
cargo generate-lockfile
pnpm install --frozen-lockfile=false
bash scripts/check-workspace.sh
cargo check --workspace
pnpm --dir web exec svelte-check --tsconfig ./tsconfig.json
```

Expected: all commands exit 0.

**Step 4: Commit**

```bash
git add .
git commit -m "chore: initialize MyFitAnalytics workspace"
```

---

### Task 2: Define canonical host contracts, WIT worlds, and manifest schemas

**Files:**

- Create: `crates/mfa-contracts/Cargo.toml`
- Create: `crates/mfa-contracts/src/lib.rs`
- Create: `crates/mfa-contracts/src/asset.rs`
- Create: `crates/mfa-contracts/src/capability.rs`
- Create: `crates/mfa-contracts/src/module.rs`
- Create: `crates/mfa-contracts/src/observation.rs`
- Create: `crates/mfa-contracts/src/dashboard.rs`
- Create: `crates/mfa-contracts/src/locale.rs`
- Create: `crates/mfa-contracts/src/error.rs`
- Create: `modules/sdk/wit/source-api.wit`
- Create: `modules/sdk/wit/dashboard-api.wit`
- Create: `modules/sdk/schemas/source-manifest.schema.json`
- Create: `modules/sdk/schemas/dashboard-manifest.schema.json`
- Create: `modules/sdk/schemas/locale-manifest.schema.json`
- Test: `crates/mfa-contracts/tests/contract_roundtrip.rs`
- Test: `crates/mfa-contracts/tests/schema_examples.rs`

**Interfaces:**

```rust
pub struct ContractVersion(pub semver::Version);
pub struct CapabilityId(pub String);
pub struct ModuleId(pub String);
pub struct AssetMetadata { pub asset_id: uuid::Uuid, pub file_name: String, pub media_type: String, pub byte_len: u64 }
pub trait ReadOnlyAsset: Send + Sync { fn metadata(&self) -> AssetMetadata; fn read_at(&self, offset: u64, max_bytes: u32) -> Result<Vec<u8>, AssetReadError>; }
pub struct SourceBatch { pub records: Vec<CanonicalObservation>, pub extensions: Vec<ExtensionRecord>, pub issues: Vec<MappingIssue> }
pub enum CanonicalObservation { NutritionItem(NutritionItem), BodyMeasurement(BodyMeasurement), ActivityEvent(ActivityEvent), ActivityDay(ActivityDay), HeartRate(HeartRateObservation), WorkoutSession(WorkoutSession), ExerciseSet(ExerciseSet), PhaseEvent(PhaseEvent) }
pub struct DashboardRequirement { pub capability: CapabilityId, pub extension: Option<ExtensionRequirement> }
pub enum AvailabilityState { MissingCapability, MissingDependency, IncompatibleContract, WaitingForData, InsufficientCoverage, Ready, DisabledByUser }
pub struct LocaleBundle { pub locale: String, pub namespace: String, pub messages: std::collections::BTreeMap<String, String> }
```

**Step 1: Write failing serialization and schema tests**

Test these facts:

- every enum uses stable tagged JSON rather than Rust variant order;
- `LocalDate`, `LocalDateTime`, and `UtcInstant` are distinct serialized types;
- extension records require `{namespace, contract_version, record_type, payload}`;
- source manifests require `module_type: source`, package/API/mapping versions, capabilities, accepted file patterns, entrypoint hash, and English namespace;
- dashboard manifests require declared base and extension dependencies;
- locale manifests reject executable entries.

Run: `cargo test -p mfa-contracts`

Expected: FAIL because the crate and schemas are absent.

**Step 2: Implement the Rust vocabulary**

Use newtypes for IDs and temporal semantics. Add `#[serde(tag = "type", content = "value", rename_all = "snake_case")]` to versioned unions. Reject blank IDs and non-semver contract versions through `TryFrom` constructors. Keep user-facing messages out of error enums; errors carry stable codes and structured fields.

**Step 3: Define the source WIT world**

`source-api.wit` imports one host-owned read-only resource and exports the four operations required by the source contract:

```wit
package myfitanalytics:source@1.0.0;

interface host-asset {
  record asset-metadata {
    asset-id: string,
    file-name: string,
    media-type: string,
    byte-len: u64,
  }

  resource asset-reader {
    metadata: func() -> asset-metadata;
    read-at: func(offset: u64, max-bytes: u32) -> result<list<u8>, string>;
  }
}

world source-module {
  import host-asset;
  use host-asset.{asset-reader};

  export metadata: func() -> string;
  export detect: func(asset: borrow<asset-reader>) -> u8;
  export validate: func(asset: borrow<asset-reader>) -> result<string, string>;
  export parse: func(asset: borrow<asset-reader>) -> result<string, string>;
}
```

The metadata, validation report, and observation batch JSON strings are canonical serialized `SourceDescriptor`, `SourceValidation`, and `SourceBatch`. The host validates them against Rust types immediately; JSON is used here to keep the 1.0 WIT surface stable while canonical record variants evolve under explicit contract versions. The resource exposes immutable chunked reads only and does not reveal an archive path.

**Step 4: Define the dashboard WIT world**

`dashboard-api.wit` exports a pure typed-view projection:

```wit
package myfitanalytics:dashboard@1.0.0;

world dashboard-module {
  export describe: func() -> string;
  export compose: func(input-json: string) -> result<string, string>;
}
```

`input-json` is a validated `DashboardInput`; output is a validated declarative `DashboardDocument`. The schema permits cards, tables, status panels, and supported chart specifications, but no JavaScript, HTML, URLs, SQL, or event handlers.

**Step 5: Run contract tests**

Run:

```bash
cargo test -p mfa-contracts
cargo fmt --all --check
cargo clippy -p mfa-contracts --all-targets -- -D warnings
```

Expected: PASS.

**Step 6: Commit**

```bash
git add crates/mfa-contracts modules/sdk
git commit -m "feat: define versioned module contracts"
```

---

### Task 3: Validate, install, update, disable, and remove module packages

**Files:**

- Create: `crates/mfa-module-host/Cargo.toml`
- Create: `crates/mfa-module-host/src/lib.rs`
- Create: `crates/mfa-module-host/src/package.rs`
- Create: `crates/mfa-module-host/src/registry.rs`
- Create: `crates/mfa-module-host/src/store.rs`
- Create: `crates/mfa-module-host/src/error.rs`
- Test: `crates/mfa-module-host/tests/package_lifecycle.rs`
- Test: `crates/mfa-module-host/tests/package_security.rs`
- Fixture: `crates/mfa-module-host/tests/fixtures/valid-source.mfasource`
- Fixture: `crates/mfa-module-host/tests/fixtures/traversal-source.mfasource`

**Interfaces:**

```rust
pub struct PackageInstaller { store_root: std::path::PathBuf }
impl PackageInstaller {
    pub fn inspect(&self, package: &std::path::Path) -> Result<InspectedPackage, PackageError>;
    pub fn install(&self, package: &std::path::Path) -> Result<InstalledModule, PackageError>;
    pub fn set_enabled(&self, id: &ModuleId, enabled: bool) -> Result<(), PackageError>;
    pub fn uninstall(&self, id: &ModuleId) -> Result<(), PackageError>;
}
pub trait ModuleRegistry {
    fn list(&self) -> Result<Vec<InstalledModule>, PackageError>;
    fn resolve_active(&self, id: &ModuleId) -> Result<InstalledModule, PackageError>;
}
```

**Step 1: Write failing lifecycle tests**

Prove:

- install copies into `<store>/<module-id>/<version>/<package-hash>/` through a staging directory and atomic rename;
- repeated installation is idempotent;
- update keeps old version until the new package passes inspection;
- disable persists without deleting bytes;
- uninstall removes only the selected installed package/version and leaves embedded catalog entries available for later reinstall;
- registry reconstruction from manifests survives a missing mutable index.

Run: `cargo test -p mfa-module-host --test package_lifecycle`

Expected: FAIL because package host behavior does not exist.

**Step 2: Write failing security tests**

Create zip fixtures that contain `../escape`, absolute paths, symlinks, duplicate `module.json`, hash mismatch, wrong extension, incompatible host API, an executable in a locale package, and more than the configured uncompressed byte limit. Each must return a distinct stable error code.

Run: `cargo test -p mfa-module-host --test package_security`

Expected: FAIL for missing validation.

**Step 3: Implement package inspection and atomic installation**

Compute SHA-256 over the original package bytes and each declared payload. Normalize every zip path before extraction. Parse manifest JSON under the matching schema, validate package extension against module type, and compare API semver against the host-supported range. Persist mutable enablement separately as `state.json` using temp-file plus rename.

**Step 4: Run tests and lint**

Run:

```bash
cargo test -p mfa-module-host --test package_lifecycle
cargo test -p mfa-module-host --test package_security
cargo clippy -p mfa-module-host --all-targets -- -D warnings
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/mfa-module-host
git commit -m "feat: add safe module package lifecycle"
```

---

### Task 4: Execute source and dashboard components under strict Wasmtime limits

**Files:**

- Create: `crates/mfa-module-host/src/runtime.rs`
- Create: `crates/mfa-module-host/src/source_runtime.rs`
- Create: `crates/mfa-module-host/src/dashboard_runtime.rs`
- Create: `crates/mfa-module-host/src/limits.rs`
- Create: `crates/mfa-module-host/tests/fixtures/guest-source/Cargo.toml`
- Create: `crates/mfa-module-host/tests/fixtures/guest-source/src/lib.rs`
- Create: `crates/mfa-module-host/tests/fixtures/guest-dashboard/Cargo.toml`
- Create: `crates/mfa-module-host/tests/fixtures/guest-dashboard/src/lib.rs`
- Test: `crates/mfa-module-host/tests/runtime_contract.rs`
- Test: `crates/mfa-module-host/tests/runtime_limits.rs`

**Interfaces:**

```rust
pub struct RuntimeLimits { pub max_memory_bytes: usize, pub fuel: u64, pub timeout: std::time::Duration, pub max_output_bytes: usize }
pub struct ComponentRuntime;
impl ComponentRuntime {
    pub async fn invoke_source(&self, module: &InstalledModule, asset: std::sync::Arc<dyn ReadOnlyAsset>, limits: RuntimeLimits) -> Result<SourceBatch, RuntimeError>;
    pub async fn invoke_dashboard(&self, module: &InstalledModule, input: DashboardInput, limits: RuntimeLimits) -> Result<DashboardDocument, RuntimeError>;
}
```

**Step 1: Write a failing happy-path component test**

Build the fixture guest as a Wasm component. It describes one capability and transforms a known byte payload into one body measurement. Assert the host validates both descriptor and result before returning them.

Run: `cargo test -p mfa-module-host --test runtime_contract`

Expected: FAIL because no runtime bindings exist.

**Step 2: Write failing isolation and limit tests**

Prove that:

- no WASI filesystem, socket, environment, clock, or random imports are linked;
- fuel exhaustion returns `module_fuel_exhausted`;
- epoch timeout returns `module_timeout`;
- memory growth is capped;
- oversized output is rejected before deserialization;
- malformed JSON and undeclared capability output are rejected;
- one guest failure does not poison the next invocation.

Run: `cargo test -p mfa-module-host --test runtime_limits`

Expected: FAIL.

**Step 3: Implement the runtime**

Configure Wasmtime with component model, async support, fuel consumption, and epoch interruption. Use `StoreLimitsBuilder` for linear memory and table limits. Create a fresh store per invocation, link only generated WIT bindings, validate manifest hash before compilation, and cache compiled components by package hash. Do not construct a WASI context.

**Step 4: Run the runtime suite**

Run:

```bash
cargo test -p mfa-module-host --test runtime_contract
cargo test -p mfa-module-host --test runtime_limits
```

Expected: PASS, including timeout tests under two seconds each.

**Step 5: Commit**

```bash
git add crates/mfa-module-host
git commit -m "feat: sandbox wasm module execution"
```

---

### Task 5: Implement capability providers and locale fallback

**Files:**

- Create: `crates/mfa-config/Cargo.toml`
- Create: `crates/mfa-config/src/lib.rs`
- Create: `crates/mfa-config/src/settings.rs`
- Create: `crates/mfa-config/src/atomic_file.rs`
- Create: `crates/mfa-module-host/src/capabilities.rs`
- Create: `crates/mfa-module-host/src/locales.rs`
- Create: `modules/locales/en/module.json`
- Create: `modules/locales/en/messages.json`
- Test: `crates/mfa-config/tests/settings_recovery.rs`
- Test: `crates/mfa-module-host/tests/capability_registry.rs`
- Test: `crates/mfa-module-host/tests/locale_resolution.rs`

**Interfaces:**

```rust
pub struct AppSettings { pub schema_version: u32, pub locale: String, pub active_providers: std::collections::BTreeMap<CapabilityId, ModuleId> }
pub struct CapabilityRegistry;
impl CapabilityRegistry {
    pub fn resolve(&self, modules: &[InstalledModule], settings: &AppSettings) -> Result<ProviderResolution, CapabilityError>;
}
pub struct LocaleResolver;
impl LocaleResolver {
    pub fn message(&self, locale: &str, namespace: &str, key: &str, args: &serde_json::Value) -> ResolvedMessage;
}
```

**Step 1: Write failing settings and capability tests**

Test atomic save/reload, recovery from interrupted temp files, schema-version rejection, one-active-provider enforcement, disabled provider behavior, missing provider behavior, and deterministic ordering. A capability may be offered by many installed sources but resolves to at most one active provider.

Run: `cargo test -p mfa-config && cargo test -p mfa-module-host --test capability_registry`

Expected: FAIL.

**Step 2: Write failing locale tests**

Test resolution order: selected locale module namespace → executable module English namespace → core English namespace → visible stable missing-key marker. Validate placeholder names and reject namespace collisions across different module IDs.

Run: `cargo test -p mfa-module-host --test locale_resolution`

Expected: FAIL.

**Step 3: Implement atomic settings, provider resolution, and locale lookup**

Serialize sorted maps to a same-directory temporary file, sync the file, rename, and sync the parent directory where supported. Never infer a provider from data presence; use enabled module manifests and explicit settings. Parse ICU-style named placeholders without evaluating code.

**Step 4: Run tests**

Run:

```bash
cargo test -p mfa-config
cargo test -p mfa-module-host --test capability_registry
cargo test -p mfa-module-host --test locale_resolution
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/mfa-config crates/mfa-module-host modules/locales/en
git commit -m "feat: resolve capabilities and locale fallback"
```

---

### Task 6: Prove the Tauri/Svelte boundary with a minimal dashboard shell

**Files:**

- Create: `src-tauri/src/app.rs`
- Create: `src-tauri/src/commands.rs`
- Create: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/capabilities/default.json`
- Create: `web/src/main.ts`
- Create: `web/src/App.svelte`
- Create: `web/src/lib/types.ts`
- Create: `web/src/lib/transport.ts`
- Create: `web/src/lib/tauri-transport.ts`
- Create: `web/src/lib/mock-transport.ts`
- Create: `web/src/lib/i18n.ts`
- Create: `web/src/styles.css`
- Test: `src-tauri/tests/command_contract.rs`
- Test: `web/src/App.test.ts`

**Interfaces:**

```rust
#[tauri::command]
async fn get_bootstrap_state(state: tauri::State<'_, AppState>) -> Result<BootstrapState, CommandError>;

#[tauri::command]
async fn list_modules(state: tauri::State<'_, AppState>) -> Result<Vec<ModuleView>, CommandError>;
```

```ts
export interface AppTransport {
  getBootstrapState(): Promise<BootstrapState>;
  listModules(): Promise<ModuleView[]>;
}
```

**Step 1: Write failing Rust command tests**

Construct `AppState` with temporary config/module roots. Assert bootstrap returns English locale, empty provider selection, and installed bundled module metadata without exposing filesystem paths or internal errors.

Run: `cargo test -p myfitanalytics --test command_contract`

Expected: FAIL because commands are absent.

**Step 2: Write failing Svelte tests**

Using `MockTransport`, assert the app renders product title, module list, locale, loading state, and a structured error state. Assert no code imports `@tauri-apps/api` outside `tauri-transport.ts`.

Run: `pnpm --dir web test -- --run`

Expected: FAIL.

**Step 3: Implement the composition root and typed transport**

Create the native service graph in `app.rs`, register only the two commands, and restrict Tauri capabilities to the main window and required core commands. The Svelte app receives an `AppTransport`; production injects `TauriTransport`, tests inject `MockTransport`.

**Step 4: Run the foundation gate**

Run:

```bash
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm --dir web test -- --run
pnpm --dir web exec svelte-check --tsconfig ./tsconfig.json
pnpm --dir web build
cargo tauri build --debug --no-bundle
```

Expected: every command exits 0. The debug binary embeds `web/dist`; it does not require a production HTTP process.

**Step 5: Commit**

```bash
git add src-tauri web
git commit -m "feat: connect typed desktop and web shells"
```

## Plan Completion Evidence

Record the exact toolchain versions and complete gate output in `docs/superpowers/evidence/foundation.md`. The gate is closed only when the fake source package installation, Wasmtime invocation, capability selection, locale fallback, Tauri command contract, and frontend build all pass in the same working tree.
