# MyFitAnalytics Storage and Ingestion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan. Use `superpowers:test-driven-development` for every behavior and `superpowers:verification-before-completion` before closing the plan.

**Goal:** Turn stable inbox files into immutable archived assets and atomic, provenance-complete DuckDB snapshots through one actor-owned connection, with deterministic recovery from duplicates, errors, and crashes.

**Architecture:** Filesystem discovery, stability checks, copying, hashing, Wasm parsing, and validation occur outside the database actor. A bounded `DatabaseService` channel owns one DuckDB connection on one dedicated thread. Only immutable validated batches cross into `CommitSnapshot`, which writes provenance and canonical data and switches the active logical snapshot in one transaction.

**Tech Stack:** Rust 1.94.0, Tokio, `notify`, SHA-256, DuckDB 1.5.5 through `duckdb-rs ~1.10505.0` with `bundled`, Serde, Chrono, `tempfile`, `tracing`.

**Spec:** [MVP-SPEC.md Sections 9–13, 15, 18, 19.2, 19.4–19.5, 20](</Users/simarglok/Library/Mobile Documents/iCloud~md~obsidian/Documents/Simarglok/MyFitAnalytics/MVP-SPEC.md>)

## Global Constraints

- Start only after the Foundation gate passes.
- Never put DuckDB, config, logs, quarantine, recovery, module store, or temp files in `workspace_root`.
- Archive bytes are append-only. No application path deletes or overwrites a completed archive file.
- Do not remove an inbox file before archive copy, destination hash verification, and no-overwrite rename succeed or an exact archived duplicate is confirmed.
- No `duckdb::Connection`, transaction, prepared statement, or query row may leave the actor thread.
- Every mutable database write is a typed actor command. Parsing and hashing never run on the actor thread.
- A failed asset never replaces the previous active logical snapshot.

---

### Task 1: Create workspace paths, stable-file scanning, and immutable archive copies

**Files:**

- Modify: `crates/mfa-config/src/settings.rs`
- Create: `crates/mfa-config/src/paths.rs`
- Create: `crates/mfa-archive/Cargo.toml`
- Create: `crates/mfa-archive/src/lib.rs`
- Create: `crates/mfa-archive/src/scanner.rs`
- Create: `crates/mfa-archive/src/stability.rs`
- Create: `crates/mfa-archive/src/archive.rs`
- Create: `crates/mfa-archive/src/naming.rs`
- Create: `crates/mfa-archive/src/error.rs`
- Test: `crates/mfa-config/tests/path_policy.rs`
- Test: `crates/mfa-archive/tests/stability.rs`
- Test: `crates/mfa-archive/tests/archive_immutability.rs`

**Interfaces:**

```rust
pub struct AppPaths { pub app_data: PathBuf, pub database: PathBuf, pub recovery: PathBuf, pub quarantine: PathBuf, pub module_store: PathBuf, pub logs: PathBuf, pub tmp: PathBuf }
pub struct WorkspacePaths { pub root: PathBuf }
impl WorkspacePaths {
    pub fn source_inbox(&self, source: &ModuleId) -> PathBuf;
    pub fn source_archive(&self, source: &ModuleId) -> PathBuf;
}
pub struct FileFingerprint { pub size: u64, pub modified: std::time::SystemTime }
pub struct StabilityTracker;
impl StabilityTracker { pub fn observe(&mut self, path: &Path, fingerprint: FileFingerprint) -> StabilityState; }
pub struct ArchiveCoordinator;
impl ArchiveCoordinator { pub fn archive(&self, candidate: StableCandidate, received_at: UtcInstant) -> Result<ArchivedAsset, ArchiveError>; }
```

**Step 1: Write failing path-policy tests**

Assert enabling a source creates only `<workspace>/inbox/<id>` and `<workspace>/archive/<id>`. Reject a workspace root equal to or nested inside application data, and reject application data nested inside workspace. Confirm no provider-specific directory inference exists.

Run: `cargo test -p mfa-config --test path_policy`

Expected: FAIL.

**Step 2: Write failing stability tests**

Assert hidden files, editor temporaries, `.part`, `.tmp`, and archive temp suffixes are ignored. A candidate becomes stable only after two consecutive scans return equal size and modification time and a full open/read succeeds. Watcher events and periodic scans must produce the same `ScanRequest` type.

Run: `cargo test -p mfa-archive --test stability`

Expected: FAIL.

**Step 3: Write failing archive tests**

Using a temporary workspace, prove:

- the path is `<source>/<YYYY>/<YYYY-MM-DD>/<timestamp-with-micros>Z--<64-char-sha256>--<sanitized-name>`;
- copying uses a same-directory temporary path;
- destination bytes are rehashed before rename;
- rename refuses overwrite;
- exact bytes return the existing asset identity;
- same filename with different bytes creates a second asset;
- a copy/hash/rename failure leaves inbox untouched;
- successful archive or confirmed duplicate permits inbox deletion;
- no archive API exposes delete or overwrite.

Run: `cargo test -p mfa-archive --test archive_immutability`

Expected: FAIL.

**Step 4: Implement paths, stability, and archive coordination**

Use `OpenOptions::create_new(true)` for the final-path claim. Copy into a unique same-directory temp file, `sync_all`, calculate SHA-256 from destination bytes, compare with the source digest, then rename. Sanitize names to Unicode alphanumeric plus `._-`, replacing runs of other characters with `_`. Return an explicit `ArchiveDisposition::Created` or `ArchiveDisposition::ExistingExactDuplicate`.

**Step 5: Run tests and commit**

```bash
cargo test -p mfa-config --test path_policy
cargo test -p mfa-archive
cargo clippy -p mfa-archive --all-targets -- -D warnings
git add crates/mfa-config crates/mfa-archive
git commit -m "feat: archive stable inbox assets immutably"
```

---

### Task 2: Build the single-owner DuckDB actor and migration system

**Files:**

- Create: `crates/mfa-db/Cargo.toml`
- Create: `crates/mfa-db/src/lib.rs`
- Create: `crates/mfa-db/src/actor.rs`
- Create: `crates/mfa-db/src/command.rs`
- Create: `crates/mfa-db/src/error.rs`
- Create: `crates/mfa-db/src/migrations.rs`
- Create: `crates/mfa-db/migrations/0001_provenance.sql`
- Create: `crates/mfa-db/migrations/0002_canonical.sql`
- Create: `crates/mfa-db/migrations/0003_active_snapshots.sql`
- Test: `crates/mfa-db/tests/actor_ownership.rs`
- Test: `crates/mfa-db/tests/migrations.rs`

**Interfaces:**

```rust
pub struct DatabaseService { sender: tokio::sync::mpsc::Sender<DatabaseCommand> }
impl DatabaseService {
    pub async fn start(path: &Path, capacity: usize) -> Result<Self, DatabaseError>;
    pub async fn execute<R>(&self, command: impl IntoDatabaseCommand<R>) -> Result<R, DatabaseError>;
    pub async fn shutdown(self) -> Result<(), DatabaseError>;
}
pub enum DatabaseCommand { RegisterReceipt(RegisterReceipt), RegisterAsset(RegisterAsset), StartAttempt(StartAttempt), CommitSnapshot(CommitSnapshot), FailAttempt(FailAttempt), QueryView(QueryView), ReconcileArchive(ReconcileArchive), MarkInterrupted, HealthCheck, Shutdown }
```

**Step 1: Write failing ownership and bounded-channel tests**

Instrument the actor with a test-only thread ID. Assert all opens and operations occur on that one thread, concurrent producers receive typed results in command order, a full channel applies backpressure, shutdown drains accepted commands, and no public API returns DuckDB-native types.

Run: `cargo test -p mfa-db --test actor_ownership`

Expected: FAIL.

**Step 2: Write failing migration tests**

Assert a fresh database reaches schema version 3; repeated startup is idempotent; a tampered migration checksum fails closed; migration failure rolls back; unsupported future schema enters a stable incompatible-schema error.

The provenance migration must define unique `source_asset.byte_sha256`, receipt outcomes, immutable asset columns, attempts with version/package/API/mapping metadata, source records, lineage, extension records, and data-quality items.

Run: `cargo test -p mfa-db --test migrations`

Expected: FAIL.

**Step 3: Implement the actor**

Spawn one named OS thread. Open `duckdb::Connection` inside its closure, run migrations, then synchronously process commands received from a bounded Tokio channel through blocking receive. Return DTOs through per-command oneshot channels. Convert every DuckDB error to a stable `DatabaseError` at the boundary.

Use:

```toml
duckdb = { version = "~1.10505.0", features = ["bundled"] }
```

**Step 4: Implement transactional migrations**

Create `schema_migration(version, name, checksum, applied_at)`. Embed SQL with `include_str!`, hash it at startup, and apply each migration in one transaction. Use explicit column lists and constraints rather than `SELECT *` in persistent views.

**Step 5: Run tests and commit**

```bash
cargo test -p mfa-db --test actor_ownership
cargo test -p mfa-db --test migrations
cargo clippy -p mfa-db --all-targets -- -D warnings
git add crates/mfa-db
git commit -m "feat: serialize DuckDB through one actor"
```

---

### Task 3: Persist provenance, canonical records, and atomic logical snapshots

**Files:**

- Create: `crates/mfa-db/src/provenance.rs`
- Create: `crates/mfa-db/src/snapshot.rs`
- Create: `crates/mfa-db/src/validation.rs`
- Create: `crates/mfa-db/src/views.rs`
- Modify: `crates/mfa-db/src/command.rs`
- Modify: `crates/mfa-db/migrations/0002_canonical.sql`
- Modify: `crates/mfa-db/migrations/0003_active_snapshots.sql`
- Test: `crates/mfa-db/tests/provenance.rs`
- Test: `crates/mfa-db/tests/snapshot_replacement.rs`
- Test: `crates/mfa-db/tests/query_consistency.rs`

**Interfaces:**

```rust
pub struct ValidatedSnapshotBatch { pub logical_key: LogicalSnapshotKey, pub attempt: AttemptIdentity, pub source_records: Vec<SourceRecord>, pub observations: Vec<CanonicalObservation>, pub extensions: Vec<ExtensionRecord>, pub lineage: Vec<LineageLink>, pub issues: Vec<DataQualityItem> }
pub struct CommitSnapshot(pub std::sync::Arc<ValidatedSnapshotBatch>);
pub struct SnapshotCommitResult { pub snapshot_id: uuid::Uuid, pub changed_capabilities: Vec<CapabilityId>, pub counts: RecordCounts }
pub struct QueryView { pub request: ViewRequest }
```

**Step 1: Write failing provenance tests**

Prove receipt rows capture every discovery outcome; asset rows contain only byte-immutable facts; manual retry creates a new attempt; source-record identity includes asset, sheet/CSV name, and row number; every canonical row has lineage; extension payloads require a registered matching contract.

Run: `cargo test -p mfa-db --test provenance`

Expected: FAIL.

**Step 2: Write failing replacement tests**

Commit two valid snapshots for the same logical key. Assert the second becomes active atomically, historical assets/attempts/records remain queryable, removed historical entries disappear from active views, repeated identical rows retain multiplicity, and failure before commit leaves the first snapshot active.

Run: `cargo test -p mfa-db --test snapshot_replacement`

Expected: FAIL.

**Step 3: Write a failing partial-read test**

Pause a transaction through a test-only failpoint after canonical insert but before active-snapshot switch. Queue a query concurrently. Assert the query returns either the entire old snapshot or entire new snapshot, never the paused partial state.

Run: `cargo test -p mfa-db --test query_consistency`

Expected: FAIL.

**Step 4: Implement command-side validation and snapshot commit**

Validate counts, entity IDs, source-record references, lineage coverage, temporal type compatibility, finite numeric values, declared capabilities, and extension contract versions before starting SQL. Within one transaction insert provenance, canonical rows keyed by `snapshot_id`, derived daily base tables, and then upsert `active_snapshot(logical_key, snapshot_id)`. Mark the attempt succeeded only in the same transaction.

Active views join through `active_snapshot`; they never depend on an `is_active` flag spread across canonical tables.

**Step 5: Run tests and commit**

```bash
cargo test -p mfa-db --test provenance
cargo test -p mfa-db --test snapshot_replacement
cargo test -p mfa-db --test query_consistency
git add crates/mfa-db
git commit -m "feat: commit provenance-complete logical snapshots"
```

---

### Task 4: Orchestrate the bounded ingestion queue end to end

**Files:**

- Create: `crates/mfa-ingestion/Cargo.toml`
- Create: `crates/mfa-ingestion/src/lib.rs`
- Create: `crates/mfa-ingestion/src/queue.rs`
- Create: `crates/mfa-ingestion/src/pipeline.rs`
- Create: `crates/mfa-ingestion/src/validator.rs`
- Create: `crates/mfa-ingestion/src/events.rs`
- Create: `crates/mfa-ingestion/src/error.rs`
- Test: `crates/mfa-ingestion/tests/queue_semantics.rs`
- Test: `crates/mfa-ingestion/tests/end_to_end.rs`

**Interfaces:**

```rust
pub enum ScanReason { Startup, Watcher, Periodic, Manual }
pub struct ScanRequest { pub reason: ScanReason, pub requested_at: UtcInstant }
pub struct IngestionCoordinator;
impl IngestionCoordinator {
    pub async fn request_scan(&self, request: ScanRequest) -> Result<ScanTicket, IngestionError>;
    pub async fn retry_asset(&self, asset_id: AssetId) -> Result<AttemptId, IngestionError>;
}
pub enum CoreEvent { WorkStateChanged(WorkState), DataChanged { capabilities: Vec<CapabilityId>, dashboards: Vec<ModuleId> }, QualityChanged }
```

**Step 1: Write failing queue tests**

Assert pending scans coalesce, one path has at most one active job, assets process sequentially by discovery order, one asset failure does not stop later jobs, and filename equality has no identity effect. Queue state is memory-only and reconstructable from archive plus database.

Run: `cargo test -p mfa-ingestion --test queue_semantics`

Expected: FAIL.

**Step 2: Write a failing synthetic end-to-end test**

Install the Foundation fake source package, place bytes in its inbox, issue a manual scan, and assert this exact order through event probes:

```text
stable → archive verified → inbox removed → receipt/asset registered
→ attempt started → guest parsed → host validated → snapshot committed
→ DataChanged emitted
```

Also prove an exact duplicate records a duplicate receipt, removes inbox bytes, skips guest invocation, and creates no canonical duplicates.

Run: `cargo test -p mfa-ingestion --test end_to_end`

Expected: FAIL.

**Step 3: Implement the coordinator**

Resolve the enabled compatible source module from the inbox directory, call the archive coordinator, delete the inbox file only after verified disposition, register receipt/asset, skip duplicate parsing, start an attempt with immutable module metadata, read archive bytes, invoke Wasmtime outside DuckDB, validate the batch, and send `CommitSnapshot`. Publish only capability/dashboard IDs after commit.

**Step 4: Run tests and commit**

```bash
cargo test -p mfa-ingestion
cargo clippy -p mfa-ingestion --all-targets -- -D warnings
git add crates/mfa-ingestion
git commit -m "feat: process assets through bounded ingestion queue"
```

---

### Task 5: Add reconciliation, retry policy, crash recovery, and safe rebuild

**Files:**

- Create: `crates/mfa-archive/src/reconcile.rs`
- Create: `crates/mfa-ingestion/src/retry.rs`
- Create: `crates/mfa-ingestion/src/recovery.rs`
- Create: `crates/mfa-ingestion/src/rebuild.rs`
- Create: `crates/mfa-ingestion/src/health.rs`
- Test: `crates/mfa-ingestion/tests/reconciliation.rs`
- Test: `crates/mfa-ingestion/tests/fault_injection.rs`
- Test: `crates/mfa-ingestion/tests/rebuild.rs`

**Interfaces:**

```rust
pub enum FailureClass { Waiting, AssetFailure, TransientFailure, CriticalFailure }
pub struct RetryPolicy { pub max_transient_attempts: u8, pub delays: [Duration; 3] }
pub enum HealthState { Healthy, Working, Attention, Blocked }
pub struct RebuildService;
impl RebuildService { pub async fn preflight(&self) -> Result<RebuildPlan, RebuildError>; pub async fn rebuild(&self, confirmation: RebuildConfirmation) -> Result<RebuildResult, RebuildError>; }
```

**Step 1: Write failing reconciliation tests**

Assert orphaned immutable archive files re-enter normal registration/ingestion; missing archived bytes produce a critical quality item without deleting database provenance; temp archive copies are ignored; reconciliation after a crash between rename and registration is idempotent.

Run: `cargo test -p mfa-ingestion --test reconciliation`

Expected: FAIL.

**Step 2: Write failing fault-injection tests**

Inject failures at archive copy, hash verify, inbox delete, asset registration, guest parse, host validation, transaction start, canonical insert, active switch, and event emission. Assert the specification’s failure class, retry count, inbox/archive state, attempt state, active snapshot, queue continuation, and aggregate health state for each point.

On startup, remaining `running` attempts must become `interrupted` before reconciliation. Deterministic asset errors do not auto-retry. Transient operations attempt at most three times with test-controlled time.

Run: `cargo test -p mfa-ingestion --test fault_injection`

Expected: FAIL.

**Step 3: Write failing rebuild tests**

Assert preflight lists missing/incompatible source packages; confirmed rebuild copies the old database to a timestamped non-overwrite recovery path, builds and validates a temporary database entirely from archive, and atomically replaces active database only after success. A failed rebuild leaves both active database and recovery copy unchanged.

Run: `cargo test -p mfa-ingestion --test rebuild`

Expected: FAIL.

**Step 4: Implement recovery services**

Represent failpoints as injected traits, not production environment variables. Use a clock trait for deterministic backoff. Recovery mode permits query/config/diagnostics commands but rejects scans and commits. Rebuild creates a separate temporary `DatabaseService`; shut down both actors before atomic database-file replacement, then start one actor on the validated replacement.

**Step 5: Run tests and commit**

```bash
cargo test -p mfa-ingestion --test reconciliation
cargo test -p mfa-ingestion --test fault_injection
cargo test -p mfa-ingestion --test rebuild
git add crates/mfa-archive crates/mfa-ingestion
git commit -m "feat: recover ingestion and rebuild from archive"
```

---

### Task 6: Integrate storage services into Tauri commands and close the gate

**Files:**

- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/commands.rs`
- Create: `src-tauri/src/events.rs`
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/lib/transport.ts`
- Modify: `web/src/lib/tauri-transport.ts`
- Test: `src-tauri/tests/ingestion_commands.rs`
- Create: `crates/mfa-integration-tests/Cargo.toml`
- Create: `crates/mfa-integration-tests/src/lib.rs`
- Test: `crates/mfa-integration-tests/tests/storage_gate.rs`

**Interfaces:**

```rust
#[tauri::command] async fn set_workspace_root(path: String, state: State<'_, AppState>) -> Result<WorkspaceView, CommandError>;
#[tauri::command] async fn refresh_now(state: State<'_, AppState>) -> Result<ScanTicketView, CommandError>;
#[tauri::command] async fn get_ingestion_status(state: State<'_, AppState>) -> Result<IngestionStatusView, CommandError>;
#[tauri::command] async fn list_quality_items(state: State<'_, AppState>) -> Result<Vec<QualityItemView>, CommandError>;
#[tauri::command] async fn retry_asset(asset_id: String, state: State<'_, AppState>) -> Result<AttemptView, CommandError>;
```

**Step 1: Write failing command and concurrency tests**

Assert command DTOs expose stable codes, not filesystem internals or DuckDB errors. Launch 32 concurrent query/refresh calls around a snapshot commit and prove all complete without a second DuckDB connection or partial view. Confirm `DataChanged` contains IDs only.

Run: `cargo test -p myfitanalytics --test ingestion_commands`

Expected: FAIL.

**Step 2: Write the storage acceptance test**

`crates/mfa-integration-tests/tests/storage_gate.rs` must use temporary app-data/workspace roots and the fake source package to prove create, duplicate, replacement, parse failure, transaction failure, crash reconciliation, and rebuild paths in one serial scenario.

Run: `cargo test -p mfa-integration-tests --test storage_gate -- --test-threads=1`

Expected: FAIL before command integration.

**Step 3: Implement command adapters and event forwarding**

Keep commands thin: validate strings, call service methods, map typed DTOs, and return. Never hold Tauri state locks across `.await`; services are cloneable channel handles. Frontend refreshes view models after `DataChanged` rather than accepting row payloads in events.

**Step 4: Run the storage gate**

```bash
cargo test --workspace -- --test-threads=1
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm --dir web test -- --run
pnpm --dir web exec svelte-check --tsconfig ./tsconfig.json
```

Expected: PASS. The full Rust suite is serial here so actor/fault-injection ordering is deterministic; tests that do not share state may later opt into parallel execution explicitly.

**Step 5: Commit**

```bash
git add src-tauri web crates/mfa-integration-tests
git commit -m "feat: expose resilient ingestion through desktop commands"
```

## Plan Completion Evidence

Write `docs/superpowers/evidence/storage-ingestion.md` with the gate command output and a table mapping archive, duplicate, snapshot replacement, concurrency, fault, reconciliation, and rebuild scenarios to passing test names. Close the plan only when the synthetic asset completes through the production actor and Wasmtime boundaries.
