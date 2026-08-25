#!/usr/bin/env bash
set -euo pipefail

cargo test -p mfa-integration-tests --test storage_gate -- --test-threads=1
cargo test -p myfitanalytics --test ingestion_commands -- --test-threads=1
