[CmdletBinding()]
param([string]$InstallerPath)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (!$InstallerPath) { $InstallerPath = Join-Path (Split-Path -Parent $PSScriptRoot) 'Install-GrokZh.ps1' }
. $InstallerPath -LibraryOnly

function Assert-LockFailure {
    param([string]$Install)
    $unexpected = $null
    $failed = $false
    try { $unexpected = Open-GrokInstallerLock -InstallDir $Install } catch { $failed = $true }
    finally { if ($null -ne $unexpected) { $unexpected.Dispose() } }
    if (!$failed) { throw "Unsafe or contended installer lock was accepted: $Install" }
}

$root = Join-Path ([IO.Path]::GetTempPath()) ('grok installer lock ' + [guid]::NewGuid().ToString('N'))
$handle = $null
New-Item -ItemType Directory -Path $root | Out-Null
try {
    $install = Join-Path $root 'active'
    $handle = Open-GrokInstallerLock -InstallDir $install
    Assert-LockFailure $install
    Assert-LockFailure $install.ToUpperInvariant()
    Assert-LockFailure (Join-Path $root 'unused/../active')
    $handle.Dispose(); $handle = $null
    if (!(Test-Path -LiteralPath ($install + '.install.lock') -PathType Leaf)) { throw 'Released lock file was unlinked.' }
    $handle = Open-GrokInstallerLock -InstallDir $install
    $handle.Dispose(); $handle = $null

    # A junction at the lock path must be opened/rejected as the link itself,
    # never followed to (or allowed to mutate) an unrelated target.
    $unrelated = Join-Path $root 'unrelated'
    New-Item -ItemType Directory -Path $unrelated | Out-Null
    [IO.File]::WriteAllText((Join-Path $unrelated 'keep.txt'), 'untouched')
    $linkedInstall = Join-Path $root 'linked'
    New-Item -ItemType Junction -Path ($linkedInstall + '.install.lock') -Target $unrelated | Out-Null
    try { Assert-LockFailure $linkedInstall }
    finally { [IO.Directory]::Delete($linkedInstall + '.install.lock') }
    $linkedParent = Join-Path $root 'redirected-parent'
    New-Item -ItemType Junction -Path $linkedParent -Target $unrelated | Out-Null
    try { Assert-LockFailure (Join-Path $linkedParent 'active') }
    finally { [IO.Directory]::Delete($linkedParent) }
    if ([IO.File]::ReadAllText((Join-Path $unrelated 'keep.txt')) -cne 'untouched') { throw 'Reparse refusal modified unrelated files.' }
    if (@(Get-ChildItem -LiteralPath $unrelated -Force).Count -ne 1) { throw 'Reparse refusal created a lock in an unrelated directory.' }

    $seed = Join-Path $root 'seed.txt'
    [IO.File]::WriteAllText($seed, 'not-an-installer-lock')
    $hardlinkedInstall = Join-Path $root 'hardlinked'
    New-Item -ItemType HardLink -Path ($hardlinkedInstall + '.install.lock') -Target $seed | Out-Null
    Assert-LockFailure $hardlinkedInstall
    if ([IO.File]::ReadAllText($seed) -cne 'not-an-installer-lock') { throw 'Hard-link refusal modified unrelated data.' }
    Write-Host "Installer stable lock: contention, case/path identity, retry, reparse and hard-link refusals passed (PowerShell $($PSVersionTable.PSVersion))."
} finally {
    if ($null -ne $handle) { $handle.Dispose() }
    Remove-Item -LiteralPath $root -Recurse -Force
}
