# MyFitAnalytics

MyFitAnalytics is a local-first, private workspace for personal fitness analytics. It is intended to keep personal health data on the user's machine, make future imports and transformations auditable, and support extensible providers without requiring a hosted account or data export.

## Status

Plan 4 (analytics and dashboard UI) is currently **Draft/INCOMPLETE**. The
previously approved persisted-provider hydration and fail-closed profile
guard/runbook corrections are historical records from source revision
`41bbf5f686bb996000c424064913a123c6d19987`. The earlier provider correction at
`892e1c8ea6ce1ce751a56df4558d11656f5e1f2c` and its full/package/DMG/hygiene
evidence are historical and labeled separately below. The current bounded UI
correction was verified against baseline
`8ea3f0774a46dc24a3a90bc1e3cc6183ddde6261` and its parent
`bed42a76179efbc9cebe1600eac8d58cf2a1ac3e`; no future amended hash is used as
evidence. Historical product/package evidence remains separately labeled.
Analytics diagnostics do not expose private records, raw payloads, or raw error
details; `Settings` intentionally displays the chosen workspace and inbox paths.

The bounded UI correction fixes post-success Settings refresh of the
AppShell/dashboard snapshot,
preserves local Settings-page selection and guards older in-flight reloads, and
keeps the Sources & quality failure rows/actions visible when a retry rejects
while reporting a separate retry error. Focused implementer evidence is 22/22
tests across the three affected files; the interim complete web run is 63/63
tests in 13 files and Svelte check reports 0 errors with 2 existing warnings.
Final manager automatic source/web verification passed all 33 intended rows
across the two ledgers: `/private/tmp/mfa-plan4-ui-manager.GjHxmr/gates.tsv`
passed 21 rows through `workspace`, and
`/private/tmp/mfa-plan4-ui-manager.GjHxmr/continuation-gates.tsv` passed the
remaining 12 rows from
`rustfmt` through `diff-unchanged`. The original interrupted
`disk-before-rustfmt` exit 70 remains preserved as a separate historical
attempt; the manager's `head-unchanged` and byte-for-byte `diff-unchanged`
checks passed. The separate app/DMG package-gate, DMG audit, and correction-round
hygiene remain **PENDING**, as does the corrected native retest. Scoped space
recovery is documented by the retained audits: first only 19 validated
rebuildable Mach-O test executables, then only the isolated worktree's
regenerable `target/debug/build` cache. No source, dependency, profile, backup,
release artifact, or evidence log was removed.

The manager-recorded P6 native run is **FAIL** for the stale shell state after
module/provider Settings actions and for rejected Retry hiding the single
synthetic failure row. Its successful observations include fixture import,
coverage `4/31`, stable `Attention` equal to 1, one quality row, normal Quit,
and exact six-root guarded restoration; those Quit/storage/restore observations
are recorded separately from the failed UI assertions. The fixed-source native
retest remains **NOT RUN/PENDING**, not PASS; the fresh staging root
`/private/tmp/mfa-plan4-ui-retest.aWTdix` was not run. Remaining native pages and
Phase events CRUD are incomplete, and the PR remains Draft. Detailed native
evidence is retained at
`/private/tmp/mfa-plan4-native-approved.P6u51Q/manager-evidence.md`; the genuine
pre-fix Settings RED log is
`/private/tmp/mfa-plan4-ui-manager.GjHxmr/settings-red.log`.

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
[`docs/testing/plan4-packaged-native-manual.md`](docs/testing/plan4-packaged-native-manual.md).
The manager-recorded run against the pre-fix package is **FAIL** for the stale
shell and rejected Retry behaviors; no fixed-source native retest has run, so
that retest remains **PENDING** and no packaged-native PASS is claimed. A final
external Terra High review remains **PENDING** and requires new informed
authorization; the product is not classified Ready.

Historical pre-correction package evidence anchored at
`bed42a76179efbc9cebe1600eac8d58cf2a1ac3e` remains separate. Its earlier
packaging attempts and artifacts are not current evidence: an initial
`cargo tauri build --bundles app` exited 0; the first full packaging attempt,
`CI=true cargo tauri build -vv`, exited 1 during sandbox dependency resolution;
an approved `pnpm install --frozen-lockfile` using the local dependency store
exited 0 with runtime pnpm `11.19.0`; `cargo tauri build --ci -vv` exited 1
during sandbox DMG image creation; and the approved cached-dependency rerun
exited 0 and produced both app and DMG. Its historical `.app` executable
SHA-256 was
`41a90db23fc22755ff13394ddc9940adefe6eb1a9d20764b6ffcebb5cc3f96ed`, and its
historical DMG SHA-256 was
`f6e56b75e856a56760b01380fcf1f24919817497b7977e9a45649ed0c44ac97e`.

A prior manual run at `273997a` is historical only: it passed workspace/import
visibility and a stable one-attention observation but failed Sources & quality,
which was empty despite the attention item. That native run remains **FAIL** and
is never retroactively **PASS**; the old test runner remains disabled. It cannot
validate the current product correction. XCUITest is a separate deferred
follow-up at
[`docs/superpowers/tasks/2026-08-29-xcuitest-native-acceptance.md`](docs/superpowers/tasks/2026-08-29-xcuitest-native-acceptance.md).

The current narrow correction changes absent-provider classification without
auto-selecting providers or rewriting settings. When a required provider is not
selected, navigation and dashboard commands report `MissingCapability` with the
`Configure source` action. After explicit provider selection, including
`activity.days`, the no-snapshot state is `WaitingForData` with `Import data`.
A selected provider that fails its manifest capability contract remains
`IncompatibleContract` with `Update module` precedence over another missing
capability.

Focused correction-round evidence for this local source checkpoint:

- RED: `cargo test -p myfitanalytics --test dashboard_commands missing_provider_selection_is_reported_in_navigation_and_dashboard -- --exact --nocapture` exited `101` before the adapter correction, with the absent-provider command path returning `IncompatibleContract` instead of the required `MissingCapability`.
- GREEN: `cargo test -p myfitanalytics --test dashboard_commands -- --nocapture` exited `0` with 9 tests passed, including navigation/dashboard missing selection and the explicit-selection transition to `WaitingForData`.
- GREEN: the private incompatible-precedence adapter test exited `0` with 1 test passed; the existing public unoffered-provider rejection test exited `0` with 1 test passed.
- `cargo fmt --all -- --check` and `git diff --check` both exited `0`.
- The manager-provided focused Clippy and historical-update probe logs exited `0`.
  The probe showed `MissingCapability` before and after both explicit module
  updates, then `WaitingForData` only after explicit new-capability selection;
  automatic provider state was unchanged by updates and changed only after
  explicit selection. This records the human-approved current criterion for the
  historical packages: initially advertised `base` and `MyNetDiary` pending
  updates clear after both explicit updates, while unselected `activity.days`
  remains `MissingCapability` with `Configure source` before and after. The
  genuine `IncompatibleContract` behavior remains a mandatory automatic-test
  boundary; this scenario does not prove native clearing of a real
  incompatibility.

Fresh manager verification for source revision
`892e1c8ea6ce1ce751a56df4558d11656f5e1f2c`:

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
  locked Playwright `1.62.1` and Chromium revision `1234`; profile guard checks
  reported 20 passed synthetic tests plus 2 independent checks; Svelte reported
  0 errors and 2 pre-existing `AppShell` warnings. Formatting, Clippy, release,
  source/dashboard/provider/lifecycle/ingestion/web, determinism, privacy, and
  unchanged-source checks all passed.
- The package-gate runner used `CI=true`, `CARGO_NET_OFFLINE=true`, and
  `CARGO_BUILD_JOBS=2`; `cargo tauri build --bundles app` and the full
  `cargo tauri build` both completed. All 9 `package-gates.tsv` rows exited `0`,
  and the runner exited `0`: both required entitlements, strict codesign,
  `hdiutil verify`, exact byte equality for the 3 packaged module resources
  against `dist`, and unchanged source HEAD passed. Signing was ad hoc and not
  notarized.
- `audit-dmg.sh` exited `0`: the DMG was attached read-only without Finder,
  the embedded app passed strict signature verification, the complete app-tree
  diff was identical to the current bundle, and the image detached successfully.
- `hygiene.sh` exited `0`: secret/key/token and sensitive-filename scans had no
  matches; exactly 7 checked-in XLS fixtures and 2 CSV fixtures were reviewed
  as synthetic privacy-verified fixtures; marker hits were limited to the old
  scan description and a lock-integrity substring.
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

The fresh automatic source/package checks and artifact audit pass for the
frozen source revision. Native acceptance remains **NOT RUN**: the mandatory
prior-production-package → current-package normal-install scenario must first
show pending updates named `base` and `MyNetDiary`; after explicit `Update` on
both modules those notices must disappear. For unselected `activity.days`, the
expected `MissingCapability`/`Configure source` state, visible as `Required data
is not available yet` with `Configure source`, must remain the same before and
after both updates. No automatic package update or provider selection is
allowed. Only explicit `Use for activity.days` may change selection and yield
`WaitingForData`/`Import data` before data arrives. This criterion remains
**PENDING**, never `N/A`, and does not prove clearing a genuine
`IncompatibleContract`; mandatory automatic incompatible-provider tests remain
separate. The recorded automated/package verification involved no app launch,
native UI assertion, interactive screenshot, or profile operation. A final
external Terra High review remains **PENDING** and requires new informed
authorization; the product remains Draft/INCOMPLETE and the PR remains
OPEN/Draft.

As of 2026-08-31, the app is stopped and the explicitly approved copy-based
recovery is complete within its authorized scope. The available Application
Support copy was restored; its normalized tree hash matches the chosen
available source, **not** the original baseline. Preferences were restored hash-identically to the original baseline and passed
syntax validation. Live Caches and WebKit were first copied and verified; the
live originals were then moved into private quarantine, while the pre-restore
copies remain separately retained. Prior live Preferences were preserved the
same way before restoration. New empty live directories were created. Originally
absent Saved Application State and HTTPStorages remain absent. All original
backups, prior recovery variants, the failed-run snapshot, and the original
before-manifest were preserved unchanged. Independent recheck verified the
restoration, preserved copies, empty directories, absent roots, and unchanged
baseline. No deletion or app launch occurred; the cause of the original
mismatch remains unknown. This recovery does not satisfy the historical
hash-identical acceptance gate: full original-baseline six-root equivalence
remains unproven.

An earlier read-only aggregate profile audit, with moved root labels
canonicalized back to their original names, recorded Application Support and
Preferences matching and originally absent Saved Application State and
HTTPStorages still absent, but Caches and WebKit not matching their recorded
baselines. The dated 2026-08-31 pre-recovery audit retained in
`docs/superpowers/evidence/analytics-ui.md` records the guarded-abort state
before approved recovery; it supersedes that earlier Application Support
observation. The approved recovery status is now current; it does not reclassify
the historical native run or establish
original-baseline equality. Full original-baseline six-root equivalence remains
unproven and the historical hash-identical acceptance gate remains unsatisfied.
No third-party foreground-control daemon or macOS permission change is required
for the current criterion. The repository includes the Rust workspace, shared
versioned contracts, module package validation and installation,
Storage/Ingestion, bundled Hevy and MyNetDiary source modules, deterministic
base analytics, a sandboxed Wasm component host, capability/provider
resolution, locale fallback, and a typed Tauri-to-Svelte dashboard shell.

The backend supplies the initial dashboard window as an inclusive 31-day range
ending at the latest available observation, or at the current local date when
no observation exists; the frontend does not invent a production date default.

Task 7's automated gate and the approved manual packaged-native scenario use
only checked-in synthetic MyNetDiary and Hevy fixtures, isolated temporary
roots, and the application command services. Plan 5 (desktop lifecycle,
tray/background behavior, and release packaging) remains deferred and out of
scope. Local macOS `.app` and DMG bundles, when built, are development
validation artifacts; notarization and publishing remain deferred, out of scope,
and unclaimed.

## Architecture

- **Rust workspace:** `mfa-contracts`, `mfa-module-host`, and `mfa-config` provide the typed foundation and host-side services.
- **Tauri desktop shell:** `src-tauri` composes the native application and exposes a typed command boundary without a production localhost server.
- **Svelte web UI:** `web` contains the frontend shell and its mock and Tauri transports.
- **Versioned module contracts:** source, dashboard, and locale modules use explicit manifests, schemas, and WIT/API contracts.
- **Sandboxed Wasm execution:** source and dashboard components run through Wasmtime with host-controlled assets and runtime limits.
- **Capability/provider resolution:** the host validates enabled modules and resolves active providers for declared capabilities.

## Repository layout

```text
crates/                 Rust foundation crates and integration tests
src-tauri/              Tauri desktop shell, commands, capabilities, and icons
web/                    Svelte, TypeScript, Vite, and Vitest frontend
modules/sdk/            Module schemas and WIT contracts
modules/locales/en/     Core English locale catalog
scripts/                Workspace checks
docs/superpowers/plans/  Roadmap and implementation plans
```

## Prerequisites

This checkout uses Rust 1.94.0, Node.js 24.19.0, pnpm 11.23.0, and Tauri CLI 2.11.0. On this Mac, initialize the provisioned toolchain path from the repository root before Node or Tauri commands:

```bash
export PATH="$PWD/.tools/pnpm/node_modules/.bin:$PWD/.tools/node-v24.19.0-darwin-arm64/bin:$HOME/.cargo/bin:/opt/homebrew/opt/rustup/bin:$PATH"
```

Install the locked frontend dependencies when setting up the checkout:

```bash
pnpm install --frozen-lockfile
```

## Development

Run the Svelte development server:

```bash
pnpm --dir web dev
```

Run the desktop shell in development mode:

```bash
cargo tauri dev
```

## Test, check, and build

Run the Rust workspace tests and checks:

```bash
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

Run the non-foreground production-path dashboard gate:

```bash
bash scripts/run-dashboard-gate.sh
```

Run the frontend tests, type checks, and production build:

```bash
pnpm --dir web test -- --run
pnpm --dir web exec svelte-check --tsconfig ./tsconfig.json
pnpm --dir web build
```

Build a local macOS application bundle without release credentials or publishing it:

```bash
cargo tauri build --bundles app
```

The root shortcuts `pnpm run test`, `pnpm run check`, and `pnpm run build` delegate to the corresponding frontend commands.

## Plans and roadmap

- [MVP roadmap](docs/superpowers/plans/2026-08-25-myfitanalytics-mvp-roadmap.md)
- [Foundation plan](docs/superpowers/plans/2026-08-25-myfitanalytics-foundation.md)
- [Storage and ingestion plan](docs/superpowers/plans/2026-08-25-myfitanalytics-storage-ingestion.md)
- [Source modules plan](docs/superpowers/plans/2026-08-25-myfitanalytics-source-modules.md)
- [Analytics and dashboard UI plan](docs/superpowers/plans/2026-08-25-myfitanalytics-analytics-ui.md)
- [Plan 4 packaged-native manual acceptance](docs/testing/plan4-packaged-native-manual.md)
- [XCUITest native acceptance follow-up](docs/superpowers/tasks/2026-08-29-xcuitest-native-acceptance.md)
- [Dashboard module authoring](docs/dashboard-module-authoring.md)
- [Desktop lifecycle and release plan](docs/superpowers/plans/2026-08-25-myfitanalytics-desktop-release.md)

## License

MyFitAnalytics is licensed under the [Apache License 2.0](LICENSE).
