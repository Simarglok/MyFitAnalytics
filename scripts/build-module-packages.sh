#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$ROOT/dist/modules"
rm -rf "$DIST"
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
  python3 "$ROOT/modules/sdk/tools/build_mfasource.py" \
    --wasm "$ROOT/target/wasm32-unknown-unknown/release/mfa_source_${module}.wasm" \
    --module-dir "$ROOT/modules/sources/$module" \
    --output "$DIST/${module}.mfasource"
  printf 'built %s.mfasource\n' "$module"
done

cargo component build --manifest-path "$ROOT/modules/dashboards/base/Cargo.toml" \
  --release --target wasm32-unknown-unknown
python3 "$ROOT/modules/sdk/tools/build_mfadashboard.py" \
  --wasm "$ROOT/target/wasm32-unknown-unknown/release/mfa_dashboard_base.wasm" \
  --module-dir "$ROOT/modules/dashboards/base" \
  --output "$DIST/base.mfadashboard"
printf '%s\n' 'built base.mfadashboard'
