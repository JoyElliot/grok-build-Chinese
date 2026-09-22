# 动态界面译文

模型、远端提示和命令标签、订阅门槛、官方托管 MCP、官方技能与官方插件市场的中文显示映射。官方继续负责目录、权限、能力、价格和功能行为；本目录只更新界面文字。模型名、命令、工具 ID、参数、URL、用户内容和发给模型的正文保持原值。条款及同意正文不通过此目录覆盖。

每个子目录（`models`、`settings`、`mcp`、`skills`、`marketplace`）有独立 `manifest.json` 和不可变 `catalogs/<version>.json`。第一次需要安装支持本功能的客户端，此后维护译文无需发布新程序。发布路径固定为本仓库 `zh-dev/community/display-translations`，与公告一样通过 GitHub Raw 分发。

## 维护

复制对应目录的最新完整 catalog，递增 `version`；保留 `schema_version: 1`、`locale: "zh-CN"` 和对应 `domain`。每条记录含 `field`、`context`、`translation`，以及二选一的 `source`（准确原文）或 `source_sha256`（原文 UTF-8 SHA-256）。不做大小写、空白或标点归一化。修改官方原文后，旧译文不再匹配。

| domain | field | context |
|---|---|---|
| models | description | 模型 ID |
| models | effort_label / effort_description | 模型 ID、推理选项 ID |
| settings | tip / gate_message / gate_label | 空数组 |
| settings | command_tag | canonical 命令名 |
| mcp | connector_label | connector ID |
| mcp | tool_label / tool_description | connector ID、tool ID、官方完整说明 SHA-256 |
| skills | label / description | 官方技能 slug（bundled 或产品内置） |
| marketplace | description / category | 插件名、官方仓库相对路径 |

生成摘要并校验，例如：

```text
python -B .github/scripts/validate-display-translations.py --domain models --write-manifest 2 --base HEAD
python -B .github/scripts/validate-display-translations.py --base HEAD
```

审核后将新 catalog 与 manifest 一起推送到 `zh-dev`。旧版本不可修改、删除或回退；撤回译文应发布更高版本并删除对应记录。每个 catalog 最大 256 KiB、512 条，单条文字最大 16 KiB。校验工作流不是分发开关，直接推送后 Raw 可用即可能被客户端读取。

## 客户端

复用公告的异步下载、SHA-256 校验、缓存原子替换、超时、版本防回退和失败保留机制。各域检查只由对应官方数据的实际加载/刷新触发，不另设轮询间隔、不等待译文才显示官方数据。重复触发合并；网络失败等下一次官方加载再试。启动先用内置版本和已验证缓存，译文更新后重绘界面。

缓存位于 `~/.grok/cache/grok-zh/<domain>-translations.json`，遵守 `GROK_HOME`。固定 HTTPS 请求不携带账号令牌、不跟随重定向；连接超时 2 秒、单请求 5 秒、一轮最多 8 秒。设置 `GROK_ZH_TRANSLATIONS_OFFLINE=1`、`GROK_ZH_ANNOUNCEMENTS_OFFLINE=1` 或 `GROK_CHANGELOG_OFFLINE=1` 可关闭新增译文网络请求，缓存仍可用。

只对已验证的官方来源应用映射。自定义模型、用户上传技能、本地/第三方 MCP 和自定义市场保持原样。新增一种客户端不支持的模型能力或协议仍需要升级程序。
