$ErrorActionPreference = 'Stop'
$env:PYTHONUTF8 = '1'
Set-Location $PSScriptRoot
New-Item -ItemType Directory -Force results | Out-Null
$stage = 'source provenance'
$exitCode = 1
try {
    python verify_sources.py --check
    if ($LASTEXITCODE -ne 0) { throw 'Exact source provenance check failed.' }
    if (!(Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Rust toolchain missing; no selection test result is claimed.' }
    $stage = 'minimal harness dependency resolution'
    cargo metadata --format-version 1 1> results/resolved-cargo-metadata.json 2> results/cargo-metadata.log
    if ($LASTEXITCODE -ne 0) { throw 'Selection harness dependency resolution failed.' }
    python validate_resolved_lock.py
    if ($LASTEXITCODE -ne 0) { throw 'Resolved dependency identity/checksum is outside pinned historical records.' }
    Copy-Item -LiteralPath Cargo.lock -Destination results/resolved-Cargo.lock -Force
    $stage = 'exact-source offline selection matrix'
    cargo test --locked -- --nocapture 2>&1 | Tee-Object results/rust-tests.log
    $exitCode = $LASTEXITCODE
    python write_execution_report.py --exit-code $exitCode --stage $stage
    if ($LASTEXITCODE -ne 0) { throw 'Execution report did not verify all selection assertions.' }
    if ($exitCode -ne 0) { throw "Selection tests failed with exit code $exitCode." }
} catch {
    $_ | Out-String | Add-Content results/launcher-error.log
    python write_execution_report.py --exit-code $exitCode --stage $stage
    throw
}
