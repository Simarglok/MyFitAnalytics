# MyFitAnalytics MVP Implementation Roadmap

> **For agentic workers:** Execute the linked plans in order. Every plan is an independently reviewable gate and requires `superpowers:test-driven-development` while coding plus `superpowers:verification-before-completion` before its gate is closed.

**Goal:** Deliver the approved MyFitAnalytics desktop MVP through five sequential, independently testable implementation plans.

**Architecture:** A Tauri 2 desktop shell hosts a Svelte UI and a Rust core. The core owns an actor-serialized DuckDB connection, immutable raw archive, Wasmtime Component Model runtime, canonical capability registry, analytics, and dashboard projection. Source, dashboard, and locale modules remain installable packages rather than compile-time application features.

**Tech Stack:** Rust 1.94.0 (edition 2024), Tauri 2.11, Wasmtime 47 Component Model, DuckDB 1.5.5 through `duckdb-rs ~1.10505.0`, Node.js 24.19 LTS, pnpm 11.23, Svelte 5, TypeScript, Vite 8, Apache ECharts 6, Vitest, Playwright, GitHub Actions.

**Spec:** [Approved MVP-SPEC.md](</Users/simarglok/Library/Mobile Documents/iCloud~md~obsidian/Documents/Simarglok/MyFitAnalytics/MVP-SPEC.md>)

## Global Constraints

- The repository is empty and is not initialized as Git at plan creation time. The Foundation plan owns initialization.
- Implementation follows strict red-green-refactor cycles. A production behavior is added only after its focused test fails for the expected reason.
- Only `DatabaseService` opens DuckDB. Tests that need a database must also use the actor boundary.
- Source and dashboard guests receive bytes and typed host calls only. They receive no filesystem, network, credential, DuckDB, raw SQL, or executable frontend access.
- English is the executable-module fallback language. Additional languages are data-only locale packages.
- Every persisted or inter-process shape is versioned and serialized through types from `mfa-contracts`.
- Exact dependency versions are recorded in `Cargo.lock` and `pnpm-lock.yaml`. Semver requirements in manifests must not silently cross a DuckDB or Wasmtime contract version.
- Each plan is committed separately. Do not start the next plan while the current acceptance gate is failing.

## Dependency Graph

```text
Foundation
    ↓
Storage and ingestion
    ↓
Bundled source modules
    ↓
Analytics and dashboard UI
    ↓
Desktop lifecycle and release
```

## Plan 1 — Foundation and Module Runtime

**Plan:** [2026-08-25-myfitanalytics-foundation.md](./2026-08-25-myfitanalytics-foundation.md)

**Delivers:** Initialized repository, Rust/TypeScript workspaces, shared canonical contracts, WIT source/dashboard interfaces, package validation and installation, Wasmtime sandbox limits, capability registry, English locale fallback, and a minimal Tauri/Svelte window driven through a typed bridge.

**Acceptance gate:** A signed-off fake source component can be installed from a package, invoked with in-memory bytes under Wasmtime limits, and exposed to the UI as an installed module and selected capability provider. Rust, frontend, schema, and WIT tests pass.

## Plan 2 — Storage, Archive, and Ingestion

**Depends on:** Plan 1.

**Plan:** [2026-08-25-myfitanalytics-storage-ingestion.md](./2026-08-25-myfitanalytics-storage-ingestion.md)

**Delivers:** Settings and workspace tree, stable-file scanner, immutable content-addressed archive receipts, actor-owned DuckDB schema, provenance, logical snapshots, ingestion queue, retry classification, reconciliation, crash recovery, rebuild, and consistent read snapshots.

**Acceptance gate:** A synthetic package processes a stable inbox asset end-to-end, commits one logical snapshot, archives the exact bytes, deletes the verified inbox file, remains idempotent on replay, and recovers correctly from injected failures.

## Plan 3 — Bundled Source Modules

**Depends on:** Plan 2.

**Plan:** [2026-08-25-myfitanalytics-source-modules.md](./2026-08-25-myfitanalytics-source-modules.md)

**Delivers:** MyNetDiary `.xls` and Hevy CSV guest components, real contract fixtures, canonical observations, extension records, mapping warnings, module manifests, English namespaces, build/package scripts, source contract conformance tests, native workspace/package pickers, and a minimal Settings workflow for installing, updating, enabling, disabling, and uninstalling module packages without rebuild or restart.

**Acceptance gate:** Original-format MyNetDiary and Hevy fixtures import through the same Wasmtime and ingestion boundaries used in production; expected canonical rows, lineage, warnings, replacement semantics, and active-provider behavior are proven by integration tests. In a packaged macOS application, the user can choose a workspace and install, disable, re-enable, and uninstall a module through Settings without rebuilding or restarting the application.

## Plan 4 — Analytics and Dashboard UI

**Depends on:** Plan 3.

**Plan:** [2026-08-25-myfitanalytics-analytics-ui.md](./2026-08-25-myfitanalytics-analytics-ui.md)

**Delivers:** Deterministic analytics queries, coverage and availability resolver, base dashboard component, typed dashboard view model, Overview/Body/Nutrition/Activity/Strength/Sources pages, provider selection and dependency-aware extensions to the existing Settings module management, data quality, refresh progress, localization, and frontend accessibility tests.

**Acceptance gate:** Golden datasets produce approved weight, nutrition, activity, strength, and TDEE results; every graph shows either typed ready data or a precise non-ready state; the Svelte application renders the complete base dashboard from mock and real Tauri transports.

## Plan 5 — Desktop Lifecycle and Release

**Depends on:** Plan 4.

**Plan:** [2026-08-25-myfitanalytics-desktop-release.md](./2026-08-25-myfitanalytics-desktop-release.md)

**Delivers:** Tray commands, close-to-background behavior, single-instance focus, optional autostart, periodic refresh, recovery-mode UI, structured local logging, hardened permissions, cross-platform CI, macOS packaging smoke, and final acceptance evidence.

**Acceptance gate:** Full Rust and frontend suites pass on macOS, Windows, and Linux CI; macOS bundle starts, opens/focuses its dashboard, refreshes from tray, remains alive after window close, and quits cleanly; recovery and rebuild preserve the raw archive.

## Final MVP Gate

The MVP is complete only when all five plan gates are closed and the Section 20 acceptance criteria in the approved specification have corresponding automated evidence or an explicitly recorded macOS packaging smoke result. A passing focused suite in a later plan does not substitute for the full final gate.
