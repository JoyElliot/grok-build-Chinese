Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Net.Http
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$onlineScript = Join-Path (Split-Path -Parent $PSScriptRoot) 'Install-GrokZhOnline.ps1'
. $onlineScript
$script:Checks = 0
function Assert-True([bool]$Value, [string]$Message) {
    if (!$Value) { throw "断言失败：$Message" }
    $script:Checks++
}
function Assert-Throws([scriptblock]$Action, [string]$Message) {
    $thrown = $false
    try { & $Action | Out-Null } catch { $thrown = $true }
    Assert-True $thrown $Message
}
function Get-TestDigest([byte[]]$Bytes) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
}

# Validate names as strings only: never attempt to open reserved DOS devices.
foreach ($prefix in @('COM', 'LPT', 'com', 'lpt')) {
    foreach ($digit in @([char]0x00b9, [char]0x00b2, [char]0x00b3)) {
        foreach ($path in @("$prefix$digit", "docs/$prefix$digit.txt", "$prefix$digit/readme.md")) {
            Assert-Throws { Assert-OnlinePackageRelativePath $path } "拒绝上标数字设备路径：$path"
        }
    }
}
foreach ($path in @('docs/中文说明.txt', 'COM10.txt', 'LPT10/readme.md', 'COM¹notes.txt')) {
    Assert-OnlinePackageRelativePath $path
}

# Inject a transport into the normal HttpClient API. Production URLs, redirect
# validation and digest checks remain active; no alternate-source CLI is added.
if (!('GrokOnlineTestHandler' -as [type])) {
    $mockCode = @'
using System;
using System.Collections.Generic;
using System.Net;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
public sealed class GrokOnlineTestHandler : HttpMessageHandler {
    public readonly Dictionary<string, byte[]> Bodies = new Dictionary<string, byte[]>();
    public readonly Dictionary<string, int> Statuses = new Dictionary<string, int>();
    public readonly Dictionary<string, string> Redirects = new Dictionary<string, string>();
    public readonly Dictionary<string, long> Lengths = new Dictionary<string, long>();
    public readonly Dictionary<string, int> FailuresRemaining = new Dictionary<string, int>();
    public readonly List<string> Requests = new List<string>();
    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken) {
        string url = request.RequestUri.AbsoluteUri;
        Requests.Add(url);
        int status = Statuses.ContainsKey(url) ? Statuses[url] : (Bodies.ContainsKey(url) ? 200 : 404);
        if (FailuresRemaining.ContainsKey(url)) {
            if (FailuresRemaining[url] > 0) FailuresRemaining[url]--;
            else status = 200;
        }
        var response = new HttpResponseMessage((HttpStatusCode)status);
        response.Content = new ByteArrayContent(Bodies.ContainsKey(url) ? Bodies[url] : new byte[0]);
        if (Redirects.ContainsKey(url)) response.Headers.Location = new Uri(Redirects[url], UriKind.RelativeOrAbsolute);
        if (Lengths.ContainsKey(url)) response.Content.Headers.ContentLength = Lengths[url];
        return Task.FromResult(response);
    }
}
'@
    if ($PSVersionTable.PSVersion.Major -ge 6) { Add-Type -TypeDefinition $mockCode }
    else { Add-Type -ReferencedAssemblies System.Net.Http -TypeDefinition $mockCode }
}

$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('grok-zh-online-test-' + [guid]::NewGuid().ToString('N'))
$userPathBefore = [Environment]::GetEnvironmentVariable('Path', 'User')
$processPathBefore = $env:Path
$windowsDirectory = Split-Path -Parent $PSScriptRoot
$utf8 = [Text.UTF8Encoding]::new($false)
$originalClientFactory = (Get-Item Function:New-OnlineHttpClient).ScriptBlock

function New-TestZip {
    param([string]$Name, [switch]$Legacy, [string]$Extra, [string]$Missing,
        [string]$ManifestText, [string]$Corrupt, [switch]$Symlink, [string]$BuildInfo, [hashtable]$Renames = @{})
    $file = Join-Path $testRoot "$Name.zip"
    $archive = [IO.Compression.ZipFile]::Open($file, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($name in @($script:OnlinePackageFiles) + @('SHA256SUMS.txt') + @($Extra)) {
            if (!$name -or $name -ceq $Missing) { continue }
            $logical = if ($Renames.ContainsKey($name)) { $Renames[$name] } else { $name }
            $member = if ($Legacy) { $logical } else { "grok-zh-1.0.13-windows-x86_64-gnu/$logical" }
            $entry = $archive.CreateEntry($member)
            if ($Symlink -and $name -ceq $Extra) { $entry.ExternalAttributes = -1610612736 }
            $bytes = if ($name -ceq 'BUILD-INFO.txt' -and $BuildInfo) { $utf8.GetBytes($BuildInfo) }
                elseif ($name -ceq 'SHA256SUMS.txt' -and $ManifestText) { $utf8.GetBytes($ManifestText) }
                elseif ($name -ceq $Extra -or $name -ceq $Corrupt) { $utf8.GetBytes('unexpected') }
                else { [IO.File]::ReadAllBytes((Join-Path $fixture $name)) }
            $stream = $entry.Open()
            try { $stream.Write($bytes, 0, $bytes.Length) } finally { $stream.Dispose() }
        }
    } finally { $archive.Dispose() }
    return $file
}

function New-TestRelease {
    param([string]$Version = '1.0.13', [byte[]]$ZipBytes, [switch]$Legacy)
    $tag = if ($Legacy) { "v$Version" } else { "release-v$Version" }
    $zipName = "grok-zh-$Version-windows-x86_64-gnu.zip"
    $hash = Get-TestDigest $ZipBytes
    $sidecar = $utf8.GetBytes("$hash  $zipName`n")
    $names = @($zipName, "$zipName.sha256")
    if (!$Legacy) { $names += @("grok-zh-$Version-macos-aarch64.tar.gz", "grok-zh-$Version-macos-aarch64.tar.gz.sha256", "grok-zh-$Version-linux-x86_64-gnu.tar.gz", "grok-zh-$Version-linux-x86_64-gnu.tar.gz.sha256") }
    $assets = foreach ($name in $names) {
        $body = if ($name -ceq $zipName) { $ZipBytes } elseif ($name -ceq "$zipName.sha256") { $sidecar } else { $utf8.GetBytes('fixture') }
        [pscustomobject]@{ name = $name; state = 'uploaded'; size = [long]$body.Length
            digest = 'sha256:' + (Get-TestDigest $body)
            browser_download_url = "https://github.com/JoyElliot/grok-build-Chinese/releases/download/$tag/$name" }
    }
    return [pscustomobject]@{ tag_name = $tag; immutable = $true; draft = $false; prerelease = $false; assets = @($assets)
        body = "- 本次中文更新`n`n## 上游更新`n`n- 来自目标 Release 的内容" }
}

function New-TestTransport {
    param($Release, [byte[]]$ZipBytes)
    $handler = [GrokOnlineTestHandler]::new()
    $handler.Bodies[$api] = $utf8.GetBytes((ConvertTo-Json -InputObject @($Release) -Depth 8))
    $contract = Get-OnlineReleaseContract $Release
    $handler.Bodies[$contract.Archive.Url] = $ZipBytes
    return $handler
}

function Invoke-TestLauncher([string]$Launcher, [string]$WorkingDirectory) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = Join-Path $env:SystemRoot 'System32\cmd.exe'
    $info.Arguments = '/d /s /c ""' + $Launcher + '" "参数 空格" "literal&value""'
    $info.WorkingDirectory = $WorkingDirectory
    $info.UseShellExecute = $false; $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true
    $info.StandardOutputEncoding = $utf8; $info.StandardErrorEncoding = $utf8
    $process = [Diagnostics.Process]::new(); $process.StartInfo = $info
    try {
        $null = $process.Start()
        $stdout = $process.StandardOutput.ReadToEndAsync(); $stderr = $process.StandardError.ReadToEndAsync()
        if (!$process.WaitForExit(10000)) { $process.Kill(); throw '启动包装测试超时。' }
        return [pscustomobject]@{ Out = $stdout.GetAwaiter().GetResult(); Err = $stderr.GetAwaiter().GetResult(); Code = $process.ExitCode }
    } finally { $process.Dispose() }
}

try {
    $null = [IO.Directory]::CreateDirectory($testRoot)
    $fixture = Join-Path $testRoot 'fixture'
    $shared = Join-Path $testRoot '共享数据'
    $null = [IO.Directory]::CreateDirectory($fixture)
    $null = [IO.Directory]::CreateDirectory($shared)
    $driveRoot = [IO.Path]::GetPathRoot($testRoot)
    $absentStage = Join-Path $driveRoot ('grok-zh-absent-cleanup-test-' + [guid]::NewGuid().ToString('N'))
    Assert-True (!(Test-Path -LiteralPath $absentStage)) '盘根清理回归使用不存在的随机路径'
    Remove-OnlineOwnedTree -Path $absentStage -Parent $driveRoot -Prefix 'grok-zh-absent-cleanup-test-'
    Assert-True (!(Test-Path -LiteralPath $absentStage)) '盘根父目录的边界检查兼容尾斜杠'
    [IO.File]::WriteAllText((Join-Path $shared 'auth.json'), 'preserve-auth', $utf8)
    [IO.File]::WriteAllText((Join-Path $shared 'config.toml'), 'preserve-config', $utf8)
    foreach ($name in $script:OnlinePackageFiles) {
        $path = Join-Path $fixture $name
        $null = [IO.Directory]::CreateDirectory((Split-Path -Parent $path))
        $source = Join-Path $windowsDirectory $name
        if (Test-Path -LiteralPath $source -PathType Leaf) { Copy-Item -LiteralPath $source -Destination $path }
        else { [IO.File]::WriteAllText($path, "fixture $name", $utf8) }
    }
    [IO.File]::WriteAllText((Join-Path $fixture 'BUILD-INFO.txt'), "Version: 1.0.13`n", $utf8)
    $csharp = Join-Path $testRoot 'Program.cs'
    [IO.File]::WriteAllText($csharp, @'
using System;
class Program {
    static int Main(string[] args) {
        Console.OutputEncoding = new System.Text.UTF8Encoding(false);
        if (args.Length == 1 && args[0] == "--version") { Console.Write("grok-zh 1.0.13 (abc1234) [stable]"); return 0; }
        Console.WriteLine("cwd=" + Environment.CurrentDirectory);
        Console.WriteLine("args=" + String.Join("|", args));
        Console.Error.WriteLine("fixture-stderr");
        return 23;
    }
}
'@, $utf8)
    $compiler = Join-Path $env:SystemRoot 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    & $compiler /nologo /target:exe "/out:$(Join-Path $fixture 'grok-zh.exe')" $csharp
    if ($LASTEXITCODE -ne 0) { throw '版本检查 fixture 编译失败。' }
    $manifestLines = foreach ($name in $script:OnlinePackageFiles) { "$(Get-TestDigest ([IO.File]::ReadAllBytes((Join-Path $fixture $name))))  $name" }
    $manifestText = ($manifestLines -join "`n") + "`n"
    [IO.File]::WriteAllText((Join-Path $fixture 'SHA256SUMS.txt'), $manifestText, $utf8)
    $zipPath = New-TestZip 'valid'
    $zipBytes = [IO.File]::ReadAllBytes($zipPath)
    $release = New-TestRelease -ZipBytes $zipBytes
    $contract = Get-OnlineReleaseContract $release
    $api = 'https://api.github.com/repos/JoyElliot/grok-build-Chinese/releases?per_page=100&page=1'

    Assert-True ($contract.Version.Text -ceq '1.0.13' -and !$contract.Legacy) '现代正式版合同'
    $noteContract = Get-OnlineReleaseContract $release
    $noteCases = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../../../crates/codegen/xai-grok-update/tests/fixtures/release-notes-display.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    foreach ($case in $noteCases) {
        foreach ($body in @($case.body, $case.body.Replace("`n", "`r`n"))) {
            $noteContract.ReleaseNotes = $body
            $noteOutput = @(Show-OnlineReleaseNotes $noteContract 6>&1) -join "`n"
            $expectedBody = [string]$case.expected
            if (!$expectedBody) { $expectedBody = '此版本未提供更新日志，详情见 Release 页面。' }
            $expectedOutput = "`n更新日志 · v1.0.13`n`n$expectedBody`n`nRelease：https://github.com/JoyElliot/grok-build-Chinese/releases/tag/release-v1.0.13`n"
            Assert-True ($noteOutput -ceq $expectedOutput) "更新日志展示共享样例：$($case.name)"
        }
    }
    $noteContract.ReleaseNotes = "中文$([char]27)[2J$([char]7)`r`n下一行"
    $noteOutput = @(Show-OnlineReleaseNotes $noteContract 6>&1) -join "`n"
    Assert-True (!$noteOutput.Contains([string][char]27) -and !$noteOutput.Contains([string][char]7)) '远端正文不输出终端控制字符'
    $noteContract.ReleaseNotes = $null
    $emptyNotes = @(Show-OnlineReleaseNotes $noteContract 6>&1) -join "`n"
    Assert-True ($emptyNotes.Contains('未提供更新日志') -and $emptyNotes.Contains('/tag/release-v1.0.13')) '空正文保留目标版本链接'
    $roundTrip = ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject @($release) -Depth 8)
    Assert-True ((Get-OnlineReleaseContract @($roundTrip)[0]).Version.Text -ceq '1.0.13') 'PS5/7 JSON 数组兼容'
    foreach ($mutation in @(
        { param($r) $r.immutable = $false }, { param($r) $r.draft = $true },
        { param($r) $r.prerelease = $true }, { param($r) $r.tag_name = 'v1.0.13' },
        { param($r) $r.assets[0].state = 'new' }, { param($r) $r.assets[0].digest = $null },
        { param($r) $r.assets[0].size = 536870913L },
        { param($r) $r.assets[0].browser_download_url = 'https://evil.invalid/a.zip' },
        { param($r) $r.assets += $r.assets[0] }
    )) {
        $copy = ($release | ConvertTo-Json -Depth 8) | ConvertFrom-Json
        & $mutation $copy
        Assert-Throws { Get-OnlineReleaseContract $copy } '拒绝无效发布元数据'
    }
    Assert-True ((Compare-OnlineVersion (ConvertTo-OnlineVersion '1.0.99') $contract.Version) -gt 0) '数值版本排序'
    Assert-True ((Compare-OnlineVersion (ConvertTo-OnlineVersion '1.0.13-rc.1') $contract.Version) -lt 0) '同版预发布可升级到正式版'
    Assert-Throws { ConvertTo-OnlineVersion '01.0.13' } '拒绝非规范版本'
    foreach ($uri in @('http://github.com/a', 'https://github.com.evil.invalid/a', 'https://github.com:444/a', 'https://user@github.com/a')) {
        Assert-Throws { Assert-OnlineDownloadUri ([uri]$uri) } '拒绝越界下载地址'
    }

    $handler = New-TestTransport -Release $release -ZipBytes $zipBytes
    $client = [Net.Http.HttpClient]::new($handler)
    try {
        $download = Join-Path $testRoot 'download.zip'
        Receive-OnlineFile -Client $client -Uri $contract.Archive.Url -Destination $download -MaximumBytes $zipBytes.Length -ExpectedBytes $zipBytes.Length -ExpectedSha256 $contract.Archive.Sha256
        Assert-True ((Get-FileHash -LiteralPath $download).Hash -ieq $contract.Archive.Sha256) '下载验证正确'
        Assert-Throws { Receive-OnlineFile -Client $client -Uri $contract.Archive.Url -Destination $download -MaximumBytes 1 } '不覆盖已有下载文件'
        Assert-Throws { Receive-OnlineFile -Client $client -Uri $contract.Archive.Url -Destination (Join-Path $testRoot 'bad-hash.zip') -MaximumBytes $zipBytes.Length -ExpectedSha256 ('0' * 64) } '摘要失败停止'
        Assert-True (!(Test-Path -LiteralPath (Join-Path $testRoot 'bad-hash.zip'))) '摘要失败清理自身下载'
        $selected = Get-LatestOnlineRelease -Client $client -WorkDirectory $testRoot
        Assert-True ($selected.Version.Text -ceq '1.0.13') '分页 API 选择正式版'
    } finally { $client.Dispose() }

    $paged = New-TestTransport -Release $release -ZipBytes $zipBytes
    $invalidNewer = New-TestRelease -Version '1.0.99' -ZipBytes $zipBytes
    $invalidNewer.assets += $invalidNewer.assets[0]
    $paged.Bodies[$api] = $utf8.GetBytes((ConvertTo-Json -InputObject (@($invalidNewer) * 100) -Depth 8))
    $paged.Bodies[$api.Replace('&page=1', '&page=2')] = $utf8.GetBytes((ConvertTo-Json -InputObject @($release) -Depth 8))
    $pageWork = Join-Path $testRoot 'paged'
    $null = [IO.Directory]::CreateDirectory($pageWork)
    $client = [Net.Http.HttpClient]::new($paged)
    try { Assert-True ((Get-LatestOnlineRelease -Client $client -WorkDirectory $pageWork).Version.Text -ceq '1.0.13') '分页跳过不可验证的新版本并回退' }
    finally { $client.Dispose() }
    Assert-True ($paged.Requests.Count -eq 2) '完整分页直到最后一页'

    foreach ($case in @('redirect', 'truncated', 'oversize', '403', '404', '429')) {
        $handler = New-TestTransport -Release $release -ZipBytes $zipBytes
        switch ($case) {
            'redirect' { $handler.Statuses[$contract.Archive.Url] = 302; $handler.Redirects[$contract.Archive.Url] = 'https://evil.invalid/a' }
            'truncated' { $handler.Bodies[$contract.Archive.Url] = [byte[]](1, 2); $handler.Lengths[$contract.Archive.Url] = $zipBytes.Length }
            'oversize' { $handler.Lengths[$contract.Archive.Url] = $zipBytes.Length + 1 }
            '404' { $handler.Statuses[$contract.Archive.Url] = 404 }
            '403' { $handler.Statuses[$contract.Archive.Url] = 403 }
            '429' { $handler.Statuses[$contract.Archive.Url] = 429 }
        }
        $client = [Net.Http.HttpClient]::new($handler)
        $failedPath = Join-Path $testRoot "$case.zip"
        try { Assert-Throws { Receive-OnlineFile -Client $client -Uri $contract.Archive.Url -Destination $failedPath -MaximumBytes $zipBytes.Length -ExpectedBytes $zipBytes.Length } "网络负例 $case" }
        finally { $client.Dispose() }
        Assert-True (!(Test-Path -LiteralPath $failedPath)) "网络负例 $case 无残留文件"
        if ($case -in @('truncated', '429')) { Assert-True ($handler.Requests.Count -eq 3) '可重试故障总共最多三次' }
        if ($case -in @('403', '404')) { Assert-True ($handler.Requests.Count -eq 1) '权限和不存在的资源不重复请求' }
    }
    $recovering = New-TestTransport -Release $release -ZipBytes $zipBytes
    $recovering.Statuses[$contract.Archive.Url] = 429
    $recovering.FailuresRemaining[$contract.Archive.Url] = 2
    $client = [Net.Http.HttpClient]::new($recovering)
    try { Receive-OnlineFile -Client $client -Uri $contract.Archive.Url -Destination (Join-Path $testRoot 'retry-success.zip') -MaximumBytes $zipBytes.Length -ExpectedBytes $zipBytes.Length -ExpectedSha256 $contract.Archive.Sha256 }
    finally { $client.Dispose() }
    Assert-True ($recovering.Requests.Count -eq 3) '短暂限流后重试成功'

    $package = Expand-VerifiedOnlinePackage -ArchivePath $zipPath -Destination (Join-Path $testRoot 'extract') -Contract $contract
    Assert-True (@(Get-ChildItem -LiteralPath (Split-Path -Parent $package)).Count -eq 1) '现代包保留唯一顶层目录'
    Assert-True ((Get-OnlineExecutableVersion (Join-Path $package 'grok-zh.exe')).Text -ceq '1.0.13') '真实子进程版本检查'
    foreach ($case in @('extra', 'traversal', 'duplicate', 'missing', 'hash', 'blank', 'symlink')) {
        $parameters = @{ Name = "invalid-$case" }
        switch ($case) {
            'extra' { $parameters.Extra = 'extra.txt' }
            'traversal' { $parameters.Extra = '../escape.txt' }
            'duplicate' { $parameters.Extra = 'GROK-ZH.EXE' }
            'missing' { $parameters.Missing = 'rg.exe' }
            'hash' { $parameters.Corrupt = 'rg.exe' }
            'blank' { $parameters.ManifestText = $manifestText + "`n" }
            'symlink' { $parameters.Extra = 'link'; $parameters.Symlink = $true }
        }
        $badZip = New-TestZip @parameters
        Assert-Throws { Expand-VerifiedOnlinePackage -ArchivePath $badZip -Destination (Join-Path $testRoot "extract-$case") -Contract $contract } "拒绝非法 ZIP：$case"
    }
    $legacyText = ($manifestLines[0..6] -join "`n") + "`n"
    $legacyZip = New-TestZip -Name 'legacy' -Legacy -ManifestText $legacyText
    $legacyRelease = New-TestRelease -Version '1.0.8' -Legacy -ZipBytes ([IO.File]::ReadAllBytes($legacyZip))
    $legacy = Get-OnlineReleaseContract $legacyRelease
    $legacyPackage = Expand-VerifiedOnlinePackage -ArchivePath $legacyZip -Destination (Join-Path $testRoot 'legacy-extract') -Contract $legacy
    Assert-True (Test-Path -LiteralPath (Join-Path $legacyPackage 'licenses/project/NOTICE')) '桥接包仍保留全部许可证'
    $wrongLegacy = New-TestZip -Name 'legacy-profile-modern' -ManifestText $legacyText
    Assert-Throws { Expand-VerifiedOnlinePackage -ArchivePath $wrongLegacy -Destination (Join-Path $testRoot 'legacy-profile-extract') -Contract $contract } '现代包不能使用旧清单'
    $noSidecars = ($release | ConvertTo-Json -Depth 8) | ConvertFrom-Json
    $noSidecars.assets = @($noSidecars.assets | Where-Object { !$_.name.EndsWith('.sha256') })
    Assert-True ((Get-OnlineReleaseContract $noSidecars).Archive.Sha256 -ceq $contract.Archive.Sha256) '三个安装包无需独立校验文件'
    $noSidecars.assets = @($noSidecars.assets[0])
    Assert-True ((Get-OnlineReleaseContract $noSidecars).Version.Text -ceq '1.0.13') '当前平台不依赖其他平台附件'
    $ignoredSidecar = ($release | ConvertTo-Json -Depth 8) | ConvertFrom-Json
    $ignoredSidecar.assets[1].digest = $null
    Assert-True ((Get-OnlineReleaseContract $ignoredSidecar).Archive.Sha256 -ceq $contract.Archive.Sha256) '旧 sidecar 元数据不参与新版校验'

    $armRelease = New-TestRelease -Version '1.0.36' -ZipBytes $zipBytes
    $armName = 'grok-zh-1.0.36-windows-aarch64-msvc.zip'
    $armRelease.assets[0].name = $armName
    $armRelease.assets[0].browser_download_url = "https://github.com/JoyElliot/grok-build-Chinese/releases/download/release-v1.0.36/$armName"
    try {
        $script:OnlinePlatformSuffix = 'windows-aarch64-msvc'
        $script:OnlineTargetTriple = 'aarch64-pc-windows-msvc'
        Assert-True ((Get-OnlineReleaseContract $armRelease).Archive.Name -ceq $armName) 'Windows ARM64 选择原生完整包'
        Assert-Throws { Get-OnlineReleaseContract $release } 'Windows ARM64 不选择历史 x64 包'
        $armProtocol = "Version: 1.0.36`nGROK-UPDATE-PROTOCOL-BEGIN`n" +
            '{"schema":1,"version":"1.0.36","platform":"aarch64-pc-windows-msvc","mode":"executable-only","manifest":"SHA256SUMS.txt","executable":"grok-zh.exe","installer":"Install-GrokZh.ps1"}' +
            "`nGROK-UPDATE-PROTOCOL-END`n"
        Assert-True ((Read-OnlinePackageProtocol $armProtocol '1.0.36').platform -ceq 'aarch64-pc-windows-msvc') 'Windows ARM64 协议平台一致'
    } finally {
        $script:OnlinePlatformSuffix = 'windows-x86_64-gnu'
        $script:OnlineTargetTriple = 'x86_64-pc-windows-gnu'
    }

    $protocolFields = [ordered]@{ schema = 1; version = '1.0.13'; platform = 'x86_64-pc-windows-gnu'
        mode = 'executable-only'; manifest = 'SHA256SUMS.txt'; executable = 'grok-zh.exe'; installer = 'Install-GrokZh.ps1' }
    foreach ($case in @('valid', 'extension', 'no-header', 'nested', 'collision', 'unknown', 'platform', 'version', 'incomplete', 'duplicate', 'unsafe', 'installer-metadata', 'same-entry')) {
        $fields = @{}
        foreach ($key in $protocolFields.Keys) { $fields[$key] = $protocolFields[$key] }
        switch ($case) {
            'unknown' { $fields.schema = 99 }
            'platform' { $fields.platform = 'aarch64-apple-darwin' }
            'version' { $fields.version = '1.0.14' }
            'unsafe' { $fields.executable = '../grok-zh.exe' }
            'installer-metadata' { $fields.installer = 'BUILD-INFO.txt' }
            'same-entry' { $fields.installer = 'GROK-ZH.EXE' }
            'nested' { $fields.executable = 'bin/grok-zh.exe'; $fields.installer = 'setup/install.ps1' }
        }
        $info = "Version: 1.0.13`nGROK-UPDATE-PROTOCOL-BEGIN`n$($fields | ConvertTo-Json -Compress)`nGROK-UPDATE-PROTOCOL-END`n"
        if ($case -eq 'incomplete') { $info = $info.Replace('GROK-UPDATE-PROTOCOL-END', '') }
        if ($case -eq 'duplicate') { $info += $info }
        if ($case -eq 'no-header') { $info = $info.Replace("Version: 1.0.13`n", '') }
        $newLines = @($manifestLines | Where-Object { !$_.EndsWith('  BUILD-INFO.txt') }) + @("$(Get-TestDigest ($utf8.GetBytes($info)))  BUILD-INFO.txt")
        $extraName = ''
        if ($case -eq 'extension') { $extraName = 'docs/future.txt'; $newLines += "$(Get-TestDigest ($utf8.GetBytes('unexpected')))  $extraName" }
        if ($case -eq 'collision') { $extraName = 'RG.EXE/readme.txt'; $newLines += "$(Get-TestDigest ($utf8.GetBytes('unexpected')))  $extraName" }
        $renames = @{}
        if ($case -eq 'nested') {
            $renames = @{ 'grok-zh.exe' = 'bin/grok-zh.exe'; 'Install-GrokZh.ps1' = 'setup/install.ps1' }
            $newLines = @($newLines | ForEach-Object { $_.Replace('  grok-zh.exe', '  bin/grok-zh.exe').Replace('  Install-GrokZh.ps1', '  setup/install.ps1') })
        }
        $protocolZip = New-TestZip -Name "protocol-$case" -BuildInfo $info -ManifestText (($newLines -join "`n") + "`n") -Extra $extraName -Renames $renames
        $output = Join-Path $testRoot "protocol-extract-$case"
        if ($case -in @('valid', 'extension', 'no-header', 'nested')) {
            $protocolPackage = Expand-VerifiedOnlinePackage $protocolZip $output $contract
            Assert-True (Test-Path -LiteralPath (Join-Path $protocolPackage $fields.executable)) "新协议可校验：$case"
            if ($case -eq 'no-header') { $declaredPackage = $protocolPackage; $declaredZipBytes = [IO.File]::ReadAllBytes($protocolZip) }
        } else { Assert-Throws { Expand-VerifiedOnlinePackage $protocolZip $output $contract } "新协议不静默降级：$case" }
        if ($case -eq 'collision') { Assert-True (!(Test-Path -LiteralPath $output)) '大小写目录冲突在写入前被拒绝' }
    }

    $bridgeCheck = Join-Path (Split-Path -Parent (Split-Path -Parent $windowsDirectory)) '.github/scripts/verify-protocol-bridge.ps1'
    $bridgeRelease = New-TestRelease -ZipBytes $declaredZipBytes
    $bridgeTransport = New-TestTransport -Release $bridgeRelease -ZipBytes $declaredZipBytes
    $client = [Net.Http.HttpClient]::new($bridgeTransport)
    try { & $bridgeCheck -Release $bridgeRelease -Client $client }
    finally { $client.Dispose() }
    Assert-True ($bridgeTransport.Requests.Count -eq 1) '退休预检验证过渡包实际内容且无需 sidecar'
    $client = [Net.Http.HttpClient]::new((New-TestTransport -Release $release -ZipBytes $zipBytes))
    try { Assert-Throws { & $bridgeCheck -Release $release -Client $client } '旧协议包不能仅凭标签被指定为新引擎过渡包' }
    finally { $client.Dispose() }

    function New-OnlineHttpClient { return [Net.Http.HttpClient]::new($script:TestTransport) }
    $normal = Join-Path $testRoot '中文 空格安装\bin'
    $script:TestTransport = New-TestTransport -Release $release -ZipBytes $zipBytes
    $installOutput = @(Invoke-GrokZhOnline -Mode Install -InstallDir $normal -GrokHome $shared -NoPathUpdate -NonInteractive 6>&1) -join "`n"
    Assert-True ($installOutput.Contains($release.body)) '安装成功展示实际 Release 原正文'
    Assert-True ($installOutput.Contains('/releases/tag/release-v1.0.13')) '正文链接绑定实际版本'
    Assert-True (@($script:TestTransport.Requests | Where-Object { $_.EndsWith('.sha256') }).Count -eq 0) '在线安装不请求独立 sha256 文件'
    $declaredRelease = New-TestRelease -ZipBytes $declaredZipBytes
    $declaredRelease.assets = @($declaredRelease.assets | Where-Object { !$_.name.EndsWith('.sha256') })
    $script:TestTransport = New-TestTransport -Release $declaredRelease -ZipBytes $declaredZipBytes
    $declaredTarget = Join-Path $testRoot '新协议完整安装'
    Invoke-GrokZhOnline -Mode Install -InstallDir $declaredTarget -GrokHome $shared -NoPathUpdate -NonInteractive
    Assert-True (Test-Path -LiteralPath (Join-Path $declaredTarget 'grok-zh.exe')) '新协议单包复用旧包内安装器成功'
    Assert-True (Test-Path -LiteralPath (Join-Path $normal '.grok-zh-install.json')) '在线流程委托真实包内安装器'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $normal 'Install-GrokZh.ps1'))) '安装脚本不进入运行目录'
    $firstMarker = [IO.File]::ReadAllText((Join-Path $normal '.grok-zh-install.json'))
    $script:TestTransport = New-TestTransport -Release $release -ZipBytes $zipBytes
    $skipOutput = @(Invoke-GrokZhOnline -Mode Install -InstallDir $normal -GrokHome $shared -NoPathUpdate -NonInteractive 6>&1) -join "`n"
    Assert-True (!$skipOutput.Contains('更新日志 ·')) '同版本未安装不展示更新成功日志'
    Assert-True ([IO.File]::ReadAllText((Join-Path $normal '.grok-zh-install.json')) -ceq $firstMarker) '同版本默认不重装'
    Assert-True ($script:TestTransport.Requests.Count -eq 1) '同版本退出不下载 ZIP'
    $script:TestTransport = New-TestTransport -Release $release -ZipBytes $zipBytes
    Invoke-GrokZhOnline -Mode Install -InstallDir $normal -GrokHome $shared -NoPathUpdate -NonInteractive -Repair
    Assert-True ([IO.File]::ReadAllText((Join-Path $normal '.grok-zh-install.json')) -cne $firstMarker) '显式同版修复'
    $oldProgramSource = [IO.File]::ReadAllText($csharp).Replace('grok-zh 1.0.13 (abc1234)', 'grok-zh 1.0.12 (abc1234)')
    $oldProgramFile = Join-Path $testRoot 'OldProgram.cs'
    [IO.File]::WriteAllText($oldProgramFile, $oldProgramSource, $utf8)
    & $compiler /nologo /target:exe "/out:$(Join-Path $normal 'grok-zh.exe')" $oldProgramFile
    if ($LASTEXITCODE -ne 0) { throw '旧版升级 fixture 编译失败。' }
    Assert-True ((Get-OnlineExecutableVersion (Join-Path $normal 'grok-zh.exe')).Text -ceq '1.0.12') '建立旧版升级场景'
    $script:TestTransport = New-TestTransport -Release $release -ZipBytes $zipBytes
    Invoke-GrokZhOnline -Mode Install -InstallDir $normal -GrokHome $shared -NoPathUpdate -NonInteractive
    Assert-True ((Get-OnlineExecutableVersion (Join-Path $normal 'grok-zh.exe')).Text -ceq '1.0.13') '较旧的安装升级为最新正式版'
    Assert-True (!(Get-OnlineInstallMarker $normal).previous_install_backup) '普通升级成功清除备份引用'
    Assert-True (@(Get-ChildItem -LiteralPath (Split-Path -Parent $normal) -Directory -Filter 'bin.previous.*').Count -eq 0) '普通升级不累积旧目录'
    $older = New-TestRelease -Version '1.0.9' -ZipBytes $zipBytes
    $script:TestTransport = New-TestTransport -Release $older -ZipBytes $zipBytes
    Assert-Throws { Invoke-GrokZhOnline -Mode Install -InstallDir $normal -GrokHome $shared -NoPathUpdate -NonInteractive } '较新已安装版本禁止降级'
    Assert-True ($script:TestTransport.Requests.Count -eq 1) '降级拒绝发生在下载前'

    $portable = Join-Path $testRoot '便携版 空格'
    $script:TestTransport = New-TestTransport -Release $release -ZipBytes $zipBytes
    Invoke-GrokZhOnline -Mode Portable -PortableDir $portable -GrokHome $shared -NonInteractive
    Assert-True (@(Get-ChildItem -LiteralPath $portable -Force).Count -eq 3) '新便携版顶层恰好三项'
    $portableMarker = Get-OnlineInstallMarker (Join-Path $portable 'app')
    Assert-True ($portableMarker.portable_root -ceq $portable) '便携归属记录使用最终目录'
    $script:TestTransport = New-TestTransport -Release $release -ZipBytes $zipBytes
    Invoke-GrokZhOnline -Mode Portable -PortableDir $portable -GrokHome $shared -NonInteractive -Repair
    Assert-True (@(Get-ChildItem -LiteralPath $portable -Force).Count -eq 3) '更新后顶层仍为三项'
    $portableMarker = Get-OnlineInstallMarker (Join-Path $portable 'app')
    Assert-True (!$portableMarker.previous_portable_backup) '便携升级成功清除备份引用'
    Assert-True (@(Get-ChildItem -LiteralPath $testRoot -Directory -Filter '便携版 空格.previous.*').Count -eq 0) '便携升级成功删除旧目录'
    # A lock left by the built-in updater is managed data, not a personal file.
    [IO.File]::WriteAllText((Join-Path $portable 'app\grok-zh.exe.update.lock'), '', $utf8)
    Install-OnlinePortable -Package $package -Root ($portable.ToUpperInvariant()) -SharedHome $shared -Version '1.0.13'
    Assert-True (!(Get-OnlineInstallMarker (Join-Path $portable 'app')).previous_portable_backup) '便携升级可清理更新锁文件并接受路径大小写变化'
    $guidePath = Join-Path $portable '使用说明.md'
    $savedGuide = [IO.File]::ReadAllText($guidePath)
    [IO.File]::Delete($guidePath)
    $null = [IO.Directory]::CreateDirectory($guidePath)
    [IO.File]::WriteAllText((Join-Path $guidePath '个人文件.txt'), 'keep-me', $utf8)
    Assert-Throws { Assert-OnlinePortableRoot $portable } '同名说明目录不能绕过个人文件保护'
    Assert-True ([IO.File]::ReadAllText((Join-Path $guidePath '个人文件.txt')) -ceq 'keep-me') '拒绝无效便携结构时个人文件保持原样'
    [IO.File]::Delete((Join-Path $guidePath '个人文件.txt'))
    [IO.Directory]::Delete($guidePath)
    [IO.File]::WriteAllText($guidePath, $savedGuide, $utf8)
    $cwdFixture = Join-Path $testRoot '调用目录'
    $null = [IO.Directory]::CreateDirectory($cwdFixture)
    foreach ($launchCase in @(@((Join-Path $portable '启动.cmd'), 'args=参数 空格|literal&value'), @((Join-Path $portable 'app\agent-zh.cmd'), 'args=agent|参数 空格|literal&value'))) {
        $launch = Invoke-TestLauncher $launchCase[0] $cwdFixture
        Assert-True ($launch.Out.Contains("cwd=$cwdFixture") -and $launch.Out.Contains($launchCase[1])) '启动器透明传递参数与工作目录'
        Assert-True ($launch.Code -eq 23 -and $launch.Err.Trim() -ceq 'fixture-stderr') '启动器透明保留退出码与标准错误'
    }
    $savedPortableMarker = [IO.File]::ReadAllText((Join-Path $portable 'app\.grok-zh-install.json'))
    Assert-Throws { Install-OnlinePortable -Package $package -Root $portable -SharedHome $shared -Version '1.0.14' } '暂存版本校验失败时不切换'
    Assert-True ([IO.File]::ReadAllText((Join-Path $portable 'app\.grok-zh-install.json')) -ceq $savedPortableMarker) '暂存失败保持原便携目录'
    $originalVersionReader = (Get-Item Function:Get-OnlineExecutableVersion).ScriptBlock
    try {
        function Get-OnlineExecutableVersion([string]$Executable) {
            if ($Executable -ceq (Join-Path $portable 'app\grok-zh.exe')) { throw '模拟激活后版本检查失败' }
            return & $originalVersionReader $Executable
        }
        Assert-Throws { Install-OnlinePortable -Package $package -Root $portable -SharedHome $shared -Version '1.0.13' } '激活后失败触发回滚'
    } finally { Set-Item Function:Get-OnlineExecutableVersion -Value $originalVersionReader }
    Assert-True ([IO.File]::ReadAllText((Join-Path $portable 'app\.grok-zh-install.json')) -ceq $savedPortableMarker) '激活失败完整恢复旧便携目录'
    $cancelGate = [Threading.ManualResetEventSlim]::new($false)
    $cancelRun = [PowerShell]::Create()
    try {
        $cancelScript = {
            param($ScriptPath, $VerifiedPackage, $RootPath, $SharedPath, $Gate)
            . $ScriptPath
            $originalReader = (Get-Item Function:Get-OnlineExecutableVersion).ScriptBlock
            function Get-OnlineExecutableVersion([string]$Executable) {
                if ($Executable -ceq (Join-Path $RootPath 'app\grok-zh.exe')) {
                    $Gate.Set()
                    while ($true) { Start-Sleep -Milliseconds 100 }
                }
                return & $originalReader $Executable
            }
            Install-OnlinePortable -Package $VerifiedPackage -Root $RootPath -SharedHome $SharedPath -Version '1.0.13'
        }
        $null = $cancelRun.AddScript($cancelScript.ToString()).AddArgument($onlineScript).AddArgument($package).AddArgument($portable).AddArgument($shared).AddArgument($cancelGate)
        $pendingCancel = $cancelRun.BeginInvoke()
        if (!$cancelGate.Wait(15000)) { throw '取消回归未到达便携激活后的检查点。' }
        $cancelRun.Stop()
        try { $null = $cancelRun.EndInvoke($pendingCancel) } catch { }
    } finally { $cancelRun.Dispose(); $cancelGate.Dispose() }
    Assert-True ([IO.File]::ReadAllText((Join-Path $portable 'app\.grok-zh-install.json')) -ceq $savedPortableMarker) '停止 PowerShell 流水线仍恢复旧便携目录'
    Assert-True (@(Get-ChildItem -LiteralPath $testRoot -Force | Where-Object { $_.Name.StartsWith('.便携版 空格.grok-zh-stage-') }).Count -eq 0) '便携失败清理本次暂存目录'
    [IO.File]::WriteAllText((Join-Path $portable '个人文件.txt'), 'keep-me', $utf8)
    Assert-Throws { Assert-OnlinePortableRoot $portable } '便携目录有个人文件时拒绝整体更新'
    Assert-True ([IO.File]::ReadAllText((Join-Path $portable '个人文件.txt')) -ceq 'keep-me') '个人文件保持原样'
    $whatIfTarget = Join-Path $testRoot 'what-if'
    $temporaryBefore = @(Get-ChildItem -LiteralPath ([IO.Path]::GetTempPath()) -Directory -Filter 'grok-zh-online-*' | ForEach-Object { $_.FullName })
    $script:TestTransport = New-TestTransport -Release $release -ZipBytes $zipBytes
    Invoke-GrokZhOnline -Mode Install -InstallDir $whatIfTarget -GrokHome $shared -NoPathUpdate -NonInteractive -WhatIf
    Assert-True (!(Test-Path -LiteralPath $whatIfTarget)) 'WhatIf 不创建安装目录'
    $temporaryAfter = @(Get-ChildItem -LiteralPath ([IO.Path]::GetTempPath()) -Directory -Filter 'grok-zh-online-*' | Where-Object { $_.FullName -notin $temporaryBefore })
    Assert-True ($temporaryAfter.Count -eq 0) 'WhatIf 仍清理本次下载与解压目录'
    $script:TestTransport = New-TestTransport -Release $release -ZipBytes $zipBytes
    Invoke-GrokZhOnline -Mode Install -InstallDir $whatIfTarget -GrokHome $shared -NonInteractive -VerifyOnly
    Assert-True (!(Test-Path -LiteralPath $whatIfTarget)) 'VerifyOnly 不创建安装目录'
    $script:TestTransport = New-TestTransport -Release $release -ZipBytes $zipBytes
    Invoke-GrokZhOnline -Mode Portable -GrokHome $shared -NonInteractive -VerifyOnly
    Assert-True ($script:TestTransport.Requests.Count -eq 2) 'VerifyOnly 便携模式无需目标目录且不下载 sidecar'
    Assert-Throws { Invoke-GrokZhOnline -GrokHome ([IO.Path]::GetTempPath()) -NonInteractive -VerifyOnly } '拒绝共享数据与临时下载目录重叠'
    $command = [IO.File]::ReadAllText((Join-Path $windowsDirectory 'ONLINE-INSTALL-COMMAND.txt'), $utf8).Trim()
    $repoRoot = Split-Path -Parent (Split-Path -Parent $windowsDirectory)
    foreach ($document in @((Join-Path $repoRoot 'README.md'), (Join-Path $windowsDirectory 'INSTALL-WINDOWS.md'))) {
        Assert-True ([IO.File]::ReadAllText($document, $utf8).Contains($command)) '文档使用统一在线安装命令'
    }
    $parseTokens = $null; $parseErrors = $null
    $null = [Management.Automation.Language.Parser]::ParseInput($command, [ref]$parseTokens, [ref]$parseErrors)
    Assert-True (@($parseErrors).Count -eq 0) '一行入口在当前 PowerShell 版本语法有效'
    Assert-True ([Environment]::GetEnvironmentVariable('Path', 'User') -ceq $userPathBefore) '用户 Path 未变'
    Assert-True ($env:Path -ceq $processPathBefore) '进程 Path 未变'
    Assert-True ([IO.File]::ReadAllText((Join-Path $shared 'auth.json')) -ceq 'preserve-auth') '共享登录未变'
    Assert-True ([IO.File]::ReadAllText((Join-Path $shared 'config.toml')) -ceq 'preserve-config') '共享配置未变'
    Write-Host "在线安装器测试通过：$script:Checks 项断言（PowerShell $($PSVersionTable.PSVersion)）。" -ForegroundColor Green
} finally {
    Set-Item Function:New-OnlineHttpClient -Value $originalClientFactory
    Assert-True ([Environment]::GetEnvironmentVariable('Path', 'User') -ceq $userPathBefore) '异常路径也不修改用户 Path'
    Assert-True ($env:Path -ceq $processPathBefore) '异常路径也不修改进程 Path'
    Remove-OnlineOwnedTree -Path $testRoot -Parent ([IO.Path]::GetTempPath()) -Prefix 'grok-zh-online-test-'
}
