#!/usr/bin/env python3
"""Build and verify the bundled source and dashboard packages twice."""

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
EXPECTED_PACKAGES = {"mynetdiary.mfasource", "hevy.mfasource", "base.mfadashboard"}
FORBIDDEN = (b"wasi:", b"duckdb", b"raw-sql", b"credentials")


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def build() -> None:
    subprocess.run(["bash", str(ROOT / "scripts" / "build-module-packages.sh")], cwd=ROOT, check=True)


def verify_production_layout() -> None:
    actual = {
        path.name
        for path in DIST.iterdir()
        if path.is_file() and path.suffix in {".mfasource", ".mfadashboard"}
    }
    if actual != EXPECTED_PACKAGES:
        raise SystemExit(
            f"production module package layout mismatch: expected {sorted(EXPECTED_PACKAGES)}, "
            f"found {sorted(actual)}"
        )


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
        if manifest["package_format_version"] != "1.0.0":
            raise SystemExit(f"{path.name}: package format is not 1.0.0")
        if manifest["localization_namespace"] != locale["namespace"] or locale["locale"] != "en":
            raise SystemExit(f"{path.name}: English locale namespace mismatch")
        if manifest["module_type"] == "source":
            if manifest["source_api_version"] != "1.0.0" or manifest["mapping_version"] != "1.0.0":
                raise SystemExit(f"{path.name}: source contract versions are not 1.0.0")
        elif manifest["module_type"] == "dashboard":
            if manifest["dashboard_api_version"] != "1.0.0":
                raise SystemExit(f"{path.name}: dashboard API version is not 1.0.0")
        else:
            raise SystemExit(f"{path.name}: unsupported production module type")
        lowered = wasm.lower()
        for marker in FORBIDDEN:
            if marker in lowered:
                raise SystemExit(f"{path.name}: forbidden guest marker {marker!r}")
    return digest(package_bytes)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="mfa-package-verify-") as directory:
        first = Path(directory)
        build()
        verify_production_layout()
        for package_name in sorted(EXPECTED_PACKAGES):
            shutil.copy2(DIST / package_name, first / package_name)
        build()
        verify_production_layout()
        for package_name in sorted(EXPECTED_PACKAGES):
            package = DIST / package_name
            if package.read_bytes() != (first / package.name).read_bytes():
                raise SystemExit(f"{package.name}: package is not deterministic")
            print(f"{package}: sha256:{verify_package(package)}")
    print("verified deterministic MyNetDiary, Hevy, and base dashboard packages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
