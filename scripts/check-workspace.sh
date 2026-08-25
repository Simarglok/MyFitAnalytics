#!/usr/bin/env bash
set -euo pipefail

test "$(rustc --version | awk '{print $2}')" = "1.94.0"
test "$(node --version)" = "v24.19.0"
test "$(pnpm --version)" = "11.23.0"
cargo metadata --no-deps --format-version 1 >/dev/null
pnpm --dir web exec vite --version >/dev/null
cargo tauri --version >/dev/null
