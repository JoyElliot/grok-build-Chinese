# Package the native ARM64 MSVC executable with the same Windows layout and
# update protocol used by the existing x64 GNU release.
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

function Assert-Arm64PE([string]$Path) {
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 256 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
        throw "不是 PE 可执行文件：$Path"
    }
    $pe = [BitConverter]::ToInt32($bytes, 0x3c)
    if ($pe -lt 0 -or $pe + 26 -gt $bytes.Length -or
        $bytes[$pe] -ne 0x50 -or $bytes[$pe + 1] -ne 0x45 -or
        $bytes[$pe + 2] -ne 0 -or $bytes[$pe + 3] -ne 0) {
        throw "PE 头无效：$Path"
    }
    $machine = [BitConverter]::ToUInt16($bytes, $pe + 4)
    $optional = [BitConverter]::ToUInt16($bytes, $pe + 24)
    if ($machine -ne 0xaa64 -or $optional -ne 0x20b) {
        throw "不是 ARM64 PE32+：$Path (machine=$machine)"
    }
}

if ($env:TARGET -cne 'aarch64-pc-windows-msvc' -or
    $env:PACKAGE_NAME -cne "grok-zh-$env:GROK_VERSION-windows-aarch64-msvc" -or
    $env:ARCHIVE_NAME -cne "$env:PACKAGE_NAME.zip") {
    throw 'ARM64 打包元数据不一致。'
}
$dist = Join-Path $env:GITHUB_WORKSPACE 'dist'
$package = Join-Path $dist $env:PACKAGE_NAME
New-Item -ItemType Directory -Path $package -Force | Out-Null
$built = Join-Path $env:CARGO_TARGET_DIR "$env:TARGET\release-dist\grok-zh.exe"
if (!(Test-Path -LiteralPath $built -PathType Leaf)) { throw "缺少 ARM64 构建结果：$built" }
Assert-Arm64PE $built
$program = Join-Path $package 'grok-zh.exe'
Copy-Item -LiteralPath $built -Destination $program
foreach ($arguments in @(
    @('--version'), @('--help'), @('agent', '--help'), @('update', '--help')
)) {
    $lines = @(& $program @arguments)
    $output = $lines | Select-Object -First 1
    if ($LASTEXITCODE -ne 0 -or !$output) { throw "ARM64 程序冒烟失败：$($arguments -join ' ')" }
    if ($arguments.Count -eq 1 -and $arguments[0] -eq '--version' -and
        $output -notmatch "^grok-zh $([regex]::Escape($env:GROK_VERSION)) \(") {
        throw "ARM64 程序版本不一致：$output"
    }
}

foreach ($name in @('agent-zh.cmd', '一键安装.cmd', '[可选]替换原始启动方式.cmd',
                    'Install-GrokZh.ps1', 'INSTALL-WINDOWS.md')) {
    Copy-Item -LiteralPath (Join-Path 'packaging\windows' $name) -Destination $package
}
$rgVersion = '15.1.0'
$rgArchive = Join-Path $env:RUNNER_TEMP 'ripgrep-arm64.zip'
$rgRoot = Join-Path $env:RUNNER_TEMP 'ripgrep-arm64'
Invoke-WebRequest -Uri "https://github.com/BurntSushi/ripgrep/releases/download/$rgVersion/ripgrep-$rgVersion-aarch64-pc-windows-msvc.zip" -OutFile $rgArchive -MaximumRetryCount 3 -RetryIntervalSec 5 -TimeoutSec 120
$rgHash = (Get-FileHash -LiteralPath $rgArchive -Algorithm SHA256).Hash
if ($rgHash -cne '00D931FB5237C9696CA49308818EDB76D8EB6FC132761CB2A1BD616B2DF02F8E') {
    throw "ARM64 ripgrep ZIP SHA-256 不匹配：$rgHash"
}
Expand-Archive -LiteralPath $rgArchive -DestinationPath $rgRoot
$rgExe = Get-ChildItem -LiteralPath $rgRoot -Filter rg.exe -Recurse -File | Select-Object -First 1
if (!$rgExe) { throw 'ARM64 ripgrep ZIP 缺少 rg.exe。' }
Assert-Arm64PE $rgExe.FullName
Copy-Item -LiteralPath $rgExe.FullName -Destination (Join-Path $package 'rg.exe')

Copy-Item -LiteralPath 'LICENSE' -Destination (Join-Path $package 'LICENSE-grok-build.txt')
$rgLicenses = Join-Path $package 'licenses\ripgrep'
$projectNotices = Join-Path $package 'licenses\project'
New-Item -ItemType Directory -Path $rgLicenses, $projectNotices -Force | Out-Null
foreach ($name in @('COPYING', 'LICENSE-MIT', 'UNLICENSE')) {
    $source = Get-ChildItem -LiteralPath $rgRoot -Filter $name -Recurse -File | Select-Object -First 1
    if (!$source) { throw "ARM64 ripgrep ZIP 缺少许可证：$name" }
    Copy-Item -LiteralPath $source.FullName -Destination $rgLicenses
}
Copy-Item -LiteralPath 'THIRD-PARTY-NOTICES' -Destination $projectNotices
Copy-Item -LiteralPath 'crates\codegen\xai-grok-tools\THIRD_PARTY_NOTICES.md' -Destination $projectNotices
Copy-Item -LiteralPath 'third_party\NOTICE' -Destination $projectNotices

$sourceRev = (Get-Content -LiteralPath 'SOURCE_REV' -Raw).Trim()
$buildInfo = @"
Grok Build 简体中文 Windows ARM64 MSVC
Product: grok-build-zh
Version: $env:GROK_VERSION
Commit: $env:GITHUB_SHA
Upstream source revision: $sourceRev
Repository: $env:GITHUB_REPOSITORY
Target: $env:TARGET
Profile: release-dist (Thin LTO, debug=0; native ARM64 MSVC)
PE: ARM64 PE32+ verified
Executable smoke-tested by CI: true
GitHub Immutable Release required: true
GitHub Actions artifact attestation required: true
Default commands after installation: grok-zh, agent-zh
"@
$utf8 = [Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText((Join-Path $package 'BUILD-INFO.txt'), $buildInfo.Trim() + "`n", $utf8)
python .github/scripts/write-package-protocol.py --package $package --version $env:GROK_VERSION --platform $env:TARGET
if ($LASTEXITCODE -ne 0) { throw '生成 ARM64 包内更新协议失败。' }

$manifestNames = @(
    'grok-zh.exe', 'agent-zh.cmd', 'rg.exe', '一键安装.cmd',
    '[可选]替换原始启动方式.cmd', 'Install-GrokZh.ps1',
    'INSTALL-WINDOWS.md', 'LICENSE-grok-build.txt', 'BUILD-INFO.txt',
    'licenses/ripgrep/COPYING', 'licenses/ripgrep/LICENSE-MIT',
    'licenses/ripgrep/UNLICENSE', 'licenses/project/THIRD-PARTY-NOTICES',
    'licenses/project/THIRD_PARTY_NOTICES.md', 'licenses/project/NOTICE'
)
$manifest = foreach ($name in $manifestNames) {
    $hash = (Get-FileHash -LiteralPath (Join-Path $package $name) -Algorithm SHA256).Hash
    "$hash  $name"
}
[IO.File]::WriteAllText((Join-Path $package 'SHA256SUMS.txt'), ($manifest -join "`n") + "`n", $utf8)
$actualNames = @(Get-ChildItem -LiteralPath $package -Recurse -File | ForEach-Object {
    [IO.Path]::GetRelativePath($package, $_.FullName).Replace('\', '/')
})
$expectedNames = @($manifestNames + 'SHA256SUMS.txt')
[Array]::Sort($actualNames, [StringComparer]::Ordinal)
[Array]::Sort($expectedNames, [StringComparer]::Ordinal)
if (($actualNames -join "`n") -cne ($expectedNames -join "`n")) {
    throw "ARM64 包文件集合不精确：$($actualNames -join ', ')"
}
$zip = Join-Path $dist $env:ARCHIVE_NAME
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory(
    $package, $zip, [IO.Compression.CompressionLevel]::SmallestSize, $true
)
if ((Get-Item -LiteralPath $zip).Length -gt 536870912L) {
    throw 'ARM64 ZIP 超过更新器 512 MiB 上限。'
}
$verifyRoot = Join-Path $env:RUNNER_TEMP "grok-zh-arm64-verify-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT"
if (Test-Path -LiteralPath $verifyRoot) { throw 'ARM64 ZIP 验证目录已存在。' }
Expand-Archive -LiteralPath $zip -DestinationPath $verifyRoot
$roots = @(Get-ChildItem -LiteralPath $verifyRoot -Force)
if ($roots.Count -ne 1 -or !$roots[0].PSIsContainer -or $roots[0].Name -cne $env:PACKAGE_NAME) {
    throw 'ARM64 ZIP 顶层目录不精确。'
}
$extracted = $roots[0].FullName
$roundTripNames = @(Get-ChildItem -LiteralPath $extracted -Recurse -File | ForEach-Object {
    [IO.Path]::GetRelativePath($extracted, $_.FullName).Replace('\', '/')
})
[Array]::Sort($roundTripNames, [StringComparer]::Ordinal)
if (($roundTripNames -join "`n") -cne ($expectedNames -join "`n")) {
    throw 'ARM64 ZIP 解包后的文件集合不精确。'
}
foreach ($name in $manifestNames) {
    $original = (Get-FileHash -LiteralPath (Join-Path $package $name) -Algorithm SHA256).Hash
    $extractedHash = (Get-FileHash -LiteralPath (Join-Path $extracted $name) -Algorithm SHA256).Hash
    if ($original -cne $extractedHash) { throw "ARM64 ZIP 解包哈希不一致：$name" }
}
Assert-Arm64PE (Join-Path $extracted 'grok-zh.exe')
Assert-Arm64PE (Join-Path $extracted 'rg.exe')
$versionOutput = (& (Join-Path $extracted 'grok-zh.exe') --version | Select-Object -First 1)
if ($LASTEXITCODE -ne 0 -or $versionOutput -notmatch "^grok-zh $([regex]::Escape($env:GROK_VERSION)) \(") {
    throw "ARM64 ZIP 解包版本冒烟失败：$versionOutput"
}
$installPreview = Join-Path $env:RUNNER_TEMP "grok-zh-arm64-install-preview-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT"
& (Join-Path $extracted 'Install-GrokZh.ps1') -PackageDir $extracted -InstallDir $installPreview -NoPathUpdate -WhatIf
if (!$? -or (Test-Path -LiteralPath $installPreview)) {
    throw 'ARM64 ZIP 包内安装器预演失败。'
}
$hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$zip.sha256", "$hash  $env:ARCHIVE_NAME", $utf8)
