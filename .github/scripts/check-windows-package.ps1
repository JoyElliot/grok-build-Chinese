[CmdletBinding()]
param([ValidateSet('preview', 'release')][string]$Mode = 'preview')

$ErrorActionPreference = 'Stop'
& "$PSScriptRoot/tests/Test-Write-ReleaseNotes.ps1"
& "$PSScriptRoot/tests/Test-ReleasePolicy.ps1"
& "$PSScriptRoot/tests/Test-WindowsValidation.ps1"
foreach ($test in @('test_release_workflow.py', 'test_windows_validation.py', 'test_package_protocol.py', 'test_windows_binary.py')) {
  & python -B "$PSScriptRoot/tests/$test"
  if ($LASTEXITCODE -ne 0) { throw "$test failed: $LASTEXITCODE" }
}
foreach ($test in @('Test-Install-GrokZh.ps1', 'Test-Install-GrokZhOnline.ps1')) {
  $path = Join-Path $PSScriptRoot "../../packaging/windows/tests/$test"
  & (Join-Path $PSHOME 'pwsh.exe') -NoProfile -NonInteractive -File $path
  if ($LASTEXITCODE -ne 0) { throw "PowerShell 7 $test failed: $LASTEXITCODE" }
  & powershell.exe -NoProfile -NonInteractive -File $path
  if ($LASTEXITCODE -ne 0) { throw "Windows PowerShell 5.1 $test failed: $LASTEXITCODE" }
}
if ($Mode -eq 'preview') {
  $path = Join-Path $PSScriptRoot '../../crates/codegen/xai-grok-locale/locales/zh-CN-metadata.json'
  $metadata = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
  $count = @($metadata.PSObject.Properties).Count
  if ($count -lt 2000) { throw "zh-CN 目录条目数异常偏少：$count" }
  if (Select-String -LiteralPath $path -SimpleMatch '词元' -Quiet) {
    throw 'zh-CN 目录必须保留 Token，而不是“词元”。'
  }
  "已验证 $count 个 zh-CN 条目。"
}
