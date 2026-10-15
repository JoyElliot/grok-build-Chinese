[CmdletBinding()]
param([string]$Gcc = 'gcc')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$windows = Split-Path -Parent $PSScriptRoot
$repo = [IO.Path]::GetFullPath((Join-Path $windows '../..'))
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('grok-embedded-test-' + [guid]::NewGuid().ToString('N'))
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
        if (args.Length == 1 && (args[0] == "--version" || args[0] == "-V")) { Console.WriteLine("grok-zh VERSION (fixture)"); return 0; }
        if (args.Length == 1 && args[0] == "--hold") { Console.WriteLine("ready"); System.Threading.Thread.Sleep(5000); return 0; }
        if (args.Length == 1 && args[0] == "--stdin") {
            using (var bytes = new System.IO.MemoryStream()) {
                Console.OpenStandardInput().CopyTo(bytes);
                Console.Write(Convert.ToBase64String(bytes.ToArray()));
            }
            return 0;
        }
        foreach (string arg in args) Console.WriteLine("arg=" + Convert.ToBase64String(System.Text.Encoding.UTF8.GetBytes(arg)));
        Console.WriteLine("cwd=" + Environment.CurrentDirectory);
        Console.Error.WriteLine("fixture-stderr");
        return 23;
    }
}
'@.Replace('VERSION', $Version))
    & (Join-Path $env:SystemRoot 'Microsoft.NET/Framework64/v4.0.30319/csc.exe') /nologo /target:exe /platform:x64 "/out:$Destination" $source
    if ($LASTEXITCODE -ne 0) { throw 'fixture 编译失败。' }
}
function Invoke-Launcher([string]$Executable, [string]$Arguments, [byte[]]$InputBytes = @()) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName=$Executable; $info.Arguments=$Arguments; $info.WorkingDirectory=$testRoot
    $info.UseShellExecute=$false; $info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
    $info.RedirectStandardInput=($InputBytes.Length -gt 0)
    $info.StandardOutputEncoding=$utf8; $info.StandardErrorEncoding=$utf8
    $process=[Diagnostics.Process]::new(); $process.StartInfo=$info
    try {
        # .NET Framework eagerly creates an AutoFlush StreamWriter at Start;
        # even using BaseStream later cannot undo a BOM emitted at that point.
        $producerEncoding=[Console]::InputEncoding
        try {
            if ($InputBytes.Length -gt 0) { [Console]::InputEncoding=$utf8 }
            $null=$process.Start()
        } finally { [Console]::InputEncoding=$producerEncoding }
        $stdout=$process.StandardOutput.ReadToEndAsync(); $stderr=$process.StandardError.ReadToEndAsync()
        if ($InputBytes.Length -gt 0) {
            # .NET Framework's text writer inherits Console.InputEncoding and
            # may prepend a BOM. Test the launcher's pipe bytes independently
            # of either host's text encoding; the fixture also reads raw bytes.
            $inputStream=$process.StandardInput.BaseStream
            $inputStream.Write($InputBytes,0,$InputBytes.Length)
            $process.StandardInput.Close()
        }
        if (!$process.WaitForExit(30000)) { $process.Kill(); throw '真实启动器 fixture 超时。' }
        return [pscustomobject]@{ Code=$process.ExitCode; Out=$stdout.GetAwaiter().GetResult(); Err=$stderr.GetAwaiter().GetResult() }
    } finally { $process.Dispose() }
}
function New-LaunchRoot([string]$Name, [string]$Source) {
    $directory=Join-Path $testRoot $Name
    $null=[IO.Directory]::CreateDirectory($directory)
    Copy-Item -LiteralPath $Source -Destination (Join-Path $directory 'grok-zh.exe')
    Write-Utf8 (Join-Path $directory 'user-owned.txt') 'retain'
    return (Join-Path $directory 'grok-zh.exe')
}
function Start-FixtureProcess([string]$Executable) {
    $start=[Diagnostics.ProcessStartInfo]::new()
    $start.FileName=$Executable; $start.Arguments='normal'; $start.WorkingDirectory=$testRoot
    $start.UseShellExecute=$false; $start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $start.StandardOutputEncoding=$utf8; $start.StandardErrorEncoding=$utf8
    $process=[Diagnostics.Process]::new(); $process.StartInfo=$start
    $null=$process.Start()
    return [pscustomobject]@{ Process=$process; Out=$process.StandardOutput.ReadToEndAsync(); Err=$process.StandardError.ReadToEndAsync() }
}
$oldHome=$env:GROK_HOME; $oldPath=$env:Path; $oldTemp=$env:TEMP; $oldTmp=$env:TMP
$userPath=[Environment]::GetEnvironmentVariable('Path','User')
try {
    $null=[IO.Directory]::CreateDirectory($testRoot)
    $env:GROK_HOME=Join-Path $testRoot 'shared-home'
    $null=[IO.Directory]::CreateDirectory($env:GROK_HOME)
    Write-Utf8 (Join-Path $env:GROK_HOME 'preserve.txt') 'retain-home'
    $native=Join-Path $testRoot 'native-package'
    $null=[IO.Directory]::CreateDirectory($native)
    $files=@('grok-zh.exe','agent-zh.cmd','rg.exe','一键安装.cmd','[可选]替换原始启动方式.cmd',
        'Install-GrokZh.ps1','INSTALL-WINDOWS.md','LICENSE-grok-build.txt','BUILD-INFO.txt',
        'licenses/ripgrep/COPYING','licenses/ripgrep/LICENSE-MIT','licenses/ripgrep/UNLICENSE',
        'licenses/project/THIRD-PARTY-NOTICES','licenses/project/THIRD_PARTY_NOTICES.md','licenses/project/NOTICE', 'future-resource.dat')
    foreach ($name in $files) {
        $path=Join-Path $native $name
        $null=[IO.Directory]::CreateDirectory((Split-Path -Parent $path))
        Write-Utf8 $path "fixture $name"
    }
    Copy-Item -LiteralPath (Join-Path $windows 'Install-GrokZh.ps1') -Destination (Join-Path $native 'Install-GrokZh.ps1') -Force
    New-FakeExe (Join-Path $native 'grok-zh.exe') '1.0.99'
    Write-Utf8 (Join-Path $native 'BUILD-INFO.txt') "Grok Build Windows MSVC fixture`nVersion: 1.0.99`nTarget: x86_64-pc-windows-msvc`nProfile: fixture`n"
    & python -B (Join-Path $repo '.github/scripts/write-package-protocol.py') --package $native --version 1.0.99 --platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'fixture 协议生成失败。' }
    $lines=foreach ($name in $files) { "$((Get-FileHash -LiteralPath (Join-Path $native $name) -Algorithm SHA256).Hash.ToLowerInvariant())  $name" }
    Write-Utf8 (Join-Path $native 'SHA256SUMS.txt') (($lines -join "`n")+"`n")
    $artifacts=Join-Path $testRoot 'artifacts'
    & python -B (Join-Path $repo '.github/scripts/build-windows-embedded-bootstrap.py') --output $artifacts --version 1.0.99 --payload-package $native --cc $Gcc
    if ($LASTEXITCODE -ne 0) { throw '内嵌 GNU fixture 编译失败。' }
    $source=Join-Path $artifacts 'grok-zh.exe'
    $launcher=New-LaunchRoot 'first-launch' $source
    $root=Split-Path -Parent $launcher
    $runtime=Join-Path $root '.grok-zh-msvc'
    $env:TEMP=Join-Path $testRoot 'missing-temp'; $env:TMP=$env:TEMP
    $clock=[Diagnostics.Stopwatch]::StartNew()
    $pure=Invoke-Launcher $launcher '--version'
    Assert-True ($pure.Code -eq 0 -and $pure.Out.Contains('1.0.99 (GNU migration launcher)')) '纯版本输出'
    Assert-True ($clock.Elapsed.TotalSeconds -lt 10 -and !(Test-Path -LiteralPath $runtime)) '版本检查不依赖 TEMP 或触发迁移'
    $candidate=Join-Path $root 'candidate.exe'
    Copy-Item -LiteralPath $source -Destination $candidate
    Assert-True ((Invoke-Launcher $candidate '-V').Code -eq 0) '旧更新器候选文件可以检查版本'
    Assert-True ((Invoke-Launcher $candidate 'normal').Code -ne 0) '非规范文件名不执行迁移'
    $env:TEMP=$oldTemp; $env:TMP=$oldTmp
    $first=Invoke-Launcher $launcher 'one "中文 空格" ""'
    Assert-True ($first.Code -eq 23) '原应用退出码透传'
    Assert-True ($first.Out.Contains('arg=b25l') -and $first.Out.Contains('arg=')) '参数透传包含空参数'
    Assert-True ($first.Out.Contains("cwd=$testRoot") -and !$first.Out.Contains('离线迁移')) '工作目录保留且迁移提示不污染 stdout'
    Assert-True ($first.Err.Contains('fixture-stderr')) 'stderr 保留'
    Assert-True ((Get-Content -LiteralPath (Join-Path $runtime '.grok-zh-bootstrap-ready') -Raw) -ceq "1.0.99`nwindows-x64-gnu-to-msvc-embedded-v1`n") '最后提交 ready'
    Assert-True (Test-Path -LiteralPath (Join-Path $runtime 'future-resource.dat')) '完整内层包资源被安装'
    Assert-True ((Get-FileHash -LiteralPath (Join-Path $runtime 'grok-zh.exe')).Hash -ceq (Get-FileHash -LiteralPath (Join-Path $native 'grok-zh.exe')).Hash) '实际 MSVC 候选被安装'
    [byte[]]$inputBytes=@(0,1,2,10,13,127,128,255)
    $env:TEMP=Join-Path $testRoot 'missing-temp'; $env:TMP=$env:TEMP
    $stdin=Invoke-Launcher $launcher '--stdin' $inputBytes
    Assert-True ($stdin.Code -eq 0 -and $stdin.Out -ceq [Convert]::ToBase64String($inputBytes)) 'ready 快路径不依赖 TEMP 且保留 stdin 字节'
    foreach ($next in @('1.0.100','1.0.101')) {
        $nextExe=Join-Path $testRoot "next-$next.exe"
        New-FakeExe $nextExe $next
        Copy-Item -LiteralPath $nextExe -Destination (Join-Path $runtime 'grok-zh.exe') -Force
        $nextVersion=Invoke-Launcher $launcher '--version'
        Assert-True ($nextVersion.Out.Contains("grok-zh $next ")) '保留原入口跨两次原生更新报告子程序版本'
        Assert-True ((Invoke-Launcher $launcher 'normal').Code -eq 23) '原入口继续启动后续 MSVC 版本'
        Assert-True ((Invoke-Launcher $candidate '--version').Out.Contains('1.0.99 (GNU migration launcher)')) '候选版本不误用邻近新运行时'
    }
    $env:TEMP=$oldTemp; $env:TMP=$oldTmp
    $broken=New-LaunchRoot 'damaged' $source
    $stream=[IO.File]::Open($broken,'Open','ReadWrite','None')
    try { $null=$stream.Seek(-64,[IO.SeekOrigin]::End); $stream.WriteByte(0) } finally { $stream.Dispose() }
    $failure=Invoke-Launcher $broken 'normal'
    Assert-True ($failure.Code -ne 0 -and !(Test-Path -LiteralPath (Join-Path (Split-Path -Parent $broken) '.grok-zh-msvc/.grok-zh-bootstrap-ready'))) '损坏内嵌包无 ready 且可恢复'
    Copy-Item -LiteralPath $source -Destination $broken -Force
    Assert-True ((Invoke-Launcher $broken 'normal').Code -eq 23) '修复入口后可重试'
    $parallel=New-LaunchRoot 'parallel-first-launch' $source
    $firstProcess=Start-FixtureProcess $parallel
    $secondProcess=Start-FixtureProcess $parallel
    try {
        foreach ($item in @($firstProcess,$secondProcess)) {
            if (!$item.Process.WaitForExit(30000)) { $item.Process.Kill(); throw '并发首启超时。' }
            $errorText=$item.Err.GetAwaiter().GetResult(); $null=$item.Out.GetAwaiter().GetResult()
            Assert-True ($item.Process.ExitCode -eq 23) "两个并发首启均启动原生程序：$errorText"
        }
    } finally { $firstProcess.Process.Dispose(); $secondProcess.Process.Dispose() }
    $parallelRuntime=Join-Path (Split-Path -Parent $parallel) '.grok-zh-msvc'
    Assert-True ((Get-Content -LiteralPath (Join-Path $parallelRuntime '.grok-zh-bootstrap-ready') -Raw) -ceq "1.0.99`nwindows-x64-gnu-to-msvc-embedded-v1`n") '并发首启只提交完整 ready'
    # An interruption after full installer commit but before ready must recover
    # from the owned native installation rather than overwrite it.
    Remove-Item -LiteralPath (Join-Path $parallelRuntime '.grok-zh-bootstrap-ready')
    Write-Utf8 (Join-Path $parallelRuntime 'preserve-recovery.txt') 'retain-recovery'
    Assert-True ((Invoke-Launcher $parallel 'normal').Code -eq 23) '安装提交后中断可恢复 ready'
    Assert-True ((Get-Content -LiteralPath (Join-Path $parallelRuntime 'preserve-recovery.txt') -Raw) -ceq 'retain-recovery') '恢复不覆盖已提交目录'
    $incomplete=New-LaunchRoot 'unowned-interruption' $source
    $incompleteRuntime=Join-Path (Split-Path -Parent $incomplete) '.grok-zh-msvc'
    $null=[IO.Directory]::CreateDirectory($incompleteRuntime)
    Write-Utf8 (Join-Path $incompleteRuntime 'personal.txt') 'retain-unowned'
    Assert-True ((Invoke-Launcher $incomplete 'normal').Code -ne 0) '不覆盖无归属的中断目录'
    Assert-True ((Get-Content -LiteralPath (Join-Path $incompleteRuntime 'personal.txt') -Raw) -ceq 'retain-unowned') '无归属目录文件保留'
    # Load the exact offline library emitted into the GNU executable, then
    # exercise the bounded lock waiter with a real exclusive file handle.
    $library=Join-Path $testRoot 'library'; $null=[IO.Directory]::CreateDirectory($library)
    Copy-Item -LiteralPath (Join-Path $windows 'Install-GrokZh.ps1') -Destination $library
    Copy-Item -LiteralPath (Join-Path $windows 'migration/Invoke-EmbeddedMigration.ps1') -Destination $library
    $writeLibrary=@'
import importlib.util,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location('builder',sys.argv[1]); b=importlib.util.module_from_spec(spec); spec.loader.exec_module(b)
Path(sys.argv[2]).write_bytes(b.offline_validation_bytes())
'@
    $writeLibrary | python -B - (Join-Path $repo '.github/scripts/build-windows-embedded-bootstrap.py') (Join-Path $library 'Embedded-Validation.ps1')
    if ($LASTEXITCODE -ne 0) { throw '离线校验库输出失败。' }
    . (Join-Path $library 'Invoke-EmbeddedMigration.ps1') -ContextPath 'unused'
    $lockPath=Join-Path $testRoot 'held.update.lock'
    $held=[IO.File]::Open($lockPath,'CreateNew','ReadWrite','None')
    try {
        $wait=[Diagnostics.Stopwatch]::StartNew()
        Assert-Throws { Enter-MigrationLock -Path $lockPath -WaitMilliseconds 250 } '锁等待有界退出'
        Assert-True ($wait.Elapsed.TotalSeconds -lt 5) '锁等待没有 20 分钟静默'
    } finally { $held.Dispose() }
    Assert-True ((Get-Content -LiteralPath (Join-Path $root 'user-owned.txt') -Raw) -ceq 'retain') '原目录个人文件不变'
    Assert-True ((Get-Content -LiteralPath (Join-Path $env:GROK_HOME 'preserve.txt') -Raw) -ceq 'retain-home') 'GROK_HOME 不变'
    Assert-True ([Environment]::GetEnvironmentVariable('Path','User') -ceq $userPath) '用户 Path 不变'
    Write-Host "Embedded offline migration fixture checks passed: $script:Checks"
} finally {
    $env:GROK_HOME=$oldHome; $env:Path=$oldPath; $env:TEMP=$oldTemp; $env:TMP=$oldTmp
    if (Test-Path -LiteralPath $testRoot) { Remove-Item -LiteralPath $testRoot -Recurse -Force }
}
