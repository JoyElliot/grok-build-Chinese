param([string]$NativeExe = $env:GROK_DIRECT_NATIVE_EXE)
$ErrorActionPreference = 'Stop'
$env:PYTHONUTF8 = '1'
Set-Location $PSScriptRoot
New-Item -ItemType Directory -Force results, fixtures | Out-Null
$stage = 'source provenance'
$exitCode = 1
try {
    python verify_sources.py --check
    if ($LASTEXITCODE -ne 0) { throw 'Historical source provenance check failed.' }
    if ([string]::IsNullOrWhiteSpace($NativeExe) -or !(Test-Path -LiteralPath $NativeExe -PathType Leaf)) {
        throw 'GROK_DIRECT_NATIVE_EXE must point to the verified native grok-zh.exe; surrounding package files must be present.'
    }
    $stage = 'native fixture construction'
    python prepare_native_fixture.py (Split-Path -Parent $NativeExe)
    if ($LASTEXITCODE -ne 0) { throw 'Native fixture construction or pinned payload verification failed.' }
    if (!(Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Rust toolchain is required; no parser test result is claimed.' }
    $stage = 'historical dependency lock resolution'
    # No full application build. Resolve the tiny parser harness only, seeded
    # from the exact historical dependency closure. Cargo may prune unused
    # records. Reject any new version/checksum outside the historical lock.
    cargo metadata --format-version 1 1> results/resolved-cargo-metadata.json 2> results/cargo-metadata.log
    if ($LASTEXITCODE -ne 0) { throw 'Parser harness dependency resolution failed.' }
    python validate_resolved_lock.py
    if ($LASTEXITCODE -ne 0) { throw 'Resolved dependencies are outside the pinned historical dependency set.' }
    Copy-Item -LiteralPath Cargo.lock -Destination results/resolved-Cargo.lock -Force
    $stage = 'historical Rust parser tests'
    cargo test --locked -- --nocapture 2>&1 | Tee-Object results/rust-tests.log
    $exitCode = $LASTEXITCODE
    python write_execution_report.py --exit-code $exitCode --stage $stage
    if ($LASTEXITCODE -ne 0) { throw 'Parser execution report did not verify successful native-payload coverage.' }
    if ($exitCode -ne 0) { throw "Historical Rust parser tests failed with exit code $exitCode." }
} catch {
    $_ | Out-String | Add-Content results/launcher-error.log
    python write_execution_report.py --exit-code $exitCode --stage $stage
    throw
}
