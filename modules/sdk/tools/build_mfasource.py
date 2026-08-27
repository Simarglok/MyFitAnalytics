#!/usr/bin/env python3
"""Build a deterministic MyFitAnalytics source module package."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from zipfile import ZIP_STORED, ZipFile, ZipInfo


PACKAGE_FORMAT_VERSION = "1.0.0"
DEFAULT_APP_RANGE = ">=0.1.0"


def digest(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def parse_extension(value: str) -> dict[str, str]:
    namespace, separator, version = value.partition("@")
    if not separator or not namespace or not version:
        raise argparse.ArgumentTypeError("extension contract must be namespace@version")
    return {"namespace": namespace, "contract_version": version}


def build_package(args: argparse.Namespace) -> str:
    wasm = Path(args.wasm).read_bytes()
    entrypoint_hash = digest(wasm)
    module_dir = Path(args.module_dir) if args.module_dir else None
    if module_dir:
        manifest = json.loads((module_dir / "module.json").read_text())
        manifest["entrypoint_hash"] = entrypoint_hash
        manifest["artifact_signatures"] = [entrypoint_hash]
        module_entries = [("module.json", canonical_json(manifest)), ("module.wasm", wasm)]
        locale = module_dir / "locales" / "en.json"
        if locale.exists():
            module_entries.append(("locales/en.json", canonical_json(json.loads(locale.read_text()))))
    else:
        extensions = [parse_extension(value) for value in args.extension]
        manifest = {
            "module_type": "source",
            "module_id": args.module_id,
            "module_version": args.module_version,
            "package_format_version": PACKAGE_FORMAT_VERSION,
            "source_api_version": args.source_api_version,
            "mapping_version": args.mapping_version,
            "compatible_app_versions": [DEFAULT_APP_RANGE],
            "provided_capabilities": sorted(set(args.capability)),
            "accepted_file_patterns": sorted(set(args.accepted_pattern)),
            "artifact_signatures": [entrypoint_hash],
            "extension_contracts": sorted(
                extensions,
                key=lambda item: (item["namespace"], item["contract_version"]),
            ),
            "settings_schema": json.loads(args.settings_schema),
            "entrypoint_hash": entrypoint_hash,
            "localization_namespace": args.localization_namespace,
        }
        module_entries = [("module.json", canonical_json(manifest)), ("module.wasm", wasm)]
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(output, "w", compression=ZIP_STORED, allowZip64=False) as archive:
        for name, data in module_entries:
            info = ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_STORED
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data)
    return digest(output.read_bytes())


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--wasm", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--module-id")
    parser.add_argument("--module-version", default="1.0.0")
    parser.add_argument("--source-api-version", default="1.0.0")
    parser.add_argument("--mapping-version", default="1.0.0")
    parser.add_argument("--localization-namespace")
    parser.add_argument("--capability", action="append", default=[])
    parser.add_argument("--accepted-pattern", action="append", default=[])
    parser.add_argument("--extension", action="append", default=[])
    parser.add_argument("--settings-schema", default="{}")
    parser.add_argument("--module-dir")
    args = parser.parse_args()
    if not args.module_dir and (not args.module_id or not args.localization_namespace):
        parser.error("--module-id and --localization-namespace are required without --module-dir")
    print(build_package(args))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
