$ErrorActionPreference = 'Stop'
$scriptPath = Join-Path (Split-Path -Parent $PSScriptRoot) 'validate-release-policy.ps1'
$policy = [pscustomobject]@{ publish_legacy_sha256 = $true; legacy_bridge_tag = $null }
if (!(& $scriptPath -Policy $policy -Releases @() -CurrentTag release-v1.0.16)) { throw '过渡期应继续发布 sidecar。' }
$policy.publish_legacy_sha256 = $false
function Assert-Fails($Action) {
    $failed = $false
    try { & $Action | Out-Null } catch { $failed = $true }
    if (!$failed) { throw '不完整的退休策略未被拒绝。' }
}
Assert-Fails { & $scriptPath -Policy $policy -Releases @() -CurrentTag release-v1.0.30 }
$policy.legacy_bridge_tag = 'release-v1.0.20'
$assets = @('windows-x86_64-gnu.zip', 'macos-aarch64.tar.gz', 'linux-x86_64-gnu.tar.gz') | ForEach-Object {
    $name = "grok-zh-1.0.20-$_"
    foreach ($file in @($name, "$name.sha256")) { [pscustomobject]@{ name = $file; state = 'uploaded'; digest = 'sha256:' + ('ab' * 32) } }
}
$bridge = [pscustomobject]@{ tag_name = $policy.legacy_bridge_tag; draft = $false; prerelease = $false; immutable = $true; assets = @($assets) }
$oldest = [pscustomobject]@{ tag_name = 'v1.0.8'; draft = $false; prerelease = $false; immutable = $true; assets = @(
    'grok-zh-1.0.8-windows-x86_64-gnu.zip', 'grok-zh-1.0.8-windows-x86_64-gnu.zip.sha256'
) | ForEach-Object { [pscustomobject]@{ name = $_; state = 'uploaded'; digest = 'sha256:' + ('ab' * 32) } } }
if (& $scriptPath -Policy $policy -Releases @($bridge, $oldest) -CurrentTag release-v1.0.30) { throw '退休策略应只公开归档。' }
if (& $scriptPath -Policy $policy -Releases @($bridge, $oldest) -CurrentTag release-v1.0.30-rc.1) { throw '预发布也可以停止 sidecar。' }
Assert-Fails { & $scriptPath -Policy $policy -Releases @($bridge) -CurrentTag release-v1.0.30 }
$oldest.immutable = $false
Assert-Fails { & $scriptPath -Policy $policy -Releases @($bridge, $oldest) -CurrentTag release-v1.0.30 }
$oldest.immutable = $true
$savedOldAssets = $oldest.assets; $oldest.assets = @($oldest.assets[0])
Assert-Fails { & $scriptPath -Policy $policy -Releases @($bridge, $oldest) -CurrentTag release-v1.0.30 }
$oldest.assets = $savedOldAssets
Assert-Fails { & $scriptPath -Policy $policy -Releases @($bridge) -CurrentTag v1.0.8 }
Assert-Fails { & $scriptPath -Policy $policy -Releases @($bridge) -CurrentTag release-v1.0.20 }
$bridge.immutable = $false
Assert-Fails { & $scriptPath -Policy $policy -Releases @($bridge, $oldest) -CurrentTag release-v1.0.30 }
$bridge.immutable = $true; $bridge.assets = @($assets | Select-Object -Skip 1)
Assert-Fails { & $scriptPath -Policy $policy -Releases @($bridge, $oldest) -CurrentTag release-v1.0.30 }
$policy.legacy_bridge_tag = 'release-v1.0.13'
Assert-Fails { & $scriptPath -Policy $policy -Releases @() -CurrentTag release-v1.0.30 }

# Exercise the publisher's actual asset selection without downloading or publishing.
$workflowPath = Join-Path $PSScriptRoot '../../workflows/zh-release-windows.yml'
$workflow = Get-Content -LiteralPath $workflowPath -Raw -Encoding UTF8
$assembly = [regex]::Match($workflow, '(?ms)^          \$assets = @\(.*?(?=^          \$notes = \$env:RELEASE_NOTES_PATH)')
$selection = [regex]::Match($workflow, '(?ms)^          # Select public Release assets.*?(?=^          \$releasePages = @\()')
if (!$assembly.Success -or !$selection.Success) { throw '未找到发布器的实际资产选择脚本。' }
$assembleAssets = [scriptblock]::Create($assembly.Value)
$selectPublicAssets = [scriptblock]::Create($selection.Value)
$fixtureEnv = @{
    ZIP_NAME = 'grok-zh-1.0.36-windows-x86_64-gnu.zip'
    MAC_ARCHIVE_NAME = 'grok-zh-1.0.36-macos-aarch64.tar.gz'
    LINUX_ARCHIVE_NAME = 'grok-zh-1.0.36-linux-x86_64-gnu.tar.gz'
    WINDOWS_ARM_ARCHIVE_NAME = 'grok-zh-1.0.36-windows-aarch64-msvc.zip'
    MAC_INTEL_ARCHIVE_NAME = 'grok-zh-1.0.36-macos-x86_64.tar.gz'
    LINUX_ARM_ARCHIVE_NAME = 'grok-zh-1.0.36-linux-aarch64-gnu.tar.gz'
    INCLUDE_MACOS = 'false'
    INCLUDE_LINUX = 'false'
    INCLUDE_NEW_PLATFORMS = 'false'
    PUBLISH_LEGACY_SHA256 = 'true'
}
$savedEnv = @{}
function Assert-AssetNames($ActualPaths, $ExpectedNames) {
    $actualNames = @($ActualPaths | ForEach-Object { Split-Path $_ -Leaf } | Sort-Object)
    $expectedSorted = @($ExpectedNames | Sort-Object)
    if (($actualNames -join "`n") -cne ($expectedSorted -join "`n")) {
        throw "发布资产集合错误：$($actualNames -join ', ')"
    }
}
function Assert-PublicAssets([int]$Platforms, [bool]$KeepLegacy) {
    $env:INCLUDE_MACOS = ($Platforms -ge 3).ToString().ToLowerInvariant()
    $env:INCLUDE_LINUX = $env:INCLUDE_MACOS
    $env:INCLUDE_NEW_PLATFORMS = ($Platforms -eq 6).ToString().ToLowerInvariant()
    $env:PUBLISH_LEGACY_SHA256 = $KeepLegacy.ToString().ToLowerInvariant()
    $dist = [IO.Path]::GetTempPath()
    . $assembleAssets
    $original = @($env:ZIP_NAME)
    if ($Platforms -ge 3) { $original += @($env:MAC_ARCHIVE_NAME, $env:LINUX_ARCHIVE_NAME) }
    $archives = @($original)
    if ($Platforms -eq 6) { $archives += @($env:WINDOWS_ARM_ARCHIVE_NAME, $env:MAC_INTEL_ARCHIVE_NAME, $env:LINUX_ARM_ARCHIVE_NAME) }
    # Every platform still has an internal checksum for the pre-publication gate.
    Assert-AssetNames $assets @($archives | ForEach-Object { $_; "$_.sha256" })
    . $selectPublicAssets
    $expected = @($archives)
    if ($KeepLegacy) { $expected += @($original | ForEach-Object { "$_.sha256" }) }
    Assert-AssetNames $assets $expected
}
try {
    foreach ($key in $fixtureEnv.Keys) {
        $savedEnv[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
        [Environment]::SetEnvironmentVariable($key, $fixtureEnv[$key], 'Process')
    }
    foreach ($platforms in @(1, 3, 6)) {
        Assert-PublicAssets $platforms $true
        Assert-PublicAssets $platforms $false
    }
    $env:PUBLISH_LEGACY_SHA256 = 'invalid'
    Assert-Fails { . $selectPublicAssets }
} finally {
    foreach ($key in $savedEnv.Keys) { [Environment]::SetEnvironmentVariable($key, $savedEnv[$key], 'Process') }
}
Write-Host 'Release 过渡、退休策略与实际公开附件集合测试通过。'
