$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$runner = Join-Path $PSScriptRoot '../run-windows-validation.ps1'
$manifest = Get-Content (Join-Path $PSScriptRoot '../windows-validation-tests.json') -Raw | ConvertFrom-Json
$temp = Join-Path ([IO.Path]::GetTempPath()) "grok-validation-test-$([guid]::NewGuid())"
$variables = @('RUNNER_TEMP', 'CARGO_TARGET_DIR', 'CARGO_BUILD_JOBS', 'GITHUB_STEP_SUMMARY',
  'PATH', 'GROK_VALIDATION_TEST_LOG', 'GROK_VALIDATION_TEST_FAIL_AT')
$previous = @{}
foreach ($name in $variables) { $previous[$name] = [Environment]::GetEnvironmentVariable($name) }
function Reset-CargoFixture([int]$FailAt = -1) {
  [IO.File]::WriteAllText($env:GROK_VALIDATION_TEST_LOG, '')
  $env:GROK_VALIDATION_TEST_FAIL_AT = [string]$FailAt
}
try {
  New-Item -ItemType Directory -Path $temp -Force | Out-Null
  # A PowerShell function consumes the bare -- end-of-parameters token, unlike
  # native cargo. Keep the production invocation boundary with an actual EXE.
  $source = Join-Path $temp 'cargo.cs'
  $fixture = Join-Path $temp 'cargo.exe'
  @'
using System;
using System.IO;
using System.Text;
class Program {
    static int Main(string[] args) {
        string log = Environment.GetEnvironmentVariable("GROK_VALIDATION_TEST_LOG");
        File.AppendAllLines(log, new[] { string.Join(" ", args) }, new UTF8Encoding(false));
        int failAt = int.Parse(Environment.GetEnvironmentVariable("GROK_VALIDATION_TEST_FAIL_AT"));
        return File.ReadAllLines(log).Length == failAt ? 17 : 0;
    }
}
'@ | Set-Content -LiteralPath $source -Encoding utf8
  & (Join-Path $env:SystemRoot 'Microsoft.NET/Framework64/v4.0.30319/csc.exe') /nologo /target:exe "/out:$fixture" $source
  if ($LASTEXITCODE -ne 0) { throw "Cargo fixture compilation failed: $LASTEXITCODE" }
  $env:PATH = "$temp;$env:PATH"
  $env:GROK_VALIDATION_TEST_LOG = Join-Path $temp 'cargo-calls.txt'
  $resolvedCargo = Get-Command cargo -ErrorAction Stop
  if ($resolvedCargo.CommandType -ne 'Application' -or $resolvedCargo.Source -ne $fixture) {
    throw "Cargo fixture was not selected: $($resolvedCargo.CommandType) $($resolvedCargo.Source)"
  }
  # Guard the fixture itself against the function-mock regression before
  # checking the runner. Both formatting shapes must retain the literal --.
  Reset-CargoFixture
  & cargo fmt --all -- --check
  if ($LASTEXITCODE -ne 0) { throw 'Cargo fixture workspace probe failed.' }
  & cargo fmt -p fixture-package -- --check
  if ($LASTEXITCODE -ne 0) { throw 'Cargo fixture package probe failed.' }
  $calls = @(Get-Content -LiteralPath $env:GROK_VALIDATION_TEST_LOG)
  $probeExpected = @('fmt --all -- --check', 'fmt -p fixture-package -- --check')
  if ($calls.Count -ne 2 -or ($calls -join "`n") -cne ($probeExpected -join "`n")) {
    throw "Native Cargo fixture changed arguments. Expected: [$($probeExpected -join '; ')]; actual: [$($calls -join '; ')]"
  }
  $env:RUNNER_TEMP = $temp
  $env:CARGO_TARGET_DIR = Join-Path $temp 'target'
  $env:CARGO_BUILD_JOBS = '4'
  $env:GITHUB_STEP_SUMMARY = Join-Path $temp 'step-summary.md'
  foreach ($mode in @('preview', 'release')) {
    foreach ($suite in @('core', 'ui')) {
      foreach ($formatScope in @('workspace', 'suite')) {
        Reset-CargoFixture
        & $runner -Mode $mode -Suite $suite -FormatScope $formatScope -SkipResourceMonitor
        $calls = @(Get-Content -LiteralPath $env:GROK_VALIDATION_TEST_LOG)
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
        if ($calls.Count -ne ($expected.Count + $formatCommands.Count)) {
          throw "Unexpected cargo invocation count for $mode/$suite/$formatScope. Expected: $($expected.Count + $formatCommands.Count); actual: $($calls.Count); calls: [$($calls -join '; ')]"
        }
        for ($index = 0; $index -lt $formatCommands.Count; $index++) {
          if ($calls[$index] -cne $formatCommands[$index]) {
            throw "Incorrect package formatting selection for $mode/$suite/$formatScope at index ${index}. Expected: [$($formatCommands[$index])]; actual: [$($calls[$index])]"
          }
        }
        for ($index = 0; $index -lt $expected.Count; $index++) {
          $command = "test --frozen --timings -j 4 $($expected[$index] -join ' ')"
          if ($calls[$index + $formatCommands.Count] -cne $command -or
              ($summary.commands[$index].arguments -join ' ') -cne $command) {
            throw "Validation commands differ from the shared manifest for $mode/$suite/$formatScope at index ${index}. Expected: [$command]; actual: [$($calls[$index + $formatCommands.Count])]; summary: [$($summary.commands[$index].arguments -join ' ')]"
          }
        }
      }
    }
  }
  # A failing test must stop subsequent commands and retain its exit status.
  Reset-CargoFixture -FailAt 2
  $failed = $false
  try { & $runner -Suite ui -SkipResourceMonitor } catch { $failed = $true }
  $calls = @(Get-Content -LiteralPath $env:GROK_VALIDATION_TEST_LOG)
  $summary = Get-Content (Join-Path $temp 'grok-zh-windows-validation-ui/summary.json') -Raw | ConvertFrom-Json
  if (!$failed -or $calls.Count -ne 2 -or $summary.outcome -ne 'failure' -or $summary.commands[-1].exit_code -ne 17) {
    throw 'Failed tests did not stop the suite or preserve diagnostics.'
  }
  # MSVC's per-package formatting cannot silently skip failed format checks.
  Reset-CargoFixture -FailAt 1
  $failed = $false
  try { & $runner -Suite ui -FormatScope suite -SkipResourceMonitor } catch { $failed = $true }
  $calls = @(Get-Content -LiteralPath $env:GROK_VALIDATION_TEST_LOG)
  $summary = Get-Content (Join-Path $temp 'grok-zh-windows-validation-ui/summary.json') -Raw | ConvertFrom-Json
  if (!$failed -or $calls.Count -ne 1 -or $summary.outcome -ne 'failure' -or $summary.commands.Count -ne 0) {
    throw 'A failed package format check did not stop the suite.'
  }
  # A successful format check must not hide a failing test under the MSVC path.
  $formatCount = @($manifest.preview.ui | ForEach-Object { $_[1] } | Select-Object -Unique).Count
  Reset-CargoFixture -FailAt ($formatCount + 2)
  $failed = $false
  try { & $runner -Suite ui -FormatScope suite -SkipResourceMonitor } catch { $failed = $true }
  $calls = @(Get-Content -LiteralPath $env:GROK_VALIDATION_TEST_LOG)
  $summary = Get-Content (Join-Path $temp 'grok-zh-windows-validation-ui/summary.json') -Raw | ConvertFrom-Json
  if (!$failed -or $calls.Count -ne ($formatCount + 2) -or
      $summary.outcome -ne 'failure' -or $summary.commands.Count -ne 2 -or $summary.commands[-1].exit_code -ne 17) {
    throw 'MSVC validation did not retain the failed test status.'
  }
  # Exercise the actual monitor startup/cleanup with a tiny mocked Cargo run.
  Reset-CargoFixture
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
