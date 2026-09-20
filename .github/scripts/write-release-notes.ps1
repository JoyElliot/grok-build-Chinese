[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $CurrentTag,

    [Parameter(Mandatory = $true)]
    [string] $OutputPath,

    [string] $Repository = $env:GITHUB_REPOSITORY,

    [string] $GitHubToken = $env:GH_TOKEN,

    [string] $TranslationMapPath,

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
if (!$TranslationMapPath) {
    $githubRoot = Split-Path -Parent $PSScriptRoot
    $TranslationMapPath = Join-Path (Join-Path $githubRoot 'release-notes') 'commit-titles.zh-CN.json'
}
if (!(Test-Path -LiteralPath $TranslationMapPath -PathType Leaf)) {
    throw "缺少提交标题中文映射：$TranslationMapPath"
}

function Invoke-Git {
    param(
        [Parameter(Mandatory = $true)]
        [string[]] $Arguments,
        [switch] $AllowFailure
    )

    $output = @(& git @Arguments 2>&1)
    $exitCode = $LASTEXITCODE
    $global:LASTEXITCODE = 0
    if ($exitCode -ne 0 -and !$AllowFailure) {
        throw "git $($Arguments -join ' ') 失败（exit $exitCode）：$($output -join [Environment]::NewLine)"
    }
    [pscustomobject]@{
        ExitCode = $exitCode
        Lines = @($output | ForEach-Object { $_.ToString() })
    }
}

function Get-CommitRecord([string] $Commit) {
    $shaResult = Invoke-Git -Arguments @('rev-parse', '--verify', "$Commit^{commit}")
    $sha = $shaResult.Lines[0].Trim().ToLowerInvariant()
    if ($sha -notmatch '^[0-9a-f]{40}$') {
        throw "Git 未返回有效的完整提交 SHA：$Commit -> $sha"
    }
    $parentsResult = Invoke-Git -Arguments @('rev-list', '--parents', '-n', '1', $sha)
    $parts = @($parentsResult.Lines[0].Trim() -split '\s+')
    $subjectResult = Invoke-Git -Arguments @(
        '-c', 'i18n.logOutputEncoding=UTF-8',
        'show', '-s', '--format=%s', $sha
    )
    [pscustomobject]@{
        Sha = $sha
        ShortSha = $sha.Substring(0, 7)
        Parents = @($parts | Select-Object -Skip 1)
        Subject = $subjectResult.Lines[0]
    }
}

function Read-TranslationMap([string] $Path) {
    $document = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if (!$document.PSObject.Properties['schema'] -or
        ($document.schema -isnot [int] -and $document.schema -isnot [long]) -or
        $document.schema -ne 1) {
        throw "提交标题中文映射 schema 必须为 1：$Path"
    }
    if (!$document.PSObject.Properties['entries'] -or
        $null -eq $document.entries -or
        $document.entries -isnot [System.Collections.IList]) {
        throw "提交标题中文映射缺少 entries 数组：$Path"
    }

    $bySha = @{}
    foreach ($entry in @($document.entries)) {
        $sha = ([string]$entry.sha).ToLowerInvariant()
        $sourceSubject = [string]$entry.source_subject
        $title = [string]$entry.title_zh
        if ($sha -notmatch '^[0-9a-f]{40}$') {
            throw "中文映射包含无效提交 SHA：$sha"
        }
        if ($bySha.ContainsKey($sha)) {
            throw "中文映射包含重复提交 SHA：$sha"
        }
        if ([string]::IsNullOrWhiteSpace($sourceSubject) -or $sourceSubject -match '[\x00-\x1F\x7F]') {
            throw "中文映射 source_subject 必须是非空单行文本：$sha"
        }
        if ([string]::IsNullOrWhiteSpace($title) -or $title -match '[\x00-\x1F\x7F]' -or !(Test-ContainsChinese $title)) {
            throw "中文映射 title_zh 必须是包含中文的非空单行文本：$sha"
        }
        $bySha[$sha] = [pscustomobject]@{
            SourceSubject = $sourceSubject
            Title = $title
        }
    }
    return $bySha
}

$translations = Read-TranslationMap $TranslationMapPath

function Resolve-ChineseTitle($Record) {
    if ($Record.Subject -match '^[A-Za-z][A-Za-z0-9_-]*(?:\([^)]+\))?!?:\s*(?<body>.+)$' -and
        (Test-ContainsChinese $matches.body)) {
        return $matches.body
    }
    if (Test-ContainsChinese $Record.Subject) {
        return $Record.Subject
    }
    if (!$translations.ContainsKey($Record.Sha)) {
        throw "提交 $($Record.ShortSha) 的英文标题没有中文映射：$($Record.Subject)"
    }
    $translation = $translations[$Record.Sha]
    if ($translation.SourceSubject -cne $Record.Subject) {
        throw "提交 $($Record.ShortSha) 的 source_subject 与 Git 历史不一致：map=$($translation.SourceSubject) git=$($Record.Subject)"
    }
    return $translation.Title
}

function New-CommitLine($Record, [string] $Repo) {
    $title = ConvertTo-MarkdownLinkText (Resolve-ChineseTitle $Record)
    return "- [$title](https://github.com/$Repo/commit/$($Record.Sha)) (``$($Record.ShortSha)``)"
}

function Get-PublishedReleaseTags {
    if (!$GitHubToken -or !$Repository) {
        throw '必须提供 GitHub token 和 repository，防止把失败但残留的 Git tag 误当作已发布基线。'
    }

    $headers = @{
        Accept = 'application/vnd.github+json'
        Authorization = "Bearer $GitHubToken"
        'X-GitHub-Api-Version' = '2022-11-28'
    }
    $tags = [System.Collections.Generic.List[string]]::new()
    for ($page = 1; ; $page++) {
        $uri = "https://api.github.com/repos/$Repository/releases?per_page=100&page=$page"
        $pageResult = Invoke-RestMethod -Method Get -Uri $uri -Headers $headers
        $releases = @($pageResult)
        foreach ($release in $releases) {
            $tagName = [string]$release.tag_name
            $tagVersion = Get-ReleaseTagVersion $tagName
            $tagPrerelease = $tagVersion -and $tagVersion.Contains('-')
            if (!$release.draft -and $release.immutable -and
                ([bool]$release.prerelease -eq $tagPrerelease) -and
                $tagVersion) {
                $tags.Add([string]$release.tag_name)
            }
        }
        if ($releases.Count -lt 100) { break }
    }
    return @($tags)
}

$currentCommitRecord = Get-CommitRecord $CurrentTag
$currentCommit = $currentCommitRecord.Sha
if ($PSBoundParameters.ContainsKey('PublishedReleaseTags')) {
    $publishedTags = @($PublishedReleaseTags)
    foreach ($tag in $publishedTags) {
        if (!(Test-ReleaseTag $tag)) {
            throw "PublishedReleaseTags 包含无效 Tag：$tag"
        }
    }
} else {
    $publishedTags = @(Get-PublishedReleaseTags)
}

$previousTag = $null
$firstParentCommits = Invoke-Git -Arguments @('rev-list', '--first-parent', $currentCommit) -AllowFailure
if ($firstParentCommits.ExitCode -eq 0) {
    $publishedByCommit = @{}
    foreach ($tag in $publishedTags) {
        if ($tag -eq $CurrentTag) { continue }
        $tagCommitResult = Invoke-Git -Arguments @('rev-parse', '--verify', "$tag^{commit}") -AllowFailure
        if ($tagCommitResult.ExitCode -ne 0 -or $tagCommitResult.Lines.Count -eq 0) {
            throw "已发布 Release Tag 无法在本地 Git 历史中解析：$tag"
        }
        $tagCommit = $tagCommitResult.Lines[0].Trim().ToLowerInvariant()
        if (!$publishedByCommit.ContainsKey($tagCommit)) {
            $publishedByCommit[$tagCommit] = $tag
        }
    }
    foreach ($commit in $firstParentCommits.Lines) {
        $candidate = $commit.Trim().ToLowerInvariant()
        if ($publishedByCommit.ContainsKey($candidate)) {
            $previousTag = $publishedByCommit[$candidate]
            break
        }
    }
}

if ($previousTag) {
    $range = "$previousTag..$CurrentTag"
    $log = Invoke-Git -Arguments @('rev-list', '--reverse', '--first-parent', $range)
    $releaseCommitIds = @($log.Lines | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    Write-Host "Release notes 基线：$previousTag"
} else {
    $releaseCommitIds = @($currentCommit)
    Write-Host 'Release notes 基线：无已发布 Release，仅记录当前 Tag 提交。'
}

$releaseCommits = @($releaseCommitIds | ForEach-Object { Get-CommitRecord $_ })
$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add('## 本次更新')
$lines.Add('')
if ($releaseCommits.Count -eq 0) {
    $lines.Add('- （无新增提交）')
} else {
    foreach ($record in $releaseCommits) {
        $lines.Add((New-CommitLine $record $Repository))
    }
}

$upstreamDefinitions = @{}
$localMergeDefinitions = @{}
$mapDocument = Get-Content -LiteralPath $TranslationMapPath -Raw | ConvertFrom-Json
if ($mapDocument.PSObject.Properties['upstream_merges']) {
    if ($null -eq $mapDocument.upstream_merges -or
        $mapDocument.upstream_merges -isnot [System.Collections.IList]) {
        throw "提交标题中文映射 upstream_merges 必须是数组：$TranslationMapPath"
    }
    foreach ($definition in @($mapDocument.upstream_merges)) {
        $mergeSha = ([string]$definition.merge_sha).ToLowerInvariant()
        $firstParent = ([string]$definition.first_parent).ToLowerInvariant()
        $upstreamTip = ([string]$definition.upstream_tip).ToLowerInvariant()
        $upstreamBase = ([string]$definition.upstream_base).ToLowerInvariant()
        foreach ($value in @($mergeSha, $firstParent, $upstreamTip, $upstreamBase)) {
            if ($value -notmatch '^[0-9a-f]{40}$') {
                throw "上游合并定义包含无效 SHA：$value"
            }
        }
        if ($upstreamDefinitions.ContainsKey($mergeSha)) {
            throw "上游合并定义包含重复 merge_sha：$mergeSha"
        }
        $upstreamDefinitions[$mergeSha] = [pscustomobject]@{
            FirstParent = $firstParent
            Tip = $upstreamTip
            Base = $upstreamBase
        }
    }
}
if ($mapDocument.PSObject.Properties['local_merges']) {
    if ($null -eq $mapDocument.local_merges -or
        $mapDocument.local_merges -isnot [System.Collections.IList]) {
        throw "提交标题中文映射 local_merges 必须是数组：$TranslationMapPath"
    }
    foreach ($merge in @($mapDocument.local_merges)) {
        $mergeSha = ([string]$merge).ToLowerInvariant()
        if ($mergeSha -notmatch '^[0-9a-f]{40}$') {
            throw "本地合并定义包含无效 SHA：$mergeSha"
        }
        if ($localMergeDefinitions.ContainsKey($mergeSha) -or $upstreamDefinitions.ContainsKey($mergeSha)) {
            throw "合并提交存在重复分类：$mergeSha"
        }
        $localMergeDefinitions[$mergeSha] = $true
    }
}

$upstreamGroups = [System.Collections.Generic.List[object]]::new()
$seenUpstreamCommits = @{}
foreach ($record in $releaseCommits) {
    if ($record.Parents.Count -gt 1 -and
        !$upstreamDefinitions.ContainsKey($record.Sha) -and
        !$localMergeDefinitions.ContainsKey($record.Sha)) {
        throw "合并提交 $($record.ShortSha) 尚未在中文映射中分类为 upstream_merges 或 local_merges。"
    }
}

# A reviewed upstream merge can arrive inside a local PR merge. Keep the main
# release list on the first-parent chain, but discover audited upstream merges
# throughout the new range. A first release still describes only its own commit.
$upstreamMergeIds = @($currentCommit)
if ($previousTag) {
    $mergeLog = Invoke-Git -Arguments @('rev-list', '--reverse', '--topo-order', '--merges', $range)
    $upstreamMergeIds = @($mergeLog.Lines | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}
foreach ($mergeId in $upstreamMergeIds) {
    if (!$upstreamDefinitions.ContainsKey($mergeId)) {
        continue
    }
    $record = Get-CommitRecord $mergeId

    $definition = $upstreamDefinitions[$record.Sha]
    if ($record.Parents.Count -ne 2 -or
        $record.Parents[0] -cne $definition.FirstParent -or
        $record.Parents[1] -cne $definition.Tip) {
        throw "上游合并 $($record.ShortSha) 的父提交与已审核定义不一致。"
    }
    $baseResult = Invoke-Git -Arguments @('merge-base', $definition.FirstParent, $definition.Tip)
    $actualBase = $baseResult.Lines[0].Trim().ToLowerInvariant()
    if ($actualBase -cne $definition.Base) {
        throw "上游合并 $($record.ShortSha) 的 merge-base 与已审核定义不一致：map=$($definition.Base) git=$actualBase"
    }
    $upstreamResult = Invoke-Git -Arguments @('rev-list', '--reverse', '--topo-order', "$($definition.Base)..$($definition.Tip)")
    $upstreamCommits = [System.Collections.Generic.List[object]]::new()
    foreach ($commit in $upstreamResult.Lines) {
        $upstreamRecord = Get-CommitRecord $commit.Trim()
        if (!$seenUpstreamCommits.ContainsKey($upstreamRecord.Sha)) {
            $seenUpstreamCommits[$upstreamRecord.Sha] = $true
            $upstreamCommits.Add($upstreamRecord)
        }
    }
    $upstreamGroups.Add([pscustomobject]@{
        MergeTitle = Resolve-ChineseTitle $record
        Base = $definition.Base
        Tip = $definition.Tip
        Commits = $upstreamCommits
    })
}

if ($upstreamGroups.Count -gt 0) {
    $lines.Add('')
    $lines.Add('## 上游更新')
    foreach ($group in $upstreamGroups) {
        $lines.Add('')
        $lines.Add("### $(ConvertTo-MarkdownLinkText $group.MergeTitle)")
        $compareText = "$($group.Base.Substring(0, 7))...$($group.Tip.Substring(0, 7))"
        $lines.Add("- [查看上游变更范围 $compareText](https://github.com/$UpstreamRepository/compare/$($group.Base)...$($group.Tip))")
        foreach ($upstreamRecord in $group.Commits) {
            $lines.Add((New-CommitLine $upstreamRecord $UpstreamRepository))
        }
    }
}

$commandPath = Join-Path $PSScriptRoot '../../packaging/windows/ONLINE-INSTALL-COMMAND.txt'
$installCommand = [IO.File]::ReadAllText($commandPath, [Text.Encoding]::UTF8).Trim()
if (!$installCommand -or $installCommand.Contains("`n") -or $installCommand.Contains("`r")) {
    throw 'Windows 在线安装命令必须为非空单行。'
}
$lines.Add('')
$lines.Add('## Windows 中文安装')
$lines.Add('')
$lines.Add('在 PowerShell 中粘贴下面一行，按中文菜单安装、更新或创建便携版。此入口始终安装最新正式版；本页为历史版或预发布时，请从下方附件手动下载对应版本。')
$lines.Add('')
$lines.Add('```powershell')
$lines.Add($installCommand)
$lines.Add('```')
$lines.Add('')
$lines.Add('默认与官方版共存，无需管理员权限。完整说明见 [Windows 安装文档](https://github.com/JoyElliot/grok-build-Chinese/blob/zh-dev/packaging/windows/INSTALL-WINDOWS.md)。')

$parent = Split-Path -Parent $OutputPath
if ($parent) {
    [IO.Directory]::CreateDirectory($parent) | Out-Null
}
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText(
    $OutputPath,
    (($lines -join [Environment]::NewLine) + [Environment]::NewLine),
    $utf8NoBom
)

$linkedCommitCount = $releaseCommits.Count + $seenUpstreamCommits.Count
Write-Host "Release notes 已写入 $OutputPath（$linkedCommitCount 条带链接提交）。"
