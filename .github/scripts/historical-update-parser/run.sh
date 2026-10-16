#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
python3 verify_sources.py --check
printf '%s\n' 'Native Windows transaction tests require Run-HistoricalParser.ps1 and its credential-free direct test-EXE runner. No substitute test execution was performed.' >&2
exit 2
