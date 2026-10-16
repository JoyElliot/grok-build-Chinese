<#
.SYNOPSIS
下载最新正式 Release，以中文菜单安装 Grok Build 中文社区版。
.DESCRIPTION
仅从 JoyElliot/grok-build-Chinese 下载并验证当前 Windows 架构的完整包，再调用包内安装器。
支持 Windows PowerShell 5.1 和 PowerShell 7；不需要管理员权限。
.PARAMETER Mode
Menu 显示中文菜单；Install 共存安装；Commands 打开包内命令设置菜单；Portable 创建便携目录。
.PARAMETER InstallDir
自定义程序目录。省略时使用当前用户 LocalAppData 下的 Programs\grok-zh\bin。
.PARAMETER PortableDir
便携版根目录；顶层只有启动.cmd、使用说明.md 和 app。
.PARAMETER GrokHome
共享数据目录，仅用于安装边界检查，不持久化环境变量。
.PARAMETER NoPathUpdate
不修改用户 Path。便携版始终不修改 Path。
.PARAMETER Repair
允许重新安装相同版本；任何模式都不自动降级。
.PARAMETER NonInteractive
使用参数完成共存或便携安装，不显示菜单。相同版本需另加 -Repair。
.PARAMETER VerifyOnly
只下载、校验完整包并检查候选程序版本，不安装。
.PARAMETER WindowsX64Runtime
Windows x64 工具链。默认保留 GNU；MSVC 用于显式安装和迁移入口。
.PARAMETER Version
安装指定的正式版本。迁移入口用它固定到启动器对应版本，避免下载途中跳到另一版本。
.EXAMPLE
& .\Install-GrokZhOnline.ps1
.EXAMPLE
& .\Install-GrokZhOnline.ps1 -Mode Portable -PortableDir 'D:\Apps\Grok 中文版'
.EXAMPLE
& .\Install-GrokZhOnline.ps1 -VerifyOnly -NonInteractive
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [ValidateSet('Menu', 'Install', 'Commands', 'Portable')][string]$Mode = 'Menu',
    [string]$InstallDir,
    [string]$PortableDir,
    [string]$GrokHome,
    [switch]$NoPathUpdate,
    [switch]$Repair,
    [switch]$NonInteractive,
    [switch]$VerifyOnly,
    [ValidateSet('Gnu', 'Msvc')][string]$WindowsX64Runtime = 'Gnu',
    [string]$Version
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$script:OnlineRepo = 'JoyElliot/grok-build-Chinese'
$script:OnlinePlatformSuffix = 'windows-x86_64-gnu'
$script:OnlineTargetTriple = 'x86_64-pc-windows-gnu'
$script:OnlinePackageFiles = @(
    'grok-zh.exe', 'agent-zh.cmd', 'rg.exe', '一键安装.cmd',
    '[可选]替换原始启动方式.cmd', 'Install-GrokZh.ps1', 'INSTALL-WINDOWS.md',
    'LICENSE-grok-build.txt', 'BUILD-INFO.txt', 'licenses/ripgrep/COPYING',
    'licenses/ripgrep/LICENSE-MIT', 'licenses/ripgrep/UNLICENSE',
    'licenses/project/THIRD-PARTY-NOTICES', 'licenses/project/THIRD_PARTY_NOTICES.md',
    'licenses/project/NOTICE'
)
$script:OnlineUtf8 = [Text.UTF8Encoding]::new($false, $true)

function Get-OnlineProperty {
    param($Object, [string]$Name)
    if ($null -eq $Object -or $null -eq $Object.PSObject.Properties[$Name]) {
        throw "发布信息缺少字段：$Name"
    }
    return $Object.$Name
}

function ConvertFrom-OnlineUtf8 {
    param([byte[]]$Bytes)
    $text = $script:OnlineUtf8.GetString($Bytes)
    if ($text.Length -gt 0 -and $text[0] -eq [char]0xfeff) { $text = $text.Substring(1) }
    return $text
}

function ConvertTo-OnlineVersion {
    param([string]$Text, [switch]$StableOnly)
    if ($Text -cnotmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$') {
        throw "无法识别版本号：$Text"
    }
    $pre = if ($matches.ContainsKey(4)) { $matches[4] } else { '' }
    $build = if ($matches.ContainsKey(5)) { $matches[5] } else { '' }
    if ($StableOnly -and ($pre -or $build)) { throw "不是正式版本：$Text" }
    return [pscustomobject]@{
        Text = $Text; Major = [uint64]$matches[1]; Minor = [uint64]$matches[2]
        Patch = [uint64]$matches[3]; Pre = $pre
    }
}

function Compare-OnlineVersion {
    param($Left, $Right)
    foreach ($part in @('Major', 'Minor', 'Patch')) {
        if ($Left.$part -gt $Right.$part) { return 1 }
        if ($Left.$part -lt $Right.$part) { return -1 }
    }
    if ($Left.Pre -and !$Right.Pre) { return -1 }
    if (!$Left.Pre -and $Right.Pre) { return 1 }
    return 0
}

function Get-OnlineNativeMachine {
    if (!('GrokOnlineNativeMachine' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class GrokOnlineNativeMachine {
    [DllImport("kernel32.dll")]
    private static extern IntPtr GetCurrentProcess();
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool IsWow64Process2(IntPtr process, out ushort processMachine, out ushort nativeMachine);
    public static ushort Get() {
        ushort processMachine, nativeMachine;
        if (!IsWow64Process2(GetCurrentProcess(), out processMachine, out nativeMachine))
            throw new Win32Exception(Marshal.GetLastWin32Error());
        return nativeMachine;
    }
}
'@
    }
    try { return [GrokOnlineNativeMachine]::Get() }
    catch [System.Management.Automation.MethodInvocationException] {
        if ($_.Exception.InnerException -isnot [EntryPointNotFoundException]) { throw }
        # IsWow64Process2 is unavailable on Windows older than 10 1709.
        if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') { return 0xAA64 }
        if ($env:PROCESSOR_ARCHITECTURE -eq 'AMD64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'AMD64') { return 0x8664 }
        throw '无法确定 Windows 系统架构。'
    }
}

function Get-OnlineReleaseContract {
    param($Release)
    foreach ($flag in @('draft', 'prerelease', 'immutable')) {
        if ((Get-OnlineProperty $Release $flag) -isnot [bool]) { throw "发布标志无效：$flag" }
    }
    if ($Release.draft -or $Release.prerelease -or !$Release.immutable) { throw '仅接受不可变的正式 Release。' }
    $tag = [string](Get-OnlineProperty $Release 'tag_name')
    if ($tag -cmatch '^release-v(.+)$') { $modern = $true; $versionText = $matches[1] }
    elseif ($tag -cmatch '^v(.+)$') { $modern = $false; $versionText = $matches[1] }
    else { throw "不支持的发布标签：$tag" }
    $version = ConvertTo-OnlineVersion $versionText -StableOnly
    $legacy = (Compare-OnlineVersion $version (ConvertTo-OnlineVersion '1.0.8')) -le 0
    if ($modern -eq $legacy) { throw "发布标签与版本不匹配：$tag" }
    $name = "grok-zh-$versionText-$script:OnlinePlatformSuffix.zip"
    $assets = @(Get-OnlineProperty $Release 'assets')
    $assets = @($assets | Where-Object { (Get-OnlineProperty $_ 'name') -ceq $name })
    if ($assets.Count -ne 1) { throw 'Release 缺少当前平台安装包，或存在同名重复附件。' }
    $verified = [Collections.Generic.Dictionary[string, object]]::new([StringComparer]::Ordinal)
    foreach ($asset in $assets) {
        $assetName = [string](Get-OnlineProperty $asset 'name')
        if ($verified.ContainsKey($assetName) -or
            (Get-OnlineProperty $asset 'state') -cne 'uploaded') { throw "无效的发布附件：$assetName" }
        $sizeValue = Get-OnlineProperty $asset 'size'
        if ($sizeValue -isnot [int] -and $sizeValue -isnot [long]) { throw "附件大小无效：$assetName" }
        $size = [long]$sizeValue
        $maximum = 536870912L
        if ($size -le 0 -or $size -gt $maximum) { throw "附件大小超过限制：$assetName" }
        $url = [string](Get-OnlineProperty $asset 'browser_download_url')
        if ($url -cne "https://github.com/$script:OnlineRepo/releases/download/$tag/$assetName") {
            throw "附件下载地址不属于预期发布：$assetName"
        }
        $digest = [string](Get-OnlineProperty $asset 'digest')
        if ($digest -cnotmatch '^sha256:([0-9a-fA-F]{64})$') { throw "附件缺少有效 SHA-256：$assetName" }
        $verified.Add($assetName, [pscustomobject]@{
            Name = $assetName; Url = $url; Size = $size; Sha256 = $matches[1].ToLowerInvariant()
        })
    }
    return [pscustomobject]@{
        Version = $version; Tag = $tag; Legacy = $legacy; Archive = $verified[$name]
        PackageRoot = $name.Substring(0, $name.Length - 4)
        ReleaseNotes = [string](Get-OnlineProperty $Release 'body')
    }
}

function Assert-OnlineDownloadUri {
    param([uri]$Uri)
    $hosts = @('api.github.com', 'github.com', 'release-assets.githubusercontent.com',
        'github-releases.githubusercontent.com', 'objects.githubusercontent.com')
    if (!$Uri.IsAbsoluteUri -or $Uri.Scheme -cne 'https' -or $Uri.Port -ne 443 -or
        $Uri.UserInfo -or $Uri.Fragment -or $hosts -cnotcontains $Uri.DnsSafeHost) {
        throw '下载地址或重定向目标不在允许的 GitHub HTTPS 来源内。'
    }
}

function New-OnlineHttpClient {
    Add-Type -AssemblyName System.Net.Http
    # .NET Framework uses the system certificate store and proxy. No certificate bypass.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $handler = [Net.Http.HttpClientHandler]::new()
    $handler.AllowAutoRedirect = $false
    $proxyText = @($env:HTTPS_PROXY, $env:HTTP_PROXY) | Where-Object { ![string]::IsNullOrWhiteSpace($_) } | Select-Object -First 1
    if ($proxyText) {
        $proxyUri = [uri]$proxyText
        if (!$proxyUri.IsAbsoluteUri -or $proxyUri.Scheme -notin @('http', 'https')) {
            $handler.Dispose()
            throw 'HTTPS_PROXY/HTTP_PROXY 需要使用 HTTP 代理地址。'
        }
        $handler.Proxy = [Net.WebProxy]::new($proxyUri)
    }
    $client = [Net.Http.HttpClient]::new($handler)
    $client.Timeout = [Threading.Timeout]::InfiniteTimeSpan
    $client.DefaultRequestHeaders.UserAgent.ParseAdd('grok-build-zh-online-installer')
    $client.DefaultRequestHeaders.Add('X-GitHub-Api-Version', '2026-03-10')
    return $client
}

function Wait-OnlineTask {
    param($Task)
    while (!$Task.IsCompleted) { $null = $Task.Wait(100) }
    return $Task.GetAwaiter().GetResult()
}

function Receive-OnlineFile {
    param($Client, [uri]$Uri, [string]$Destination, [long]$MaximumBytes,
        [long]$ExpectedBytes = -1, [string]$ExpectedSha256, [string]$Label = '下载')
    Assert-OnlineDownloadUri $Uri
    if (Test-Path -LiteralPath $Destination) { throw "下载目标已存在：$Destination" }
    $watch = [Diagnostics.Stopwatch]::StartNew()
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        $response = $null; $inputStream = $null; $outputStream = $null; $created = $false
        $cancel = [Threading.CancellationTokenSource]::new()
        try {
            $next = $Uri
            for ($redirects = 0; ; $redirects++) {
                Assert-OnlineDownloadUri $next
                $remaining = 1200000 - $watch.ElapsedMilliseconds
                if ($remaining -le 0) { throw [TimeoutException]::new('下载总耗时超过 20 分钟。') }
                $cancel.CancelAfter([int][Math]::Min(30000, $remaining))
                $response = Wait-OnlineTask ($Client.GetAsync($next, [Net.Http.HttpCompletionOption]::ResponseHeadersRead, $cancel.Token))
                $status = [int]$response.StatusCode
                if ($status -in @(301, 302, 303, 307, 308)) {
                    if ($redirects -ge 5 -or $null -eq $response.Headers.Location) { throw '下载重定向次数过多或缺少目标。' }
                    $next = [uri]::new($next, $response.Headers.Location)
                    $response.Dispose(); $response = $null
                    continue
                }
                if ($status -ne 200) {
                    $error = [Exception]::new("GitHub 返回 HTTP $status。")
                    $error.Data['Retryable'] = $status -in @(408, 429, 500, 502, 503, 504)
                    throw $error
                }
                break
            }
            $length = $response.Content.Headers.ContentLength
            if ($null -ne $length -and ($length -gt $MaximumBytes -or
                ($ExpectedBytes -ge 0 -and $length -ne $ExpectedBytes))) { throw '下载声明大小与发布信息不一致。' }
            $inputStream = Wait-OnlineTask ($response.Content.ReadAsStreamAsync())
            $outputStream = [IO.File]::Open($Destination, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
            $created = $true
            $buffer = [byte[]]::new(65536); $received = 0L
            while ($true) {
                $remaining = 1200000 - $watch.ElapsedMilliseconds
                if ($remaining -le 0) { throw [TimeoutException]::new('下载总耗时超过 20 分钟。') }
                $cancel.CancelAfter([int][Math]::Min(30000, $remaining))
                try { $count = Wait-OnlineTask ($inputStream.ReadAsync($buffer, 0, $buffer.Length, $cancel.Token)) }
                catch {
                    $readFailure = $_.Exception.GetBaseException()
                    if ($readFailure -is [IO.IOException]) { $readFailure.Data['Retryable'] = $true; throw $readFailure }
                    throw
                }
                if ($watch.ElapsedMilliseconds -ge 1200000) { throw [TimeoutException]::new('下载总耗时超过 20 分钟。') }
                if ($count -eq 0) { break }
                $received += $count
                if ($received -gt $MaximumBytes -or ($ExpectedBytes -ge 0 -and $received -gt $ExpectedBytes)) { throw '下载内容超过允许大小。' }
                $outputStream.Write($buffer, 0, $count)
                if ($ExpectedBytes -gt 0) {
                    $percent = [int][Math]::Min(100, 100.0 * $received / $ExpectedBytes)
                    $speed = $received / [Math]::Max(0.1, $watch.Elapsed.TotalSeconds) / 1MB
                    Write-Progress -Id 10 -Activity $Label -PercentComplete $percent -Status (
                        '{0:N1} / {1:N1} MiB，{2:N1} MiB/s' -f ($received / 1MB), ($ExpectedBytes / 1MB), $speed)
                }
            }
            $outputStream.Dispose(); $outputStream = $null
            if ($ExpectedBytes -ge 0 -and $received -ne $ExpectedBytes) {
                $incomplete = [IO.IOException]::new('下载尚未完成，实际大小与发布信息不一致。')
                $incomplete.Data['Retryable'] = $true
                throw $incomplete
            }
            if ($ExpectedSha256 -and (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash -ine $ExpectedSha256) {
                throw '下载内容的 SHA-256 与 GitHub 发布信息不一致。'
            }
            return
        } catch {
            $failure = $_.Exception.GetBaseException()
            if ($outputStream) { $outputStream.Dispose(); $outputStream = $null }
            if ($created -and (Test-Path -LiteralPath $Destination)) { [IO.File]::Delete($Destination) }
            $retryable = $failure.Data['Retryable'] -eq $true -or
                $failure -is [Net.Http.HttpRequestException] -or $failure -is [OperationCanceledException] -or
                $failure -is [TimeoutException]
            if (!$retryable -or $attempt -eq 3 -or $watch.ElapsedMilliseconds -ge 1200000) { throw }
            Write-Host "网络暂时不可用，正在重试（$attempt/2）..." -ForegroundColor Yellow
            Start-Sleep -Seconds $attempt
        } finally {
            if ($inputStream) { $inputStream.Dispose() }
            if ($outputStream) { $outputStream.Dispose() }
            if ($response) { $response.Dispose() }
            $cancel.Dispose()
            Write-Progress -Id 10 -Activity $Label -Completed
        }
    }
}

function Get-LatestOnlineRelease {
    param($Client, [string]$WorkDirectory)
    $best = $null
    for ($page = 1; $page -le 100; $page++) {
        $metadataPath = Join-Path $WorkDirectory "releases-$page.json"
        Receive-OnlineFile -Client $Client -Uri "https://api.github.com/repos/$script:OnlineRepo/releases?per_page=100&page=$page" `
            -Destination $metadataPath -MaximumBytes 8388608 -Label '正在检查最新正式版本'
        $raw = ConvertFrom-OnlineUtf8 ([IO.File]::ReadAllBytes($metadataPath))
        if (!$raw.TrimStart().StartsWith('[')) { throw 'GitHub 未返回有效的 Release 列表。' }
        # Windows PowerShell 5.1 emits JSON arrays as one pipeline object.
        # Assign first, then enumerate, so both 5.1 and 7 see individual releases.
        $parsed = ConvertFrom-Json -InputObject $raw
        $releases = @()
        if ($null -ne $parsed) { $releases = @($parsed) }
        foreach ($release in $releases) {
            try { $candidate = Get-OnlineReleaseContract $release }
            catch { Write-Verbose "忽略无法校验的 Release：$($_.Exception.Message)"; continue }
            if ($null -eq $best -or (Compare-OnlineVersion $candidate.Version $best.Version) -gt 0) { $best = $candidate }
        }
        if ($releases.Count -lt 100) {
            if ($null -eq $best) { throw '未找到可验证的 Windows 正式 Release。' }
            return $best
        }
    }
    throw 'Release 列表超过查询上限，未执行安装。'
}

function Get-ExactOnlineRelease {
    param($Client, [string]$Version, [string]$WorkDirectory)
    $parsed = ConvertTo-OnlineVersion $Version -StableOnly
    $tag = if ((Compare-OnlineVersion $parsed (ConvertTo-OnlineVersion '1.0.8')) -le 0) { "v$Version" } else { "release-v$Version" }
    $metadataPath = Join-Path $WorkDirectory 'exact-release.json'
    Receive-OnlineFile -Client $Client -Uri "https://api.github.com/repos/$script:OnlineRepo/releases/tags/$tag" `
        -Destination $metadataPath -MaximumBytes 8388608 -Label '正在核对指定正式版本'
    $release = ConvertFrom-Json -InputObject (ConvertFrom-OnlineUtf8 ([IO.File]::ReadAllBytes($metadataPath)))
    $contract = Get-OnlineReleaseContract $release
    if ($contract.Tag -cne $tag -or $contract.Version.Text -cne $Version) { throw '指定 Release 的版本或标签不一致。' }
    return $contract
}

function Assert-OnlinePathChain {
    param([string]$Path)
    $cursor = $Path
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw "路径包含链接或重解析点：$cursor" }
        }
        $parent = Split-Path -Parent $cursor
        if (!$parent -or $parent -ieq $cursor) { break }
        $cursor = $parent
    }
}

function Resolve-OnlinePath {
    param([string]$Path)
    $expanded = [Environment]::ExpandEnvironmentVariables($Path.Trim().Trim('"'))
    if (!$expanded -or $expanded -match '(?i)\$env:|%[^%]+%' -or ![IO.Path]::IsPathRooted($expanded)) {
        throw '请提供完整的绝对路径，并先展开环境变量。'
    }
    $full = [IO.Path]::GetFullPath($expanded).TrimEnd('\', '/')
    if ($full -ieq [IO.Path]::GetPathRoot($expanded).TrimEnd('\', '/')) { throw '不能把磁盘根目录作为安装目录。' }
    Assert-OnlinePathChain $full
    return $full
}

function Test-OnlinePathOverlap {
    param([string]$Left, [string]$Right)
    return $Left -ieq $Right -or $Left.StartsWith("$Right\", [StringComparison]::OrdinalIgnoreCase) -or
        $Right.StartsWith("$Left\", [StringComparison]::OrdinalIgnoreCase)
}

function Remove-OnlineOwnedTree {
    param([string]$Path, [string]$Parent, [string]$Prefix)
    $full = [IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    $base = [IO.Path]::GetFullPath($Parent).TrimEnd('\', '/')
    if ((Split-Path -Parent $full).TrimEnd('\', '/') -ine $base -or !(Split-Path -Leaf $full).StartsWith($Prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw '临时目录清理边界检查失败。'
    }
    Assert-OnlinePathChain $full
    if (Test-Path -LiteralPath $full) { Remove-Item -LiteralPath $full -Recurse -Force -WhatIf:$false -Confirm:$false }
}

function Assert-OnlinePackageRelativePath {
    param([string]$Path)
    if (!$Path -or $Path.Trim() -cne $Path -or $Path -match '[\\:<>"|?*\x00-\x1f\x7f]') { throw "包内路径无效：$Path" }
    foreach ($part in $Path.Split('/')) {
        if (!$part -or $part -in @('.', '..') -or $part.EndsWith('.') -or $part.EndsWith(' ') -or
            $part -match '^(?i:CON|PRN|AUX|NUL|COM[1-9\u00b9\u00b2\u00b3]|LPT[1-9\u00b9\u00b2\u00b3])(?:\.|$)') { throw "包内路径无效：$Path" }
    }
}

function Read-OnlineZipText {
    param($Entry)
    if ($Entry.Length -gt 65536) { throw '包内元数据超过大小限制。' }
    $stream = $Entry.Open()
    $memory = [IO.MemoryStream]::new()
    try {
        $chunk = [byte[]]::new(4096)
        while (($read = $stream.Read($chunk, 0, $chunk.Length)) -gt 0) {
            if ($memory.Length + $read -gt $Entry.Length) { throw '包内元数据实际大小超过声明。' }
            $memory.Write($chunk, 0, $read)
        }
        if ($memory.Length -ne $Entry.Length) { throw '包内元数据未完整解压。' }
        return ConvertFrom-OnlineUtf8 ($memory.ToArray())
    } finally { $stream.Dispose(); $memory.Dispose() }
}

function Read-OnlinePackageProtocol {
    param([string]$Text, [string]$Version)
    $begin = 'GROK-UPDATE-PROTOCOL-BEGIN'; $end = 'GROK-UPDATE-PROTOCOL-END'
    if (!$Text.Contains($begin) -and !$Text.Contains($end)) { return $null }
    $lines = @($Text -split '\r?\n')
    $starts = @(for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -ceq $begin) { $i } })
    $ends = @(for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -ceq $end) { $i } })
    if ($starts.Count -ne 1 -or $ends.Count -ne 1 -or $starts[0] + 1 -ge $ends[0]) { throw '包内更新协议块不完整或重复。' }
    $json = $lines[($starts[0] + 1)..($ends[0] - 1)] -join "`n"
    $protocol = ConvertFrom-Json -InputObject $json
    foreach ($field in @('schema', 'version', 'platform', 'mode', 'manifest', 'executable', 'installer')) {
        $value = Get-OnlineProperty $protocol $field
        if ([regex]::Matches($json, '"' + $field + '"\s*:').Count -ne 1) { throw "更新协议字段重复或无效：$field" }
        if ($field -eq 'schema') {
            if (($value -isnot [int] -and $value -isnot [long]) -or $value -ne 1) { throw '不支持此更新协议，请更新在线安装入口。' }
        } elseif ($value -isnot [string]) { throw "更新协议字段类型无效：$field" }
    }
    if ($protocol.mode -cne 'executable-only' -or $protocol.manifest -cne 'SHA256SUMS.txt') { throw '不支持此更新协议，请更新在线安装入口。' }
    if ($protocol.version -cne $Version -or $protocol.platform -cne $script:OnlineTargetTriple) { throw '包内更新协议与发布版本或平台不一致。' }
    Assert-OnlinePackageRelativePath $protocol.executable
    Assert-OnlinePackageRelativePath $protocol.installer
    if ($protocol.executable -in @('BUILD-INFO.txt', 'SHA256SUMS.txt') -or
        $protocol.installer -in @('BUILD-INFO.txt', 'SHA256SUMS.txt') -or
        $protocol.executable -ieq $protocol.installer) { throw '更新协议入口无效或重叠。' }
    return $protocol
}

function Expand-VerifiedOnlinePackage {
    param([string]$ArchivePath, [string]$Destination, $Contract)
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    if (Test-Path -LiteralPath $Destination) { throw '解压目标必须是新的临时目录。' }
    $archive = [IO.Compression.ZipFile]::OpenRead($ArchivePath)
    try {
        if ($archive.Entries.Count -eq 0 -or $archive.Entries.Count -gt 128) { throw 'ZIP 文件数量超出限制。' }
        $entries = [Collections.Generic.Dictionary[string, object]]::new([StringComparer]::Ordinal)
        $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        $total = 0L
        $directories = @()
        foreach ($entry in $archive.Entries) {
            $name = $entry.FullName
            $directory = $name.EndsWith('/')
            $unixType = ([uint32]([long]$entry.ExternalAttributes -band 0xffffffffL) -shr 16) -band 0xf000
            if ($name.Contains('\') -or $name.Contains('//') -or $name.Contains(':') -or $name.StartsWith('/') -or
                ($unixType -ne 0 -and $unixType -ne 0x8000 -and $unixType -ne 0x4000) -or
                ($entry.ExternalAttributes -band 0x400) -ne 0 -or
                ($unixType -eq 0x4000 -and !$directory) -or ($unixType -eq 0x8000 -and $directory)) { throw "ZIP 包含不安全的成员：$name" }
            $parts = $name.TrimEnd('/').Split('/')
            if (@($parts | Where-Object { $_ -in @('', '.', '..') }).Count -gt 0) { throw "ZIP 路径无效：$name" }
            if (!$Contract.Legacy) {
                if ($name -ceq "$($Contract.PackageRoot)/") {
                    if (!$seen.Add($name) -or $entry.Length -ne 0) { throw 'ZIP 顶层目录重复或不为空。' }
                    continue
                }
                $prefix = "$($Contract.PackageRoot)/"
                if (!$name.StartsWith($prefix, [StringComparison]::Ordinal)) { throw "ZIP 必须只有指定顶层目录：$($Contract.PackageRoot)" }
                $name = $name.Substring($prefix.Length)
            }
            $logical = $name.TrimEnd('/')
            Assert-OnlinePackageRelativePath $logical
            if (!$seen.Add($logical)) { throw "ZIP 包含重复路径：$logical" }
            if ($directory) {
                if ($entry.Length -ne 0) { throw "ZIP 目录包含文件内容：$logical" }
                $directories += $logical
            } else {
                if ($entry.Length -gt 536870912 -or $entry.Length -lt 0) { throw 'ZIP 单文件超过大小限制。' }
                $total += $entry.Length
                if ($total -gt 805306368) { throw 'ZIP 解压后总大小超过限制。' }
                $entries.Add($logical, $entry)
            }
        }
        foreach ($name in @('BUILD-INFO.txt', 'SHA256SUMS.txt')) {
            if (!$entries.ContainsKey($name)) { throw "ZIP 缺少文件：$name" }
        }
        $protocol = Read-OnlinePackageProtocol (Read-OnlineZipText $entries['BUILD-INFO.txt']) $Contract.Version.Text
        if ($entries['SHA256SUMS.txt'].Length -gt 65536) { throw '包内校验清单超过大小限制。' }
        $manifestStream = $entries['SHA256SUMS.txt'].Open()
        try {
            $memory = [IO.MemoryStream]::new()
            try {
                $chunk = [byte[]]::new(4096)
                while (($read = $manifestStream.Read($chunk, 0, $chunk.Length)) -gt 0) {
                    if ($memory.Length + $read -gt $entries['SHA256SUMS.txt'].Length) { throw '包内清单实际大小超过声明。' }
                    $memory.Write($chunk, 0, $read)
                }
                if ($memory.Length -ne $entries['SHA256SUMS.txt'].Length) { throw '包内清单未完整解压。' }
                $manifestText = ConvertFrom-OnlineUtf8 ($memory.ToArray())
            } finally { $memory.Dispose() }
        } finally { $manifestStream.Dispose() }
        $manifestNames = if ($Contract.Legacy) { @($script:OnlinePackageFiles[0..6]) } else { $script:OnlinePackageFiles }
        $hashes = [Collections.Generic.Dictionary[string, string]]::new([StringComparer]::Ordinal)
        if ($manifestText.EndsWith("`n")) {
            $manifestText = $manifestText.Substring(0, $manifestText.Length - 1)
            if ($manifestText.EndsWith("`r")) { $manifestText = $manifestText.Substring(0, $manifestText.Length - 1) }
        }
        $lines = $manifestText -split '\r?\n'
        foreach ($line in $lines) {
            if ($line -cnotmatch '^([0-9A-Fa-f]{64})  (.+)$') { throw '包内 SHA256SUMS.txt 格式无效。' }
            $expected = $matches[1]; $name = $matches[2]
            Assert-OnlinePackageRelativePath $name
            if ((!$protocol -and $manifestNames -cnotcontains $name) -or $name -ieq 'SHA256SUMS.txt' -or $hashes.ContainsKey($name)) { throw "包内清单包含额外或重复文件：$name" }
            $hashes.Add($name, $expected)
        }
        if ($protocol) {
            foreach ($required in @('BUILD-INFO.txt', $protocol.executable, $protocol.installer)) {
                if (!$hashes.ContainsKey($required)) { throw "新协议清单缺少必要文件：$required" }
            }
            $packageFiles = @($hashes.Keys)
        } else {
            if ($hashes.Count -ne $manifestNames.Count) { throw '包内 SHA256SUMS.txt 未覆盖完整文件集合。' }
            $packageFiles = $script:OnlinePackageFiles
        }
        $fileNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($name in $entries.Keys) { $null = $fileNames.Add($name) }
        $allowedDirs = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($name in $packageFiles) {
            if (!$entries.ContainsKey($name)) { throw "ZIP 缺少清单文件：$name" }
            $parent = $name
            while ($parent.Contains('/')) {
                $parent = $parent.Substring(0, $parent.LastIndexOf('/'))
                if ($fileNames.Contains($parent)) { throw "ZIP 文件和目录冲突：$parent" }
                $null = $allowedDirs.Add($parent)
            }
        }
        if ($entries.Count -ne $packageFiles.Count + 1) { throw 'ZIP 包含清单之外的文件。' }
        foreach ($directory in $directories) {
            if (!$allowedDirs.Contains($directory)) { throw "ZIP 包含额外目录：$directory" }
        }
        $null = [IO.Directory]::CreateDirectory($Destination)
        $package = if ($Contract.Legacy) { $Destination } else { Join-Path $Destination $Contract.PackageRoot }
        foreach ($name in $entries.Keys) {
            $target = Join-Path $package $name
            $null = [IO.Directory]::CreateDirectory((Split-Path -Parent $target))
            $inputStream = $entries[$name].Open()
            $output = [IO.File]::Open($target, [IO.FileMode]::CreateNew)
            try {
                $buffer = [byte[]]::new(65536); $written = 0L
                while (($count = $inputStream.Read($buffer, 0, $buffer.Length)) -gt 0) {
                    $written += $count
                    if ($written -gt $entries[$name].Length) { throw "ZIP 文件实际大小与声明不符：$name" }
                    $output.Write($buffer, 0, $count)
                }
                if ($written -ne $entries[$name].Length) { throw "ZIP 文件尚未完整解压：$name" }
            } finally { $inputStream.Dispose(); $output.Dispose() }
            if ($hashes.ContainsKey($name) -and (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -ine $hashes[$name]) {
                throw "包内文件 SHA-256 不匹配：$name"
            }
        }
        return $package
    } finally { $archive.Dispose() }
}

function Get-OnlineExecutableVersion {
    param([string]$Executable)
    Assert-OnlinePathChain $Executable
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $Executable; $info.Arguments = '--version'
    $info.UseShellExecute = $false; $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true
    $process = [Diagnostics.Process]::new(); $process.StartInfo = $info
    try {
        if (!$process.Start()) { throw '无法启动版本检查。' }
        $stdout = $process.StandardOutput.ReadToEndAsync(); $stderr = $process.StandardError.ReadToEndAsync()
        if (!$process.WaitForExit(10000)) {
            # Only this newly-created --version child is stopped; never an existing session.
            $process.Kill(); $process.WaitForExit()
            throw '程序版本检查超时。'
        }
        $text = $stdout.GetAwaiter().GetResult().Trim()
        $null = $stderr.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0 -or $text.Length -gt 4096 -or
            $text -cnotmatch '^grok-zh (\S+)(?: \([^()\r\n]{1,128}\))?(?: \[[^\[\]\r\n]{1,32}\])?$') {
            throw '程序没有返回有效的 grok-zh 版本。'
        }
        $version = ConvertTo-OnlineVersion $matches[1]
        $version | Add-Member -NotePropertyName DisplayText -NotePropertyValue $text
        return $version
    } finally { $process.Dispose() }
}

function Get-OnlineInstallMarker {
    param([string]$Directory)
    if (!(Test-Path -LiteralPath $Directory)) { return $null }
    Assert-OnlinePathChain $Directory
    $markerPath = Join-Path $Directory '.grok-zh-install.json'
    Assert-OnlinePathChain $markerPath
    if (!(Test-Path -LiteralPath $markerPath -PathType Leaf)) { throw "目标目录已存在，但不属于社区安装器：$Directory" }
    $marker = [IO.File]::ReadAllText($markerPath) | ConvertFrom-Json
    if ((Get-OnlineProperty $marker 'product') -cne 'grok-build-zh' -or
        (Resolve-OnlinePath (Get-OnlineProperty $marker 'install_dir')) -ine $Directory) { throw '现有安装的归属记录与目标目录不一致。' }
    return $marker
}

function Invoke-OnlinePackageInstaller {
    param([string]$Package, [string]$Directory, [string]$SharedHome, [Parameter(Mandatory = $true)][string]$Version,
        [switch]$Commands, [switch]$NoPathUpdate)
    $buildInfo = ConvertFrom-OnlineUtf8 ([IO.File]::ReadAllBytes((Join-Path $Package 'BUILD-INFO.txt')))
    $protocol = Read-OnlinePackageProtocol $buildInfo $Version
    $installer = if ($protocol) { $protocol.installer } else { 'Install-GrokZh.ps1' }
    $arguments = @('-NoLogo', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
        (Join-Path $Package $installer), '-PackageDir', $Package,
        '-InstallDir', $Directory, '-ShowProgress')
    if ($SharedHome) { $arguments += @('-GrokHome', $SharedHome) }
    if ($Commands) { $arguments += '-InteractiveCommandSetup' }
    if ($NoPathUpdate) { $arguments += '-NoPathUpdate' }
    $hostExe = Join-Path $PSHOME $(if ($PSVersionTable.PSEdition -eq 'Core') { 'pwsh.exe' } else { 'powershell.exe' })
    & $hostExe @arguments | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "包内安装器返回错误（退出码 $LASTEXITCODE），请按上方中文提示处理。" }
}

function Assert-OnlinePortableRoot {
    param([string]$Root)
    if (!(Test-Path -LiteralPath $Root)) { return }
    $marker = Get-OnlineInstallMarker (Join-Path $Root 'app')
    if ($null -eq $marker -or (Get-OnlineProperty $marker 'portable_layout') -ne 1 -or
        (Get-OnlineProperty $marker 'portable_root') -ine $Root) { throw '现有目录不是受管便携版，请选择新目录。' }
    $children = @(Get-ChildItem -LiteralPath $Root -Force)
    if ($children.Count -ne 3 -or @($children | Where-Object { $_.Name -cnotin @('启动.cmd', '使用说明.md', 'app') }).Count -gt 0) {
        throw '便携目录包含额外文件，请先移出个人文件后再更新。'
    }
    foreach ($child in $children) { Assert-OnlinePathChain $child.FullName }
    if (!(Test-Path -LiteralPath (Join-Path $Root '启动.cmd') -PathType Leaf) -or
        !(Test-Path -LiteralPath (Join-Path $Root '使用说明.md') -PathType Leaf) -or
        !(Test-Path -LiteralPath (Join-Path $Root 'app') -PathType Container)) { throw '便携目录的入口、说明或程序目录类型不正确。' }
}

function Get-OnlineChangelogBody {
    param([string]$Body)
    # Match the generated Release-page footer, not authored links or code examples.
    $fence = $null
    $fenceWidth = 0
    $detailsStart = -1
    $offset = 0
    $notes = $Body
    foreach ($line in $Body.Split([char]10)) {
        $text = $line.Trim()
        if ($text -match '^(`{3,}|~{3,})') {
            $marker = $Matches[1]
            if ($null -eq $fence) {
                $fence = $marker[0]
                $fenceWidth = $marker.Length
            } elseif ($marker[0] -ceq $fence -and $marker.Length -ge $fenceWidth -and !$text.Substring($marker.Length).Trim()) {
                $fence = $null
            }
        }
        if ($null -eq $fence) {
            if ($detailsStart -ge 0 -and $line.TrimEnd() -ceq '<summary>下载与安装</summary>') {
                $notes = $Body.Substring(0, $detailsStart)
                break
            }
            if ($text) {
                $detailsStart = -1
                if ($line.TrimEnd() -ceq '<details>') { $detailsStart = $offset }
            }
        }
        $offset += $line.Length + 1
    }
    $notes = $notes.TrimEnd()
    $lastLine = $notes.LastIndexOf([char]10) + 1
    $footer = $notes.Substring($lastLine)
    if ($null -eq $fence -and ($footer.StartsWith('[完整变更](') -or $footer.StartsWith('[上游完整变更（'))) {
        $notes = $notes.Substring(0, $lastLine).TrimEnd()
    }
    return $notes.Trim()
}

function Show-OnlineReleaseNotes {
    param($Release)
    $body = [string](Get-OnlineProperty $Release 'ReleaseNotes')
    # Release Markdown is displayed as text, never evaluated as PowerShell.
    $body = Get-OnlineChangelogBody ($body.Replace("`r`n", "`n"))
    $body = [regex]::Replace($body, '[\x00-\x08\x0B-\x1F\x7F-\x9F]', '').Trim()
    if ($body.Length -gt 131072) {
        $body = $body.Substring(0, 131072) + "`n（正文较长，完整内容见下方 Release 链接。）"
    }
    Write-Host "`n更新日志 · v$($Release.Version.Text)`n" -ForegroundColor Cyan
    if ($body) { Write-Host $body }
    else { Write-Host '此版本未提供更新日志，详情见 Release 页面。' }
    Write-Host "`nRelease：https://github.com/$script:OnlineRepo/releases/tag/$($Release.Tag)`n"
}

function Remove-OnlinePreviousPortable {
    param([string]$Backup, [string]$Root, [int]$Depth = 0)
    if (!$Backup -or !(Test-Path -LiteralPath $Backup)) { return $null }
    if ($Depth -ge 32) { return $Backup }
    try {
        $resolved = Resolve-OnlinePath $Backup
        $current = Resolve-OnlinePath $Root
        $parent = Split-Path -Parent $current
        $leaf = Split-Path -Leaf $current
        if ((Split-Path -Parent $resolved) -ine $parent -or
            (Split-Path -Leaf $resolved) -notmatch ('^' + [regex]::Escape($leaf) + '\.previous\.\d{8}-\d{6}-[a-f0-9]{8}$')) { return $Backup }
        Assert-OnlinePathChain $resolved
        $children = @(Get-ChildItem -LiteralPath $resolved -Force)
        if ($children.Count -ne 3 -or @($children | Where-Object { $_.Name -cnotin @('启动.cmd', '使用说明.md', 'app') }).Count -gt 0) { return $Backup }
        if (!(Test-Path -LiteralPath (Join-Path $resolved '启动.cmd') -PathType Leaf) -or
            !(Test-Path -LiteralPath (Join-Path $resolved '使用说明.md') -PathType Leaf) -or
            !(Test-Path -LiteralPath (Join-Path $resolved 'app') -PathType Container)) { return $Backup }
        $app = Join-Path $resolved 'app'
        $markerPath = Join-Path $app '.grok-zh-install.json'
        Assert-OnlinePathChain $markerPath
        $oldMarker = [IO.File]::ReadAllText($markerPath) | ConvertFrom-Json
        if ((Get-OnlineProperty $oldMarker 'product') -cne 'grok-build-zh' -or
            (Get-OnlineProperty $oldMarker 'portable_layout') -ne 1 -or
            (Get-OnlineProperty $oldMarker 'portable_root') -ine $current -or
            (Get-OnlineProperty $oldMarker 'install_dir') -ine (Join-Path $current 'app')) { return $Backup }
        $known = @('.grok-zh-install.json', '.grok-zh-update-result.json', 'grok-zh.exe.update.lock', 'SHA256SUMS.txt', 'grok.cmd', 'agent.cmd', 'BUILD-INFO.txt', 'LICENSE-grok-build.txt',
            'licenses\ripgrep\COPYING', 'licenses\ripgrep\LICENSE-MIT', 'licenses\ripgrep\UNLICENSE',
            'licenses\project\THIRD-PARTY-NOTICES', 'licenses\project\THIRD_PARTY_NOTICES.md', 'licenses\project\NOTICE')
        foreach ($line in Get-Content -LiteralPath (Join-Path $app 'SHA256SUMS.txt') -Encoding UTF8) {
            if ($line -match '^[a-fA-F0-9]{64}  (.+)$') { $known += $matches[1].Replace('/', '\') }
        }
        foreach ($item in Get-ChildItem -LiteralPath $resolved -Recurse -Force) {
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { return $Backup }
            if ($item.PSIsContainer) { continue }
            if ($item.FullName.StartsWith("$app\", [StringComparison]::OrdinalIgnoreCase)) {
                $relative = $item.FullName.Substring($app.Length + 1)
                # Never remove historical official-command backups or extra data.
                # Match only exact top-level files generated by the updater.
                if ($relative -notin $known -and
                    $relative -cnotmatch '^grok-zh\.exe\.(?:old(?:\.[0-9]+-[0-9]+\.old)?|[0-9]+-[0-9]+\.(?:download\.zip|candidate\.exe))$') { return $Backup }
            }
            $handle = [IO.File]::Open($item.FullName, 'Open', 'Read', 'None')
            $handle.Dispose()
        }
        $older = Get-OnlineProperty $oldMarker 'previous_portable_backup'
        $pendingOlder = if ($older) { Remove-OnlinePreviousPortable -Backup $older -Root $current -Depth ($Depth + 1) } else { $null }
        Remove-OnlineOwnedTree -Path $resolved -Parent $parent -Prefix "$leaf.previous."
        return $pendingOlder
    } catch { return $Backup }
}

function Install-OnlinePortable {
    param([string]$Package, [string]$Root, [string]$SharedHome, [string]$Version)
    Assert-OnlinePortableRoot $Root
    $parent = Split-Path -Parent $Root; $leaf = Split-Path -Leaf $Root
    $null = [IO.Directory]::CreateDirectory($parent)
    $prefix = ".$leaf.grok-zh-stage-"
    $stage = Join-Path $parent ($prefix + [guid]::NewGuid().ToString('N'))
    $backup = if (Test-Path -LiteralPath $Root) { "$Root.previous.$(Get-Date -Format 'yyyyMMdd-HHmmss')-$([guid]::NewGuid().ToString('N').Substring(0, 8))" } else { $null }
    $committed = $false; $stageCreated = $false
    try {
        if (Test-Path -LiteralPath $stage) { throw '便携暂存路径已存在，请重新运行。' }
        $null = [IO.Directory]::CreateDirectory($stage)
        $stageCreated = $true
        $stageApp = Join-Path $stage 'app'
        Invoke-OnlinePackageInstaller -Package $Package -Directory $stageApp -SharedHome $SharedHome -Version $Version -NoPathUpdate
        $markerPath = Join-Path $stageApp '.grok-zh-install.json'
        $marker = Get-OnlineInstallMarker $stageApp
        if ($null -eq $marker) { throw '便携部署未生成安装归属记录。' }
        if ((Get-OnlineExecutableVersion (Join-Path $stageApp 'grok-zh.exe')).Text -cne $Version) {
            throw '便携暂存程序版本未通过核对，原目录保持原样。'
        }
        $marker.install_dir = Join-Path $Root 'app'
        $marker | Add-Member -NotePropertyName portable_layout -NotePropertyValue 1
        $marker | Add-Member -NotePropertyName portable_root -NotePropertyValue $Root
        $marker | Add-Member -NotePropertyName previous_portable_backup -NotePropertyValue $backup
        [IO.File]::WriteAllText($markerPath, ($marker | ConvertTo-Json -Depth 6), $script:OnlineUtf8)
        $launcher = "@echo off`r`n`"%~dp0app\grok-zh.exe`" %*`r`nexit /b %ERRORLEVEL%`r`n"
        [IO.File]::WriteAllText((Join-Path $stage '启动.cmd'), $launcher, [Text.Encoding]::ASCII)
        $guide = "# Grok Build 中文社区版 $Version`r`n`r`n双击 启动.cmd；在终端中也可运行 .\启动.cmd 并传入原有参数。`r`n代理入口：.\app\agent-zh.cmd stdio。`r`n`r`n程序位于 app，本便携版不修改 PATH。账号、会话和配置仍与官方版共用 ~/.grok（或 GROK_HOME）。`r`n请勿把个人文件存入 app。更新完整便携版时重新运行在线安装命令并选择此目录；成功后自动清理旧版本，失败时回滚。`r`n"
        [IO.File]::WriteAllText((Join-Path $stage '使用说明.md'), $guide, $script:OnlineUtf8)
        # Recheck paths and ownership immediately before the two directory moves.
        Assert-OnlinePathChain $Root; Assert-OnlinePortableRoot $Root
        if ($backup) {
            Assert-OnlinePathChain $backup
            if ((Split-Path -Parent $backup) -ine $parent -or (Test-Path -LiteralPath $backup)) { throw '便携备份路径无效。' }
            [IO.Directory]::Move($Root, $backup)
        }
        [IO.Directory]::Move($stage, $Root)
        $null = Get-OnlineInstallMarker (Join-Path $Root 'app')
        if ((Get-OnlineExecutableVersion (Join-Path $Root 'app\grok-zh.exe')).Text -cne $Version) {
            throw '便携程序激活后的版本未通过核对。'
        }
        $committed = $true
        if ($backup) {
            $pendingBackup = Remove-OnlinePreviousPortable -Backup $backup -Root $Root
            $marker.previous_portable_backup = $pendingBackup
            [IO.File]::WriteAllText((Join-Path $Root 'app\.grok-zh-install.json'), ($marker | ConvertTo-Json -Depth 6), $script:OnlineUtf8)
            if ($pendingBackup) { Write-Warning "旧便携目录暂未清理（文件占用或含需保留的内容），下次更新将重试：$pendingBackup" }
        }
    } finally {
        # .NET moves also run during pipeline cancellation. Keep every owned
        # directory intact if recovery itself fails; never delete the old backup.
        if ($stageCreated -and !$committed) {
            if (!(Test-Path -LiteralPath $stage) -and (Test-Path -LiteralPath $Root) -and
                (!$backup -or (Test-Path -LiteralPath $backup))) {
                Assert-OnlinePathChain $Root
                [IO.Directory]::Move($Root, $stage)
            }
            if ($backup -and (Test-Path -LiteralPath $backup) -and !(Test-Path -LiteralPath $Root)) {
                Assert-OnlinePathChain $backup
                [IO.Directory]::Move($backup, $Root)
            }
        }
        if ($stageCreated) { Remove-OnlineOwnedTree -Path $stage -Parent $parent -Prefix $prefix }
    }
}

function Invoke-GrokZhOnline {
    [CmdletBinding(SupportsShouldProcess = $true)]
    param([string]$Mode = 'Menu', [string]$InstallDir, [string]$PortableDir, [string]$GrokHome,
        [switch]$NoPathUpdate, [switch]$Repair, [switch]$NonInteractive, [switch]$VerifyOnly,
        [ValidateSet('Gnu', 'Msvc')][string]$WindowsX64Runtime = 'Gnu', [string]$Version)
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT -or ![Environment]::Is64BitOperatingSystem) {
        throw '此在线安装入口只支持 Windows x64 或 ARM64。'
    }
    $nativeMachine = Get-OnlineNativeMachine
    if ($nativeMachine -eq 0xAA64) {
        $script:OnlinePlatformSuffix = 'windows-aarch64-msvc'
        $script:OnlineTargetTriple = 'aarch64-pc-windows-msvc'
        $architecture = 'ARM64'
    } elseif ($nativeMachine -eq 0x8664) {
        $runtime = $WindowsX64Runtime.ToLowerInvariant()
        $script:OnlinePlatformSuffix = "windows-x86_64-$runtime"
        $script:OnlineTargetTriple = "x86_64-pc-windows-$runtime"
        $architecture = 'x64'
    } else {
        throw ('不支持的 Windows 原生架构：0x{0:X4}' -f $nativeMachine)
    }
    if ($NonInteractive -and $Mode -eq 'Commands') { throw '命令设置需要交互选择，请去掉 -NonInteractive。' }
    if (!$InstallDir) { $InstallDir = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs\grok-zh\bin' }
    $explicitHome = $GrokHome
    if (!$GrokHome) { $GrokHome = if ($env:GROK_HOME) { $env:GROK_HOME } else { Join-Path ([Environment]::GetFolderPath('UserProfile')) '.grok' } }
    $shared = Resolve-OnlinePath $GrokHome
    $temporaryParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
    Assert-OnlinePathChain $temporaryParent
    $work = Join-Path $temporaryParent ('grok-zh-online-' + [guid]::NewGuid().ToString('N'))
    if (Test-OnlinePathOverlap $shared $work) { throw '临时下载目录不能与共享数据目录重叠，请调整 TEMP 或 GROK_HOME。' }
    $client = $null; $workCreated = $false; $oldTls = [Net.ServicePointManager]::SecurityProtocol
    try {
        if (Test-Path -LiteralPath $work) { throw '临时下载目录已存在，请重新运行。' }
        $null = [IO.Directory]::CreateDirectory($work)
        $workCreated = $true
        Assert-OnlinePathChain $work
        Write-Host "`nGrok Build 中文社区版安装`n" -ForegroundColor Cyan
        Write-Host '正在检查最新正式版本...'
        $client = New-OnlineHttpClient
        $release = if ($Version) { Get-ExactOnlineRelease -Client $client -Version $Version -WorkDirectory $work } else { Get-LatestOnlineRelease -Client $client -WorkDirectory $work }
        Write-Host "系统：Windows $architecture`n最新正式版：$($release.Version.Text)`n默认安装位置：$InstallDir`n"
        if ($Mode -eq 'Menu' -and !$NonInteractive -and !$VerifyOnly) {
            Write-Host "1. 安装或更新（推荐，与官方版共存）`n2. 安装并设置 grok / agent 启动命令`n3. 自定义安装目录`n4. 创建或更新便携版（不修改 PATH）`n0. 退出"
            do { $choice = Read-Host '请选择 [0-4]' } while ($choice -notmatch '^[0-4]$')
            switch ($choice) {
                '0' { Write-Host '已取消。'; return }
                '1' { $Mode = 'Install' }
                '2' { $Mode = 'Commands' }
                '3' { $Mode = 'Install'; $InstallDir = Read-Host '请输入程序安装目录的完整路径' }
                '4' { $Mode = 'Portable' }
            }
        }
        if ($VerifyOnly) { $target = $work; $programDirectory = $null }
        elseif ($Mode -eq 'Portable') {
            if (!$PortableDir) {
                if ($NonInteractive) { throw '便携模式需要 -PortableDir。' }
                $PortableDir = Read-Host '请输入便携版根目录的完整路径'
            }
            $target = Resolve-OnlinePath $PortableDir
            Assert-OnlinePortableRoot $target
            $programDirectory = Join-Path $target 'app'
        } else { $target = Resolve-OnlinePath $InstallDir; $programDirectory = $target }
        if (!$VerifyOnly -and ((Test-OnlinePathOverlap $target $shared) -or (Test-OnlinePathOverlap $target $work))) { throw '安装目录不能与共享数据或临时下载目录重叠。' }
        $beforeMarker = $null
        if (!$VerifyOnly) {
            $existing = Get-OnlineInstallMarker $programDirectory
            if ($existing) {
                $installed = Get-OnlineExecutableVersion (Join-Path $programDirectory 'grok-zh.exe')
                Write-Host "当前安装版本：$($installed.Text)"
                $comparison = Compare-OnlineVersion $installed $release.Version
                if ($comparison -gt 0) { throw "当前版本 $($installed.Text) 高于最新正式版 $($release.Version.Text)，不会自动降级。" }
                if ($comparison -eq 0 -and !$Repair) {
                    if ($NonInteractive) { Write-Host '已经是最新正式版；需要修复安装时请加 -Repair。'; return }
                    if ((Read-Host '版本相同。输入 1 修复安装，其他输入退出') -cne '1') { Write-Host '已取消。'; return }
                }
                $beforeMarker = (Get-FileHash -LiteralPath (Join-Path $programDirectory '.grok-zh-install.json') -Algorithm SHA256).Hash
            }
        }
        $zipPath = Join-Path $work $release.Archive.Name
        Write-Host '正在下载并校验完整安装包...'
        Receive-OnlineFile -Client $client -Uri $release.Archive.Url -Destination $zipPath -MaximumBytes $release.Archive.Size `
            -ExpectedBytes $release.Archive.Size -ExpectedSha256 $release.Archive.Sha256 -Label '正在下载最新正式版'
        Write-Host '正在校验 ZIP 路径、包内文件与 SHA-256...'
        $package = Expand-VerifiedOnlinePackage -ArchivePath $zipPath -Destination (Join-Path $work 'package') -Contract $release
        $protocol = Read-OnlinePackageProtocol (ConvertFrom-OnlineUtf8 ([IO.File]::ReadAllBytes((Join-Path $package 'BUILD-INFO.txt')))) $release.Version.Text
        $executable = if ($protocol) { $protocol.executable } else { 'grok-zh.exe' }
        $candidate = Get-OnlineExecutableVersion (Join-Path $package $executable)
        if ($candidate.Text -cne $release.Version.Text) { throw '候选程序版本与 Release 不一致。' }
        if ($VerifyOnly) { Write-Host "验证通过：$($candidate.Text)。未执行安装。" -ForegroundColor Green; return }
        if (!$PSCmdlet.ShouldProcess($target, "安装 Grok Build 中文社区版 $($candidate.Text)")) { return }
        if ($Mode -eq 'Portable') {
            Install-OnlinePortable -Package $package -Root $target -SharedHome $shared -Version $candidate.Text
        } else {
            Invoke-OnlinePackageInstaller -Package $package -Directory $target -SharedHome $explicitHome -Version $candidate.Text `
                -Commands:($Mode -eq 'Commands') -NoPathUpdate:$NoPathUpdate
            $markerPath = Join-Path $programDirectory '.grok-zh-install.json'
            if (!(Test-Path -LiteralPath $markerPath) -or ($beforeMarker -and
                (Get-FileHash -LiteralPath $markerPath -Algorithm SHA256).Hash -ceq $beforeMarker)) {
                Write-Host '安装未执行或已在包内菜单取消。'; return
            }
        }
        $null = Get-OnlineInstallMarker $programDirectory
        $actual = Get-OnlineExecutableVersion (Join-Path $programDirectory 'grok-zh.exe')
        if ($actual.Text -cne $candidate.Text) { throw '安装后的程序版本未通过核对，请保留旧安装备份并检查上方输出。' }
        Write-Host "`n安装完成：$($actual.Text)`n位置：$target" -ForegroundColor Green
        Show-OnlineReleaseNotes -Release $release
        if ($Mode -eq 'Portable') { Write-Host '双击目录中的 启动.cmd，或在终端中运行该入口；未修改 PATH。' }
        elseif ($NoPathUpdate) { Write-Host '未修改 PATH，请通过上述目录中的 grok-zh.exe 启动。' }
        else { Write-Host '请重新打开终端，输入 grok-zh 启动。' }
    } finally {
        if ($client) { $client.Dispose() }
        [Net.ServicePointManager]::SecurityProtocol = $oldTls
        if ($workCreated) { Remove-OnlineOwnedTree -Path $work -Parent $temporaryParent -Prefix 'grok-zh-online-' }
    }
}

# Dot-sourcing exposes pure helpers for offline tests without starting a download.
if ($MyInvocation.InvocationName -ne '.') {
    try { Invoke-GrokZhOnline @PSBoundParameters }
    catch { [Console]::Error.WriteLine("安装未完成：$($_.Exception.GetBaseException().Message)"); exit 1 }
}
