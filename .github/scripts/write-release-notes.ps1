[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $CurrentTag,

    [Parameter(Mandatory = $true)]
    [string] $OutputPath,

    [string] $Repository = $env:GITHUB_REPOSITORY,

    [string] $GitHubToken = $env:GH_TOKEN,

    [string] $NotesPath,

    [string] $UpstreamRepository = 'xai-org/grok-build',

    [AllowEmptyCollection()]
    [string[]] $PublishedReleaseTags
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

function Get-ReleaseTagVersion([string] $Tag) {
    $pattern = '^(?<namespace>v|release-v)(?<version>(?<major>0|[1-9][0-9]*)\.(?<minor>0|[1-9][0-9]*)\.(?<patch>0|[1-9][0-9]*)(?:-(?<prerelease>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?)$'
    if ($Tag -cnotmatch $pattern) { return $null }
    $version = $matches['version']
    $prerelease = [string]$matches['prerelease']
    foreach ($number in @($matches['major'], $matches['minor'], $matches['patch'])) {
        [uint64]$parsedNumber = 0
        if ($number -and ![uint64]::TryParse([string]$number, [ref]$parsedNumber)) {
            return $null
        }
    }
    if ($prerelease) {
        foreach ($identifier in ($prerelease -split '\.')) {
            if ($identifier -cmatch '^[0-9]+$') {
                [uint64]$parsedIdentifier = 0
                if (($identifier.Length -gt 1 -and $identifier.StartsWith('0')) -or
                    ![uint64]::TryParse($identifier, [ref]$parsedIdentifier)) {
                    return $null
                }
            }
        }
    }
    return $version
}

function Test-ReleaseTag([string] $Tag) {
    return $null -ne (Get-ReleaseTagVersion $Tag)
}

function Test-RepositoryName([string] $Value) {
    return $Value -match '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$'
}

function Test-ContainsChinese([string] $Text) {
    return $Text -match '[\u3400-\u4DBF\u4E00-\u9FFF]'
}

function ConvertTo-MarkdownLinkText([string] $Text) {
    $singleLine = [regex]::Replace($Text, '[\x00-\x1F\x7F]', ' ').Trim()
    return $singleLine.Replace('&', '&amp;').Replace('<', '&lt;').Replace('>', '&gt;').Replace('\', '\\').Replace('[', '\[').Replace(']', '\]').Replace('*', '\*').Replace('_', '\_').Replace('`', '\`')
}

if (!(Test-ReleaseTag $CurrentTag)) {
    throw "CurrentTag 必须是严格三段 vA.B.C、release-vA.B.C 或对应预发布格式，且不得含 build metadata、数字前导零：$CurrentTag"
}
if (!(Test-RepositoryName $Repository)) {
    throw "Repository 必须是 owner/name：$Repository"
}
if (!(Test-RepositoryName $UpstreamRepository)) {
    throw "UpstreamRepository 必须是 owner/name：$UpstreamRepository"
}
if (!$NotesPath) {
    $NotesPath = Join-Path $PSScriptRoot "../release-notes/versions/$CurrentTag.json"
}
if (!(Test-Path -LiteralPath $NotesPath -PathType Leaf)) {
    throw "缺少经过整理的中文发布说明：$NotesPath；请先编写版本重点，不自动回退为提交清单。"
}

function Invoke-Git {
    param([string[]] $Arguments, [switch] $AllowFailure)
    $output = @(& git @Arguments 2>&1)
    $exitCode = $LASTEXITCODE
    $global:LASTEXITCODE = 0
    if ($exitCode -ne 0 -and !$AllowFailure) {
        throw "git $($Arguments -join ' ') 失败（exit $exitCode）：$($output -join [Environment]::NewLine)"
    }
    [pscustomobject]@{ ExitCode = $exitCode; Lines = @($output | ForEach-Object { $_.ToString() }) }
}
function Resolve-Commit([string] $Ref) {
    $result = Invoke-Git -Arguments @('rev-parse', '--verify', "$Ref^{commit}")
    $sha = $result.Lines[0].Trim().ToLowerInvariant()
    if ($sha -notmatch '^[0-9a-f]{40}$') { throw "无效提交：$Ref" }
    return $sha
}
function Test-Ancestor([string] $Ancestor, [string] $Descendant) {
    $result = Invoke-Git -Arguments @('merge-base', '--is-ancestor', $Ancestor, $Descendant) -AllowFailure
    if ($result.ExitCode -gt 1) { throw "无法核验提交祖先关系：$Ancestor -> $Descendant" }
    return $result.ExitCode -eq 0
}
function Get-PublishedReleaseTags {
    if (!$GitHubToken) { throw '必须提供 GitHub token 或 PublishedReleaseTags 以核对已发布基线。' }
    $headers = @{
        Accept = 'application/vnd.github+json'
        Authorization = "Bearer $GitHubToken"
        'X-GitHub-Api-Version' = '2022-11-28'
    }
    $releases = [System.Collections.Generic.List[object]]::new()
    for ($page = 1; ; $page++) {
        # Assign before wrapping: Invoke-RestMethod emits a JSON array as one
        # pipeline object, so @(Invoke-RestMethod ...) would nest the page.
        $pageResult = Invoke-RestMethod -Method Get -Uri "https://api.github.com/repos/$Repository/releases?per_page=100&page=$page" -Headers $headers
        $items = @($pageResult)
        foreach ($release in $items) {
            # Historical releases may have a prerelease display name but a stable
            # tag. Notes editing must preserve that metadata, not reinterpret it.
            if (!$release.draft -and $release.immutable -and (Test-ReleaseTag $release.tag_name)) {
                $releases.Add($release)
            }
        }
        if ($items.Count -lt 100) { break }
    }
    $current = @($releases | Where-Object { $_.tag_name -ceq $CurrentTag })
    $eligible = @($releases)
    if ($current.Count -eq 1) {
        $eligible = @($eligible | Where-Object { [datetime]$_.published_at -lt [datetime]$current[0].published_at })
    }
    return @($eligible | Sort-Object { [datetime]$_.published_at } -Descending | ForEach-Object { $_.tag_name })
}

$document = Get-Content -LiteralPath $NotesPath -Raw | ConvertFrom-Json
foreach ($field in @('schema', 'tag', 'previous_tag', 'community', 'upstream')) {
    if (!$document.PSObject.Properties[$field]) { throw "发布说明缺少字段：$field" }
}
if (($document.schema -isnot [int] -and $document.schema -isnot [long]) -or $document.schema -ne 1) {
    throw '发布说明 schema 必须为整数 1。'
}
if ($document.tag -isnot [string] -or $document.tag -cne $CurrentTag) { throw '发布说明 tag 与当前标签不一致。' }
if ($null -ne $document.previous_tag -and ($document.previous_tag -isnot [string] -or !(Test-ReleaseTag $document.previous_tag))) {
    throw 'previous_tag 必须为有效 Release 标签或 null。'
}
if ($document.community -isnot [System.Collections.IList]) { throw 'community 必须为重点数组。' }
if ($document.upstream -isnot [System.Collections.IList]) { throw 'upstream 必须为数组。' }
if ($document.community.Count -eq 0 -and $document.upstream.Count -eq 0) { throw '发布说明至少需要一条社区或上游重点。' }
$currentCommit = Resolve-Commit $CurrentTag
if ($PSBoundParameters.ContainsKey('PublishedReleaseTags')) {
    $publishedTags = @($PublishedReleaseTags)
} else {
    $publishedTags = @(Get-PublishedReleaseTags)
}
$publishedByCommit = @{}
foreach ($tag in $publishedTags) {
    if (!(Test-ReleaseTag $tag)) { throw "PublishedReleaseTags 包含无效 Tag：$tag" }
    if ($tag -ceq $CurrentTag) { continue }
    $commit = Resolve-Commit $tag
    if (!$publishedByCommit.ContainsKey($commit)) { $publishedByCommit[$commit] = $tag }
}
$previousTag = $null
$firstParents = Invoke-Git -Arguments @('rev-list', '--first-parent', $currentCommit)
foreach ($commit in $firstParents.Lines) {
    if ($publishedByCommit.ContainsKey($commit)) { $previousTag = $publishedByCommit[$commit]; break }
}
if ([string]$document.previous_tag -cne [string]$previousTag) {
    throw "发布说明基线与已发布历史不一致：notes=$($document.previous_tag) actual=$previousTag"
}
$previousCommit = if ($previousTag) { Resolve-Commit $previousTag } else { $null }

function Get-HighlightText($Entry, [string] $Tip, [string] $Base, [string] $Context) {
    if (!$Entry.PSObject.Properties['text'] -or $Entry.text -isnot [string] -or
        [string]::IsNullOrWhiteSpace($Entry.text) -or $Entry.text -match '[\x00-\x1F\x7F]' -or
        !(Test-ContainsChinese $Entry.text)) { throw "$Context 重点必须为包含中文的非空单行文本。" }
    if (!$Entry.PSObject.Properties['commits'] -or $Entry.commits -isnot [System.Collections.IList] -or $Entry.commits.Count -eq 0) {
        throw "$Context 重点缺少来源 commits 数组。"
    }
    foreach ($source in $Entry.commits) {
        if ($source -isnot [string] -or $source -cnotmatch '^[0-9a-f]{40}$') { throw "$Context 来源必须使用完整提交 SHA。" }
        $sourceCommit = Resolve-Commit $source
        if (!(Test-Ancestor $sourceCommit $Tip)) { throw "$Context 来源提交未包含在目标版本：$source" }
        if ($Base -and $Base -cne $Tip -and (Test-Ancestor $sourceCommit $Base)) {
            throw "$Context 来源提交已属于上一版或范围基线：$source"
        }
    }
    return ConvertTo-MarkdownLinkText $Entry.text
}
$lines = [System.Collections.Generic.List[string]]::new()
if ($document.community.Count -gt 0) {
    $lines.Add('## 社区版重点')
    $lines.Add('')
}
foreach ($entry in $document.community) {
    $text = Get-HighlightText $entry $currentCommit $previousCommit '社区版'
    $lines.Add("- $text")
}
$upstreamLinks = [System.Collections.Generic.List[string]]::new()
if ($document.upstream.Count -gt 0) {
    if ($lines.Count -gt 0) { $lines.Add('') }
    $lines.Add('## 上游更新')
    $seenRanges = @{}
    foreach ($group in $document.upstream) {
        foreach ($field in @('base', 'tip', 'label', 'highlights')) {
            if (!$group.PSObject.Properties[$field]) { throw "上游范围缺少字段：$field" }
        }
        if ($group.base -isnot [string] -or $group.tip -isnot [string] -or
            $group.base -cnotmatch '^[0-9a-f]{40}$' -or $group.tip -cnotmatch '^[0-9a-f]{40}$') {
            throw '上游范围必须使用完整提交 SHA。'
        }
        $base = Resolve-Commit $group.base
        $tip = Resolve-Commit $group.tip
        if ($base -ceq $tip -or !(Test-Ancestor $base $tip) -or !(Test-Ancestor $tip $currentCommit)) {
            throw '上游范围不是当前版本包含的有效祖先范围。'
        }
        if ($previousCommit -and (!(Test-Ancestor $base $previousCommit) -or (Test-Ancestor $tip $previousCommit))) {
            throw '上游范围与上一版基线不一致。'
        }
        if ($previousCommit) {
            $mergeBases = (Invoke-Git -Arguments @('merge-base', '--all', $previousCommit, $tip)).Lines
            if ($mergeBases.Count -ne 1 -or $mergeBases[0] -cne $base) {
                throw "上游基线必须等于上一版与本次上游的共同祖先：configured=$base actual=$($mergeBases -join ',')"
            }
        }
        $range = "$base...$tip"
        if ($seenRanges.ContainsKey($range)) { throw '上游范围重复。' }
        $seenRanges[$range] = $true
        if ($group.label -isnot [string] -or [string]::IsNullOrWhiteSpace($group.label) -or $group.label -match '[\x00-\x1F\x7F]') {
            throw '上游范围 label 必须为非空单行文本。'
        }
        if ($group.highlights -isnot [System.Collections.IList] -or $group.highlights.Count -eq 0) {
            throw '上游范围缺少 highlights 重点数组。'
        }
        $label = ConvertTo-MarkdownLinkText $group.label
        $lines.Add('')
        $lines.Add("$label：")
        $lines.Add('')
        foreach ($entry in $group.highlights) {
            $text = Get-HighlightText $entry $tip $base '上游'
            $lines.Add("- $text")
        }
        $upstreamLinks.Add("[上游完整变更（$label）](https://github.com/$UpstreamRepository/compare/$range)")
    }
}
if ($document.PSObject.Properties['notices']) {
    if ($document.notices -isnot [System.Collections.IList]) { throw 'notices 必须为数组。' }
    if ($document.notices.Count -gt 0) {
        $lines.Add(''); $lines.Add('## 安装与兼容性'); $lines.Add('')
        foreach ($notice in $document.notices) {
            if ($notice -isnot [string] -or [string]::IsNullOrWhiteSpace($notice) -or
                $notice -match '[\x00-\x1F\x7F]' -or !(Test-ContainsChinese $notice)) { throw '兼容性说明必须为中文单行文本。' }
            $lines.Add("- $(ConvertTo-MarkdownLinkText $notice)")
        }
    }
}
$lines.Add('')
$links = [System.Collections.Generic.List[string]]::new()
if ($previousTag) { $links.Add("[完整变更](https://github.com/$Repository/compare/$previousTag...$CurrentTag)") }
foreach ($link in $upstreamLinks) { $links.Add($link) }
if ($links.Count -gt 0) { $lines.Add(($links -join ' · ')); $lines.Add('') }
$commandPath = Join-Path $PSScriptRoot '../../packaging/windows/ONLINE-INSTALL-COMMAND.txt'
$installCommand = [IO.File]::ReadAllText($commandPath, [Text.Encoding]::UTF8).Trim()
if (!$installCommand -or $installCommand.Contains("`n") -or $installCommand.Contains("`r")) {
    throw 'Windows 在线安装命令必须为非空单行。'
}
$lines.Add('<details>')
$lines.Add('<summary>下载与安装</summary>')
$lines.Add('')
$lines.Add('本版本安装包见下方附件。以下在线命令始终安装最新正式版；安装历史版或预发布版时，请下载对应附件。')
$lines.Add('')
$lines.Add('```powershell')
$lines.Add($installCommand)
$lines.Add('```')
$lines.Add('')
$lines.Add('默认与官方版共存，无需管理员权限。[Windows 安装说明](https://github.com/JoyElliot/grok-build-Chinese/blob/zh-dev/packaging/windows/INSTALL-WINDOWS.md) · [macOS 安装说明](https://github.com/JoyElliot/grok-build-Chinese/blob/zh-dev/packaging/macos/INSTALL-MACOS.md) · [Linux 安装说明](https://github.com/JoyElliot/grok-build-Chinese/blob/zh-dev/packaging/linux/INSTALL-LINUX.md)')
$lines.Add('')
$lines.Add('</details>')
$parent = Split-Path -Parent $OutputPath
if ($parent) { [IO.Directory]::CreateDirectory($parent) | Out-Null }
[IO.File]::WriteAllText($OutputPath, (($lines -join "`n") + "`n"), (New-Object Text.UTF8Encoding($false)))
Write-Host "已生成 $CurrentTag 重点说明：$OutputPath"
