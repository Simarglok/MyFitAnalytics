#!/usr/bin/env python3
"""Build and verify the bundled source packages twice."""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parent.parent
DIST = ROOT / "dist" / "modules"
FORBIDDEN = (b"wasi:", b"duckdb", b"raw-sql", b"credentials")


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def build() -> None:
    subprocess.run(["bash", str(ROOT / "scripts" / "build-module-packages.sh")], cwd=ROOT, check=True)


def verify_package(path: Path) -> str:
    package_bytes = path.read_bytes()
    with ZipFile(path) as archive:
        names = set(archive.namelist())
        required = {"module.json", "module.wasm", "locales/en.json"}
        missing = required - names
        if missing:
            raise SystemExit(f"{path.name}: missing entries {sorted(missing)}")
        manifest = json.loads(archive.read("module.json"))
        locale = json.loads(archive.read("locales/en.json"))
        wasm = archive.read("module.wasm")
        entry_hash = f"sha256:{digest(wasm)}"
        if manifest["entrypoint_hash"] != entry_hash:
            raise SystemExit(f"{path.name}: entrypoint hash mismatch")
        if manifest["artifact_signatures"] != [entry_hash]:
            raise SystemExit(f"{path.name}: artifact signature mismatch")
        if manifest["source_api_version"] != "1.0.0" or manifest["mapping_version"] != "1.0.0":
            raise SystemExit(f"{path.name}: contract versions are not 1.0.0")
        if locale["namespace"] != manifest["localization_namespace"] or locale["locale"] != "en":
            raise SystemExit(f"{path.name}: English locale namespace mismatch")
        lowered = wasm.lower()
        for marker in FORBIDDEN:
            if marker in lowered:
                raise SystemExit(f"{path.name}: forbidden guest marker {marker!r}")
    return digest(package_bytes)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="mfa-package-verify-") as directory:
        first = Path(directory)
        build()
        for module in ("mynetdiary", "hevy"):
            shutil.copy2(DIST / f"{module}.mfasource", first / f"{module}.mfasource")
        build()
        for module in ("mynetdiary", "hevy"):
            package = DIST / f"{module}.mfasource"
            if package.read_bytes() != (first / package.name).read_bytes():
                raise SystemExit(f"{package.name}: package is not deterministic")
            print(f"{package}: sha256:{verify_package(package)}")
    print("verified deterministic MyNetDiary and Hevy packages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
