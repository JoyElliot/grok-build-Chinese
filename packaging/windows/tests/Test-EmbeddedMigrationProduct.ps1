[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$ArtifactsDirectory,
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [string]$Gcc = 'gcc')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$info=Get-Content -LiteralPath (Join-Path $PackageDirectory 'BUILD-INFO.txt') -Raw -Encoding UTF8
if ($info -cnotmatch '(?m)^Version:[ \t]*(\S+)[ \t]*\r?$') { throw '产品包缺少版本。' }
$testVersion=$matches[1]
$parent=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
$work=Join-Path $parent ('grok-embedded-product-test-'+[guid]::NewGuid().ToString('N'))
$utf8=[Text.UTF8Encoding]::new($false)
$oldHome=$env:GROK_HOME; $oldPath=$env:Path; $oldTemp=$env:TEMP; $oldTmp=$env:TMP
$userPath=[Environment]::GetEnvironmentVariable('Path','User')
function Run-Product([string]$Exe,[string]$Arguments) {
    $start=[Diagnostics.ProcessStartInfo]::new()
    $start.FileName=$Exe; $start.Arguments=$Arguments; $start.UseShellExecute=$false; $start.CreateNoWindow=$true
    $start.WorkingDirectory=$work
    $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $start.StandardOutputEncoding=$utf8; $start.StandardErrorEncoding=$utf8
    $p=[Diagnostics.Process]::new(); $p.StartInfo=$start
    $watch=[Diagnostics.Stopwatch]::StartNew()
    try {
        $null=$p.Start(); $stdout=$p.StandardOutput.ReadToEndAsync(); $stderr=$p.StandardError.ReadToEndAsync()
        if (!$p.WaitForExit(180000)) { $p.Kill(); throw '真实产品离线迁移/冒烟超过 180 秒。' }
        $out=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
        if ($p.ExitCode -ne 0) { throw "产品迁移/冒烟失败：$Arguments / $($p.ExitCode)`n$err" }
        return [pscustomobject]@{ Out=$out; Err=$err; Milliseconds=$watch.ElapsedMilliseconds }
    } finally { $p.Dispose() }
}
try {
    $null=[IO.Directory]::CreateDirectory($work)
    $env:GROK_HOME=Join-Path $work 'shared-home'; $null=[IO.Directory]::CreateDirectory($env:GROK_HOME)
    [IO.File]::WriteAllText((Join-Path $env:GROK_HOME 'preserve.txt'),'retain',$utf8)
    $gnuArchive=Join-Path $ArtifactsDirectory "grok-zh-$testVersion-windows-x86_64-gnu.zip"
    $installed=Join-Path $work 'legacy-entry'; $null=[IO.Directory]::CreateDirectory($installed)
    $launcher=Join-Path $installed 'grok-zh.exe'
    # Exercise the old executable-only boundary: validate the complete GNU
    # transport and embedded native package, but install only the GNU entry.
    $extract=@'
import importlib.util, shutil, sys, zipfile
from pathlib import Path
spec=importlib.util.spec_from_file_location('bundle',sys.argv[1]); b=importlib.util.module_from_spec(spec); spec.loader.exec_module(b)
result=b.validate_compat_archive(Path(sys.argv[2]),sys.argv[3])
with zipfile.ZipFile(sys.argv[2]) as z, z.open(result['outer']['root']+'/grok-zh.exe') as src, open(sys.argv[4],'xb') as dst:
    shutil.copyfileobj(src,dst,1024*1024)
'@
    $extract | python -B - (Join-Path $repo '.github/scripts/embedded-msvc-bundle.py') $gnuArchive $testVersion $launcher
    if ($LASTEXITCODE -ne 0) { throw '完整兼容包/内嵌包校验或 EXE 提取失败。' }
    [IO.File]::WriteAllText((Join-Path $installed 'personal.txt'),'retain-original',$utf8)
    $runtime=Join-Path $installed '.grok-zh-msvc'
    $env:TEMP=Join-Path $work 'nonexistent-temp'; $env:TMP=$env:TEMP
    $pure=Run-Product $launcher '--version'
    if (!$pure.Out.Contains("grok-zh $testVersion (GNU migration launcher)") -or
        $pure.Milliseconds -ge 20000 -or (Test-Path -LiteralPath $runtime)) { throw '候选版本检查必须纯读取并为旧更新器 30 秒期限留余量。' }
    $env:TEMP=$oldTemp; $env:TMP=$oldTmp
    # No compiler/toolchain directory on the execution PATH. The launcher uses
    # the trusted system PowerShell path and its embedded offline-only helpers.
    $env:Path="$env:SystemRoot\System32;$env:SystemRoot"
    $first=Run-Product $launcher '--help'
    if (!$first.Out -or !$first.Err.Contains('离线迁移完成')) { throw '首次正常启动未完成有提示的离线迁移。' }
    $actual=Join-Path $runtime 'grok-zh.exe'
    if ((Get-FileHash -LiteralPath $actual -Algorithm SHA256).Hash -cne
        (Get-FileHash -LiteralPath (Join-Path $PackageDirectory 'grok-zh.exe') -Algorithm SHA256).Hash) { throw '迁移后的真实程序与本轮 MSVC 产物不同。' }
    $ready=Join-Path $runtime '.grok-zh-bootstrap-ready'
    if ((Get-Content -LiteralPath $ready -Raw) -cne "$testVersion`nwindows-x64-gnu-to-msvc-embedded-v1`n") { throw 'ready 未在最终提交。' }
    $readyTime=(Get-Item -LiteralPath $ready).LastWriteTimeUtc
    $env:TEMP=Join-Path $work 'nonexistent-temp'; $env:TMP=$env:TEMP
    $warm=Run-Product $launcher '--help'
    foreach ($arguments in @('--version','agent --help','update --help')) {
        $result=Run-Product $launcher $arguments
        if ($result.Err.Contains('正在离线') -or $result.Err.Contains('离线迁移完成')) { throw 'ready 快路径重复解压了 payload。' }
    }
    if ((Get-Item -LiteralPath $ready).LastWriteTimeUtc -ne $readyTime -or $warm.Err.Contains('离线迁移完成')) { throw '后续启动重复执行迁移。' }
    if ((Get-Content -LiteralPath (Join-Path $installed 'personal.txt') -Raw) -cne 'retain-original' -or
        (Get-Content -LiteralPath (Join-Path $env:GROK_HOME 'preserve.txt') -Raw) -cne 'retain' -or
        [Environment]::GetEnvironmentVariable('Path','User') -cne $userPath) { throw '迁移改动了用户文件或用户 Path。' }
    Write-Host "Real offline MSVC migration passed: $testVersion; pure version $($pure.Milliseconds) ms; first launch $($first.Milliseconds) ms; warm launch $($warm.Milliseconds) ms."
    Write-Host 'GNU executable-only transport -> verified embedded full MSVC package -> real MSVC runtime; four isolated CLI smokes; ready path works without TEMP.'
} finally {
    $env:GROK_HOME=$oldHome; $env:Path=$oldPath; $env:TEMP=$oldTemp; $env:TMP=$oldTmp
    if ((Split-Path -Parent ([IO.Path]::GetFullPath($work))) -ine $parent -or !(Split-Path -Leaf $work).StartsWith('grok-embedded-product-test-')) { throw '清理边界无效。' }
    if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force }
}
