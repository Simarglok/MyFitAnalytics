# Dashboard module authoring

This guide describes the production contract for a MyFitAnalytics dashboard
module. A dashboard is a declarative WebAssembly component. It does
not open files, query DuckDB, or receive raw source rows. The host resolves
capabilities from the canonical store and passes a typed `DashboardInput` to
the component.

## Package layout

A dashboard package is a `.mfadashboard` ZIP containing:

- `module.json` — the manifest;
- `module.wasm` — the WebAssembly component entrypoint; and
- no executable files, symlinks, path traversal entries, or undeclared payloads.

The package extension and manifest `module_type` must agree. The package
installer validates the manifest schema, package/API compatibility, entrypoint
digest, archive safety, and declared dashboard capabilities before installation.
The `entrypoint_hash` is an integrity check: the host recomputes the SHA-256
digest of `module.wasm` and requires an exact match. It is not a cryptographic
signature and does not establish publisher identity. Release signing and
notarization are separate packaging controls.

A minimal manifest has this shape:

```json
{
  "module_type": "dashboard",
  "module_id": "example-dashboard",
  "module_version": "1.0.0",
  "package_format_version": "1.0.0",
  "dashboard_api_version": "1.0.0",
  "compatible_app_versions": [">=0.1.0"],
  "required_capabilities": [
    {"capability": "body.weight"}
  ],
  "required_extension_contracts": [],
  "localization_namespace": "example-dashboard",
  "entrypoint_hash": "sha256:<module.wasm sha256>"
}
```

Use only capability IDs and extension contracts that the host can grant. The
dataset resolver copies only capabilities named by `required_capabilities` and
only extension namespaces named by `required_extension_contracts` (including
inline extension requirements). A missing capability/extension is rejected;
an extension whose `ContractVersion` is not exactly equal is rejected as
`incompatible_extension`. `compatible_app_versions` is the manifest's app
version compatibility range; it is separate from the exact extension contract
version check.

Do not put credentials, personal data, filesystem paths, raw export rows, or
volatile identifiers in the manifest or a reviewed fixture.

### Docs-only cross-source example

The following is an authoring example, not a checked-in production module. A
dashboard may request canonical datasets produced by different source modules;
the host, not the dashboard, selects the active providers and combines the
grants:

```json
{
  "required_capabilities": [
    {"capability": "body.weight"},
    {"capability": "nutrition.items"}
  ],
  "required_extension_contracts": []
}
```

In the Task 7 acceptance fixture, `body.weight` comes from Hevy and
`nutrition.items` comes from MyNetDiary. The dashboard receives those two
canonical grants only; it never receives the source file paths or raw exports.

## Component contract

The component implements the `dashboard-module` world in
`modules/sdk/wit/dashboard-api.wit`:

- `describe() -> string` returns module metadata as JSON;
- `compose(input_json: string) -> result<string, string>` receives one
  `DashboardInput` JSON value and returns one `DashboardDocument` JSON value.

The host rejects malformed JSON, undeclared capability/dataset references,
unsafe strings, unknown node types, unsupported chart types, non-finite values,
excessive output, and non-declarative content. The runtime defaults are:

| Limit | Default |
| --- | ---: |
| Wasmtime linear memory | 64 MiB |
| Fuel | 10,000,000 units |
| Epoch timeout | 2 seconds |
| Guest output (`describe` and `compose`) | 1 MiB |
| Store instances/tables | 8 / 8 |

Limit failures are classified as `module_memory_limit`,
`module_fuel_exhausted`, `module_timeout`, or `module_output_limit`; malformed
or unsafe contracts use typed errors such as `module_malformed_output`,
`module_non_declarative_output`, `missing_capability_input`, and
`module_hash_mismatch`. Return cards, tables, status panels, and the supported
line/bar/scatter/calendar heatmap charts; keep HTML, scripts, URLs, SQL, and
event-handler keys out of output.

The base module uses `dashboard.page` as a host-provided routing capability.
Every page must have a stable page ID, title localization key, and stable
block keys. Page selection must happen inside the component; the host calls
one command for each requested page.

## Data and availability

The host owns the data path:

1. a source module reads a user-selected export through the read-only asset
   interface;
2. archive and ingestion code validates the batch;
3. the database actor commits a canonical snapshot atomically;
4. the command service resolves active logical snapshots and computes the
   dashboard input; and
5. the dashboard component renders the declarative document.

A dashboard must not infer readiness from a non-empty capability object.
The command view reports coverage and freshness separately. Preserve the
following availability states and their stable precedence:

- `disabled_by_user`;
- `missing_dependency`;
- `incompatible_contract`;
- `missing_capability`;
- `waiting_for_data`;
- `insufficient_coverage`; and
- `ready`.

Render an explicit unavailable/stale state rather than inventing zeros. Keep
IDs and timestamps out of semantic golden comparisons; they are transport and
freshness metadata, not analytics values.

## Localization

Every title, card label, table column, chart series name, and status message
must be declared by the module localization namespace and included in the
host allowlist. Use stable message keys such as
`base.body.daily_median`, not user-visible prose in the component output.
Unknown keys fail the command validation step.

## Build and package

From the repository root, build all checked-in production packages with:

```sh
bash scripts/build-module-packages.sh
```

The builder compiles the component for `wasm32-unknown-unknown`, converts it
to a component, calculates the `module.wasm` SHA-256, writes the manifest, and
creates the package under `dist/modules/`. Do not hand-edit the generated
hash. The application bundle has an explicit package allowlist; adding a new
module also requires an intentional bundle/configuration change.

## Tests and review

Add focused source or dashboard tests for each semantic behavior before
implementation. Use deterministic checked-in synthetic fixtures and isolated
temporary app-data/workspace roots. The end-to-end dashboard gate is:

```sh
bash scripts/run-dashboard-gate.sh
```

It rebuilds the packages, imports the synthetic MyNetDiary and Hevy exports
through archive/ingestion/canonical storage, calls each base page
through the application command service, and checks the reviewed fixture
`web/e2e/fixtures/expected-dashboard.json`. Keep that gate serial so one
actor owns the database and test results are reproducible.

When changing a dashboard contract, update the manifest/schema, component
page tests, semantic expectation fixture, and
`docs/superpowers/evidence/analytics-ui.md` together. Record exact commands
and exit codes. Do not claim native macOS acceptance from this non-foreground
gate; that is a separate acceptance step.
