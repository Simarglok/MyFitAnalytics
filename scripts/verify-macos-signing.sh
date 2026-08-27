#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="${1:-$ROOT/target/release/bundle/macos/MyFitAnalytics.app}"
REQUIRED_ENTITLEMENTS=(
    "com.apple.security.cs.allow-unsigned-executable-memory"
    "com.apple.security.cs.disable-library-validation"
)

if [[ "$(uname -s)" != "Darwin" ]]; then
    printf 'macOS signing assertion skipped on non-macOS host\n'
    exit 0
fi

if [[ ! -d "$APP" ]]; then
    printf 'missing app bundle: %s\n' "$APP" >&2
    exit 1
fi

codesign --verify --deep --strict --verbose=2 "$APP"

SIGNED_OUTPUT="$(codesign -d --entitlements :- "$APP" 2>&1 || true)"
ENTITLEMENTS="${SIGNED_OUTPUT#*<?xml}"
if [[ "$ENTITLEMENTS" == "$SIGNED_OUTPUT" ]]; then
    printf 'app has no embedded entitlements plist: %s\n' "$APP" >&2
    exit 1
fi
ENTITLEMENTS="<?xml${ENTITLEMENTS}"
ENTITLEMENTS="${ENTITLEMENTS%%</plist>*}</plist>"

printf '%s\n' "$ENTITLEMENTS" | plutil -lint - >/dev/null
verify_entitlement() {
    local required_entitlement="$1"
    local entitlement_value

    entitlement_value="$(printf '%s\n' "$ENTITLEMENTS" | ENTITLEMENT_KEY="$required_entitlement" python3 -c '
import os
import plistlib
import sys

plist = plistlib.loads(sys.stdin.buffer.read())
key = os.environ["ENTITLEMENT_KEY"]
print(str(plist.get(key, False)).lower())
')"

    if [[ "$entitlement_value" != "true" ]]; then
        printf 'required entitlement is not true: %s\n' "$required_entitlement" >&2
        exit 1
    fi

    printf 'verified macOS entitlement: %s=true\n' "$required_entitlement"
}

for required_entitlement in "${REQUIRED_ENTITLEMENTS[@]}"; do
    verify_entitlement "$required_entitlement"
done
