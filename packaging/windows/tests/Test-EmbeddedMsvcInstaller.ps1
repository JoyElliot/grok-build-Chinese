# Windows-only, offline fixtures. Run under both Windows PowerShell 5.1 and 7.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$windows = Split-Path -Parent $PSScriptRoot
$installer = Join-Path $windows 'Install-GrokZh.ps1'
$online = Join-Path $windows 'Install-GrokZhOnline.ps1'
$pathBefore = [Environment]::GetEnvironmentVariable('Path', 'User')
$processPathBefore = $env:Path
$homeBefore = $env:GROK_HOME
. $installer -LibraryOnly
. $online
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -AssemblyName System.Net.Http
$utf8 = [Text.UTF8Encoding]::new($false)
$work = Join-Path ([IO.Path]::GetTempPath()) ('grok-embedded-installer-test-' + [guid]::NewGuid().ToString('N'))
$script:Checks = 0
function Assert-Embedded([bool]$Condition, [string]$Message) {
    if (!$Condition) { throw "断言失败：$Message" }
    $script:Checks++
}
function Assert-EmbeddedThrows([scriptblock]$Action, [string]$Message) {
    $thrown = $false
    try { & $Action | Out-Null } catch { $thrown = $true }
    Assert-Embedded $thrown $Message
}
function Get-EmbeddedTestDigest([byte[]]$Bytes) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
}
function Update-EmbeddedTestManifest([string]$Root) {
    $lines = foreach ($name in $script:OnlinePackageFiles) {
        "$(Get-EmbeddedTestDigest ([IO.File]::ReadAllBytes((Join-Path $Root $name))))  $name"
    }
    [IO.File]::WriteAllText((Join-Path $Root 'SHA256SUMS.txt'), (($lines -join "`n") + "`n"), $utf8)
}
function New-EmbeddedTestInfo([string]$Version, [string]$Platform, $Descriptor = $null) {
    $protocol = [ordered]@{ schema = 1; version = $Version; platform = $Platform; mode = 'executable-only'; manifest = 'SHA256SUMS.txt'; executable = 'grok-zh.exe'; installer = 'Install-GrokZh.ps1' }
    if ($Descriptor) { $protocol.embedded_runtime = $Descriptor }
    return "Version: $Version`nTarget: $Platform`nGROK-UPDATE-PROTOCOL-BEGIN`n$($protocol | ConvertTo-Json -Depth 8 -Compress)`nGROK-UPDATE-PROTOCOL-END`n"
}
function New-EmbeddedTestPackage([string]$Version, [string]$Runtime) {
    $name = "grok-zh-$Version-windows-x86_64-$Runtime"
    $root = Join-Path $work $name
    $null = [IO.Directory]::CreateDirectory($root)
    foreach ($file in $script:OnlinePackageFiles) {
        $target = Join-Path $root $file
        $null = [IO.Directory]::CreateDirectory((Split-Path -Parent $target))
        $source = Join-Path $windows $file
        if (Test-Path -LiteralPath $source -PathType Leaf) { Copy-Item -LiteralPath $source -Destination $target }
        else { [IO.File]::WriteAllText($target, "fixture $file", $utf8) }
    }
    $sourcePath = Join-Path $work "$Version-$Runtime.cs"
    $source = 'using System; class Program { static int Main(string[] args) { if (args.Length == 1 && args[0] == "--version") { Console.Write("grok-zh ' + $Version + ' (fixture-' + $Runtime + ')"); return 0; } return 23; } }'
    [IO.File]::WriteAllText($sourcePath, $source, $utf8)
    & $compiler /nologo /target:exe /platform:x64 "/out:$(Join-Path $root 'grok-zh.exe')" $sourcePath | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'fixture 编译失败。' }
    [IO.File]::WriteAllText((Join-Path $root 'BUILD-INFO.txt'), (New-EmbeddedTestInfo $Version "x86_64-pc-windows-$Runtime"), $utf8)
    Update-EmbeddedTestManifest $root
    $archive = Join-Path $work "$name.zip"
    [IO.Compression.ZipFile]::CreateFromDirectory($root, $archive, [IO.Compression.CompressionLevel]::Optimal, $true)
    return [pscustomobject]@{ Root = $root; Archive = $archive; Version = $Version; Runtime = $Runtime }
}
function New-EmbeddedTestWrapper($Native) {
    $name = "grok-zh-$($Native.Version)-windows-x86_64-gnu"
    $root = Join-Path $work $name
    Copy-Item -LiteralPath $Native.Root -Destination $root -Recurse
    $bytes = [IO.File]::ReadAllBytes($Native.Archive)
    $hash = Get-EmbeddedTestDigest $bytes
    $descriptor = [ordered]@{ schema = 1; format = 'grok-msvc-bundle-v1'; version = $Native.Version; platform = 'x86_64-pc-windows-msvc'; archive_size = [long]$bytes.Length; archive_sha256 = $hash }
    # Deliberately non-runnable GNU PE prefix proves installers never execute or
    # deploy the wrapper when extracting a complete native package.
    $prefix = [byte[]]::new(512)
    $prefix[0] = 0x4d; $prefix[1] = 0x5a
    [BitConverter]::GetBytes([uint32]128).CopyTo($prefix, 0x3c)
    [BitConverter]::GetBytes([uint32]0x4550).CopyTo($prefix, 128)
    [BitConverter]::GetBytes([uint16]0x8664).CopyTo($prefix, 132)
    [BitConverter]::GetBytes([uint16]1).CopyTo($prefix, 134)
    [BitConverter]::GetBytes([uint16]240).CopyTo($prefix, 148)
    [BitConverter]::GetBytes([uint16]0x20b).CopyTo($prefix, 152)
    $stream = [IO.File]::Open((Join-Path $root 'grok-zh.exe'), 'Create', 'Write')
    $writer = [IO.BinaryWriter]::new($stream)
    try {
        $writer.Write($prefix); $writer.Write($bytes)
        $writer.Write([Text.Encoding]::ASCII.GetBytes('GROKMSVCBUNDLE01'))
        $writer.Write([uint64]$prefix.Length); $writer.Write([uint64]$bytes.Length)
        for ($i = 0; $i -lt 64; $i += 2) { $writer.Write([Convert]::ToByte($hash.Substring($i, 2), 16)) }
    } finally { $writer.Dispose(); $stream.Dispose() }
    [IO.File]::WriteAllText((Join-Path $root 'BUILD-INFO.txt'), (New-EmbeddedTestInfo $Native.Version 'x86_64-pc-windows-gnu' $descriptor), $utf8)
    Update-EmbeddedTestManifest $root
    $archive = Join-Path $work "$name.zip"
    [IO.Compression.ZipFile]::CreateFromDirectory($root, $archive, [IO.Compression.CompressionLevel]::Optimal, $true)
    return [pscustomobject]@{ Root = $root; Archive = $archive; Version = $Native.Version; Runtime = 'gnu'; Descriptor = $descriptor }
}
function New-EmbeddedTestRelease([string]$Version, [object[]]$Packages) {
    $tag = "release-v$Version"
    $assets = foreach ($package in $Packages) {
        $bytes = [IO.File]::ReadAllBytes($package.Archive); $name = Split-Path -Leaf $package.Archive
        [pscustomobject]@{ name = $name; state = 'uploaded'; size = [long]$bytes.Length; digest = 'sha256:' + (Get-EmbeddedTestDigest $bytes); browser_download_url = "https://github.com/JoyElliot/grok-build-Chinese/releases/download/$tag/$name" }
    }
    return [pscustomobject]@{ tag_name = $tag; immutable = $true; draft = $false; prerelease = $false; assets = @($assets); body = 'offline fixture' }
}
if (!('GrokEmbeddedInstallerTransport' -as [type])) {
    $code = @'
using System;
using System.Collections.Generic;
using System.Net;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
public sealed class GrokEmbeddedInstallerTransport : HttpMessageHandler {
    public readonly Dictionary<string, byte[]> Bodies = new Dictionary<string, byte[]>();
    public readonly List<string> Requests = new List<string>();
    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken token) {
        string url = request.RequestUri.AbsoluteUri; Requests.Add(url);
        var response = new HttpResponseMessage(Bodies.ContainsKey(url) ? HttpStatusCode.OK : HttpStatusCode.NotFound);
        response.Content = new ByteArrayContent(Bodies.ContainsKey(url) ? Bodies[url] : new byte[0]);
        return Task.FromResult(response);
    }
}
'@
    if ($PSVersionTable.PSVersion.Major -ge 6) { Add-Type -TypeDefinition $code }
    else { Add-Type -ReferencedAssemblies System.Net.Http -TypeDefinition $code }
}
function Set-EmbeddedTestTransport([object[]]$Releases, [object[]]$Packages) {
    $script:EmbeddedTransport = [GrokEmbeddedInstallerTransport]::new()
    $api = 'https://api.github.com/repos/JoyElliot/grok-build-Chinese/releases'
    $script:EmbeddedTransport.Bodies["${api}?per_page=100&page=1"] = $utf8.GetBytes((ConvertTo-Json -InputObject @($Releases) -Depth 10))
    foreach ($release in $Releases) {
        $script:EmbeddedTransport.Bodies["$api/tags/$($release.tag_name)"] = $utf8.GetBytes((ConvertTo-Json -InputObject $release -Depth 10))
    }
    foreach ($package in $Packages) {
        $name = Split-Path -Leaf $package.Archive
        $script:EmbeddedTransport.Bodies["https://github.com/JoyElliot/grok-build-Chinese/releases/download/release-v$($package.Version)/$name"] = [IO.File]::ReadAllBytes($package.Archive)
    }
}
$clientFactory = (Get-Item Function:New-OnlineHttpClient).ScriptBlock
try {
    $null = [IO.Directory]::CreateDirectory($work)
    $compiler = Join-Path $env:SystemRoot 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    $shared = Join-Path $work '共享数据'; $null = [IO.Directory]::CreateDirectory($shared)
    [IO.File]::WriteAllText((Join-Path $shared 'auth.json'), 'keep-auth', $utf8)
    [IO.File]::WriteAllText((Join-Path $shared 'config.toml'), 'keep-config', $utf8)
    $env:GROK_HOME = $shared
    $native1 = New-EmbeddedTestPackage '1.0.40' 'msvc'
    $wrapper1 = New-EmbeddedTestWrapper $native1
    $native2 = New-EmbeddedTestPackage '1.0.41' 'msvc'
    $wrapper2 = New-EmbeddedTestWrapper $native2
    $native3 = New-EmbeddedTestPackage '1.0.42' 'msvc'
    $gnuOld = New-EmbeddedTestPackage '1.0.39' 'gnu'
    $preview = New-EmbeddedTestPackage '1.0.43-zh.ci.12345' 'msvc'
    $previewTarget = Join-Path $work 'preview-install'
    & $installer -PackageDir $preview.Root -InstallDir $previewTarget -NoPathUpdate -Confirm:$false
    Assert-Embedded ((Get-GrokExecutableVersion (Join-Path $previewTarget 'grok-zh.exe')) -ceq $preview.Version) '普通 schema1 预览版本继续可安装'
    $olderPreview = New-EmbeddedTestPackage '1.0.43-zh.ci.12344' 'msvc'
    Assert-EmbeddedThrows { & $installer -PackageDir $olderPreview.Root -InstallDir $previewTarget -NoPathUpdate -Confirm:$false } '同一核心版本的旧预览不能隐式降级'
    & $installer -PackageDir $olderPreview.Root -InstallDir $previewTarget -NoPathUpdate -AllowDowngrade -Confirm:$false
    Assert-Embedded ((Get-GrokExecutableVersion (Join-Path $previewTarget 'grok-zh.exe')) -ceq $olderPreview.Version) '明确降级可安装精确旧预览'
    foreach ($pair in @(@('1.0.1-zh.ci.9', '1.0.1-zh.ci.10'), @('1.0.1-rc.1', '1.0.1-rc.alpha'), @('1.0.1-rc', '1.0.1-rc.1'), @('1.0.1-rc.999999999999999999999', '1.0.1-rc.1000000000000000000000'), @('1.0.1-rc.1', '1.0.1'))) {
        Assert-Embedded ((Compare-GrokInstallerVersion $pair[0] $pair[1]) -lt 0) 'SemVer 预发布按标识符排序'
    }
    Assert-Embedded ((Compare-GrokInstallerVersion '1.0.1+first' '1.0.1+second') -eq 0) '构建元数据不影响版本优先级'

    $resource = Join-Path $native3.Root 'future-resource.dat'
    [IO.File]::WriteAllText($resource, 'future-native-resource', $utf8)
    [IO.File]::AppendAllText((Join-Path $native3.Root 'SHA256SUMS.txt'), "$(Get-EmbeddedTestDigest ([IO.File]::ReadAllBytes($resource)))  future-resource.dat`n", $utf8)
    [IO.File]::Delete($native3.Archive)
    [IO.Compression.ZipFile]::CreateFromDirectory($native3.Root, $native3.Archive, [IO.Compression.CompressionLevel]::Optimal, $true)

    # Simulate an authentic historical installer with no new downgrade switch.
    $oldInstallerPath = Join-Path $gnuOld.Root 'Install-GrokZh.ps1'
    $oldInstaller = [IO.File]::ReadAllText($oldInstallerPath).Replace("    [switch]`$AllowDowngrade,`n", '').Replace('$AllowDowngrade', '$true')
    [IO.File]::WriteAllText($oldInstallerPath, $oldInstaller, [Text.UTF8Encoding]::new($true))
    Update-EmbeddedTestManifest $gnuOld.Root
    [IO.File]::Delete($gnuOld.Archive)
    [IO.Compression.ZipFile]::CreateFromDirectory($gnuOld.Root, $gnuOld.Archive, [IO.Compression.CompressionLevel]::Optimal, $true)

    $extract = & $installer -PackageDir $wrapper1.Root -ExtractOnly -ExtractionDirectory (Join-Path $work 'extract-only')
    Assert-Embedded ((Get-FileHash (Join-Path $extract 'grok-zh.exe')).Hash -ceq (Get-FileHash (Join-Path $native1.Root 'grok-zh.exe')).Hash) '提取的是原生完整包'
    Assert-Embedded (@(Get-ChildItem -LiteralPath $extract -Recurse -File).Count -eq 16) '原生完整包包含完整清单和 15 个文件'
    $manual = Join-Path $work '手动安装'
    & $installer -PackageDir $wrapper1.Root -InstallDir $manual -NoPathUpdate -OverrideOfficialCommands -Confirm:$false
    Assert-Embedded ((Get-GrokExecutableVersion (Join-Path $manual 'grok-zh.exe')) -ceq '1.0.40') '手动单包直接安装 MSVC'
    Assert-Embedded (!(Test-Path -LiteralPath (Join-Path $manual '.grok-zh-msvc'))) '全新安装没有迁移 sidecar'
    & $installer -PackageDir $wrapper2.Root -InstallDir $manual -NoPathUpdate -Confirm:$false
    Assert-Embedded ((Get-GrokExecutableVersion (Join-Path $manual 'grok-zh.exe')) -ceq '1.0.41') '连续单包版本直接升级'
    & $installer -PackageDir $native3.Root -InstallDir $manual -NoPathUpdate -Confirm:$false
    Assert-Embedded ((Get-GrokExecutableVersion (Join-Path $manual 'grok-zh.exe')) -ceq '1.0.42') '迁移结束后原生单包正常升级'
    Assert-Embedded ([IO.File]::ReadAllText((Join-Path $manual 'future-resource.dat')) -ceq 'future-native-resource') '原生 schema1 安装全部经清单验证的未来资源'
    Assert-EmbeddedThrows { & $installer -PackageDir $gnuOld.Root -InstallDir $manual -NoPathUpdate -Confirm:$false } '当前完整安装器默认拒绝降级'
    & $installer -PackageDir $gnuOld.Root -InstallDir $manual -NoPathUpdate -AllowDowngrade -Confirm:$false
    Assert-Embedded ((Get-GrokExecutableVersion (Join-Path $manual 'grok-zh.exe')) -ceq '1.0.39') '明确降级安装真实旧 GNU'
    Assert-Embedded (Test-Path -LiteralPath (Join-Path $manual 'grok.cmd')) '降级保留兼容命令选择'
    # A stale migrated child must never survive an installer round trip.
    $sidecar = Join-Path $manual '.grok-zh-msvc'; $null = [IO.Directory]::CreateDirectory($sidecar)
    [IO.File]::WriteAllText((Join-Path $sidecar '.grok-zh-msvc-ready.json'), '{"version":"9.9.9"}', $utf8)
    & $installer -PackageDir $wrapper2.Root -InstallDir $manual -NoPathUpdate -Confirm:$false
    Assert-Embedded (!(Test-Path -LiteralPath (Join-Path $manual '.grok-zh-msvc'))) '返回 MSVC 不继承旧 sidecar 或 ready 标记'

    function New-OnlineHttpClient { return [Net.Http.HttpClient]::new($script:EmbeddedTransport) }
    $release1 = New-EmbeddedTestRelease '1.0.40' @($native1)
    $release2 = New-EmbeddedTestRelease '1.0.41' @($wrapper2)
    $release3 = New-EmbeddedTestRelease '1.0.42' @($native3)
    $oldRelease = New-EmbeddedTestRelease '1.0.39' @($gnuOld)
    $onlineTarget = Join-Path $work '在线安装'
    Set-EmbeddedTestTransport @($release1, $release2) @($native1, $wrapper2)
    Invoke-GrokZhOnline -Mode Install -InstallDir $onlineTarget -NoPathUpdate -NonInteractive
    Assert-Embedded ((Get-GrokExecutableVersion (Join-Path $onlineTarget 'grok-zh.exe')) -ceq '1.0.41') '新版本 wrapper 优先于旧版本原生包'
    Assert-Embedded (@($script:EmbeddedTransport.Requests | Where-Object { $_.EndsWith('.zip') }).Count -eq 1) '在线单包只下载一个 ZIP'
    Assert-Embedded (!(Test-Path -LiteralPath (Join-Path $onlineTarget '.grok-zh-msvc'))) '在线安装直接部署原生程序'
    $sameRelease = New-EmbeddedTestRelease '1.0.41' @($wrapper2, $native2)
    $priorityTarget = Join-Path $work '同版本优先原生'
    Set-EmbeddedTestTransport @($sameRelease) @($wrapper2, $native2)
    Invoke-GrokZhOnline -Mode Install -InstallDir $priorityTarget -NoPathUpdate -NonInteractive
    Assert-Embedded (@($script:EmbeddedTransport.Requests | Where-Object { $_.EndsWith('windows-x86_64-msvc.zip') }).Count -eq 1) '同版本优先独立 MSVC 包'
    Assert-Embedded (@($script:EmbeddedTransport.Requests | Where-Object { $_.EndsWith('windows-x86_64-gnu.zip') }).Count -eq 0) '已有同版本原生包不下载 wrapper'
    Set-EmbeddedTestTransport @($oldRelease) @($gnuOld)
    Assert-EmbeddedThrows { Invoke-GrokZhOnline -Mode Install -InstallDir $onlineTarget -NoPathUpdate -NonInteractive -Version '1.0.39' } '显式版本仍需明确降级许可'
    Set-EmbeddedTestTransport @($oldRelease) @($gnuOld)
    Assert-EmbeddedThrows { Invoke-GrokZhOnline -Mode Install -InstallDir $onlineTarget -NoPathUpdate -NonInteractive -AllowDowngrade } '不允许自动选版本降级'
    Set-EmbeddedTestTransport @($oldRelease) @($gnuOld)
    Invoke-GrokZhOnline -Mode Install -InstallDir $onlineTarget -NoPathUpdate -NonInteractive -Version '1.0.39' -AllowDowngrade
    Assert-Embedded ((Get-GrokExecutableVersion (Join-Path $onlineTarget 'grok-zh.exe')) -ceq '1.0.39') '历史安装器无新参数仍可精确降级'
    Set-EmbeddedTestTransport @($release3) @($native3)
    Invoke-GrokZhOnline -Mode Install -InstallDir $onlineTarget -NoPathUpdate -NonInteractive -Version '1.0.42'
    Assert-Embedded ((Get-GrokExecutableVersion (Join-Path $onlineTarget 'grok-zh.exe')) -ceq '1.0.42') '历史 GNU 可返回指定 MSVC'
    Set-EmbeddedTestTransport @($oldRelease) @($gnuOld)
    Assert-EmbeddedThrows { Invoke-GrokZhOnline -Mode Install -InstallDir (Join-Path $work 'reject-gnu') -NoPathUpdate -NonInteractive -Version '1.0.39' -WindowsX64Runtime Msvc } '显式 MSVC 不安装历史 GNU'

    & {
        $before = (Get-FileHash (Join-Path $manual 'grok-zh.exe')).Hash
        function Move-Item {
            param([string]$LiteralPath, [string]$Destination)
            if ($LiteralPath.Contains('.stage.') -and $Destination -ceq $manual) { throw '模拟激活失败' }
            Microsoft.PowerShell.Management\Move-Item @PSBoundParameters
        }
        Assert-EmbeddedThrows { & $installer -PackageDir $native3.Root -InstallDir $manual -NoPathUpdate -Confirm:$false } '激活失败触发旧安装恢复'
        Assert-Embedded ((Get-FileHash (Join-Path $manual 'grok-zh.exe')).Hash -ceq $before) '激活失败后原程序完整保留'
    }

    $savedExe = (Get-FileHash (Join-Path $manual 'grok-zh.exe')).Hash
    $launcherPath = Join-Path $wrapper2.Root 'grok-zh.exe'
    $goodBytes = [IO.File]::ReadAllBytes($launcherPath)
    foreach ($case in @('magic', 'offset', 'size', 'digest', 'content', 'trailing')) {
        $bad = [byte[]]$goodBytes.Clone()
        switch ($case) {
            'magic' { $bad[$bad.Length - 64] = 0 }
            'offset' { for ($i = $bad.Length - 48; $i -lt $bad.Length - 40; $i++) { $bad[$i] = 255 } }
            'size' { $bad[$bad.Length - 40] = $bad[$bad.Length - 40] -bxor 1 }
            'digest' { $bad[$bad.Length - 1] = $bad[$bad.Length - 1] -bxor 1 }
            'content' { $bad[520] = $bad[520] -bxor 1 }
            'trailing' { $bad = [byte[]]($bad + [byte]0) }
        }
        [IO.File]::WriteAllBytes($launcherPath, $bad); Update-EmbeddedTestManifest $wrapper2.Root
        Assert-EmbeddedThrows { & $installer -PackageDir $wrapper2.Root -InstallDir $manual -NoPathUpdate -Confirm:$false } "损坏内嵌包拒绝安装：$case"
        Assert-Embedded ((Get-FileHash (Join-Path $manual 'grok-zh.exe')).Hash -ceq $savedExe) "损坏内嵌包保留原安装：$case"
    }
    [IO.File]::WriteAllBytes($launcherPath, $goodBytes); Update-EmbeddedTestManifest $wrapper2.Root
    $lowSpace = (Get-Item Function:Assert-GrokFreeSpace).ScriptBlock
    try {
        function Assert-GrokFreeSpace { param($Path, $RequiredBytes) throw '模拟磁盘空间不足' }
        Assert-EmbeddedThrows { Expand-GrokEmbeddedArchive -Launcher $launcherPath -Destination (Join-Path $work 'low-space') -Version $wrapper2.Version -ExpectedSize $wrapper2.Descriptor.archive_size -ExpectedSha256 $wrapper2.Descriptor.archive_sha256 } '空间不足在写入前停止'
        Assert-Embedded (!(Test-Path -LiteralPath (Join-Path $work 'low-space'))) '空间不足没有半成品'
    } finally { Set-Item Function:Assert-GrokFreeSpace -Value $lowSpace }
    $duplicateInfo = (New-EmbeddedTestInfo '1.0.41' 'x86_64-pc-windows-gnu' $wrapper2.Descriptor).Replace('"archive_size":', '"archive_size":1,"archive_size":')
    Assert-EmbeddedThrows { Read-GrokBundleProtocol $duplicateInfo '1.0.41' 'x86_64-pc-windows-gnu' } '拒绝内嵌描述重复 JSON 字段'
    Assert-Embedded ([IO.File]::ReadAllText((Join-Path $shared 'auth.json')) -ceq 'keep-auth') '账号数据保持原样'
    Assert-Embedded ([IO.File]::ReadAllText((Join-Path $shared 'config.toml')) -ceq 'keep-config') '配置保持原样'
    Assert-Embedded ($env:GROK_HOME -ceq $shared) '共享 GROK_HOME 环境不变'
    Assert-Embedded ([Environment]::GetEnvironmentVariable('Path', 'User') -ceq $pathBefore) '用户 PATH 保持原样'
    Assert-Embedded ($env:Path -ceq $processPathBefore) '进程 PATH 保持原样'
    Write-Host "Embedded MSVC installer checks passed: $script:Checks"
} finally {
    Set-Item Function:New-OnlineHttpClient -Value $clientFactory
    $env:GROK_HOME = $homeBefore
    if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force }
}
