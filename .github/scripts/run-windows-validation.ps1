[CmdletBinding()]
param(
  [Parameter(Mandatory)][ValidateSet('core', 'ui')][string]$Suite,
  [ValidateSet('preview', 'release')][string]$Mode = 'preview',
  [switch]$SkipResourceMonitor
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$manifest = Get-Content "$PSScriptRoot/windows-validation-tests.json" -Raw | ConvertFrom-Json
$commands = @($manifest.$Mode.$Suite)
if ($commands.Count -eq 0) { throw 'Empty validation suite.' }
foreach ($name in @('RUNNER_TEMP', 'CARGO_TARGET_DIR', 'CARGO_BUILD_JOBS')) {
  if (![Environment]::GetEnvironmentVariable($name)) { throw "Missing $name" }
}
$root = Join-Path $env:RUNNER_TEMP "grok-zh-windows-validation-$Suite"
New-Item -ItemType Directory -Path $root -Force | Out-Null
$monitorScript = Join-Path $PSScriptRoot 'windows-build-resource-monitor.ps1'
$csv = Join-Path $root 'samples.csv'
$stop = Join-Path $root 'stop'
$ready = Join-Path $root 'ready'
$monitor = $null
$clock = [Diagnostics.Stopwatch]::new()
$results = [Collections.Generic.List[object]]::new()
$outcome = 'failure'
try {
  if (!$SkipResourceMonitor) {
    foreach ($signal in @($stop, $ready)) {
      if (Test-Path -LiteralPath $signal) { Remove-Item -LiteralPath $signal }
    }
    $arguments = @('-NoLogo', '-NoProfile', '-NonInteractive', '-File', ('"{0}"' -f $monitorScript),
      '-Mode', 'Monitor', '-CsvPath', ('"{0}"' -f $csv), '-StopPath', ('"{0}"' -f $stop),
      '-ReadyPath', ('"{0}"' -f $ready), '-TargetPath', ('"{0}"' -f $env:CARGO_TARGET_DIR))
    $monitor = Start-Process -FilePath (Join-Path $PSHOME 'pwsh.exe') -ArgumentList $arguments `
      -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $root 'monitor.stdout.log') `
      -RedirectStandardError (Join-Path $root 'monitor.stderr.log')
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while (!(Test-Path -LiteralPath $ready) -and !$monitor.HasExited -and [DateTime]::UtcNow -lt $deadline) {
      Start-Sleep -Milliseconds 250
    }
    if (!(Test-Path -LiteralPath $ready)) { throw 'Resource monitor did not become ready.' }
  }
  $clock.Start()
  if ($Suite -eq 'core') {
    & cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw "cargo fmt failed: $LASTEXITCODE" }
  }
  foreach ($selection in $commands) {
    $cargoArgs = @('test', '--frozen', '--timings', '-j', $env:CARGO_BUILD_JOBS) + @($selection)
    Write-Host "::group::cargo $($cargoArgs -join ' ')"
    $commandClock = [Diagnostics.Stopwatch]::StartNew()
    $exitCode = $null
    try {
      & cargo @cargoArgs
      $exitCode = $LASTEXITCODE
      if ($exitCode -ne 0) { throw "cargo test failed: $exitCode" }
    } finally {
      $results.Add([ordered]@{ arguments = $cargoArgs; seconds = $commandClock.Elapsed.TotalSeconds; exit_code = $exitCode })
      Write-Host '::endgroup::'
    }
  }
  $outcome = 'success'
} finally {
  $clock.Stop()
  if ($null -ne $monitor) {
    New-Item -ItemType File -Path $stop -Force | Out-Null
    if (!$monitor.WaitForExit(20000)) { $monitor.Kill(); $monitor.WaitForExit() }
    $monitor.Dispose()
  }
  $summary = [ordered]@{
    suite = $Suite; mode = $Mode; outcome = $outcome
    run_id = $env:GITHUB_RUN_ID; run_attempt = $env:GITHUB_RUN_ATTEMPT; commit = $env:GITHUB_SHA
    version = $env:GROK_VERSION; toolchain = $env:RUSTUP_TOOLCHAIN
    profile_debug = $env:CARGO_PROFILE_TEST_DEBUG; cargo_incremental = $env:CARGO_INCREMENTAL
    build_jobs = $env:CARGO_BUILD_JOBS; elapsed_seconds = $clock.Elapsed.TotalSeconds
    cache_hit = $env:VALIDATION_CACHE_HIT; cache_key = $env:VALIDATION_CACHE_KEY
    cpu = $null; debug_total_bytes = $null; debug_file_count = $null
    commands = $results.ToArray()
  }
  try {
    $summary.cpu = @(Get-CimInstance Win32_Processor | Select-Object Name, NumberOfLogicalProcessors)
    $debug = Join-Path $env:CARGO_TARGET_DIR 'debug'
    if (Test-Path -LiteralPath $debug) {
      $stats = Get-ChildItem -LiteralPath $debug -Recurse -File -Force | Measure-Object Length -Sum
      $summary.debug_total_bytes = [long]$stats.Sum
      $summary.debug_file_count = [long]$stats.Count
    }
  } catch { Write-Warning "Diagnostic inventory failed: $_" }
  $summary | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $root 'summary.json') -Encoding utf8
  if ($env:GITHUB_STEP_SUMMARY) {
    "### Windows $Suite tests ($Mode)`nOutcome: $outcome; cache hit: $env:VALIDATION_CACHE_HIT`n" |
      Add-Content $env:GITHUB_STEP_SUMMARY
    foreach ($result in $results) {
      "- $($result.arguments -join ' '): $([math]::Round($result.seconds, 2)) s; exit=$($result.exit_code)" |
        Add-Content $env:GITHUB_STEP_SUMMARY
    }
  }
  if (!$SkipResourceMonitor -and (Test-Path -LiteralPath $csv)) {
    & $monitorScript -Mode Summarize -CsvPath $csv -SummaryPath (Join-Path $root 'resources.json') `
      -StepSummaryPath $env:GITHUB_STEP_SUMMARY -BuildOutcome $outcome `
      -BuildDurationSeconds ([int][math]::Min(10800, $clock.Elapsed.TotalSeconds))
  }
}
