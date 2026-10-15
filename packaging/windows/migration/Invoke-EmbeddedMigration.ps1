# Offline first-launch transaction. The compressed native ZIP is already inside
# the authenticated GNU launcher; no network helper or environment trust bypass.
[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$ContextPath)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSHOME 'Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1') -ErrorAction Stop
Import-Module (Join-Path $PSHOME 'Modules/Microsoft.PowerShell.Management/Microsoft.PowerShell.Management.psd1') -ErrorAction Stop
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
. (Join-Path $PSScriptRoot 'Embedded-Validation.ps1')
. (Join-Path $PSScriptRoot 'Install-GrokZh.ps1') -LibraryOnly

function Enter-MigrationLock {
    param([string]$Path, [int]$WaitMilliseconds = 120000)
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $lastNotice = -10000L
    while ($true) {
        Assert-OnlinePathChain $Path
        try { return [IO.File]::Open($Path, 'OpenOrCreate', 'ReadWrite', 'None') }
        catch [IO.IOException] {
            if (($_.Exception.HResult -band 0xFFFF) -notin @(32, 33)) { throw }
            if ($watch.ElapsedMilliseconds -ge $WaitMilliseconds) { throw '等待另一个安装或更新进程超时；现有文件未覆盖，请稍后重试。' }
            if ($watch.ElapsedMilliseconds - $lastNotice -ge 5000) {
                [Console]::Error.WriteLine('正在等待另一个安装或更新进程；可按 Ctrl+C 取消（最多等待 120 秒）。')
                $lastNotice = $watch.ElapsedMilliseconds
            }
            Start-Sleep -Milliseconds 200
        }
    }
}

function Set-MigrationReady {
    param([string]$Directory, [string]$Version)
    $path = Join-Path $Directory '.grok-zh-bootstrap-ready'
    Assert-OnlinePathChain $path
    $content = "$Version`nwindows-x64-gnu-to-msvc-embedded-v1`n"
    if ((Test-Path -LiteralPath $path) -and [IO.File]::ReadAllText($path) -ceq $content) { return }
    $temporary = "$path.$([guid]::NewGuid().ToString('N')).tmp"
    try {
        $bytes = [Text.Encoding]::ASCII.GetBytes($content)
        $stream = [IO.File]::Open($temporary, 'CreateNew', 'Write', 'None')
        try { $stream.Write($bytes, 0, $bytes.Length); $stream.Flush($true) } finally { $stream.Dispose() }
        if (Test-Path -LiteralPath $path) { [IO.File]::Replace($temporary, $path, [NullString]::Value) }
        else { [IO.File]::Move($temporary, $path) }
    } finally { if (Test-Path -LiteralPath $temporary) { [IO.File]::Delete($temporary) } }
}

function Test-MigrationRuntime {
    param([string]$Directory, [string]$MinimumVersion, [switch]$LockHeld)
    if (!(Test-Path -LiteralPath $Directory)) { return $false }
    $marker = Get-OnlineInstallMarker $Directory
    if (!$marker) { return $false }
    $infoPath = Join-Path $Directory 'BUILD-INFO.txt'
    Assert-OnlinePathChain $infoPath
    $info = ConvertFrom-OnlineUtf8 ([IO.File]::ReadAllBytes($infoPath))
    if ($info -cnotmatch '(?m)^Target:[ \t]*x86_64-pc-windows-msvc[ \t]*\r?$') { throw '迁移运行目录不是 Windows x64 MSVC。' }
    $exe = Join-Path $Directory 'grok-zh.exe'
    $lock = $null
    try {
        if (!$LockHeld) { $lock = Enter-MigrationLock "$exe.update.lock" }
        $actual = Get-OnlineExecutableVersion $exe
        if ($actual.DisplayText.Contains('(GNU migration launcher)')) { throw '运行目录仍然是兼容启动器，拒绝递归。' }
        return (Compare-OnlineVersion $actual (ConvertTo-OnlineVersion $MinimumVersion -StableOnly)) -ge 0
    } finally { if ($lock) { $lock.Dispose() } }
}

function Invoke-GrokEmbeddedMigration {
    param($Context, [string]$WorkDirectory)
    $version = [string](Get-OnlineProperty $Context 'version')
    $null = ConvertTo-OnlineVersion $version -StableOnly
    $size = Get-OnlineProperty $Context 'archive_size'
    $digest = Get-OnlineProperty $Context 'archive_sha256'
    if (($size -isnot [int] -and $size -isnot [long]) -or $size -le 0 -or $size -gt 536870912 -or
        $digest -isnot [string] -or $digest -cnotmatch '^[0-9a-f]{64}$') { throw '内嵌 MSVC 固定摘要或大小无效。' }
    $launcher = Resolve-OnlinePath ([string](Get-OnlineProperty $Context 'launcher'))
    if ((Split-Path -Leaf $launcher) -ine 'grok-zh.exe') { throw '迁移启动器必须保留 grok-zh.exe 文件名。' }
    Assert-OnlinePathChain $launcher
    $runtime = Join-Path (Split-Path -Parent $launcher) '.grok-zh-msvc'
    Assert-OnlinePathChain $runtime
    $lock = Enter-MigrationLock "$launcher.update.lock"
    try {
        # A valid newer child must survive an older wrapper. Explicit full-package
        # installation remains the supported downgrade operation.
        if (Test-MigrationRuntime $runtime $version) { Set-MigrationReady $runtime $version; return }
        if ((Test-Path -LiteralPath $runtime) -and !(Test-MigrationRuntime $runtime '0.0.0')) {
            throw '已有未完成或无法验证的迁移运行目录，没有覆盖现有文件。'
        }
        [Console]::Error.WriteLine('正在离线校验并展开内嵌 MSVC 完整包；可按 Ctrl+C 取消。')
        $package = Expand-GrokEmbeddedArchive -Launcher $launcher -Destination (Join-Path $WorkDirectory 'native') `
            -Version $version -ExpectedSize $size -ExpectedSha256 $digest
        $installBytes = 16777216L
        foreach ($file in Get-ChildItem -LiteralPath $package -File -Recurse) { $installBytes += $file.Length }
        Assert-GrokFreeSpace -Path $runtime -RequiredBytes $installBytes
        $candidate = Join-Path $package 'grok-zh.exe'
        $hashLines = @([IO.File]::ReadAllLines((Join-Path $package 'SHA256SUMS.txt')) | Where-Object { $_ -cmatch '^[0-9a-fA-F]{64}  grok-zh\.exe$' })
        if ($hashLines.Count -ne 1) { throw 'MSVC 包缺少唯一的主程序摘要。' }
        $candidateDigest = $hashLines[0].Substring(0, 64)
        $candidateHandle = [IO.File]::Open($candidate, 'Open', 'Read', 'Read')
        try {
            if ((Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash -ine $candidateDigest) { throw 'MSVC 候选程序摘要不一致。' }
            $actual = Get-OnlineExecutableVersion $candidate
            if ($actual.Text -cne $version -or $actual.DisplayText.Contains('(GNU migration launcher)')) { throw 'MSVC 候选程序版本或身份不一致。' }
            [Console]::Error.WriteLine('正在提交 MSVC 运行目录；原入口和用户数据保持可恢复。')
            if (Test-Path -LiteralPath $runtime) {
                Update-MigrationRuntime -Directory $runtime -Candidate $candidate -Version $version -ExpectedSha256 $candidateDigest
            } else {
                Invoke-OnlinePackageInstaller -Package $package -Directory $runtime -SharedHome '' -Version $version -NoPathUpdate
            }
        } finally { $candidateHandle.Dispose() }
        if (!(Test-MigrationRuntime $runtime $version)) { throw '迁移未生成可验证的 MSVC 运行目录。' }
        # Last commit only: interrupted extraction/installer never authorizes the
        # fast path. After this, normal launches need neither TEMP nor PowerShell.
        Set-MigrationReady $runtime $version
        [Console]::Error.WriteLine('离线迁移完成，正在启动。')
    } finally { $lock.Dispose() }
}

if ($MyInvocation.InvocationName -ne '.') {
    try {
        $context = ConvertFrom-Json -InputObject (ConvertFrom-OnlineUtf8 ([IO.File]::ReadAllBytes($ContextPath)))
        Invoke-GrokEmbeddedMigration -Context $context -WorkDirectory $PSScriptRoot
    } catch {
        [Console]::Error.WriteLine("离线迁移未完成：$($_.Exception.GetBaseException().Message)`n兼容启动器和原目录仍保留；请检查空间或权限后重试。")
        exit 1
    } finally {
        Remove-OnlineOwnedTree -Path $PSScriptRoot -Parent (Split-Path -Parent $PSScriptRoot) -Prefix 'grok-zh-embedded-'
    }
}
