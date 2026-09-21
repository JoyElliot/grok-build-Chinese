$ErrorActionPreference = 'Stop'
$runner = Join-Path $PSScriptRoot '../run-windows-validation.ps1'
$temp = Join-Path ([IO.Path]::GetTempPath()) "grok-validation-test-$([guid]::NewGuid())"
$variables = @('RUNNER_TEMP', 'CARGO_TARGET_DIR', 'CARGO_BUILD_JOBS', 'GITHUB_STEP_SUMMARY')
$previous = @{}
foreach ($name in $variables) { $previous[$name] = [Environment]::GetEnvironmentVariable($name) }
$validationTestState = @{ Calls = [Collections.Generic.List[string]]::new(); FailAt = -1 }
function cargo {
  $validationTestState.Calls.Add(($args -join ' '))
  $global:LASTEXITCODE = if ($validationTestState.Calls.Count -eq $validationTestState.FailAt) { 17 } else { 0 }
}
try {
  New-Item -ItemType Directory -Path $temp -Force | Out-Null
  $env:RUNNER_TEMP = $temp
  $env:CARGO_TARGET_DIR = Join-Path $temp 'target'
  $env:CARGO_BUILD_JOBS = '4'
  $env:GITHUB_STEP_SUMMARY = Join-Path $temp 'step-summary.md'
  foreach ($mode in @('preview', 'release')) {
    foreach ($suite in @('core', 'ui')) {
      $validationTestState.Calls.Clear()
      & $runner -Mode $mode -Suite $suite -SkipResourceMonitor
      $summary = Get-Content (Join-Path $temp "grok-zh-windows-validation-$suite/summary.json") -Raw | ConvertFrom-Json
      $expected = if ($mode -eq 'preview') { if ($suite -eq 'core') { 7 } else { 5 } } else { if ($suite -eq 'core') { 9 } else { 6 } }
      if ($summary.outcome -ne 'success' -or $summary.commands.Count -ne $expected) { throw 'Incomplete success diagnostics.' }
      $extra = if ($suite -eq 'core') { 1 } else { 0 }
      if ($validationTestState.Calls.Count -ne ($expected + $extra)) { throw 'Unexpected cargo invocation count.' }
    }
  }
  # A failing test must stop subsequent commands and retain its exit status.
  $validationTestState.Calls.Clear()
  $validationTestState.FailAt = 2
  $failed = $false
  try { & $runner -Suite ui -SkipResourceMonitor } catch { $failed = $true }
  $summary = Get-Content (Join-Path $temp 'grok-zh-windows-validation-ui/summary.json') -Raw | ConvertFrom-Json
  if (!$failed -or $validationTestState.Calls.Count -ne 2 -or $summary.outcome -ne 'failure' -or $summary.commands[-1].exit_code -ne 17) {
    throw 'Failed tests did not stop the suite or preserve diagnostics.'
  }
  # Exercise the actual monitor startup/cleanup with a tiny mocked Cargo run.
  $validationTestState.Calls.Clear()
  $validationTestState.FailAt = -1
  & $runner -Suite ui
  $resources = Join-Path $temp 'grok-zh-windows-validation-ui/resources.json'
  if (!(Test-Path -LiteralPath $resources)) { throw 'Missing resource diagnostics.' }
  'Windows validation runner tests passed.'
} finally {
  foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $previous[$name]) }
  # The GUID directory is exclusively owned by this test.
  $resolved = [IO.Path]::GetFullPath($temp)
  $allowed = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
  if (!$resolved.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe test cleanup path.' }
  if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
