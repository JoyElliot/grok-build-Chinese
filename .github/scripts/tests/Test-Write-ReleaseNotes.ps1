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
function New-Release([string] $Tag, [bool] $Prerelease, [int] $Day) {
    return [pscustomobject]@{
        tag_name = $Tag; prerelease = $Prerelease; draft = $false; immutable = $true
        published_at = ([datetimeoffset]'2026-01-01T00:00:00Z').AddDays($Day).ToString('o')
    }
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
    & git tag release-v1.0.0 $local
    & git tag v1.0.1-rc.1 $current
    & git tag v1.0.1 $current
    & git tag v1.0.2 $future
    & git tag v1.0.9 $current
    $notesPath = Join-Path $tempRoot 'notes.json'
    $outputPath = Join-Path $tempRoot 'notes.md'
    $valid = @{
        schema = 1; tag = 'v1.0.1'; prerelease = $false; previous_tag = 'v1.0.0'
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
        OutputPath = $outputPath; NotesPath = $notesPath; PublishedReleases = @(New-Release 'v1.0.0' $false 1)
    }
    Write-Notes $valid
    & $generator @invoke
    $body = Get-Content -LiteralPath $outputPath -Raw
    Assert-True $body.StartsWith('- ') '正文直接从更新内容开始'
    Assert-NotContains $body '## 社区版重点' '不再强调本 Fork 更新标题'
    Assert-Contains $body '## 上游更新' '嵌套上游合并仍显示独立重点'
    Assert-Contains $body '修复 &lt;img src=x&gt; &amp; 中文显示' '重点必须转义 HTML'
    Assert-Contains $body "https://github.com/xai-org/grok-build/compare/$base...$upstream" '上游保留完整比较链接'
    Assert-Contains $body '/compare/v1.0.0...v1.0.1' '社区基线比较链接'
    Assert-NotContains $body '上游 1.0.1：' '正文不重复列上游区间'
    Assert-True (([regex]::Matches($body, '上游 1\.0\.1')).Count -eq 1) '上游区间仅在比较链接出现一次'
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
        (New-Release 'v1.0.0' $false 1),
        (New-Release 'release-v1.0.0' $true 2),
        (New-Release 'v1.0.1-rc.1' $true 3),
        (New-Release 'v1.0.1' $false 4),
        (New-Release 'v1.0.9' $false 5)
    )
    function Invoke-RestMethod {
        param($Method, $Uri, $Headers)
        Write-Output -NoEnumerate $apiReleases
    }
    $apiInvoke = $invoke.Clone()
    $apiInvoke.Remove('PublishedReleases')
    $apiInvoke.GitHubToken = 'fixture-token'
    & $generator @apiInvoke
    Assert-Contains (Get-Content $outputPath -Raw) '/compare/v1.0.0...v1.0.1' '正式版跳过预发布和后续同 SHA 标签'
    $preview = Reset-Notes; $preview.tag = 'v1.0.1-rc.1'; $preview.prerelease = $true
    $preview.previous_tag = 'release-v1.0.0'; $preview.community[0].commits = @($current)
    Write-Notes $preview
    $apiInvoke.CurrentTag = 'v1.0.1-rc.1'
    & $generator @apiInvoke
    Assert-Contains (Get-Content $outputPath -Raw) '/compare/release-v1.0.0...v1.0.1-rc.1' '预发布仍使用上次实际发布，即使前序 Tag 不含 rc'
    $legacyPreview = Reset-Notes; $legacyPreview.tag = 'release-v1.0.0'; $legacyPreview.prerelease = $true
    $legacyPreview.upstream = @(); Write-Notes $legacyPreview
    $apiInvoke.CurrentTag = 'release-v1.0.0'
    & $generator @apiInvoke
    Assert-Contains (Get-Content $outputPath -Raw) '/compare/v1.0.0...release-v1.0.0' '当前 Tag 无 rc 后缀时仍服从真实预发布状态'
    $badStatus = Reset-Notes; $badStatus.prerelease = $true; Write-Notes $badStatus
    $apiInvoke.CurrentTag = 'v1.0.1'
    Assert-Throws { & $generator @apiInvoke } 'prerelease 与 GitHub Release 状态不一致'
    Remove-Item Function:Invoke-RestMethod

    Write-Notes $valid
    $invalidMetadata = $invoke.Clone()
    $invalidMetadata.PublishedReleases = @([pscustomobject]@{
        tag_name = 'v1.0.0'; prerelease = 'false'; draft = $false; immutable = $true; published_at = '2026-01-01T00:00:00Z'
    })
    Assert-Throws { & $generator @invalidMetadata } 'Release 元数据 prerelease 必须为布尔值'
    $invalidMetadata.PublishedReleases = @([pscustomobject]@{ tag_name = 'v1.0.0'; prerelease = $false; draft = $false; immutable = $true })
    Assert-Throws { & $generator @invalidMetadata } 'Release 元数据缺少字段：published_at'

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
    $bad = Reset-Notes; $bad.prerelease = 'false'; Write-Notes $bad
    Assert-Throws { & $generator @invoke } 'prerelease 必须为布尔值'
    $bad = Reset-Notes; $bad.tag = 'v1.0.1-rc.1'; Write-Notes $bad
    $invalidKind = $invoke.Clone(); $invalidKind.CurrentTag = $bad.tag
    Assert-Throws { & $generator @invalidKind } '尚未发布版本的 prerelease 必须与标签版本一致'
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
    $cumulativeInvoke.CurrentTag = 'v1.0.3'
    $cumulativeInvoke.PublishedReleases = @((New-Release 'v1.0.1' $false 4), (New-Release 'v1.0.0' $false 1))
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
    $invoke.CurrentTag = 'v1.0.0'; $invoke.PublishedReleases = @()
    & $generator @invoke
    Assert-NotContains (Get-Content $outputPath -Raw) '/compare/' '首发无虚构基线'

    $promotion = Reset-Notes; Write-Notes $promotion
    $invoke.CurrentTag = 'v1.0.1'
    $invoke.PublishedReleases = @((New-Release 'v1.0.1-rc.1' $true 3), (New-Release 'v1.0.0' $false 1))
    & $generator @invoke
    $promotedBody = Get-Content $outputPath -Raw
    Assert-Contains $promotedBody '/compare/v1.0.0...v1.0.1' '同提交从 RC 提升到正式版仍对照上个正式版'
    Assert-Contains $promotedBody '中文显示' '正式版重复纳入预发布社区更新'
    Assert-Contains $promotedBody '改善上游会话恢复' '正式版重复纳入预发布上游更新'
    $promotion.previous_tag = $null; Write-Notes $promotion
    $invoke.PublishedReleases = @(New-Release 'v1.0.1-rc.1' $true 3)
    & $generator @invoke
    Assert-Contains (Get-Content $outputPath -Raw) '改善上游会话恢复' '首个正式版也汇总此前全部预发布重点'

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
    $metadata = [Collections.Generic.List[object]]::new()
    $metadata.Add((New-Release $tag $records[$tag].Document.prerelease 100))
    $previous = $records[$tag].Document.previous_tag
    while ($previous) {
        Assert-True (!$earlier.Contains($previous) -and $previous -cne $tag) '版本基线不能循环'
        Assert-True $records.ContainsKey($previous) '前序版本必须保留说明'
        $earlier.Add($previous)
        $metadata.Add((New-Release $previous $records[$previous].Document.prerelease (100 - $earlier.Count)))
        $previous = $records[$previous].Document.previous_tag
    }
    & $generator -CurrentTag $tag -Repository 'JoyElliot/grok-build-Chinese' `
        -NotesPath $records[$tag].Path -PublishedReleases $metadata.ToArray() `
        -OutputPath (Join-Path $tempRoot "$tag.md")
    $rendered = Get-Content (Join-Path $tempRoot "$tag.md") -Raw
    Assert-NotContains $rendered '## 社区版重点' '历史版本也移除重复标题'
    if ($tag -in @('release-v1.0.12', 'release-v1.0.12-rc.1')) {
        Assert-NotContains $rendered '## 安装与兼容性' 'RC 不再单设安装兼容性区块'
        Assert-NotContains $rendered '预发布' 'RC 不重复强调预发布身份'
    }
    if ($tag -eq 'release-v1.0.12') {
        Assert-Contains $rendered '## 已知问题' 'rc2 保留已知问题'
        Assert-Contains $rendered 'Linux 归档超过旧更新器解包限制' 'rc2 已知升级限制必须保留'
    }
    $validatedCount++
}
Write-Host "Release notes 回归通过；已核验 $validatedCount 份已打标签版本说明的来源和范围。"
