# MyFitAnalytics

MyFitAnalytics is a local-first, private workspace for personal fitness analytics. It is intended to keep personal health data on the user's machine, make future imports and transformations auditable, and support extensible providers without requiring a hosted account or data export.

## Status

Plan 4 (analytics and dashboard UI) is complete at product scope. At the
authoritative current HEAD
`25c54806d06cb7c46b93cef1d7be5b4b93f9eec5`, the mandatory Rust/web gates,
deterministic module/package checks, and fresh app/DMG build checks are green.
Packaged-native macOS acceptance remains Draft/INCOMPLETE: a fresh launch showed
a visible 1200x800 window with no crash in process/WebKit logs, but CuaDriver
lacks Accessibility and Screen Recording permissions, so no interactive
assertions or screenshots are validly claimed. The repository includes the Rust
workspace, shared versioned contracts, module package validation and
installation, Storage/Ingestion, bundled Hevy and MyNetDiary source modules,
deterministic base analytics, a sandboxed Wasm component host,
capability/provider resolution, locale fallback, and a typed Tauri-to-Svelte
dashboard shell.

The backend supplies the initial dashboard window as an inclusive 31-day range
ending at the latest available observation, or at the current local date when
no observation exists; the frontend does not invent a production date default.

Task 7's automated gate and the packaged-native acceptance use only checked-in
synthetic MyNetDiary and Hevy fixtures, isolated temporary roots, and the
application command services. The current packaged-native acceptance is
incomplete until CuaDriver Accessibility and Screen Recording are granted, the
full fresh acceptance is rerun, native evidence is updated, and final Terra
review is obtained. Plan 5 (desktop lifecycle, tray/background behavior, and
release packaging) remains deferred and out of scope. Local macOS `.app` and DMG
bundles, when built, are development validation artifacts; notarization and
publishing remain deferred, out of scope, and unclaimed.

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
- [Dashboard module authoring](docs/dashboard-module-authoring.md)
- [Desktop lifecycle and release plan](docs/superpowers/plans/2026-08-25-myfitanalytics-desktop-release.md)

## License

MyFitAnalytics is licensed under the [Apache License 2.0](LICENSE).
