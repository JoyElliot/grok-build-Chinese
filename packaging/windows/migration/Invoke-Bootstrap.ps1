# Embedded in the GNU compatibility launcher. Only a normal launch invokes this
# file; --version never downloads, installs, or runs this script.
[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$ContextPath)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# A PowerShell 7 caller can pass its module search path through the native shim
# to Windows PowerShell. Load this host's own built-ins by absolute path.
Import-Module (Join-Path $PSHOME 'Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1') -ErrorAction Stop
Import-Module (Join-Path $PSHOME 'Modules/Microsoft.PowerShell.Management/Microsoft.PowerShell.Management.psd1') -ErrorAction Stop
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
. (Join-Path $PSScriptRoot 'Install-GrokZhOnline.ps1')

function Enter-MigrationLock {
    param([string]$Path, [int]$WaitMilliseconds = 1200000)
    $watch = [Diagnostics.Stopwatch]::StartNew()
    while ($true) {
        try { return [IO.File]::Open($Path, 'OpenOrCreate', 'ReadWrite', 'None') }
        catch [IO.IOException] {
            if (($_.Exception.HResult -band 0xFFFF) -notin @(32, 33) -or $watch.ElapsedMilliseconds -ge $WaitMilliseconds) { throw }
            Start-Sleep -Milliseconds 200
        }
    }
}

function Set-MigrationReady {
    param([string]$Directory, [string]$Version)
    $path = Join-Path $Directory '.grok-zh-bootstrap-ready'
    Assert-OnlinePathChain $path
    $content = "$Version`nwindows-x64-gnu-to-msvc-v1`n"
    if ((Test-Path -LiteralPath $path) -and [IO.File]::ReadAllText($path) -ceq $content) { return }
    $temporary = "$path.$([guid]::NewGuid().ToString('N')).tmp"
    try {
        [IO.File]::WriteAllText($temporary, $content, [Text.Encoding]::ASCII)
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
        if (!$LockHeld) {
            Assert-OnlinePathChain "$exe.update.lock"
            $lock = Enter-MigrationLock "$exe.update.lock"
        }
        $actual = Get-OnlineExecutableVersion $exe
        if ($actual.DisplayText.Contains('(GNU migration launcher)')) { throw '运行目录仍然是兼容启动器，拒绝递归。' }
        if ((Compare-OnlineVersion $actual (ConvertTo-OnlineVersion $MinimumVersion -StableOnly)) -lt 0) { return $false }
        # MSVC self-updates replace only the EXE, so its initial hash/version
        # is deliberately not frozen in the launcher forever.
        return $true
    } finally { if ($lock) { $lock.Dispose() } }
}

function Invoke-MigrationFileReplace {
    param([string]$Stage, [string]$Executable, [string]$Backup)
    [IO.File]::Replace($Stage, $Executable, $Backup)
}

function Update-MigrationRuntime {
    param([string]$Directory, [string]$Candidate, [string]$Version,
        [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ExpectedSha256)
    # Used only after the pinned helper verifies the full MSVC archive and its
    # executable-only protocol. Keep support files and user files in place,
    # just as subsequent Rust MSVC updates do; never swap the whole directory.
    $exe = Join-Path $Directory 'grok-zh.exe'
    $lockPath = "$exe.update.lock"
    Assert-OnlinePathChain $lockPath
    $lock = Enter-MigrationLock $lockPath
    $stage = "$exe.$([guid]::NewGuid().ToString('N')).migration-candidate"
    $backup = "$exe.$([guid]::NewGuid().ToString('N')).migration-backup"
    $stageCreated = $false
    $preserveRecovery = $false
    try {
        # An MSVC update may have finished while we downloaded or waited.
        if (Test-MigrationRuntime $Directory $Version -LockHeld) { return }
        if (!(Test-MigrationRuntime $Directory '0.0.0' -LockHeld)) { throw '现有 MSVC 运行目录无法验证，未替换文件。' }
        Assert-OnlinePathChain $Candidate
        Assert-OnlinePathChain $stage
        Assert-OnlinePathChain $backup
        if (Test-Path -LiteralPath $backup) { throw '升级备份路径已存在，未替换文件。' }
        $inputStream = [IO.File]::OpenRead($Candidate)
        try {
            $output = [IO.File]::Open($stage, 'CreateNew', 'Write', 'None')
            $stageCreated = $true
            try { $inputStream.CopyTo($output); $output.Flush($true) } finally { $output.Dispose() }
        } finally { $inputStream.Dispose() }
        if ((Get-FileHash -LiteralPath $stage -Algorithm SHA256).Hash -ine $ExpectedSha256) { throw '升级暂存程序摘要不匹配，未执行或替换原程序。' }
        $actual = Get-OnlineExecutableVersion $stage
        if ($actual.Text -cne $Version -or $actual.DisplayText.Contains('(GNU migration launcher)')) { throw '升级暂存程序校验失败，未替换原程序。' }
        $oldDigest = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
        # ReplaceFile can fail after moving the original to its backup (1177).
        # Restore that exact old image before deleting our staged candidate.
        # Never truncate/copy over a live EXE or overwrite a newly present EXE.
        try { Invoke-MigrationFileReplace $stage $exe $backup }
        catch {
            $replaceError = $_
            if (!(Test-Path -LiteralPath $exe)) {
                try {
                    Assert-OnlinePathChain $backup
                    if ((Get-FileHash -LiteralPath $backup -Algorithm SHA256).Hash -cne $oldDigest) { throw '旧程序备份摘要不匹配。' }
                    [IO.File]::Move($backup, $exe)
                } catch {
                    $preserveRecovery = $true
                    throw "升级与恢复均未完成；请保留备份 $backup 和候选 $stage。$($_.Exception.Message)"
                }
            }
            throw $replaceError
        }
        $stageCreated = $false
        if ((Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash -ine $ExpectedSha256 -or
            !(Test-MigrationRuntime $Directory $Version -LockHeld)) { throw "升级后校验失败；旧程序保留在 $backup。" }
        # Keep rollback files outside the Rust updater's .old cleanup pattern.
        # A running session may still hold the backup; never stop that session.
        try { [IO.File]::Delete($backup) }
        catch [IO.IOException] { [Console]::Error.WriteLine("旧程序仍被占用，备份保留：$backup") }
        catch [UnauthorizedAccessException] { [Console]::Error.WriteLine("旧程序备份保留：$backup") }
    } finally {
        try { if ($stageCreated -and !$preserveRecovery) { [IO.File]::Delete($stage) } }
        finally { $lock.Dispose() }
    }
}

function Get-MigrationAssetContract {
    param($Release, $Pin)
    foreach ($name in @('draft', 'prerelease', 'immutable')) {
        if ((Get-OnlineProperty $Release $name) -isnot [bool]) { throw '迁移 Release 标志类型无效。' }
    }
    if ($Release.draft -or $Release.prerelease -or !$Release.immutable -or
        (Get-OnlineProperty $Release 'tag_name') -cne $Pin.tag) { throw '迁移包必须属于指定的不可变正式 Release。' }
    $assets = @((Get-OnlineProperty $Release 'assets') | Where-Object { (Get-OnlineProperty $_ 'name') -ceq $Pin.asset })
    if ($assets.Count -ne 1) { throw '迁移包附件缺失或重复。' }
    $asset = $assets[0]
    $size = Get-OnlineProperty $asset 'size'
    $expectedUrl = "https://github.com/$script:OnlineRepo/releases/download/$($Pin.tag)/$($Pin.asset)"
    if ((Get-OnlineProperty $asset 'state') -cne 'uploaded' -or
        ($size -isnot [int] -and $size -isnot [long]) -or $size -ne $Pin.size -or
        (Get-OnlineProperty $asset 'browser_download_url') -cne $expectedUrl -or
        (Get-OnlineProperty $asset 'digest') -cne "sha256:$($Pin.sha256)") { throw '迁移包与启动器固定的大小、摘要或来源不一致。' }
    return [pscustomobject]@{ Url = $expectedUrl; Size = [long]$size; Sha256 = $Pin.sha256 }
}

function Expand-MigrationPackage {
    param([string]$ArchivePath, [string]$Destination, [string]$Id)
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead($ArchivePath)
    try {
        $names = @('MIGRATION.json', 'Invoke-Migration.ps1', 'SHA256SUMS.txt')
        if ($archive.Entries.Count -ne $names.Count) { throw '迁移包文件集合无效。' }
        $entries = @{}
        foreach ($entry in $archive.Entries) {
            if ($names -cnotcontains $entry.FullName -or $entries.ContainsKey($entry.FullName) -or
                $entry.Length -gt 1048576 -or (($entry.ExternalAttributes -shr 16) -band 0xF000) -eq 0xA000) { throw '迁移包含非法路径、重复文件、链接或超大文件。' }
            $entries[$entry.FullName] = $entry
        }
        $manifest = Read-OnlineZipText $entries['SHA256SUMS.txt']
        $hashes = @{}
        foreach ($line in ($manifest.TrimEnd("`r", "`n") -split '\r?\n')) {
            if ($line -cnotmatch '^([0-9a-f]{64})  (MIGRATION.json|Invoke-Migration.ps1)$' -or $hashes.ContainsKey($matches[2])) { throw '迁移包内部校验清单无效。' }
            $hashes[$matches[2]] = $matches[1]
        }
        if ($hashes.Count -ne 2 -or (Test-Path -LiteralPath $Destination)) { throw '迁移包清单不完整或暂存目录已存在。' }
        $null = [IO.Directory]::CreateDirectory($Destination)
        foreach ($name in @('MIGRATION.json', 'Invoke-Migration.ps1')) {
            $text = Read-OnlineZipText $entries[$name]
            # Preserve exact original bytes, including a UTF-8 BOM in PS5 scripts.
            $inputStream = $entries[$name].Open()
            $path = Join-Path $Destination $name
            $output = [IO.File]::Open($path, 'CreateNew', 'Write', 'None')
            try { $inputStream.CopyTo($output) } finally { $inputStream.Dispose(); $output.Dispose() }
            if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ine $hashes[$name]) { throw '迁移包内部文件校验失败。' }
        }
        $metadata = ConvertFrom-Json -InputObject (ConvertFrom-OnlineUtf8 ([IO.File]::ReadAllBytes((Join-Path $Destination 'MIGRATION.json'))))
        if ((Get-OnlineProperty $metadata 'schema') -ne 1 -or (Get-OnlineProperty $metadata 'id') -cne $Id -or
            (Get-OnlineProperty $metadata 'from') -cne 'x86_64-pc-windows-gnu' -or
            (Get-OnlineProperty $metadata 'to') -cne 'x86_64-pc-windows-msvc' -or
            (Get-OnlineProperty $metadata 'entry') -cne 'Invoke-Migration.ps1') { throw '迁移包身份或适用平台不一致。' }
        return Join-Path $Destination 'Invoke-Migration.ps1'
    } finally { $archive.Dispose() }
}

function Invoke-GrokBootstrap {
    param($Context, [string]$WorkDirectory)
    $version = [string](Get-OnlineProperty $Context 'version')
    $null = ConvertTo-OnlineVersion $version -StableOnly
    $pin = Get-OnlineProperty $Context 'migration'
    if ($pin.id -cne 'windows-x64-gnu-to-msvc-v1' -or $pin.tag -cnotmatch '^migration-windows-x64-v[1-9][0-9]*$' -or
        $pin.asset -cne 'grok-zh-migration-windows-x64.zip' -or $pin.sha256 -cnotmatch '^[0-9a-f]{64}$' -or
        ($pin.size -isnot [int] -and $pin.size -isnot [long]) -or $pin.size -le 0 -or $pin.size -gt 1048576) { throw '启动器迁移配置无效。' }
    $launcher = Resolve-OnlinePath ([string](Get-OnlineProperty $Context 'launcher'))
    if ((Split-Path -Leaf $launcher) -ine 'grok-zh.exe') { throw '迁移启动器必须保留 grok-zh.exe 文件名。' }
    Assert-OnlinePathChain $launcher
    $directory = Split-Path -Parent $launcher
    $runtime = Join-Path $directory '.grok-zh-msvc'
    Assert-OnlinePathChain $runtime
    # Serialize bootstrap launches and current GNU updates. The sidecar MSVC
    # updater has a different lock, acquired by Update-MigrationRuntime below.
    # Published older GNU updaters do not all implement this lock protocol.
    $lockPath = "$launcher.update.lock"
    Assert-OnlinePathChain $lockPath
    $lock = Enter-MigrationLock $lockPath
    $client = $null
    try {
        if (Test-MigrationRuntime $runtime $version) { Set-MigrationReady $runtime $version; return }
        if ((Test-Path -LiteralPath $runtime) -and !(Test-MigrationRuntime $runtime '0.0.0')) {
            throw '已有未完成或无法验证的迁移运行目录，没有覆盖现有文件。'
        }
        $client = New-OnlineHttpClient
        $metadataPath = Join-Path $WorkDirectory 'migration-release.json'
        Receive-OnlineFile -Client $client -Uri "https://api.github.com/repos/$script:OnlineRepo/releases/tags/$($pin.tag)" `
            -Destination $metadataPath -MaximumBytes 1048576 -Label '正在核对迁移包'
        $release = ConvertFrom-Json -InputObject (ConvertFrom-OnlineUtf8 ([IO.File]::ReadAllBytes($metadataPath)))
        $asset = Get-MigrationAssetContract $release $pin
        $archivePath = Join-Path $WorkDirectory 'migration.zip'
        Receive-OnlineFile -Client $client -Uri $asset.Url -Destination $archivePath -MaximumBytes 1048576 `
            -ExpectedBytes $asset.Size -ExpectedSha256 $asset.Sha256 -Label '正在下载迁移包'
        $entry = Expand-MigrationPackage $archivePath (Join-Path $WorkDirectory 'migration') $pin.id
        & $entry -Version $version -RuntimeDirectory $runtime -WorkDirectory $WorkDirectory -Client $client `
            -OnlineLibrary (Join-Path $PSScriptRoot 'Install-GrokZhOnline.ps1')
        if (!(Test-MigrationRuntime $runtime $version)) { throw '迁移未生成可验证的 MSVC 运行目录。' }
        Set-MigrationReady $runtime $version
    } finally {
        if ($client) { $client.Dispose() }
        $lock.Dispose()
    }
}

if ($MyInvocation.InvocationName -ne '.') {
    try {
        $context = ConvertFrom-Json -InputObject (ConvertFrom-OnlineUtf8 ([IO.File]::ReadAllBytes($ContextPath)))
        Invoke-GrokBootstrap -Context $context -WorkDirectory $PSScriptRoot
    } catch {
        [Console]::Error.WriteLine("迁移未完成：$($_.Exception.GetBaseException().Message)`n兼容启动器和原目录仍保留，可在网络恢复后重新启动。")
        exit 1
    } finally {
        $parent = Split-Path -Parent $PSScriptRoot
        Remove-OnlineOwnedTree -Path $PSScriptRoot -Parent $parent -Prefix 'grok-zh-migration-'
    }
}
