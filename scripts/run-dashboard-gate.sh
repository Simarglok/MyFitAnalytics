#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Build fresh checked-in source/dashboard components before the gate.
bash "$ROOT_DIR/scripts/build-module-packages.sh"

# Run one serial app-command gate against isolated synthetic roots.
cargo test -p mfa-integration-tests --test dashboard_gate -- --test-threads=1
