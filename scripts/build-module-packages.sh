#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$ROOT/dist/modules"
mkdir -p "$DIST"

if [[ "${1:-}" == "--fixture-only" ]]; then
  python3 "$ROOT/modules/sdk/tools/build_mfasource.py" \
    --wasm "$ROOT/crates/mfa-module-host/tests/fixtures/guest-source.wasm" \
    --output "$DIST/guest-source.mfasource" \
    --module-id guest-source \
    --localization-namespace source.guest \
    --capability body.weight \
    --accepted-pattern '*.fixture'
  exit 0
fi

if [[ ! -d "$ROOT/modules/sources/mynetdiary" || ! -d "$ROOT/modules/sources/hevy" ]]; then
  printf '%s\n' 'source module crates are not present; use --fixture-only during Task 1' >&2
  exit 2
fi

for module in mynetdiary hevy; do
  cargo component build --manifest-path "$ROOT/modules/sources/$module/Cargo.toml" \
    --release --target wasm32-unknown-unknown
  printf 'built %s\n' "$module"
done
