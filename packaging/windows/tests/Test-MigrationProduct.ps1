[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$ArtifactsDirectory,
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [string]$PreviousPackageDirectory,
    [string]$Gcc = 'gcc')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
. (Join-Path $repo 'packaging/windows/Install-GrokZhOnline.ps1')
$info=Get-Content -LiteralPath (Join-Path $PackageDirectory 'BUILD-INFO.txt') -Raw -Encoding UTF8
if ($info -notmatch '(?m)^Version:\s*(\S+)\s*$') { throw '产品包缺少版本。' }
$testVersion=$matches[1]
$pin=Get-Content -LiteralPath (Join-Path $ArtifactsDirectory 'migration-pin.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$parent=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\','/')
$work=Join-Path $parent ('grok-product-migration-test-'+[guid]::NewGuid().ToString('N'))
$utf8=[Text.UTF8Encoding]::new($false)
$oldHome=$env:GROK_HOME
$oldPath=$env:Path
$userPath=[Environment]::GetEnvironmentVariable('Path','User')
function Run-Product([string]$Exe,[string]$Arguments) {
    $start=[Diagnostics.ProcessStartInfo]::new()
    $start.FileName=$Exe; $start.Arguments=$Arguments; $start.UseShellExecute=$false; $start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
    $start.StandardOutputEncoding=$utf8; $start.StandardErrorEncoding=$utf8
    $p=[Diagnostics.Process]::new(); $p.StartInfo=$start
    try {
        $null=$p.Start(); $stdout=$p.StandardOutput.ReadToEndAsync(); $stderr=$p.StandardError.ReadToEndAsync()
        if (!$p.WaitForExit(180000)) { $p.Kill(); throw '产品迁移/冒烟超时。' }
        $out=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
        if ($p.ExitCode -ne 0) { throw "产品迁移/冒烟失败：$Arguments / $($p.ExitCode)`n$err" }
        return $out
    } finally { $p.Dispose() }
}
function Asset([string]$Tag,[string]$Name,[string]$File) {
    return [ordered]@{name=$Name;state='uploaded';size=(Get-Item -LiteralPath $File).Length;
        digest='sha256:'+(Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash.ToLowerInvariant();
        browser_download_url="https://github.com/JoyElliot/grok-build-Chinese/releases/download/$Tag/$Name"}
}
try {
    $null=[IO.Directory]::CreateDirectory($work)
    $env:GROK_HOME=Join-Path $work 'shared-home'; $null=[IO.Directory]::CreateDirectory($env:GROK_HOME)
    [IO.File]::WriteAllText((Join-Path $env:GROK_HOME 'preserve.txt'),'retain',$utf8)
    $routes=[ordered]@{}
    $helper=Join-Path $ArtifactsDirectory $pin.asset
    $helperAsset=Asset $pin.tag $pin.asset $helper
    $helperMetadata=Join-Path $work 'helper-release.json'
    [IO.File]::WriteAllText($helperMetadata,([ordered]@{tag_name=$pin.tag;immutable=$true;draft=$false;prerelease=$false;assets=@($helperAsset)}|ConvertTo-Json -Depth 6),$utf8)
    $routes["https://api.github.com/repos/JoyElliot/grok-build-Chinese/releases/tags/$($pin.tag)"]=$helperMetadata
    $routes[$helperAsset.browser_download_url]=$helper
    $archiveName="grok-zh-$testVersion-windows-x86_64-msvc.zip"
    $archive=Join-Path (Split-Path -Parent $PackageDirectory) $archiveName
    $asset=Asset "release-v$testVersion" $archiveName $archive
    $metadata=Join-Path $work 'msvc-release.json'
    [IO.File]::WriteAllText($metadata,([ordered]@{tag_name="release-v$testVersion";immutable=$true;draft=$false;prerelease=$false;body='CI experiment; not a published Release';assets=@($asset)}|ConvertTo-Json -Depth 6),$utf8)
    $routes["https://api.github.com/repos/JoyElliot/grok-build-Chinese/releases/tags/release-v$testVersion"]=$metadata
    $routes[$asset.browser_download_url]=$archive
    $previousVersion=$null
    if ($PreviousPackageDirectory) {
        $previousInfo=Get-Content -LiteralPath (Join-Path $PreviousPackageDirectory 'BUILD-INFO.txt') -Raw -Encoding UTF8
        if ($previousInfo -notmatch '(?m)^Version:\s*(\S+)\s*$') { throw '旧产品包缺少版本。' }
        $previousVersion=$matches[1]
        if ((Compare-OnlineVersion (ConvertTo-OnlineVersion $previousVersion) (ConvertTo-OnlineVersion $testVersion)) -ge 0) { throw '旧产品必须低于新产品版本。' }
        $previousName="grok-zh-$previousVersion-windows-x86_64-msvc.zip"
        $previousArchive=Join-Path (Split-Path -Parent $PreviousPackageDirectory) $previousName
        $previousAsset=Asset "release-v$previousVersion" $previousName $previousArchive
        $previousMetadata=Join-Path $work 'previous-release.json'
        [IO.File]::WriteAllText($previousMetadata,([ordered]@{tag_name="release-v$previousVersion";immutable=$true;draft=$false;prerelease=$false;body='CI experiment';assets=@($previousAsset)}|ConvertTo-Json -Depth 6),$utf8)
        $routes["https://api.github.com/repos/JoyElliot/grok-build-Chinese/releases/tags/release-v$previousVersion"]=$previousMetadata
        $routes[$previousAsset.browser_download_url]=$previousArchive
    }
    $log=Join-Path $work 'requests.txt'
    $transport=Join-Path $work 'transport.json'
    [IO.File]::WriteAllText($transport,([ordered]@{log=$log;files=$routes}|ConvertTo-Json -Depth 5),$utf8)
    $fixture=Join-Path $work 'fixture-launcher'
    & python -B (Join-Path $repo '.github/scripts/tests/build-migration-fixture-launcher.py') --output $fixture `
        --pin (Join-Path $ArtifactsDirectory 'migration-pin.json') --transport $transport --version $testVersion --cc $Gcc
    if ($LASTEXITCODE -ne 0) { throw '离线网络 fixture 启动器构建失败。' }

    # Use the real compatibility ZIP builder. Then emulate the released
    # updater's exact boundary: validate the entire archive, copy only its EXE,
    # run --version, and let a later normal launch perform the migration.
    $builder=Join-Path $repo '.github/scripts/build-windows-migration-bootstrap.py'
    $buildFixture=@'
import importlib.util, sys
from pathlib import Path
spec=importlib.util.spec_from_file_location('b',sys.argv[1]); b=importlib.util.module_from_spec(spec); spec.loader.exec_module(b)
b.build_compat_package(Path(sys.argv[2]),Path(sys.argv[3]),sys.argv[4])
'@
    $buildFixture | python -B - $builder $fixture $PackageDirectory $testVersion
    if ($LASTEXITCODE -ne 0) { throw '兼容ZIP构建失败。' }
    $gnuName="grok-zh-$testVersion-windows-x86_64-gnu.zip"
    $gnuArchive=Join-Path $fixture $gnuName
    $gnuAsset=Asset "release-v$testVersion" $gnuName $gnuArchive
    $release=[pscustomobject]@{tag_name="release-v$testVersion";immutable=$true;draft=$false;prerelease=$false;body='fixture';assets=@([pscustomobject]$gnuAsset)}
    $contract=Get-OnlineReleaseContract $release
    $unpacked=Expand-VerifiedOnlinePackage $gnuArchive (Join-Path $work 'gnu-verified') $contract
    $installed=Join-Path $work 'old-installed'; $null=[IO.Directory]::CreateDirectory($installed)
    $launcher=Join-Path $installed 'grok-zh.exe'
    $expectedRequests=4
    if ($previousVersion) {
        & python -B (Join-Path $repo '.github/scripts/tests/build-migration-fixture-launcher.py') --output $installed `
            --pin (Join-Path $ArtifactsDirectory 'migration-pin.json') --transport $transport --version $previousVersion --cc $Gcc
        if ($LASTEXITCODE -ne 0) { throw '旧产品启动器构建失败。' }
        $null=Run-Product $launcher '--help'
        if (!(Run-Product $launcher '--version').Contains("grok-zh $previousVersion ") -or @(Get-Content -LiteralPath $log).Count -ne 4) { throw '真实旧MSVC初始迁移失败。' }
        [IO.File]::WriteAllText((Join-Path $installed '.grok-zh-msvc/personal.txt'),'retain-runtime-data',$utf8)
        $expectedRequests=8
    }
    Copy-Item -LiteralPath (Join-Path $unpacked 'grok-zh.exe') -Destination $launcher
    $versionOutput=Run-Product $launcher '--version'
    $beforeRequests=if (Test-Path -LiteralPath $log) { @(Get-Content -LiteralPath $log).Count } else { 0 }
    if (!$versionOutput.Contains("grok-zh $testVersion (GNU migration launcher)") -or $beforeRequests -ne ($expectedRequests-4)) { throw '旧更新器版本检查触发了迁移或版本不匹配。' }
    # All toolchain directories are absent from PATH for native CLI acceptance.
    $env:Path="$env:SystemRoot\System32;$env:SystemRoot"
    $help=Run-Product $launcher '--help'
    if (!$help -or @(Get-Content -LiteralPath $log).Count -ne $expectedRequests) { throw '迁移/恢复升级未完整执行，或发生重复下载。' }
    $runtime=Join-Path $installed '.grok-zh-msvc/grok-zh.exe'
    if ((Get-FileHash -LiteralPath $runtime -Algorithm SHA256).Hash -cne (Get-FileHash -LiteralPath (Join-Path $PackageDirectory 'grok-zh.exe') -Algorithm SHA256).Hash) { throw '迁移后的实际EXE与本轮MSVC产物不同。' }
    foreach ($arguments in @('--version','--help','agent --help','update --help')) { $null=Run-Product $launcher $arguments }
    if (@(Get-Content -LiteralPath $log).Count -ne $expectedRequests) { throw '后续CLI启动又下载了迁移包。' }
    if ($previousVersion -and (Get-Content -LiteralPath (Join-Path $installed '.grok-zh-msvc/personal.txt') -Raw) -cne 'retain-runtime-data') { throw '恢复升级未保留运行目录个人文件。' }
    if ((Get-Content -LiteralPath (Join-Path $env:GROK_HOME 'preserve.txt') -Raw) -cne 'retain' -or [Environment]::GetEnvironmentVariable('Path','User') -cne $userPath) { throw '迁移改动了用户数据或用户PATH。' }
    Write-Host "Real MSVC product migration passed: $testVersion; GNU EXE-only ZIP -> pinned helper -> verified MSVC package -> 4 isolated CLI smokes; no repeat download."
    if ($previousVersion) { Write-Host "Real MSVC launcher replacement recovery passed: $previousVersion -> $testVersion; existing runtime and user files retained." }
} finally {
    $env:GROK_HOME=$oldHome; $env:Path=$oldPath
    if ((Split-Path -Parent ([IO.Path]::GetFullPath($work))) -ine $parent -or !(Split-Path -Leaf $work).StartsWith('grok-product-migration-test-')) { throw '清理边界无效。' }
    if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force }
}
