# Source-module authoring contract

This document is the contract for a third-party source module that imports an
exported file and returns MyFitAnalytics canonical observations. A source
module is a Wasm component packaged as a deterministic `.mfasource` archive.
The host owns files, persistence, provenance, provider selection, and all
external capabilities.

## Repository layout

A source module lives under `modules/sources/<module-id>/`:

```text
modules/sources/example/
├── Cargo.toml
├── module.json
├── locales/
│   └── en.json
├── src/
│   ├── lib.rs
│   └── component.rs
└── tests/
    └── fixtures/
```

The shared SDK is under `modules/sdk/`:

```text
modules/sdk/
├── rust/src/lib.rs
├── wit/source-api.wit
└── tools/build_mfasource.py
```

The source API is `myfitanalytics:source@1.0.0`. Keep the module implementation
independent of the desktop application and do not open files or databases from
the guest.

## Manifest

`module.json` is canonical JSON input to the package builder. The following
fields are required for a source module:

| Field | Contract |
| --- | --- |
| `module_type` | Exactly `source`. |
| `module_id` | Stable lowercase identifier. It is part of persisted provenance and must not be renamed after release. |
| `module_version` | Semver for the module implementation and mapping behavior. |
| `package_format_version` | Current package format, `1.0.0`. |
| `source_api_version` | WIT/source contract version, currently `1.0.0`. |
| `mapping_version` | Semver for canonical mapping rules. Increment it when output semantics change. |
| `compatible_app_versions` | Explicit semver ranges accepted by the host, for example `[">=0.1.0"]`. |
| `provided_capabilities` | Canonical capability IDs, sorted and deduplicated. Examples: `nutrition.items`, `body.weight`, `strength.sets`. |
| `accepted_file_patterns` | Content/file declarations such as `*.csv` and `text/csv`. Detection must still inspect bytes and headers; filenames are not identity. |
| `artifact_signatures` | SHA-256 signatures for accepted guest artifacts. The package builder writes the entrypoint digest. |
| `extension_contracts` | Optional declared namespaces, versions, and JSON payload schemas. Every emitted extension must be declared. |
| `settings_schema` | JSON Schema for module settings. Do not infer settings from filenames or defaults. |
| `entrypoint_hash` | SHA-256 digest of `module.wasm`, written by the package builder. |
| `localization_namespace` | Stable English namespace, for example `source.example`. |

The builder rewrites `entrypoint_hash` and `artifact_signatures` from the Wasm
bytes. Do not hand-edit those digests in a built package.

A complete source manifest has the same shape as the bundled modules:

```json
{
  "module_type": "source",
  "module_id": "example",
  "module_version": "1.0.0",
  "package_format_version": "1.0.0",
  "source_api_version": "1.0.0",
  "mapping_version": "1.0.0",
  "compatible_app_versions": [">=0.1.0"],
  "provided_capabilities": ["body.weight"],
  "accepted_file_patterns": ["*.csv", "text/csv"],
  "artifact_signatures": ["sha256:0000000000000000000000000000000000000000000000000000000000000000"],
  "extension_contracts": [],
  "settings_schema": {},
  "entrypoint_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
  "localization_namespace": "source.example"
}
```

## WIT exports and host asset access

The guest imports only `host-asset` and exports five functions:

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
    metadata: func() -> asset-metadata,
    read-at: func(offset: u64, max-bytes: u32) -> result<list<u8>, string>,
  }
}

world source-module {
  import host-asset;
  use host-asset.{asset-reader};
  export metadata: func() -> string;
  export contract-version: func() -> string;
  export detect: func(asset: borrow<asset-reader>) -> u8;
  export validate: func(asset: borrow<asset-reader>) -> result<string, string>;
  export parse: func(asset: borrow<asset-reader>) -> result<string, string>;
}
```

`detect` returns a confidence from `0` to `100`. It must be based on actual
bytes and headers/content structure. A successful `validate` result is a JSON
`SourceValidation`; a successful `parse` result is a JSON `SourceBatch`.
Malformed input must return a typed error or a `valid: false` validation result,
not a fabricated empty snapshot.

### Minimal guest component

The following is the smallest complete shape of a Rust component. An actual
module replaces `make_batch` with its parser and canonical mapper.

```rust
// src/component.rs
wit_bindgen::generate!({
    path: "../../sdk/wit/source-api.wit",
    world: "source-module",
});

use mfa_contracts::{
    BodyMeasurement, CanonicalObservation, ContractVersion, LineageHook, MappingIssue,
    SourceBatch, SourceRecord, SourceValidation,
};
use serde_json::json;
use uuid::Uuid;

struct Component;
export!(Component);

impl Guest for Component {
    fn metadata() -> String {
        r#"{
          "module_id":"example",
          "module_version":"1.0.0",
          "source_api_version":"1.0.0",
          "mapping_version":"1.0.0",
          "provided_capabilities":["body.weight"],
          "extension_contracts":[],
          "localization_namespace":"source.example"
        }"#.to_owned()
    }

    fn contract_version() -> String {
        "1.0.0".to_owned()
    }

    fn detect(asset: &AssetReader) -> u8 {
        let metadata = asset.metadata();
        if metadata.media_type == "text/csv" && read_all(asset).is_ok() {
            100
        } else {
            0
        }
    }

    fn validate(asset: &AssetReader) -> Result<String, String> {
        let metadata = asset.metadata();
        let bytes = read_all(asset)?;
        let validation = if bytes.starts_with(b"date,weight_kg\n") {
            SourceValidation {
                valid: true,
                issues: Vec::new(),
                source_module_id: "example".to_owned(),
                source_api_version: "1.0.0".parse().map_err(|e| e.to_string())?,
                logical_snapshot_key: "example:2026".to_owned(),
                schema_fingerprint: "sha256:fixture".to_owned(),
                mapping_version: "1.0.0".parse().map_err(|e| e.to_string())?,
            }
        } else {
            SourceValidation {
                valid: false,
                issues: vec![MappingIssue {
                    code: "example.invalid_headers".to_owned(),
                    message: "required headers are missing".to_owned(),
                    source_record_key: None,
                }],
                source_module_id: "example".to_owned(),
                source_api_version: "1.0.0".parse().map_err(|e| e.to_string())?,
                logical_snapshot_key: "example:invalid".to_owned(),
                schema_fingerprint: "sha256:invalid".to_owned(),
                mapping_version: "1.0.0".parse().map_err(|e| e.to_string())?,
            }
        };
        serde_json::to_string(&validation).map_err(|e| e.to_string())
    }

    fn parse(asset: &AssetReader) -> Result<String, String> {
        let bytes = read_all(asset)?;
        let batch = make_batch(&bytes)?;
        serde_json::to_string(&batch).map_err(|e| e.to_string())
    }
}

fn read_all(asset: &AssetReader) -> Result<Vec<u8>, String> {
    let metadata = asset.metadata();
    let length = u32::try_from(metadata.byte_len)
        .map_err(|_| "asset exceeds the guest read limit".to_owned())?;
    asset.read_at(0, length)
}

fn make_batch(bytes: &[u8]) -> Result<SourceBatch, String> {
    let mut rows = std::str::from_utf8(bytes).map_err(|e| e.to_string())?.lines();
    if rows.next() != Some("date,weight_kg") {
        return Err("example headers are missing".to_owned());
    }
    let row = rows.next().ok_or_else(|| "example row is missing".to_owned())?;
    let (date, weight) = row
        .split_once(',')
        .ok_or_else(|| "example row is malformed".to_owned())?;
    let local_date = date.parse().map_err(|e| e.to_string())?;
    let weight_kg = weight.parse::<f64>().map_err(|e| e.to_string())?;
    let source_record_key = "example:measurements:2".to_owned();
    let body_measurement_id = Uuid::from_u128(1);
    let record = BodyMeasurement {
        body_measurement_id,
        local_date,
        weight_kg,
        body_fat_pct: None,
        source_record_id: Some(source_record_key.clone()),
    };
    Ok(SourceBatch {
        contract_version: "1.0.0".parse::<ContractVersion>().map_err(|e| e.to_string())?,
        source_module_id: "example".to_owned(),
        source_api_version: "1.0.0".parse::<ContractVersion>().map_err(|e| e.to_string())?,
        mapping_version: "1.0.0".parse::<ContractVersion>().map_err(|e| e.to_string())?,
        schema_fingerprint: "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        logical_snapshot_key: "example:2026".to_owned(),
        source_records: vec![SourceRecord {
            source_record_key: source_record_key.clone(),
            sheet_name: Some("measurements".to_owned()),
            source_row_number: 2,
            raw_payload: json!({"date": date, "weight_kg": weight}),
        }],
        lineage: vec![LineageHook {
            canonical_entity_type: "body_measurement".to_owned(),
            canonical_entity_id: body_measurement_id.to_string(),
            source_record_key,
            mapping_version: "1.0.0".parse::<ContractVersion>().map_err(|e| e.to_string())?,
        }],
        records: vec![CanonicalObservation::BodyMeasurement(record)],
        extensions: Vec::new(),
        issues: Vec::<MappingIssue>::new(),
    })
}
```

The example deliberately returns a typed parser error until `make_batch` is
implemented; a production module must replace that function with real parsing.
The bundled Hevy component is the reference implementation for reading CSV,
validating, mapping, and serializing a complete `SourceBatch`.

## Canonical output and provenance

Every accepted input produces a logical snapshot key such as
`hevy:measurements:2026` or `mynetdiary:2026`. The host creates immutable
attempt, asset, and source-record IDs; the guest supplies stable source-record
keys.

Rules for `SourceBatch`:

1. Emit one `SourceRecord` for every accepted source row, including rows that
   are not canonicalized. Preserve the original row number, sheet name, and a
   JSON raw payload. Source keys must be unique within a batch and must not be
   derived from a default filename.
2. Emit a canonical observation only when its values pass the domain contract.
   Preserve stable deterministic IDs and local dates/times. Do not silently
   discard unsupported fields: retain them in raw payload or an extension.
3. Emit a `LineageHook` for every canonical entity. Its entity type and ID must
   match the host canonical identity. An ID-less canonical type, such as an
   `ActivityDay`, uses deterministic lineage order and still retains every
   contributing source row.
4. Mapping issues are explicit `MappingIssue` values with a stable code,
   message, and optional source-record key. Unknown exercises, unsupported
   set types, and non-fatal row problems are issues rather than silent changes.
5. Optional source-specific information belongs in a declared extension
   contract. The extension namespace/version/payload schema must match
   `module.json`; undeclared extensions are rejected before commit.
6. Replacement imports create a new immutable attempt and active snapshot.
   Removed canonical rows disappear from active views, while prior source
   rows, attempts, and lineage remain queryable as provenance.
7. A source module does not open DuckDB, execute SQL, access a network, read a
   filesystem path, load credentials, or call arbitrary host functions. It
   receives bytes and typed WIT calls only.

Canonical provider selection is explicit. If two installed modules offer the
same capability, both remain installed and their data remains separate; an
active view uses exactly one configured provider. The host applies bundled
first-profile defaults only when a capability has no explicit choice.

## Extensions and English localization

Use a stable namespace such as `example.measurement-notes` and declare its
contract in `extension_contracts`. Keep payloads JSON objects with explicit
schema requirements. Extension records carry the source row, local date/time,
and payload; they do not replace canonical values.

`locales/en.json` is required for a user-facing module and must be canonical
JSON. Its top-level namespace must equal `localization_namespace`:

```json
{
  "source.example": {
    "display_name": "Example",
    "description": "Synthetic example source"
  }
}
```

English is the fallback namespace used by package verification. Add other
locales only after English is complete; missing translations must not change
module IDs or contract behavior.

## Build, package, and test commands

Use the pinned repository toolchain. From the repository root:

```bash
# Validate deterministic synthetic fixtures and privacy rules
pnpm --dir scripts/fixtures run verify

# Run the source module's Rust unit/conformance tests
cargo test -p mfa-source-hevy -- --test-threads=1
cargo test -p mfa-source-mynetdiary -- --test-threads=1

# Build the component and deterministic .mfasource archives
bash scripts/build-module-packages.sh

# Verify byte equality, manifest hashes, locales, WIT/API versions,
# capabilities, and prohibited guest imports
bash scripts/verify-module-packages.sh

# Run the production-path package/Wasmtime/inbox/DatabaseService gate
cargo test -p mfa-integration-tests --test source_modules_gate -- --test-threads=1
cargo test -p mfa-integration-tests --test provider_selection -- --test-threads=1
```

`build-module-packages.sh` builds each component for
`wasm32-unknown-unknown` with `cargo component`, then invokes
`modules/sdk/tools/build_mfasource.py`. The builder uses stored ZIP entries,
fixed timestamps, canonical manifest JSON, and the exact Wasm bytes so two
builds produce byte-identical archives.

## Resource and capability limits

The host runs source components under Wasmtime limits. The default limits are
64 MiB maximum linear memory, 10,000,000 fuel units, a two-second execution
timeout, and a one MiB output limit. Integration tests may use tighter limits.
The host asset reader enforces bounded `read-at` requests and the package
inspector verifies the component's import surface.

A source guest may import only the WIT `host-asset` interface. It may not import
WASI, filesystem, sockets/network, environment/process access, DuckDB, raw SQL,
credential stores, or arbitrary host APIs. Do not embed tokens, passwords,
connection strings, personal exports, or real user data in fixtures, tests,
logs, or packages. Use deterministic synthetic values only.

## Versioning and compatibility

- Keep `module_id` stable forever once a package has been installed.
- Increment `module_version` for implementation changes.
- Increment `mapping_version` for canonical meaning, identity, parsing, or
  lineage changes; document the compatibility impact.
- Increment `source_api_version` only with a coordinated WIT/host contract
  release. A module declaring an unsupported API version is rejected.
- Increment `package_format_version` only when the package inspector and
  installer are changed together.
- Add a new extension namespace/version for incompatible extension payloads;
  do not reinterpret an existing payload schema.
- Keep `compatible_app_versions` explicit and bounded.
- The bundled catalog is immutable for a build. A later package digest is
  reported as an available update; it is not silently installed over a user
  choice or used to reinstall an explicitly uninstalled module.

Compatibility failures must be typed and actionable: package inspection fails
before installation for malformed manifests, unsupported versions, bad entry
hashes, missing English localization, undeclared capabilities/extensions, or
forbidden imports. Runtime failures fail the attempt without activating a
partial snapshot; previously committed snapshots and provenance remain intact.
