#!/usr/bin/env python3
"""Build a deterministic MyFitAnalytics dashboard module package."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from zipfile import ZIP_STORED, ZipFile, ZipInfo


def digest(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def build_package(args: argparse.Namespace) -> str:
    wasm = Path(args.wasm).read_bytes()
    module_dir = Path(args.module_dir)
    manifest = json.loads((module_dir / "module.json").read_text())
    entrypoint_hash = digest(wasm)
    manifest["entrypoint_hash"] = entrypoint_hash
    manifest["artifact_signatures"] = [entrypoint_hash]
    entries = [
        ("module.json", canonical_json(manifest)),
        ("module.wasm", wasm),
    ]
    locale = module_dir / "locales" / "en.json"
    if locale.exists():
        entries.append(("locales/en.json", canonical_json(json.loads(locale.read_text()))))
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(output, "w", compression=ZIP_STORED, allowZip64=False) as archive:
        for name, data in entries:
            info = ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_STORED
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data)
    return digest(output.read_bytes())


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--wasm", required=True)
    parser.add_argument("--module-dir", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    print(build_package(args))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
