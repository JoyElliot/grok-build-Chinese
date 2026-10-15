$ErrorActionPreference = 'Stop'
$runner = Join-Path $PSScriptRoot '../run-windows-validation.ps1'
$manifest = Get-Content (Join-Path $PSScriptRoot '../windows-validation-tests.json') -Raw | ConvertFrom-Json
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
      foreach ($formatScope in @('workspace', 'suite')) {
        $validationTestState.Calls.Clear()
        & $runner -Mode $mode -Suite $suite -FormatScope $formatScope -SkipResourceMonitor
        $summary = Get-Content (Join-Path $temp "grok-zh-windows-validation-$suite/summary.json") -Raw | ConvertFrom-Json
        $expected = @($manifest.$mode.$suite)
        if ($summary.outcome -ne 'success' -or $summary.commands.Count -ne $expected.Count -or $summary.format_scope -ne $formatScope) {
          throw 'Incomplete success diagnostics.'
        }
        $formatCommands = @(
          if ($formatScope -eq 'suite') {
            $expected | ForEach-Object { $_[1] } | Select-Object -Unique | ForEach-Object { "fmt -p $_ -- --check" }
          } elseif ($suite -eq 'core') { 'fmt --all -- --check' }
        )
        if ($validationTestState.Calls.Count -ne ($expected.Count + $formatCommands.Count)) { throw 'Unexpected cargo invocation count.' }
        for ($index = 0; $index -lt $formatCommands.Count; $index++) {
          if ($validationTestState.Calls[$index] -cne $formatCommands[$index]) { throw 'Incorrect package formatting selection.' }
        }
        for ($index = 0; $index -lt $expected.Count; $index++) {
          $command = "test --frozen --timings -j 4 $($expected[$index] -join ' ')"
          if ($validationTestState.Calls[$index + $formatCommands.Count] -cne $command -or
              ($summary.commands[$index].arguments -join ' ') -cne $command) {
            throw 'Validation commands differ from the shared manifest.'
          }
        }
      }
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
  # MSVC's per-package formatting cannot silently skip failed format checks.
  $validationTestState.Calls.Clear()
  $validationTestState.FailAt = 1
  $failed = $false
  try { & $runner -Suite ui -FormatScope suite -SkipResourceMonitor } catch { $failed = $true }
  $summary = Get-Content (Join-Path $temp 'grok-zh-windows-validation-ui/summary.json') -Raw | ConvertFrom-Json
  if (!$failed -or $validationTestState.Calls.Count -ne 1 -or $summary.outcome -ne 'failure' -or $summary.commands.Count -ne 0) {
    throw 'A failed package format check did not stop the suite.'
  }
  # A successful format check must not hide a failing test under the MSVC path.
  $validationTestState.Calls.Clear()
  $formatCount = @($manifest.preview.ui | ForEach-Object { $_[1] } | Select-Object -Unique).Count
  $validationTestState.FailAt = $formatCount + 2
  $failed = $false
  try { & $runner -Suite ui -FormatScope suite -SkipResourceMonitor } catch { $failed = $true }
  $summary = Get-Content (Join-Path $temp 'grok-zh-windows-validation-ui/summary.json') -Raw | ConvertFrom-Json
  if (!$failed -or $validationTestState.Calls.Count -ne $validationTestState.FailAt -or
      $summary.outcome -ne 'failure' -or $summary.commands.Count -ne 2 -or $summary.commands[-1].exit_code -ne 17) {
    throw 'MSVC validation did not retain the failed test status.'
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
