param(
    [Parameter(Mandatory = $true)]$Release,
    $Client
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '../../packaging/windows/Install-GrokZhOnline.ps1')

# Inspect authenticated package bytes, since a version number alone does not
# establish that an already published release contains the protocol migration.
# All three binaries are built from this same immutable release source commit.
$contract = Get-OnlineReleaseContract $Release
$parent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$work = Join-Path $parent ('grok-zh-bridge-check-' + [guid]::NewGuid().ToString('N'))
$ownsClient = $null -eq $Client
$created = $false
$oldTls = [Net.ServicePointManager]::SecurityProtocol
try {
    Assert-OnlinePathChain $parent
    if (Test-Path -LiteralPath $work) { throw '过渡包验证目录已存在。' }
    $null = [IO.Directory]::CreateDirectory($work)
    $created = $true
    if ($ownsClient) { $Client = New-OnlineHttpClient }
    $archive = Join-Path $work $contract.Archive.Name
    Receive-OnlineFile -Client $Client -Uri $contract.Archive.Url -Destination $archive `
        -MaximumBytes $contract.Archive.Size -ExpectedBytes $contract.Archive.Size `
        -ExpectedSha256 $contract.Archive.Sha256 -Label '正在验证永久过渡包'
    $package = Expand-VerifiedOnlinePackage -ArchivePath $archive -Destination (Join-Path $work 'package') -Contract $contract
    $info = ConvertFrom-OnlineUtf8 ([IO.File]::ReadAllBytes((Join-Path $package 'BUILD-INFO.txt')))
    if ($null -eq (Read-OnlinePackageProtocol $info $contract.Version.Text)) {
        throw '指定过渡 Release 未包含新更新协议，不能停止发布独立校验文件。'
    }
    Write-Host "已验证永久过渡包包含新协议：$($contract.Tag)"
} finally {
    if ($ownsClient -and $Client) { $Client.Dispose() }
    [Net.ServicePointManager]::SecurityProtocol = $oldTls
    if ($created) { Remove-OnlineOwnedTree -Path $work -Parent $parent -Prefix 'grok-zh-bridge-check-' }
}
