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
    param([string]$Directory, [string]$MinimumVersion)
    if (!(Test-Path -LiteralPath $Directory)) { return $false }
    $marker = Get-OnlineInstallMarker $Directory
    if (!$marker) { return $false }
    $infoPath = Join-Path $Directory 'BUILD-INFO.txt'
    Assert-OnlinePathChain $infoPath
    $info = ConvertFrom-OnlineUtf8 ([IO.File]::ReadAllBytes($infoPath))
    if ($info -cnotmatch '(?m)^Target:[ \t]*x86_64-pc-windows-msvc[ \t]*\r?$') { throw '迁移运行目录不是 Windows x64 MSVC。' }
    $exe = Join-Path $Directory 'grok-zh.exe'
    $actual = Get-OnlineExecutableVersion $exe
    if ($actual.DisplayText.Contains('(GNU migration launcher)')) { throw '运行目录仍然是兼容启动器，拒绝递归。' }
    if ((Compare-OnlineVersion $actual (ConvertTo-OnlineVersion $MinimumVersion -StableOnly)) -lt 0) { return $false }
    # The installer marker is committed with the directory. Later MSVC updates
    # replace this EXE through their own verified update path, so its initial
    # hash/version is deliberately not frozen in the launcher forever.
    return $true
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
    # Same exclusion protocol as the Rust updater. Never delete this lock file.
    $lockPath = "$launcher.update.lock"
    Assert-OnlinePathChain $lockPath
    $lock = Enter-MigrationLock $lockPath
    $client = $null
    try {
        if (Test-MigrationRuntime $runtime $version) { Set-MigrationReady $runtime $version; return }
        if (Test-Path -LiteralPath $runtime) { throw '已有未完成或较旧的迁移运行目录，请保留该目录并使用在线安装器修复；没有覆盖现有文件。' }
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
