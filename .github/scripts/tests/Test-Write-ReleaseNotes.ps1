$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Assert-True([bool] $Condition, [string] $Message) {
    if (!$Condition) { throw "断言失败：$Message" }
}
function Assert-Contains([string] $Text, [string] $Expected, [string] $Message) {
    Assert-True $Text.Contains($Expected) "$Message；缺少：$Expected"
}
function Assert-NotContains([string] $Text, [string] $Unexpected, [string] $Message) {
    Assert-True (!$Text.Contains($Unexpected)) "$Message；不应包含：$Unexpected"
}
function Assert-Throws([scriptblock] $Action, [string] $Expected) {
    try { & $Action } catch {
        Assert-Contains $_.Exception.Message $Expected '错误必须说明具体原因'
        return
    }
    throw "断言失败：预期异常 $Expected"
}

$repoRoot = (& git rev-parse --show-toplevel).Trim()
if ($LASTEXITCODE -ne 0 -or !$repoRoot) { throw '必须在 Git 仓库中运行 Release notes 测试。' }
$generator = Join-Path $repoRoot '.github/scripts/write-release-notes.ps1'
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) "grok-zh-release-notes-$([Guid]::NewGuid().ToString('N'))"
$fixtureRepo = Join-Path $tempRoot 'repo'
[IO.Directory]::CreateDirectory($fixtureRepo) | Out-Null
# Retain the small fixture for diagnostics; CI runner teardown handles its lifetime.
Write-Host "Release notes 测试目录：$tempRoot"
& git -C $fixtureRepo init --quiet
& git -C $fixtureRepo config user.name 'Release Notes Test'
& git -C $fixtureRepo config user.email 'release-notes-test@example.invalid'
[IO.File]::WriteAllText((Join-Path $fixtureRepo 'fixture.txt'), 'fixture')
& git -C $fixtureRepo add fixture.txt
& git -C $fixtureRepo commit --quiet -m 'internal base commit'
if ($LASTEXITCODE -ne 0) { throw '创建 fixture 失败。' }

Push-Location $fixtureRepo
try {
    $tree = (& git rev-parse 'HEAD^{tree}').Trim()
    $base = (& git rev-parse HEAD).Trim()
    function New-Commit([string] $Message, [string[]] $Parents) {
        $arguments = @('commit-tree', $tree, '-m', $Message)
        foreach ($parentCommit in $Parents) { $arguments += @('-p', $parentCommit) }
        $result = & git @arguments
        if ($LASTEXITCODE -ne 0) { throw '创建 fixture 提交失败。' }
        return $result.Trim()
    }
    $local = New-Commit 'internal local commit' @($base)
    $upstream = New-Commit 'Synced from monorepo' @($base)
    $inner = New-Commit 'internal upstream merge' @($local, $upstream)
    $current = New-Commit 'internal outer PR merge' @($local, $inner)
    $future = New-Commit 'future changes' @($current)
    & git tag v1.0.0 $base
    & git tag v1.0.1-rc.1 $current
    & git tag v1.0.1 $current
    & git tag v1.0.2 $future
    $notesPath = Join-Path $tempRoot 'notes.json'
    $outputPath = Join-Path $tempRoot 'notes.md'
    $valid = @{
        schema = 1; tag = 'v1.0.1'; previous_tag = 'v1.0.0'
        community = @(@{ text = '修复 <img src=x> & 中文显示'; commits = @($local) })
        upstream = @(@{
            base = $base; tip = $upstream; label = '上游 1.0.1'
            highlights = @(@{ text = '改善上游会话恢复。'; commits = @($upstream) })
        })
    }
    $validJson = $valid | ConvertTo-Json -Depth 10
    function Reset-Notes { return $validJson | ConvertFrom-Json -AsHashtable }
    function Write-Notes($Document) {
        [IO.File]::WriteAllText($notesPath, ($Document | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    }
    $invoke = @{
        CurrentTag = 'v1.0.1'; Repository = 'example/grok-build-Chinese'
        OutputPath = $outputPath; NotesPath = $notesPath; PublishedReleaseTags = @('v1.0.0')
    }
    Write-Notes $valid
    & $generator @invoke
    $body = Get-Content -LiteralPath $outputPath -Raw
    Assert-Contains $body '## 社区版重点' '正文保留社区重点'
    Assert-Contains $body '## 上游更新' '嵌套上游合并仍显示独立重点'
    Assert-Contains $body '修复 &lt;img src=x&gt; &amp; 中文显示' '重点必须转义 HTML'
    Assert-Contains $body "https://github.com/xai-org/grok-build/compare/$base...$upstream" '上游保留完整比较链接'
    Assert-Contains $body '/compare/v1.0.0...v1.0.1' '社区基线比较链接'
    Assert-Contains $body '<summary>下载与安装</summary>' '安装说明折叠'
    Assert-Contains $body '始终安装最新正式版' '历史页面说明在线命令选择最新正式版'
    $install = [IO.File]::ReadAllText((Join-Path $repoRoot 'packaging/windows/ONLINE-INSTALL-COMMAND.txt')).Trim()
    Assert-Contains $body $install '统一在线安装命令'
    foreach ($hidden in @('/commit/', 'Synced from monorepo', 'internal outer PR merge', '<img src=x>')) {
        Assert-NotContains $body $hidden '正文不得变回提交清单或注入 HTML'
    }
    $bytes = [IO.File]::ReadAllBytes($outputPath)
    Assert-True (!($bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF)) 'UTF-8 无 BOM'

    # Invoke-RestMethod emits a JSON array as one pipeline object. Include a
    # historical prerelease with a stable-looking tag and a later same-SHA tag.
    $apiReleases = @(
        [pscustomobject]@{ tag_name = 'v1.0.0'; draft = $false; immutable = $true; prerelease = $true; published_at = '2026-01-01T00:00:00Z' },
        [pscustomobject]@{ tag_name = 'v1.0.1'; draft = $false; immutable = $true; prerelease = $false; published_at = '2026-01-02T00:00:00Z' },
        [pscustomobject]@{ tag_name = 'v1.0.1-rc.1'; draft = $false; immutable = $true; prerelease = $true; published_at = '2026-01-03T00:00:00Z' },
        [pscustomobject]@{ tag_name = 'v1.0.2'; draft = $true; immutable = $false; prerelease = $false; published_at = $null }
    )
    function Invoke-RestMethod {
        param($Method, $Uri, $Headers)
        Write-Output -NoEnumerate $apiReleases
    }
    $apiInvoke = $invoke.Clone()
    $apiInvoke.Remove('PublishedReleaseTags')
    $apiInvoke.GitHubToken = 'fixture-token'
    & $generator @apiInvoke
    Assert-Contains (Get-Content $outputPath -Raw) '/compare/v1.0.0...v1.0.1' 'API 数组展开、历史预发布基线与时间截断'
    Remove-Item Function:Invoke-RestMethod

    $bad = Reset-Notes; $bad.community[0].commits = @($future); Write-Notes $bad
    Assert-Throws { & $generator @invoke } '来源提交未包含在目标版本'
    $bad = Reset-Notes; $bad.community[0].commits = @($base); Write-Notes $bad
    Assert-Throws { & $generator @invoke } '来源提交已属于上一版'
    $bad = Reset-Notes; $bad.upstream[0].highlights[0].commits = @($local); Write-Notes $bad
    Assert-Throws { & $generator @invoke } '来源提交未包含在目标版本'
    $bad = Reset-Notes; $bad.upstream[0].base = $future; Write-Notes $bad
    Assert-Throws { & $generator @invoke } '不是当前版本包含的有效祖先范围'
    $bad = Reset-Notes; $bad.upstream += $bad.upstream[0]; Write-Notes $bad
    Assert-Throws { & $generator @invoke } '上游范围重复'
    $bad = Reset-Notes; $bad.previous_tag = $null; Write-Notes $bad
    Assert-Throws { & $generator @invoke } '基线与已发布历史不一致'
    $bad = Reset-Notes; $bad.schema = $true; Write-Notes $bad
    Assert-Throws { & $generator @invoke } 'schema 必须为整数 1'
    $bad = Reset-Notes; $bad.tag = 'v1.0.2'; Write-Notes $bad
    Assert-Throws { & $generator @invoke } 'tag 与当前标签不一致'
    $bad = Reset-Notes; $bad.community[0].text = 'English only'; Write-Notes $bad
    Assert-Throws { & $generator @invoke } '非空单行文本'
    $bad = Reset-Notes; $bad.community[0].text = "中文`n换行"; Write-Notes $bad
    Assert-Throws { & $generator @invoke } '非空单行文本'
    $bad = Reset-Notes; $bad.community = @(); $bad.upstream = @(); Write-Notes $bad
    Assert-Throws { & $generator @invoke } '至少需要一条社区或上游重点'
    $onlyUpstream = Reset-Notes; $onlyUpstream.community = @(); Write-Notes $onlyUpstream
    & $generator @invoke
    Assert-NotContains (Get-Content $outputPath -Raw) '## 社区版重点' '只有上游更新时不捏造社区改动'

    # A fork may skip upstream releases. Both intermediate and latest changes
    # belong in its next release, but already published upstream changes do not.
    $middleUpstream = New-Commit 'intermediate upstream version' @($upstream)
    $latestUpstream = New-Commit 'latest upstream version' @($middleUpstream)
    $nextFork = New-Commit 'next periodic upstream sync' @($current, $latestUpstream)
    & git tag v1.0.3 $nextFork
    $cumulative = Reset-Notes
    $cumulative.tag = 'v1.0.3'; $cumulative.previous_tag = 'v1.0.1'; $cumulative.community = @()
    $cumulative.upstream[0].base = $upstream; $cumulative.upstream[0].tip = $latestUpstream
    $cumulative.upstream[0].highlights = @(
        @{ text = '纳入中间版本新增能力。'; commits = @($middleUpstream) },
        @{ text = '纳入最新版本恢复修复。'; commits = @($latestUpstream) }
    )
    $cumulativeInvoke = $invoke.Clone()
    $cumulativeInvoke.CurrentTag = 'v1.0.3'; $cumulativeInvoke.PublishedReleaseTags = @('v1.0.1', 'v1.0.0')
    Write-Notes $cumulative
    & $generator @cumulativeInvoke
    $cumulativeBody = Get-Content $outputPath -Raw
    Assert-Contains $cumulativeBody '纳入中间版本新增能力' '跨版本同步不能只展示最新单版'
    Assert-Contains $cumulativeBody '纳入最新版本恢复修复' '保留最新版本重点'
    Assert-Contains $cumulativeBody "/compare/$upstream...$latestUpstream" '覆盖上次到本次的完整上游范围'
    $cumulative.upstream[0].base = $base
    Write-Notes $cumulative
    Assert-Throws { & $generator @cumulativeInvoke } '上游基线必须等于上一版与本次上游的共同祖先'

    $first = Reset-Notes; $first.tag = 'v1.0.0'; $first.previous_tag = $null
    $first.community[0].commits = @($base); $first.upstream = @(); Write-Notes $first
    $invoke.CurrentTag = 'v1.0.0'; $invoke.PublishedReleaseTags = @()
    & $generator @invoke
    Assert-NotContains (Get-Content $outputPath -Raw) '/compare/' '首发无虚构基线'

    $promotion = Reset-Notes; $promotion.previous_tag = 'v1.0.1-rc.1'
    $promotion.community[0] = @{ text = '预发布验证完成，提升为正式版。'; commits = @($current) }
    $promotion.upstream = @(); Write-Notes $promotion
    $invoke.CurrentTag = 'v1.0.1'; $invoke.PublishedReleaseTags = @('v1.0.1-rc.1', 'v1.0.0')
    & $generator @invoke
    Assert-Contains (Get-Content $outputPath -Raw) '/compare/v1.0.1-rc.1...v1.0.1' '支持同提交正式提升'

    foreach ($invalid in @('V1.0.0', 'v1.0.0.1', 'v01.0.0', 'v1.0.0+build', 'v1.0.0-rc.01', 'v1.0.0-alpha.18446744073709551616')) {
        $invoke.CurrentTag = $invalid
        Assert-Throws { & $generator @invoke } 'CurrentTag 必须'
    }
    $invoke.CurrentTag = 'v1.0.1'; $invoke.NotesPath = Join-Path $tempRoot 'missing.json'
    Assert-Throws { & $generator @invoke } '缺少经过整理的中文发布说明'
} finally { Pop-Location }

# Validate every curated version against the real Git history without network access.
$records = @{}
foreach ($file in Get-ChildItem (Join-Path $repoRoot '.github/release-notes/versions') -Filter '*.json') {
    $record = Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json
    Assert-True ($file.BaseName -ceq $record.tag) '文件名必须与 Tag 一致'
    $records[$record.tag] = @{ Document = $record; Path = $file.FullName }
}
$validatedCount = 0
foreach ($tag in ($records.Keys | Sort-Object)) {
    & git show-ref --verify --quiet "refs/tags/$tag"
    if ($LASTEXITCODE -eq 1) {
        Write-Host "尚未创建 $tag 标签；实际范围由发布计划在打标签后核验。"
        $global:LASTEXITCODE = 0
        continue
    }
    if ($LASTEXITCODE -ne 0) { throw "无法检查标签：$tag" }
    $earlier = [Collections.Generic.List[string]]::new()
    $previous = $records[$tag].Document.previous_tag
    while ($previous) {
        Assert-True (!$earlier.Contains($previous) -and $previous -cne $tag) '版本基线不能循环'
        Assert-True $records.ContainsKey($previous) '前序版本必须保留说明'
        $earlier.Add($previous)
        $previous = $records[$previous].Document.previous_tag
    }
    & $generator -CurrentTag $tag -Repository 'JoyElliot/grok-build-Chinese' `
        -NotesPath $records[$tag].Path -PublishedReleaseTags $earlier.ToArray() `
        -OutputPath (Join-Path $tempRoot "$tag.md")
    $validatedCount++
}
Write-Host "Release notes 回归通过；已核验 $validatedCount 份已打标签版本说明的来源和范围。"
