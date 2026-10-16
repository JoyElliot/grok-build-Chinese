#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
python3 verify_sources.py --check
if ! command -v cargo >/dev/null; then
  python3 write_execution_report.py --stage 'BLOCKED: Rust toolchain missing'
  echo 'BLOCKED: Rust toolchain missing; selection tests have NOT run.' >&2
  exit 127
fi
# Tiny harness only. Cargo may prune unused optional features from copied historical lock.
cargo metadata --format-version 1 > results/resolved-cargo-metadata.json
python3 validate_resolved_lock.py
cp Cargo.lock results/resolved-Cargo.lock
set +e
cargo test --locked -- --nocapture 2>&1 | tee results/rust-tests.log
status=${PIPESTATUS[0]}
set -e
python3 write_execution_report.py --exit-code "$status" --stage 'exact-source offline selection matrix'
exit "$status"
