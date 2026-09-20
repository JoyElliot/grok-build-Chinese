# Release 说明维护

每个版本在 `versions/<Tag>.json` 保存经过整理的中文重点。CI 只校验与排版，不调用模型、不把提交标题直接拼成正文。缺少说明时停止发布，不能回退为提交清单。

## 内容规则

- 社区版通常保留 3–5 条用户可见变化，上游通常保留 4–6 条。没有足够变化时减少条数，不为凑数拆分同一修复。
- 同一功能的多次提交合成一条；不列 CI、格式、测试接线、审查与合并步骤。先修后撤的方案以最终行为为准。
- 上游以实际同步提交正文的 `Changes` 为依据，结合官方版本日志核对条件；保留平台、模式和账号限制。不能只翻译 `Synced from monorepo` 标题。
- 正式版比较本 Fork 上一个正式版与本次发布的差异，期间 Preview/RC 的重点必须在正式版重新纳入。定期同步跳过中间版本时，必须阅读并汇总完整上游区间；例如从 1.0.24 升到 1.0.35，应覆盖 1.0.25–1.0.35，不能只写最新单版日志。同类变化合并为重点。
- `previous_tag` 使用符合发布类型的最近 first-parent 祖先：正式版仅选择正式 Release，预发布可选择上次正式或预发布 Release。只有 Git 标签而没有 Release 的失败尝试不是基线。类型以 GitHub 的 `prerelease` 元数据为准，不能仅凭 Tag 拼写判断；历史 `release-v1.0.12` 实际为 rc2 预发布。
- 正文直接从本 Fork 的更新条目开始，不加“社区版重点”标题。保留“上游更新”标题，但版本区间只在末尾比较链接出现一次。
- 重要安装限制放入 `notices`，已知问题放入 `known_issues`。RC 不设置“安装与兼容性”区块或重复强调预发布身份；rc2 的 Linux 升级问题保留为“已知问题”。历史安装包的能力按当时版本写，不能套用现在的平台支持。
- 完整提交 SHA 保存在来源字段中用于核验，页面仅显示重点与末尾的完整变更链接。长安装命令收进折叠区。

## 数据格式

```json
{
  "schema": 1,
  "tag": "release-v1.0.36",
  "prerelease": false,
  "previous_tag": "release-v1.0.35",
  "community": [
    {"text": "具体用户可见的中文变化。", "commits": ["完整的40位来源提交SHA"]}
  ],
  "upstream": [
    {
      "base": "先前包含的上游提交SHA",
      "tip": "本次包含的上游提交SHA",
      "label": "上游 1.0.36",
      "highlights": [
        {"text": "依据上游 Changes 整理的中文变化。", "commits": ["范围内的官方提交SHA"]}
      ]
    }
  ],
  "notices": ["确有必要的安装或兼容性说明。"]
}
```

没有符合类型的已发布基线时，`previous_tag` 为 `null`；首个正式版应汇总此前预发布的重要变化。`prerelease` 为布尔值，编辑历史页面时必须与 GitHub 状态一致。没有社区改动或上游更新时，对应数组可为空，但两者不能同时为空。已有中文单条翻译可以复用，必须核对其来源确实处于本次范围。

上游 `base` 必须等于上一版发布提交与本次上游 `tip` 的唯一最近共同祖先（`git merge-base --all`）；不能选择更老基线重复计入已有更新。尚未创建标签的版本说明可以先提交，回归会明确记录尚未核验该版；发布计划在标签存在后执行完整来源校验。

本地检查运行 `pwsh -NoProfile -File .github/scripts/tests/Test-Write-ReleaseNotes.ps1`。发布脚本继续通过 `write-release-notes.ps1 -CurrentTag <Tag> -OutputPath <Path>` 生成正文；离线检查可传 `-PublishedReleases`，其对象须包含 GitHub 的 `tag_name`、`prerelease`、`draft`、`immutable`、`published_at` 元数据，不能仅传标签列表。测试可用 `-NotesPath` 指定说明文件。

`commit-titles.zh-CN.json` 保留为历史上游合并审查资料，不再驱动公开正文。
