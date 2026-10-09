# The versioned, immutable migration package. Installs beside the GNU launcher;
# it never renames a directory containing the running launcher or touches GROK_HOME.
[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$RuntimeDirectory,
    [Parameter(Mandatory = $true)][string]$WorkDirectory,
    [Parameter(Mandatory = $true)]$Client,
    [Parameter(Mandatory = $true)][string]$OnlineLibrary)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. $OnlineLibrary -Version $Version -WindowsX64Runtime Msvc
if ((Split-Path -Leaf $RuntimeDirectory) -cne '.grok-zh-msvc') { throw '迁移只允许使用独立的 MSVC 运行目录。' }
Assert-OnlinePathChain $RuntimeDirectory
if ((Test-Path -LiteralPath $RuntimeDirectory) -and !(Test-MigrationRuntime $RuntimeDirectory '0.0.0')) {
    throw '现有 MSVC 运行目录无法验证，未替换文件。'
}
$script:OnlineTargetTriple = 'x86_64-pc-windows-msvc'
$script:OnlinePlatformSuffix = 'windows-x86_64-msvc'
$release = Get-ExactOnlineRelease -Client $Client -Version $Version -WorkDirectory $WorkDirectory
$archive = Join-Path $WorkDirectory $release.Archive.Name
Receive-OnlineFile -Client $Client -Uri $release.Archive.Url -Destination $archive -MaximumBytes $release.Archive.Size `
    -ExpectedBytes $release.Archive.Size -ExpectedSha256 $release.Archive.Sha256 -Label '正在下载 MSVC 正式版'
$package = Expand-VerifiedOnlinePackage -ArchivePath $archive -Destination (Join-Path $WorkDirectory 'msvc-package') -Contract $release
$protocol = Read-OnlinePackageProtocol (ConvertFrom-OnlineUtf8 ([IO.File]::ReadAllBytes((Join-Path $package 'BUILD-INFO.txt')))) $Version
if (!$protocol -or $protocol.platform -cne 'x86_64-pc-windows-msvc' -or $protocol.executable -cne 'grok-zh.exe') { throw '迁移目标缺少 MSVC 完整包协议。' }
$candidate = Join-Path $package 'grok-zh.exe'
# Bind replacement to the verified package manifest, not a fresh digest of a
# path that could have changed after candidate execution or while waiting.
$candidateHashes = @([IO.File]::ReadAllLines((Join-Path $package 'SHA256SUMS.txt')) | Where-Object { $_ -cmatch '^[0-9a-fA-F]{64}  grok-zh\.exe$' })
if ($candidateHashes.Count -ne 1) { throw 'MSVC 包缺少唯一的主程序摘要。' }
$candidateDigest = $candidateHashes[0].Substring(0, 64)
$candidateHandle = [IO.File]::Open($candidate, 'Open', 'Read', 'Read')
try {
    if ((Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash -ine $candidateDigest) { throw 'MSVC 候选程序与包清单摘要不一致。' }
    $candidateVersion = Get-OnlineExecutableVersion $candidate
    if ($candidateVersion.Text -cne $Version -or $candidateVersion.DisplayText.Contains('(GNU migration launcher)')) { throw 'MSVC 候选程序版本或身份不一致。' }
    $launcher = Join-Path (Split-Path -Parent $RuntimeDirectory) 'grok-zh.exe'
    if ((Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash -ceq (Get-FileHash -LiteralPath $launcher -Algorithm SHA256).Hash) { throw '迁移目标仍然是兼容启动器，拒绝递归。' }
    if (Test-Path -LiteralPath $RuntimeDirectory) {
        Update-MigrationRuntime -Directory $RuntimeDirectory -Candidate $candidate -Version $Version -ExpectedSha256 $candidateDigest
    } else {
        Invoke-OnlinePackageInstaller -Package $package -Directory $RuntimeDirectory -SharedHome '' -Version $Version -NoPathUpdate
    }
} finally { $candidateHandle.Dispose() }
