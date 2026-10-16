#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
python3 verify_sources.py --check
if test -n "${GROK_DIRECT_NATIVE_EXE:-}"; then python3 prepare_native_fixture.py "$(dirname "$GROK_DIRECT_NATIVE_EXE")"; fi
command -v cargo >/dev/null || { echo 'BLOCKED: Rust toolchain missing; parser tests have NOT run.' >&2; exit 127; }
cargo metadata --format-version 1 > results/resolved-cargo-metadata.json
python3 validate_resolved_lock.py
cp Cargo.lock results/resolved-Cargo.lock
set +e
cargo test --locked -- --nocapture 2>&1 | tee results/rust-tests.log
status=${PIPESTATUS[0]}
set -e
python3 write_execution_report.py --exit-code "$status" --stage 'historical Rust parser tests'
exit "$status"
