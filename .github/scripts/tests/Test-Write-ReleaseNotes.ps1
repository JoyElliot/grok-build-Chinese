$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Assert-True([bool] $Condition, [string] $Message) {
    if (!$Condition) {
        throw "断言失败：$Message"
    }
}

function Assert-Contains([string] $Text, [string] $Expected, [string] $Message) {
    Assert-True $Text.Contains($Expected) "$Message；缺少：$Expected"
}

function Assert-NotContains([string] $Text, [string] $Unexpected, [string] $Message) {
    Assert-True (!$Text.Contains($Unexpected)) "$Message；不应包含：$Unexpected"
}

function Assert-Throws([scriptblock] $Action, [string] $Expected, [string] $Message) {
    try {
        & $Action
    } catch {
        Assert-Contains $_.Exception.Message $Expected $Message
        return
    }
    throw "断言失败：$Message；预期抛出异常。"
}

$repoRoot = (& git rev-parse --show-toplevel).Trim()
if ($LASTEXITCODE -ne 0 -or !$repoRoot) {
    throw '必须在 Git 仓库中运行 Release notes 测试。'
}
$generator = Join-Path $repoRoot '.github\scripts\write-release-notes.ps1'
$publishedTags = @(
    'v0.2.121-zh.ci.6',
    'v1.0.0-zh.preview.3',
    'v1.0.0-zh.preview.4',
    'v1.0.0-zh.preview.5',
    'v1.0.0-zh.preview.10'
)
$repository = 'example/grok-build-Chinese'
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) "grok-zh-release-notes-$([Guid]::NewGuid().ToString('N'))"
[IO.Directory]::CreateDirectory($tempRoot) | Out-Null

try {
    $preview3Path = Join-Path $tempRoot 'preview3.md'
    & $generator `
        -CurrentTag 'v1.0.0-zh.preview.3' `
        -OutputPath $preview3Path `
        -Repository $repository `
        -PublishedReleaseTags $publishedTags
    $preview3 = Get-Content -LiteralPath $preview3Path -Raw

    Assert-Contains $preview3 '## 本次更新' '正文必须使用中文区块标题'
    Assert-Contains $preview3 '## Windows 中文安装' '正文必须提供在线安装入口'
    $installCommand = [IO.File]::ReadAllText((Join-Path $repoRoot 'packaging/windows/ONLINE-INSTALL-COMMAND.txt'), [Text.Encoding]::UTF8).Trim()
    Assert-Contains $preview3 $installCommand 'Release 正文必须使用统一的一行安装命令'
    Assert-Contains $preview3 '此入口始终安装最新正式版' '历史或预览页必须说明在线入口版本选择'
    Assert-Contains $preview3 '## 上游更新' '上游合并必须生成独立区块'
    Assert-Contains $preview3 "[本地化 Windows 预览工作流](https://github.com/$repository/commit/a13165f9a03faec5c815e1cbddbd7fdb57e29643)" '本地提交标题必须翻译并链接完整 SHA'
    Assert-Contains $preview3 "[同步上游 1.0.0 并完成中文本地化](https://github.com/$repository/commit/983bc53f89efde6692faabf2f7ac90fde8fd3f4e)" '上游 merge 提交必须翻译并链接'
    Assert-Contains $preview3 '[查看上游变更范围 393430e...8a14c91](https://github.com/xai-org/grok-build/compare/393430ee4934bc791b0d538f304a21691c517433...8a14c91d88875a831a38b3a066b1683116bcb31c)' '上游更新必须包含 compare 链接'
    Assert-Contains $preview3 '[同步上游代码快照](https://github.com/xai-org/grok-build/commit/afbc0fb710320c7add294c2106d447ecc3e3af2e)' '上游提交必须翻译并链接'
    Assert-Contains $preview3 '[同步上游代码快照](https://github.com/xai-org/grok-build/commit/8a14c91d88875a831a38b3a066b1683116bcb31c)' '上游 tip 必须翻译并链接'
    foreach ($englishSubject in @(
        'ci: localize Windows preview workflow',
        'merge: sync upstream 1.0.0 and complete Chinese localization',
        'Synced from monorepo'
    )) {
        Assert-NotContains $preview3 $englishSubject 'Release 正文不得混入英文原始提交标题'
    }
    $preview3Bytes = [IO.File]::ReadAllBytes($preview3Path)
    Assert-True (!($preview3Bytes.Length -ge 3 -and $preview3Bytes[0] -eq 0xEF -and $preview3Bytes[1] -eq 0xBB -and $preview3Bytes[2] -eq 0xBF)) 'Release notes 必须使用 UTF-8 无 BOM'

    $preview4Path = Join-Path $tempRoot 'preview4.md'
    & $generator `
        -CurrentTag 'v1.0.0-zh.preview.4' `
        -OutputPath $preview4Path `
        -Repository $repository `
        -PublishedReleaseTags $publishedTags
    $preview4 = Get-Content -LiteralPath $preview4Path -Raw
    Assert-Contains $preview4 "[发布前检查草稿发布](https://github.com/$repository/commit/e4b838ebaddf6802483c87e249a9cd8bec7ab131)" '英文提交必须使用中文映射'
    Assert-Contains $preview4 "[将社区发布切换为仅 ZIP 资产](https://github.com/$repository/commit/9f8a1850a56a35ae8983c3ee7bf6f60782abb016)" '第二个英文提交必须使用中文映射'
    Assert-NotContains $preview4 '## 上游更新' '普通发布区间不得伪造上游更新'
    Assert-NotContains $preview4 'fix: inspect draft releases before publishing' '正文不得保留未翻译英文标题'

    $emptyMap = Join-Path $tempRoot 'empty-map.json'
    [IO.File]::WriteAllText($emptyMap, '{"schema":1,"entries":[]}', (New-Object Text.UTF8Encoding($false)))

    $fixtureRepo = Join-Path $tempRoot 'same-commit-repo'
    [IO.Directory]::CreateDirectory($fixtureRepo) | Out-Null
    & git -C $fixtureRepo init --quiet
    & git -C $fixtureRepo config user.name 'Release Notes Test'
    & git -C $fixtureRepo config user.email 'release-notes-test@example.invalid'
    [IO.File]::WriteAllText((Join-Path $fixtureRepo 'fixture.txt'), 'fixture', (New-Object Text.UTF8Encoding($false)))
    & git -C $fixtureRepo add fixture.txt
    & git -C $fixtureRepo commit --quiet -m '创建中文测试提交'
    & git -C $fixtureRepo tag 'v1.0.0-zh.preview.1'
    & git -C $fixtureRepo tag 'v1.0.0'
    if ($LASTEXITCODE -ne 0) { throw '无法创建隔离的同提交 Tag fixture。' }
    Push-Location $fixtureRepo
    try {
        $sameCommitPath = Join-Path $tempRoot 'same-commit.md'
        & $generator `
            -CurrentTag 'v1.0.0' `
            -OutputPath $sameCommitPath `
            -Repository $repository `
            -TranslationMapPath $emptyMap `
            -PublishedReleaseTags @('v1.0.0-zh.preview.1')
        $sameCommit = Get-Content -LiteralPath $sameCommitPath -Raw
        Assert-Contains $sameCommit '- （无新增提交）' '同一提交提升为新 Tag 时不应重复上一版提交'
        Assert-NotContains $sameCommit '创建中文测试提交' '同提交 Tag 不得重复已有 Release 内容'
    } finally {
        Pop-Location
    }

    [IO.File]::WriteAllText((Join-Path $fixtureRepo 'fixture.txt'), 'fixture-2', (New-Object Text.UTF8Encoding($false)))
    & git -C $fixtureRepo add fixture.txt
    & git -C $fixtureRepo commit --quiet -m '修复 <img src=x> & 链接'
    & git -C $fixtureRepo tag 'v1.0.1'
    Push-Location $fixtureRepo
    try {
        $escapedTitlePath = Join-Path $tempRoot 'escaped-title.md'
        & $generator `
            -CurrentTag 'v1.0.1' `
            -OutputPath $escapedTitlePath `
            -Repository $repository `
            -TranslationMapPath $emptyMap `
            -PublishedReleaseTags @('v1.0.0')
        $escapedTitle = Get-Content -LiteralPath $escapedTitlePath -Raw
        Assert-Contains $escapedTitle '修复 &lt;img src=x&gt; &amp; 链接' '提交标题中的 HTML 必须转义'
        Assert-NotContains $escapedTitle '<img src=x>' '正文不得注入原始 HTML'
    } finally {
        Pop-Location
    }

    [IO.File]::WriteAllText((Join-Path $fixtureRepo 'fixture.txt'), 'fixture-3', (New-Object Text.UTF8Encoding($false)))
    & git -C $fixtureRepo add fixture.txt
    & git -C $fixtureRepo commit --quiet -m '验证后续发布标签'
    & git -C $fixtureRepo tag 'release-v1.0.9'
    Push-Location $fixtureRepo
    try {
        $modernTagPath = Join-Path $tempRoot 'modern-tag.md'
        & $generator `
            -CurrentTag 'release-v1.0.9' `
            -OutputPath $modernTagPath `
            -Repository $repository `
            -TranslationMapPath $emptyMap `
            -PublishedReleaseTags @('v1.0.1')
        $modernTag = Get-Content -LiteralPath $modernTagPath -Raw
        Assert-Contains $modernTag '验证后续发布标签' 'release-v* 后续标签必须可生成更新日志'
    } finally {
        Pop-Location
    }

    Push-Location $fixtureRepo
    try {
        $fixtureTree = (& git rev-parse 'HEAD^{tree}').Trim()
        $baseCommit = (& git rev-parse 'v1.0.1').Trim()
        $localCommit = (& git rev-parse 'release-v1.0.9').Trim()
        function New-NestedFixtureCommit([string] $Subject, [string[]] $Parents) {
            $commitArgs = @('commit-tree', $fixtureTree, '-m', $Subject)
            foreach ($parentCommit in $Parents) { $commitArgs += @('-p', $parentCommit) }
            $commit = & git @commitArgs
            if ($LASTEXITCODE -ne 0) { throw '无法创建嵌套合并 fixture 提交。' }
            return $commit.Trim()
        }
        $upstreamCommit = New-NestedFixtureCommit 'Upstream change' @($baseCommit)
        $innerMerge = New-NestedFixtureCommit '同步已审核上游' @($localCommit, $upstreamCommit)
        $outerMerge = New-NestedFixtureCommit '合并同步审查 PR' @($localCommit, $innerMerge)
        $laterCommit = New-NestedFixtureCommit '后续中文修复' @($outerMerge)
        & git tag 'release-v1.0.10' $outerMerge
        & git tag 'release-v1.0.11' $laterCommit
        if ($LASTEXITCODE -ne 0) { throw '无法创建嵌套合并 fixture 标签。' }
        $nestedMapPath = Join-Path $tempRoot 'nested-map.json'
        $nestedMap = @{
            schema = 1
            entries = @(@{ sha = $upstreamCommit; source_subject = 'Upstream change'; title_zh = '上游中文更新' })
            upstream_merges = @(@{
                merge_sha = $innerMerge; first_parent = $localCommit
                upstream_tip = $upstreamCommit; upstream_base = $baseCommit
            })
            local_merges = @($outerMerge)
        }
        function Write-NestedFixtureMap {
            [IO.File]::WriteAllText($nestedMapPath, ($nestedMap | ConvertTo-Json -Depth 6), (New-Object Text.UTF8Encoding($false)))
        }
        Write-NestedFixtureMap
        $nestedPath = Join-Path $tempRoot 'nested.md'
        $nestedArgs = @{
            CurrentTag = 'release-v1.0.10'; OutputPath = $nestedPath
            Repository = $repository; TranslationMapPath = $nestedMapPath
            PublishedReleaseTags = @('release-v1.0.9')
        }
        & $generator @nestedArgs
        $nested = Get-Content -LiteralPath $nestedPath -Raw
        Assert-Contains $nested "[合并同步审查 PR](https://github.com/$repository/commit/$outerMerge)" '外层 PR 必须保留在本次更新中'
        Assert-Contains $nested '## 上游更新' '嵌套的已审核上游合并必须生成独立区块'
        Assert-Contains $nested "https://github.com/xai-org/grok-build/compare/$baseCommit...$upstreamCommit" '嵌套合并必须保留真实上游范围'
        $upstreamLink = "https://github.com/xai-org/grok-build/commit/$upstreamCommit"
        Assert-True (([regex]::Matches($nested, [regex]::Escape($upstreamLink))).Count -eq 1) '每条上游提交必须只列一次'
        Assert-NotContains $nested "https://github.com/xai-org/grok-build/commit/$outerMerge" '本地 PR 不得伪装为上游提交'

        $nestedMap.local_merges = @()
        Write-NestedFixtureMap
        Assert-Throws { & $generator @nestedArgs } '尚未在中文映射中分类' '未分类外层合并仍必须阻止发布'
        $nestedMap.local_merges = @($outerMerge)
        $nestedMap.upstream_merges[0].first_parent = $baseCommit
        Write-NestedFixtureMap
        Assert-Throws { & $generator @nestedArgs } '父提交与已审核定义不一致' '嵌套合并仍须校验父提交'
        $nestedMap.upstream_merges[0].first_parent = $localCommit
        $nestedMap.upstream_merges[0].upstream_base = $localCommit
        Write-NestedFixtureMap
        Assert-Throws { & $generator @nestedArgs } 'merge-base 与已审核定义不一致' '嵌套合并仍须校验上游基线'
        $nestedMap.upstream_merges[0].upstream_base = $baseCommit
        Write-NestedFixtureMap

        $nestedArgs.PublishedReleaseTags = @()
        & $generator @nestedArgs
        Assert-NotContains (Get-Content -LiteralPath $nestedPath -Raw) '## 上游更新' '首个 Release 不得回溯嵌套祖先'
        $nestedArgs.CurrentTag = 'release-v1.0.11'
        $nestedArgs.PublishedReleaseTags = @('release-v1.0.10')
        & $generator @nestedArgs
        Assert-NotContains (Get-Content -LiteralPath $nestedPath -Raw) '## 上游更新' '已发布的嵌套上游合并不得再次列入'
    } finally {
        Pop-Location
    }

    foreach ($invalidTag in @(
        'V1.0.0.1',
        'v1.0.0.0',
        'v1.0.0.01',
        'v1.0.0.1.2',
        'v1.0.0.1-alpha.1',
        'v1.0.0.1١',
        'v1.0.0.18446744073709551616',
        'v1.0.0-alpha.18446744073709551616'
    )) {
        Assert-Throws {
            & $generator `
                -CurrentTag $invalidTag `
                -OutputPath (Join-Path $tempRoot 'invalid-tag.md') `
                -Repository $repository `
                -TranslationMapPath $emptyMap `
                -PublishedReleaseTags @()
        } 'CurrentTag 必须' "非三段 Release Tag 必须被拒绝：$invalidTag"
    }

    $invalidSchemaMap = Join-Path $tempRoot 'invalid-schema-map.json'
    [IO.File]::WriteAllText($invalidSchemaMap, '{"schema":true,"entries":[]}', (New-Object Text.UTF8Encoding($false)))
    Assert-Throws {
        & $generator `
            -CurrentTag 'v0.2.121-zh.ci.6' `
            -OutputPath (Join-Path $tempRoot 'invalid-schema.md') `
            -Repository $repository `
            -TranslationMapPath $invalidSchemaMap `
            -PublishedReleaseTags @()
    } 'schema 必须为 1' '映射 schema 必须严格使用整数 1'

    $invalidEntriesMap = Join-Path $tempRoot 'invalid-entries-map.json'
    [IO.File]::WriteAllText(
        $invalidEntriesMap,
        '{"schema":1,"entries":{"sha":"4072da692c799c4fa9eaa469b89af6aec9dcc56d"}}',
        (New-Object Text.UTF8Encoding($false))
    )
    Assert-Throws {
        & $generator `
            -CurrentTag 'v0.2.121-zh.ci.6' `
            -OutputPath (Join-Path $tempRoot 'invalid-entries.md') `
            -Repository $repository `
            -TranslationMapPath $invalidEntriesMap `
            -PublishedReleaseTags @()
    } '缺少 entries 数组' '映射 entries 必须严格使用 JSON 数组'

    $firstReleasePath = Join-Path $tempRoot 'first-release.md'
    & $generator `
        -CurrentTag 'v0.2.121-zh.ci.6' `
        -OutputPath $firstReleasePath `
        -Repository $repository `
        -PublishedReleaseTags @()
    $firstRelease = Get-Content -LiteralPath $firstReleasePath -Raw
    Assert-Contains $firstRelease "[完成简体中文文档](https://github.com/$repository/commit/4072da692c799c4fa9eaa469b89af6aec9dcc56d)" '首个 Release 只能记录当前 Tag 提交'
    Assert-NotContains $firstRelease '同步上游并与官方版共享 Grok 用户数据' '首个 Release 不得回溯全部历史'

    Assert-Throws {
        & $generator `
            -CurrentTag 'v0.2.121-zh.ci.6' `
            -OutputPath (Join-Path $tempRoot 'must-fail.md') `
            -Repository $repository `
            -TranslationMapPath $emptyMap `
            -PublishedReleaseTags $publishedTags
    } '英文标题没有中文映射' '未翻译英文提交必须阻止发布'

    'Release notes 中文、提交链接与上游更新测试通过。'
} finally {
    if (Test-Path -LiteralPath $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
}
