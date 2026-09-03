# MyFitAnalytics

MyFitAnalytics is a local-first, private workspace for personal fitness analytics. It is intended to keep personal health data on the user's machine, make future imports and transformations auditable, and support extensible providers without requiring a hosted account or data export.

## Status

The original custom dashboard implementation is merged in `origin/main`, but
packaged-native acceptance remains incomplete. That concept is parked while the
project validates a Rill-based analytics direction; Rill integration is not yet
implemented. Project plans, decisions, tasks, and evidence are maintained in
the owner's local Obsidian workspace.

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
docs/                   Code-coupled authoring guides
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

## Documentation

Project plans, decisions, tasks, and evidence are maintained in the owner's
local Obsidian workspace. Code-coupled authoring guides remain in this
repository:

- [Source module authoring](docs/source-module-authoring.md)
- [Dashboard module authoring](docs/dashboard-module-authoring.md)

## License

MyFitAnalytics is licensed under the [Apache License 2.0](LICENSE).
