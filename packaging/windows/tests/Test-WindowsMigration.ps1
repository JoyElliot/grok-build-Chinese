[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$ArtifactsDirectory, [string]$Gcc = 'gcc')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$windows = Split-Path -Parent $PSScriptRoot
$repo = [IO.Path]::GetFullPath((Join-Path $windows '../..'))
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('grok-migration-test-' + [guid]::NewGuid().ToString('N'))
$utf8 = [Text.UTF8Encoding]::new($false)
$script:Checks = 0
function Assert-True([bool]$Value, [string]$Message) { if (!$Value) { throw "断言失败：$Message" }; $script:Checks++ }
function Assert-Throws([scriptblock]$Action, [string]$Message) {
    $failed = $false
    try { & $Action | Out-Null } catch { $failed = $true }
    Assert-True $failed $Message
}
function Write-Utf8([string]$Path, [string]$Text) { [IO.File]::WriteAllText($Path, $Text, $utf8) }
function Get-Digest([byte[]]$Bytes) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-', '').ToLowerInvariant() } finally { $sha.Dispose() }
}
function New-FakeExe([string]$Destination, [string]$Version) {
    $source = Join-Path $testRoot ('fixture-' + [guid]::NewGuid().ToString('N') + '.cs')
    Write-Utf8 $source (@'
using System;
class Program {
    static int Main(string[] args) {
        Console.OutputEncoding = new System.Text.UTF8Encoding(false);
        if (args.Length == 1 && args[0] == "--version") { Console.WriteLine("grok-zh VERSION (fixture)"); return 0; }
        if (args.Length == 1 && args[0] == "--stdin") { Console.Write(Console.In.ReadToEnd()); return 0; }
        foreach (string arg in args) Console.WriteLine("arg=" + Convert.ToBase64String(System.Text.Encoding.UTF8.GetBytes(arg)));
        Console.WriteLine("cwd=" + Environment.CurrentDirectory);
        Console.Error.WriteLine("fixture-stderr");
        return 23;
    }
}
'@.Replace('VERSION', $Version))
    & (Join-Path $env:SystemRoot 'Microsoft.NET/Framework64/v4.0.30319/csc.exe') /nologo /target:exe "/out:$Destination" $source
    if ($LASTEXITCODE -ne 0) { throw 'fixture 编译失败。' }
}
function New-ReleaseAsset([string]$Tag, [string]$Name, [byte[]]$Bytes) {
    return [pscustomobject]@{ name=$Name; state='uploaded'; size=$Bytes.Length; digest="sha256:$(Get-Digest $Bytes)";
        browser_download_url="https://github.com/JoyElliot/grok-build-Chinese/releases/download/$Tag/$Name" }
}
function New-TestContext([string]$Name) {
    $root = Join-Path $testRoot $Name
    $null = [IO.Directory]::CreateDirectory($root)
    Copy-Item -LiteralPath (Join-Path $ArtifactsDirectory 'grok-zh.exe') -Destination (Join-Path $root 'grok-zh.exe')
    Write-Utf8 (Join-Path $root 'user-owned.txt') 'retain'
    return [pscustomobject]@{ version='1.0.99'; migration=$script:Pin; launcher=(Join-Path $root 'grok-zh.exe') }
}
function New-Work([string]$Name) { $path = Join-Path $testRoot $Name; $null = [IO.Directory]::CreateDirectory($path); return $path }
function Invoke-Launcher([string]$Executable, [string]$Arguments, [string]$InputText = '') {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName=$Executable; $info.Arguments=$Arguments; $info.WorkingDirectory=$testRoot
    $info.UseShellExecute=$false; $info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
    $info.RedirectStandardInput=($InputText.Length -gt 0)
    $info.StandardOutputEncoding=$utf8; $info.StandardErrorEncoding=$utf8
    $process=[Diagnostics.Process]::new(); $process.StartInfo=$info
    try {
        $null=$process.Start(); $stdout=$process.StandardOutput.ReadToEndAsync(); $stderr=$process.StandardError.ReadToEndAsync()
        if ($InputText.Length -gt 0) { $process.StandardInput.Write($InputText); $process.StandardInput.Close() }
        if (!$process.WaitForExit(30000)) { $process.Kill(); throw '真实启动器 fixture 超时。' }
        return [pscustomobject]@{ Code=$process.ExitCode; Out=$stdout.GetAwaiter().GetResult(); Err=$stderr.GetAwaiter().GetResult() }
    } finally { $process.Dispose() }
}
$oldHome=$env:GROK_HOME; $oldPath=$env:Path
$userPath=[Environment]::GetEnvironmentVariable('Path','User')
try {
    $null=[IO.Directory]::CreateDirectory($testRoot)
    $library=New-Work 'library'
    Copy-Item -LiteralPath (Join-Path $windows 'Install-GrokZhOnline.ps1') -Destination $library
    Copy-Item -LiteralPath (Join-Path $windows 'migration/Invoke-Bootstrap.ps1') -Destination $library
    . (Join-Path $library 'Invoke-Bootstrap.ps1') -ContextPath 'unused-when-dot-sourced'
    $env:GROK_HOME=New-Work 'shared-user-data'
    Write-Utf8 (Join-Path $env:GROK_HOME 'auth.json') 'retain-auth'
    Add-Type -AssemblyName System.Net.Http
    if (!('GrokMigrationTestHandler' -as [type])) {
        $source=@'
using System;
using System.Collections.Generic;
using System.Net;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
public sealed class GrokMigrationTestHandler : HttpMessageHandler {
    public readonly Dictionary<string,byte[]> Bodies=new Dictionary<string,byte[]>();
    public readonly List<string> Requests=new List<string>();
    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request,CancellationToken token) {
        string url=request.RequestUri.AbsoluteUri; Requests.Add(url);
        var response=new HttpResponseMessage(Bodies.ContainsKey(url)?HttpStatusCode.OK:HttpStatusCode.NotFound);
        response.Content=new ByteArrayContent(Bodies.ContainsKey(url)?Bodies[url]:new byte[0]);
        return Task.FromResult(response);
    }
}
'@
        if ($PSVersionTable.PSVersion.Major -ge 6) { Add-Type -TypeDefinition $source }
        else { Add-Type -ReferencedAssemblies System.Net.Http -TypeDefinition $source }
    }
    $script:Handler=[GrokMigrationTestHandler]::new()
    function New-OnlineHttpClient { return [Net.Http.HttpClient]::new($script:Handler, $false) }
    $script:Pin=Get-Content -LiteralPath (Join-Path $ArtifactsDirectory 'migration-pin.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    # Test the C root parser without creating SMB shares or changing host state.
    $rootTest=Join-Path $testRoot 'path-root.c'
    $cSource=(Join-Path $windows 'migration/bootstrap.c').Replace('\','/')
    Write-Utf8 $rootTest ((@'
#define wmain production_wmain
#include "SOURCE"
#undef wmain
int main(void) {
    return !(path_root_length(L"C:\\dir\\file") == 3 &&
        path_root_length(L"\\\\server\\share\\dir\\file") == 14 &&
        path_root_length(L"\\\\?\\UNC\\server\\share\\dir") == 20 &&
        path_root_length(L"\\\\?\\C:\\dir") == 7 &&
        path_root_length(L"relative\\file") == 0 && path_root_length(L"\\\\server") == 0);
}
'@).Replace('SOURCE',$cSource))
    $rootExe=Join-Path $testRoot 'path-root.exe'
    & $Gcc -std=c11 -O2 -Wall -Wextra -Werror -static -I $ArtifactsDirectory $rootTest -lbcrypt -o $rootExe
    if ($LASTEXITCODE -ne 0) { throw 'C 路径根测试构建失败。' }
    & $rootExe
    Assert-True ($LASTEXITCODE -eq 0) 'C路径根解析支持盘符、UNC及扩展路径，拒绝相对路径/不完整UNC'
    $helperBytes=[IO.File]::ReadAllBytes((Join-Path $ArtifactsDirectory $Pin.asset))
    $helperAsset=New-ReleaseAsset $Pin.tag $Pin.asset $helperBytes
    $helperRelease=[pscustomobject]@{tag_name=$Pin.tag; draft=$false; prerelease=$false; immutable=$true; assets=@($helperAsset)}
    $helperApi="https://api.github.com/repos/JoyElliot/grok-build-Chinese/releases/tags/$($Pin.tag)"
    $script:Handler.Bodies[$helperApi]=$utf8.GetBytes(($helperRelease | ConvertTo-Json -Depth 6))
    $script:Handler.Bodies[$helperAsset.browser_download_url]=$helperBytes

    $package=New-Work 'grok-zh-1.0.99-windows-x86_64-msvc'
    foreach ($name in $script:OnlinePackageFiles) {
        $path=Join-Path $package $name; $null=[IO.Directory]::CreateDirectory((Split-Path -Parent $path))
        if (Test-Path -LiteralPath (Join-Path $windows $name) -PathType Leaf) { Copy-Item -LiteralPath (Join-Path $windows $name) -Destination $path }
        else { Write-Utf8 $path "fixture $name" }
    }
    New-FakeExe (Join-Path $package 'grok-zh.exe') '1.0.99'
    Write-Utf8 (Join-Path $package 'BUILD-INFO.txt') "Version: 1.0.99`nTarget: x86_64-pc-windows-msvc`n"
    & python -B (Join-Path $repo '.github/scripts/write-package-protocol.py') --package $package --version 1.0.99 --platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'fixture 包协议生成失败。' }
    $lines=foreach ($name in $script:OnlinePackageFiles) { "$((Get-FileHash -LiteralPath (Join-Path $package $name) -Algorithm SHA256).Hash.ToLowerInvariant())  $name" }
    Write-Utf8 (Join-Path $package 'SHA256SUMS.txt') (($lines -join "`n")+"`n")
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zipPath=Join-Path $testRoot 'msvc.zip'
    $zip=[IO.Compression.ZipFile]::Open($zipPath,[IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($file in Get-ChildItem -LiteralPath $package -Recurse -File) {
            $relative=$file.FullName.Substring($package.Length+1).Replace('\','/')
            $entry=$zip.CreateEntry("grok-zh-1.0.99-windows-x86_64-msvc/$relative")
            $output=$entry.Open(); $inputStream=[IO.File]::OpenRead($file.FullName)
            try { $inputStream.CopyTo($output) } finally { $inputStream.Dispose(); $output.Dispose() }
        }
    } finally { $zip.Dispose() }
    $bytes=[IO.File]::ReadAllBytes($zipPath)
    $runtimeAsset=New-ReleaseAsset 'release-v1.0.99' 'grok-zh-1.0.99-windows-x86_64-msvc.zip' $bytes
    $runtimeRelease=[pscustomobject]@{tag_name='release-v1.0.99';draft=$false;prerelease=$false;immutable=$true;assets=@($runtimeAsset);body='fixture'}
    $runtimeApi='https://api.github.com/repos/JoyElliot/grok-build-Chinese/releases/tags/release-v1.0.99'
    $script:Handler.Bodies[$runtimeApi]=$utf8.GetBytes(($runtimeRelease | ConvertTo-Json -Depth 6))
    $script:Handler.Bodies[$runtimeAsset.browser_download_url]=$bytes

    # Compile the real C entry point against a TEST-ONLY embedded transport.
    # No production environment variable, URL override or trust bypass is added.
    $transportRoot=New-Work 'transport'
    $transportLog=Join-Path $transportRoot 'requests.txt'
    $routes=[ordered]@{}
    $index=0
    foreach ($url in $script:Handler.Bodies.Keys) {
        $bodyPath=Join-Path $transportRoot "body-$index"
        [IO.File]::WriteAllBytes($bodyPath,$script:Handler.Bodies[$url]); $routes[$url]=$bodyPath; $index++
    }
    $transportPath=Join-Path $transportRoot 'transport.json'
    Write-Utf8 $transportPath (([ordered]@{log=$transportLog;files=$routes}) | ConvertTo-Json -Depth 5)
    $cold=New-Work 'cold-native-launch'
    & python -B (Join-Path $repo '.github/scripts/tests/build-migration-fixture-launcher.py') --output $cold `
        --pin (Join-Path $ArtifactsDirectory 'migration-pin.json') --transport $transportPath --cc $Gcc
    if ($LASTEXITCODE -ne 0) { throw '首次迁移 C fixture 编译失败。' }
    $coldLauncher=Join-Path $cold 'grok-zh.exe'
    $result=Invoke-Launcher $coldLauncher '--version'
    Assert-True ($result.Code -eq 0 -and !(Test-Path -LiteralPath $transportLog)) '真正的未迁移启动器--version无下载副作用'
    $result=Invoke-Launcher $coldLauncher '"first startup"'
    Assert-True ($result.Code -eq 23 -and $result.Out.Contains('arg=Zmlyc3Qgc3RhcnR1cA==')) "真实进程从首启下载一直执行到新程序：$($result.Err)"
    Assert-True (@(Get-Content -LiteralPath $transportLog).Count -eq 4) '真实首启仅下载一次迁移包及同版本完整MSVC包'
    Assert-True (!$result.Out.Contains('安装') -and !$result.Out.Contains('正在')) '首次迁移安装日志全部走stderr'
    $oldTemp=$env:TEMP; $oldTmp=$env:TMP
    try {
        $env:TEMP=Join-Path $testRoot 'missing-temp'; $env:TMP=$env:TEMP
        $result=Invoke-Launcher $coldLauncher 'offline'
        Assert-True ($result.Code -eq 23 -and @(Get-Content -LiteralPath $transportLog).Count -eq 4) '迁移后即使TEMP不可用也直接运行MSVC，无PowerShell/网络依赖'
    } finally { $env:TEMP=$oldTemp; $env:TMP=$oldTmp }

    $parallel=New-Work 'parallel-first-launch'
    $parallelLauncher=Join-Path $parallel 'grok-zh.exe'
    Copy-Item -LiteralPath $coldLauncher -Destination $parallelLauncher
    Write-Utf8 $transportLog ''
    $processes=@()
    try {
        foreach ($argument in @('parallel-A','parallel-B')) {
            $start=[Diagnostics.ProcessStartInfo]::new()
            $start.FileName=$parallelLauncher; $start.Arguments=$argument; $start.WorkingDirectory=$testRoot
            $start.UseShellExecute=$false; $start.CreateNoWindow=$true
            $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
            $p=[Diagnostics.Process]::new(); $p.StartInfo=$start; $null=$p.Start()
            $processes+=@([pscustomobject]@{Process=$p;Out=$p.StandardOutput.ReadToEndAsync();Err=$p.StandardError.ReadToEndAsync()})
        }
        foreach ($item in $processes) {
            if (!$item.Process.WaitForExit(30000)) { $item.Process.Kill(); throw '并发首次启动超时。' }
            Assert-True ($item.Process.ExitCode -eq 23) "并发启动均继续执行MSVC：$($item.Err.GetAwaiter().GetResult())"
        }
        Assert-True (@(Get-Content -LiteralPath $transportLog).Count -eq 4) '两个真实进程并发首启仅执行一次迁移下载'
    } finally { foreach ($item in $processes) { $item.Process.Dispose() } }

    $context=New-TestContext 'installed'
    $originalHash=(Get-FileHash -LiteralPath $context.launcher -Algorithm SHA256).Hash
    Invoke-GrokBootstrap -Context $context -WorkDirectory (New-Work 'initial-download')
    Assert-True ($script:Handler.Requests.Count -eq 4) '首次仅请求固定迁移包和同版本MSVC包各自的API及附件'
    Assert-True ((Get-FileHash -LiteralPath $context.launcher -Algorithm SHA256).Hash -ceq $originalHash) '迁移不替换正在运行的GNU入口'
    $runtime=Join-Path (Split-Path -Parent $context.launcher) '.grok-zh-msvc'
    Assert-True (Test-MigrationRuntime $runtime '1.0.99') '完整MSVC运行目录通过校验'
    $requestCount=$script:Handler.Requests.Count
    Invoke-GrokBootstrap -Context $context -WorkDirectory (New-Work 'second-launch')
    Assert-True ($script:Handler.Requests.Count -eq $requestCount) '后续启动不下载过渡版或迁移包'
    Assert-True ((Get-Content -LiteralPath (Join-Path $env:GROK_HOME 'auth.json') -Raw) -ceq 'retain-auth') '共享账号数据保持不变'
    Assert-True ((Get-Content -LiteralPath (Join-Path (Split-Path -Parent $context.launcher) 'user-owned.txt') -Raw) -ceq 'retain') '现有目录的用户文件保留'
    Assert-True ($env:Path -ceq $oldPath -and [Environment]::GetEnvironmentVariable('Path','User') -ceq $userPath) '迁移不修改PATH'

    $result=Invoke-Launcher $context.launcher '"空 格" "literal&value" "quote\"value" "trailing\\" ""'
    Assert-True ($result.Code -eq 23) '真实GNU启动器透传子程序退出码'
    $argsOut=@($result.Out -split '\r?\n' | Where-Object { $_.StartsWith('arg=') } | ForEach-Object { [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($_.Substring(4))) })
    Assert-True ($argsOut.Count -eq 5 -and $argsOut[0] -ceq '空 格' -and $argsOut[1] -ceq 'literal&value' -and $argsOut[2] -ceq 'quote"value' -and $argsOut[3] -ceq 'trailing\' -and $argsOut[4] -ceq '') '参数含中文、空格、引号、尾反斜杠和空参数仍完整'
    Assert-True ($result.Out.Contains("cwd=$testRoot") -and $result.Err.Contains('fixture-stderr')) '原工作目录与标准错误保留'
    Assert-True (!$result.Out.Contains('迁移') -and !$result.Out.Contains('正在')) '迁移诊断不污染程序stdout协议'
    $stdinResult=Invoke-Launcher $context.launcher '--stdin' "input-pipe`nsecond-line`n"
    Assert-True ($stdinResult.Code -eq 0 -and $stdinResult.Out -ceq "input-pipe`nsecond-line`n") '标准输入管道原样转交MSVC子程序'
    New-FakeExe (Join-Path $runtime 'grok-zh.exe') '1.0.100'
    Invoke-GrokBootstrap -Context $context -WorkDirectory (New-Work 'future-launch')
    Assert-True ($script:Handler.Requests.Count -eq $requestCount) 'MSVC自行更新到更高版本后不重新迁移或降级'
    $result=Invoke-Launcher $context.launcher '--version'
    Assert-True ($result.Code -eq 0 -and $result.Out.Trim() -ceq 'grok-zh 1.0.100 (fixture)') '已安装入口的version显示实际MSVC版本且不联网'
    $candidate=Join-Path (Split-Path -Parent $context.launcher) 'candidate.exe'
    Copy-Item -LiteralPath $context.launcher -Destination $candidate
    $result=Invoke-Launcher $candidate '--version'
    Assert-True ($result.Code -eq 0 -and $result.Out.Contains('grok-zh 1.0.99 (GNU migration launcher)')) '旧更新器候选检查仍显示候选包版本且不触发迁移'

    $lockedContext=New-TestContext 'locked'
    $lock=[IO.File]::Open("$($lockedContext.launcher).update.lock",'OpenOrCreate','ReadWrite','None')
    try { Assert-Throws { Enter-MigrationLock "$($lockedContext.launcher).update.lock" -WaitMilliseconds 20 } '与Rust更新器同一FileShare.None锁竞争时有界等待后失败' } finally { $lock.Dispose() }
    Assert-True ($script:Handler.Requests.Count -eq $requestCount) '锁冲突没有网络副作用'
    $wrongRelease=$helperRelease | ConvertTo-Json -Depth 6 | ConvertFrom-Json
    $wrongRelease.immutable=$false
    Assert-Throws { Get-MigrationAssetContract $wrongRelease $Pin } '拒绝可变迁移Release'
    $wrongRelease.immutable=$true; $wrongRelease.assets[0].digest='sha256:'+('0'*64)
    Assert-Throws { Get-MigrationAssetContract $wrongRelease $Pin } '拒绝与编译固定摘要不一致的迁移包'
    $wrongRelease=$helperRelease | ConvertTo-Json -Depth 6 | ConvertFrom-Json
    $wrongRelease.assets=@($wrongRelease.assets[0],$wrongRelease.assets[0])
    Assert-Throws { Get-MigrationAssetContract $wrongRelease $Pin } '拒绝重复迁移附件'
    $failedContext=New-TestContext 'failed-download'
    $script:Handler.Bodies[$helperAsset.browser_download_url]=[byte[]]::new($helperBytes.Length)
    Assert-Throws { Invoke-GrokBootstrap $failedContext (New-Work 'failed-work') } '下载摘要失败终止迁移'
    Assert-True (!(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $failedContext.launcher) '.grok-zh-msvc'))) '失败不创建安装目录'
    Assert-True ((Get-FileHash -LiteralPath $failedContext.launcher -Algorithm SHA256).Hash -ceq $originalHash) '失败后兼容入口仍可重试'
    $script:Handler.Bodies[$helperAsset.browser_download_url]=$helperBytes
    Invoke-GrokBootstrap $failedContext (New-Work 'retry-work')
    Assert-True (Test-MigrationRuntime (Join-Path (Split-Path -Parent $failedContext.launcher) '.grok-zh-msvc') '1.0.99') '恢复正确下载后可重试完成'
    Write-Host "Windows migration: $script:Checks checks passed (fixture runtime; not an MSVC product build)."
} finally {
    $env:GROK_HOME=$oldHome
    if ($env:Path -cne $oldPath -or [Environment]::GetEnvironmentVariable('Path','User') -cne $userPath) { throw '测试意外修改了PATH，请检查。' }
    # Exact random child of TEMP created by this test; never touch project caches.
    $resolved=[IO.Path]::GetFullPath($testRoot)
    $tempParent=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
    if ((Split-Path -Parent $resolved) -ine $tempParent -or !(Split-Path -Leaf $resolved).StartsWith('grok-migration-test-')) { throw '测试清理目录边界错误。' }
    if (Test-Path -LiteralPath $resolved) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
