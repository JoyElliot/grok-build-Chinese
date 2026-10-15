# CI 轻量门禁与预览制品保留

## 工作流入口

- 主入口仍是 [CI](../.github/workflows/zh-dev-windows-preview.yml)。原“同步资料完整性”工作流已并入最早的 `localization` job，保留原有 39 项 Python 测试，并增加工作流与保留策略回归测试。
- `localization` 不等待 runner 让行门禁；Rust 格式预检同时依赖本地化成功与让行门禁成功，六平台构建再依赖格式预检。最终汇总显式检查本地化结果。
- `zh-dev`、原 macOS 冒烟分支、面向 `zh-dev` 的 PR、每五天的缓存预热和手动构建语义保持原样。
- `sync/**` 的 push 只运行轻量检查，不启动六平台编译，也不运行会将被跳过构建误报为失败的汇总 job。同步分支现在覆盖所有非公告/动态译文路径，比旧入口的指定路径范围更广。
- 独立的 [预览制品保留维护](../.github/workflows/preview-retention.yml) 只负责存储维护，不构建产品。CI 成功结束后由 `workflow_run` 唤起；每天 UTC 03:43 再检查一次；手动运行默认 `dry_run: true`。

维护独立出来是权限隔离：CI 和 PR 不获得 `actions: write`。维护工作流只运行 `zh-dev` 默认分支上的代码，检出固定的 `github.sha`，从不检出或执行触发构建的分支代码。

## 保留规则

六平台预览包上传时设置 90 天：

1. Windows x64 GNU：`grok-zh-windows-{run_id}-{attempt}`
2. Windows ARM64 MSVC：`grok-zh-{version}-windows-aarch64-msvc-{run_id}-{attempt}`
3. Linux x64 GNU：`grok-zh-{version}-linux-x86_64-gnu-{run_id}-{attempt}`
4. Linux ARM64 GNU：`grok-zh-{version}-linux-aarch64-gnu-{run_id}-{attempt}`
5. macOS ARM64：`grok-zh-{version}-macos-aarch64-{run_id}-{attempt}`
6. macOS Intel：`grok-zh-{version}-macos-x86_64-{run_id}-{attempt}`

对应 Windows 调试符号 `grok-zh-windows-symbols-{run_id}-{attempt}` 上传时设置 30 天。GNU/MSVC 交叉编译中间包保留 7 天。

同一分支有较新提交的完整成功构建后，旧预览和符号共用首次替换时间，截止时间分别为：

`min(首次被完整成功构建替换的时间 + 14 天, 该制品原始到期时间)`

例如旧构建第 29 天被替换，六平台包保留到替换后第 14 天（约第 43 天），符号仍在原第 30 天到期，不会延长到第 43 天。

- 新旧顺序用主 CI 的 `run_number` 判断，不用可被重跑改变的 `updated_at`。较早运行即使很晚才重跑成功，也不能替换较新运行；同一 run 的不同 attempt 不互相替换。替换者还必须具有不同的 `head_sha`，并由只读 compare API 证实旧提交是新提交的祖先；同一 SHA 对的结果在单次维护进程内缓存。较大 run_number 的旧提交手动构建、分叉/回退提交也不能覆盖较新提交。同一提交的定时预热或手动重建不会触发缩期。
- 替换者必须属于同仓库、同 CI 工作流、同分支，事件为 push、schedule 或 workflow_dispatch，整体成功，并通过六平台与原生验收/汇总 job。首次认证时，同一 attempt 的六个平台包和 Windows 符号必须完整、非空且未过期，维护工作流才保存完整性证明。
- 五个带版本号的平台包还必须版本一致。Windows GNU 名称本身不带版本号，依靠同 run/attempt 身份及已有原生打包验证门禁；维护脚本不下载或执行产品制品。
- 符号达到原生 30 天期限后，已保存的可信完整性证明允许仍有效的六平台包继续作为替换者。每次仍实时检查六个平台包的身份、存活情况、run/attempt、分支、提交以及验收 job；证明不能代替缺失或过期的平台包，不能用于别的 attempt，也不能解释提前消失的符号。缺少证明时保守不清理。
- 失败、取消、缺包、部分重跑、PR 与 fork 构建都不能替换正常构建。正在排队或执行重跑的旧 run 也不会被清理。
- 找不到仍然完整可用的更新构建时，不删除当前最后一套完整成功预览。没有后续更新时，六平台包原始 90 天、符号原始 30 天到期时间各自不变。
- 自动维护的分支白名单为 `zh-dev` 和既有 `codex/free-account-localization-macos-smoke`，相互独立。其他手动分支和 PR 的制品仍按上传期限自然过期，不参与此替换策略。
- 不回溯改变已经上传、原本只保留 7/14 天的旧预览，也不把旧 90 天符号改成新 30 天符号；GitHub 不支持通过此 API 延长或缩短已有制品到期时间。仓库的制品保留设置也必须允许新上传的 90 天配置。

预览诊断仅在相应构建/验证遇到失败时上传，不因手动触发或正常成功而上传；纯取消不作为失败上传的理由。最终失败处理位于监控结束、timings、打包、验证与缓存处理之后，覆盖这些后期步骤的失败，并保留已产生的证据。既有诊断上传期限保持原样（通常 7 天，Windows 验证 14 天）。Linux 预览的长期产品包不再混入短期诊断文本。

独立 Windows MSVC 迁移实验产品仍为 14 天，未纳入六平台缩期清理。共享构建 action 的产品保留天数参数默认仍为 14 天，新增预览诊断处理与正式 Release 隔离；Release 调用与上传条件保持原样。不会删除工作流运行、日志、缓存、Tag 或 Release。

## 首次替换时间、完整性证明与安全边界

脚本见 [preview-retention.py](../.github/scripts/preview-retention.py)。每次维护先生成计划，将首次替换时间与完整性证明保存在该维护工作流自身产生的 `preview-retention-state-v2-{run_id}-{attempt}` 小型 JSON 制品中，上传成功后才允许执行清理。状态制品保留 90 天；后续运行复制仍有效的记录，超过原制品到期时间才移除对应替换记录，证明保留至其六平台包全部到期。v2 状态不读取尚未发布的旧 v1 七天策略快照。

- 所有维护事件和维护重跑共用一个并发组，`cancel-in-progress: false`。GitHub 仍可能替换尚未启动的排队运行；下一次维护会重新盘点。
- 按状态制品 ID 选择最新可信快照，而不是按维护 run_number，因此旧维护运行的重跑也能接续最近状态。已经成功保存状态、随后失败或取消的维护运行仍可提供该快照。
- 状态来源必须是本仓库指定维护工作流、默认分支和受信事件。来自 PR、fork、其他分支或其他工作流的同名制品会被忽略。可信来源的损坏 JSON 或不合法状态会中止清理。
- ZIP 只读取唯一的 `state.json`，限制压缩前后大小，不解压到文件系统，不执行内容。GitHub CLI 负责下载，并在跨主机重定向时移除认证信息。
- 每次清理先重新盘点；每个 DELETE 前再次读取目标 run、替换者及其制品和任务状态、目标制品身份与当前到期时间。API 错误、分页不完整、重复/变化记录或身份不一致都会停止清理。历史盘点从仍存在的目标制品出发，避免遍历所有历史运行耗尽请求额度。
- GitHub 没有“只有替换者仍完整时才删除目标”的原子条件删除 API。检查到 DELETE 之间仍有极短的外部操作竞态；并发组只序列化本维护工作流，不能锁住维护者手工删除或重跑。
- 若所有状态快照被人为删除或过期，会用当前仍有七类有效制品的成功构建重建证明。符号已过期且无证明的构建不能直接恢复资格；无法恢复的历史首次替换时间也可能使清理延后；不会伪造更早时间来提前删除。原生到期仍是保留上限。
- 每日调度不是精确定时器，实际删除发生在截止时间后的首次成功维护；GitHub 延迟、API 故障或限流只会延后清理。没有更新时原生到期仍独立生效。

手动 dry-run 只读取产品/状态并生成计划，既不 DELETE，也不刷新状态快照；工作流仍会上传一个保留 7 天的计划报告。上传报告不是对产品制品的修改。

## 验证与首次部署

本地离线验证（不会调用 GitHub API），覆盖 14 天精确边界、第 29 天替换的混合期限、30 天符号到期后的替换/最后成功预览、证明缺失或伪造、重跑、首次替换时间不重置、分页与删除前竞态，以及预览失败诊断和 Release 隔离：

```sh
python3 -B -m unittest discover -s .github/scripts/tests -p 'test_*.py'
python3 -B packaging/macos/tests/test_install_lock.py
```

发布到默认分支后，`workflow_run` 和每日调度会按配置自动执行维护；手动 `dry_run: true` 仅约束该次手动运行，不是全局暂停开关。若需上线前仅观察，应先只读运行脚本，或暂时关闭自动触发后手动 dry-run 审阅计划，再启用自动维护。维护不另外 dispatch 主 CI；新构建仍按原触发规则运行。代码审查或本地测试并不表示已经发布或执行过任何真实清理。

参考：[GitHub 制品 API](https://docs.github.com/en/rest/actions/artifacts)、[提交比较 API](https://docs.github.com/en/rest/commits/commits#compare-two-commits)、[工作流触发安全注意事项](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_run)、[Actions 限额](https://docs.github.com/en/actions/reference/limits)。
