# 斜杠命令

在提示框中输入 `/` 打开命令菜单。菜单会随着输入进行模糊匹配，选中命令后会立即运行。

命令来自两个地方：由智能体后端（xai-grok-shell）处理的**Shell 内置命令**，以及由 pager 前端（xai-grok-pager）处理的**pager 内置命令**。两者会显示在同一个菜单中；任何启用且带有 `user-invocable: true` 的技能也会显示在那里。如果技能复用了 `login` 这样的内置名称，内置命令保留 `/login`，技能则以 `/plugin-name:login` 的形式可用——菜单会为两者显示徽章，从而让冲突可见。

下面每个命令都会列出自己的别名（如有）。有些命令只有在某项功能或会话状态启用时才会出现，文中会直接说明。菜单还会按渲染模式过滤——参见[`/minimal` 和 `/fullscreen`](#minimal-and-fullscreen)。

---

## 会话管理

### `/new`

开始新会话并清除当前对话。别名：`/clear`。

### `/resume`

打开会话选择器，从磁盘重新加载之前的会话。

### `/dashboard`

打开[智能体 dashboard](23-dashboard.md)：此 pager 中顶层会话的实时列表（查看、回复、dispatch、固定、重命名、停止、附加）。别名：`/agents-dashboard`、`/sessions`。

它不同于管理智能体*定义*和 persona 的 `/config-agents`（别名 `/agents`）。精简模式下隐藏；可通过 `GROK_AGENT_DASHBOARD=0` 或 `[dashboard].enabled = false` 禁用。

### `/compact [context]`

压缩对话历史以回收上下文窗口空间。传入备注可告诉 Grok 要保留什么：

```
/compact
/compact 保留身份验证实现细节
```

当上下文窗口达到 85% 时，Grok 也会自动压缩（使用 `[session] auto_compact_threshold_percent` 调整）。

### `/context`

显示上下文窗口的使用方式：按类别拆分（系统提示、消息、推理和开销、可用空间），并列出工具定义、技能列表和 MCP 服务器公告及其预计令牌成本等信息行。

### `/session-info`

显示会话详情——身份验证方式、模型、轮次数以及上下文用量。别名：`/status`、`/info`。

### `/fork`

将当前会话分支到新的智能体，并保留截至当前的历史记录。

### `/rewind`（别名：`/undo`）

将对话回退到较早的轮次，并丢弃其后的所有内容。`/undo` 是同一个命令。

### `/edit-prompt`

在任一渲染模式中为提示打开外部编辑器。Grok 会依次解析 `$VISUAL`、`$EDITOR` 和 `vi`；命令值可以包含带引号的参数。保存会替换草稿但不发送，保存空文件会清除草稿。在编写器中输入 `/edit-prompt` 必然会替换原有内容，因此编辑器从空草稿开始；若要编辑**已有**草稿，请从命令面板选择**在外部编辑器中编辑提示**（精简模式也可按 `Ctrl+G`）。该路径会保留文本，并拒绝在不扁平化附件的情况下处理粘贴内容、文件引用或图像芯片。

```
/edit-prompt
```

### `/copy`

将最近一条回复的 Markdown 源文本复制到剪贴板。传入数字可复制倒数第 N 条回复，或传入文件路径将文本写入文件而不是剪贴板（通过 SSH 时本地剪贴板通常不可达，这很方便）。

```
/copy
/copy 2
/copy out.txt
/copy 2 ~/exports/last-reply.md
```

每次复制也会写入备份文件——默认是 `~/.grok/last-copy.txt`，或使用已设置的 `GROK_COPY_FILE`。已确认的复制操作会短暂显示提示（例如 `Copied!`）。未验证的 OSC 52 传送和剪贴板不可达时的回退会标出备份路径，方便恢复文本。

### `/export`

将对话导出到文件或剪贴板。

### `/quit`

退出应用。别名：`/exit`。

### `/home`

离开当前会话并返回欢迎界面。别名：`/welcome`。

### `/delete`

删除当前会话的历史记录。操作前会确认。删除前会停止正在运行的回合、后台任务和子智能体，再清除历史记录。返回欢迎界面；如果会话是从 dashboard 打开的，则返回 dashboard。

要删除不在其中的会话，请打开 `/resume` 或欢迎界面的会话列表，然后按 `d` 再按 `y`。在 dashboard 上按两次 `Ctrl+X`，或点击 `[✗]`。

### `/rename`

重命名当前会话。别名：`/title`。

```
/rename 新会话标题
```

---

## 模型和模式

### `/model <name>`

切换模型。接受模型 ID 或显示名称（不区分大小写）；对于推理模型，还可将工作量级别作为第二个参数。别名：`/m`。

```
/model grok-build
/model Grok Build
/model Reasoning X high
```

### `/effort <level>`

在不重新选择模型的情况下，为**当前**模型设置推理工作量。级别为 `low`、`medium`、`high` 和 `xhigh`，且仅在活动模型支持推理工作量时生效。

```
/effort high
```

### `/always-approve` 和 `/auto`

两者都是权限模式的真实切换项：它们会留在菜单中，再次运行当前已经启用的模式会将其关闭。

| 命令 | 关闭时 | 已启用时 |
|---|---|---|
| `/always-approve` | 跳过所有权限提示 | 返回 ask |
| `/auto` | 分类器批准安全工具（危险工具仍可能提示） | 返回 ask |

另一个模式处于活动状态时运行其中一个会切换模式——例如始终批准已启用时运行 `/auto` 会切换到 auto。只有启用 auto 权限模式功能时才会显示 `/auto`。你还可以使用 `Shift+Tab`（循环 Normal / Plan / Auto（启用时）/ Always-approve）、`Ctrl+O` 或 `/settings` 更改模式。

### `/multiline`

切换多行输入。启用后，`Enter` 插入换行，`Shift+Enter`（或 `Alt+Enter`）发送消息。轮次中途，空编写器中的普通 `Enter` 仍会强制发送队列首条后续提示。别名：`/ml`。

### `/history`

打开提示历史搜索：按最新到最旧的顺序对本会话的提示进行模糊搜索，然后按 `Enter` 或 `Tab` 将匹配项放回提示框。

快速召回时，也可在空提示框中按 `↑`。有排队提示时，该按键会把焦点移入队列面板并选中最后一行；否则，面板打开时会预填最近一条提示，`↑`/`↓` 在条目间移动（每个条目都会落入输入框），在最新条目之后按 `↓` 会关闭面板，输入会就地编辑召回的提示。

### `/compact-mode`

切换紧凑显示——减少内边距并收紧间距，使输出更密集。

### `/vim-mode`

切换 Vim 风格的回滚区按键（`j`/`k`、`h`/`l`、`g`/`G`、`y`/`Y` 等）。关闭时（默认），在回滚区中直接按字母或 `Shift+letter` 只会聚焦提示框并输入该字符。设置会持久化到 `[ui] vim_mode`。

<a id="minimal-and-fullscreen"></a>
### `/minimal` 和 `/fullscreen`

在当前进程中将会话切换到另一种渲染模式。`/minimal`（在全屏模式中提供）切换到实验性的回滚区原生模式；`/fullscreen`（在精简模式中提供；别名 `/full`）切回标准全屏模式。切换不会重启进程，因此正在运行的轮次会继续流式输出，编写器草稿、排队提示和权限模式也会保留；标记（精简模式中的已提交行、全屏模式中的 toast）会提醒如何切回。两者都只影响本次会话，不会修改 `config.toml`；`--minimal` / `--fullscreen` CLI 标志的作用域同样限于会话。要让普通 `grok-zh` 默认以指定模式打开，请使用 `/settings` → **默认屏幕模式**，或设置 `[ui] screen_mode`。（如果进程内切换在特殊终端中表现异常，可设置 `GROK_SCREEN_MODE_SWITCH=exec` 恢复旧的重启 pager 行为。）

有少数命令只能在两种模式之一中工作，因为另一种模式不存在它们驱动的界面：`/find`、`/jump`、`/timeline`、`/theme`、`/tutorial` 和 `/dashboard` 仅限全屏，而 `/expand` 仅限精简。`/workflow runs` 不同：它在全屏模式打开运行面板，在精简模式降级为文本概览，而不是拒绝执行。这些命令在不能运行它们的模式下会从命令菜单和面板中隐藏。即使你直接输入，Grok 也会说明原因，并指向真正有用的选项。如果只有另一种模式可用，就会提示切换模式：`/theme isn't available in minimal mode (minimal renders with your terminal's own palette). Run /fullscreen to switch this session.` 如果当前模式已经能以其他方式完成任务，则会改为说明该方式：`/expand isn't available in fullscreen mode: press Tab to focus the scrollback, then → on the block.` 其他命令在两种模式下都可用。注意，`--no-alt-screen` 在此仍算全屏，因此会保留仅限全屏的命令。

### `/plan`

进入计划模式。

```
/plan [description]
```

### `/view-plan`

打开当前已保存计划的预览。别名：`/show-plan`、`/plan-view`。

---

## 记忆

`/flush`、`/dream` 和 `/memory` 要求启用记忆（`--experimental-memory` 或 `GROK_MEMORY=1`）；`/memory` 还需要配置记忆后端。`/remember` 始终可用。

### `/memory`

浏览、查看和管理已保存的记忆。传入 `on` 或 `off` 可启用或禁用记忆。别名：`/mem`。

```
/memory
/memory off
```

### `/flush`

立即将当前会话的知识保存到记忆中，触发 LLM 对最重要内容进行摘要。在压缩前或任何想锁定上下文的时候使用。

### `/dream`

运行记忆整合——将会话日志合并为有组织的主题。

### `/remember`

立即将备注保存到记忆中，无需等待自动摘要。

```
/remember 暂存部署使用 eu-west 集群
```

---

## Hooks 和插件

`/hooks`、`/plugins`、`/marketplace`、`/skills` 和 `/workflows` 都会打开同一个扩展模态框，只是分别位于各自的标签页。

### `/hooks`

在 Hooks 标签页打开扩展模态框，可查看已加载的 hook、添加或删除自定义 hook，并分别切换它们。该模态框不会授予项目信任——信任模型见 [10-hooks.md](10-hooks.md)。

Shell 还会宣传单独的 `/hooks-list`、`/hooks-trust`、`/hooks-add`、`/hooks-remove` 和 `/hooks-untrust` 命令；在 pager 中，它们会折叠到 `/hooks` 模态框中。

### `/plugins`

在 Plugins 标签页打开扩展模态框，查看已安装插件、从 marketplace 安装新插件并管理信任。

Shell 还支持子命令（`/plugins list`、`/plugins install <source>`、`/plugins uninstall <name>`、`/plugins update`、`/plugins reload`）。在 pager 中，模态框以可视方式完成相同工作。

### `/marketplace`

在 Marketplace 标签页打开扩展模态框，以浏览和安装插件。

### `/skills`

在 Skills 标签页打开扩展模态框，查看已安装技能。

---

## 媒体生成

### `/imagine <description>`

根据文本描述生成图像。

```
/imagine 平静海面上空的金色夕阳，前景是棕榈树剪影
```

### `/imagine-video <description>`

根据文本（或图像）描述生成视频。它会规划镜头、生成源图像，并使用 `image_to_video` 将其制作成动画。

```
/imagine-video 爵士俱乐部里一只正在弹钢琴的猫
```

---

## 调度

### `/loop [interval] <prompt>`

按固定间隔循环运行提示。间隔可写为 `30m`、`1 hour` 或 `every 2 days`；省略时 Grok 会询问。

```
/loop 30m 检查部署状态
/loop 每小时检查部署状态
```

间隔支持 `Ns`（秒，最小 60）、`Nm`（分钟）、`Nh`（小时）或 `Nd`（天）；小于 60 秒的值会提升到最小值。循环任务在 7 天后过期；可以使用创建循环时报告的任务 ID，通过 `scheduler_delete` 取消任务。

---

## 工作流和目标

### `/goal`

设置、管理或检查自主目标。Grok 会跨轮次工作，只有在独立的证据审查确认声明后才会将目标标记为完成；如果审查无法复现结果或没有可用证据，目标会保持活动状态，或带着具体缺口暂停。

```
/goal 将身份验证模块迁移到新 API
/goal status
/goal pause
/goal resume
/goal clear
```

参数可以是 `<objective> [--budget <tokens>]`，或 `status`、`pause`、`resume`、`clear` 之一。这里的 `--budget` 是目标运行的**令牌**预算，与工作流使用的智能体数量预算分开。会话启用目标模式后才会显示 `/goal`。具体由哪个驱动运行取决于后台工作流：启用时，主机会评估每轮模型运行，并对完成候选项进行对抗式验证；禁用时，面向模型的旧版 `update_goal` 路径会报告进度并触发验证。

### `/deep-research <query>`

启动后台研究工作流。它会规划有界的问题集，收集带来源证据的结构化声明，在独立的验证分片上交叉核对每条声明，只渲染通过验证且带有已验证来源定位的声明。失败的分片、删除的声明和研究者的不确定性会作为覆盖限制报告；只要仍有任何限制，报告就会标记为 **Partial**。

```
/deep-research 比较 PostgreSQL 17 和 MySQL 9 的迁移风险
```

命令会立即返回——在 `/workflow runs` 中查看进度，最终报告会自行出现在对话中。

工作流使用绝对累计 `agent_budget` 上限约束逻辑子智能体调用：每次 `agent()` 调用和 `parallel()` 面板中的每一项都会消耗一个槽位，而 schema 修正重试不会消耗。默认值为 128，显式值可为 1–1,024；如果面板会超过剩余预算，则会在其子项启动前被拒绝。模型启动的工作流通过 `workflow` 工具设置 `agent_budget`；命名的斜杠启动可使用 `--agent-budget N` 或 JSON 参数中的 `agent_budget`。命名启动还可用 `--effort LEVEL` 或 JSON `effort` 设置子智能体推理强度，而不改变当前会话的 `/effort`；子脚本自身的 `effort` 选项优先。除此之外，主机配置的上限（默认 32）约束每次运行中同时执行的子项数量；更大的面板会排队，并仍然作为屏障。`budget()` 会将上限报告为 `total`，已接纳调用报告为 `spent`，`reserved`（始终为零）以及 `remaining`。

### `/workflow`

启动已保存的工作流，或通过会话唯一显示名称管理运行中的工作流。两次启动同一工作流时，显示名称会编号（`review-changes`、`review-changes-2`）；你无需接触内部运行 ID。单独运行 `/workflow` 会打印本会话运行的文本概览。

输入 `/workflow` 和一个空格即可自动补全已保存的工作流名称（内置、项目和用户）以及管理动词 `runs`、`pause`、`resume`、`stop`、`save`。选择名称只会填入并提供启动标志，按 Enter 后才会运行；`pause` / `resume` / `stop` / `save` 会列出本会话的运行句柄，单独输入 `/workflow stop` 不会擅自选择运行。

```
/workflow review-changes --agent-budget 256 --effort high {"target":"origin/main...HEAD"}
/workflow review-changes {"target":"origin/main...HEAD","agent_budget":256,"effort":"high"}
/workflow runs
/workflow pause review-changes
/workflow resume review-changes
/workflow stop review-changes-2
/workflow save review-changes
```

`/workflow runs` 会在完整 TUI 中打开实时的 **Workflow Runs** dashboard，显示活动运行和保留运行，而不是已保存定义目录。每行显示运行的显示名称、阶段、智能体列表、进度和结果。在运行详情视图中，`p` 暂停，`r` 恢复普通暂停，`x` 停止。预算受限的运行不能直接恢复：`r` 会返回 Shell 的拒绝（需要通过传入更高 `agent_budget` 的模型／工具恢复来提高上限），而 `x` 仍会停止。`s` 保存运行脚本，但对已知内置工作流和编号的重复句柄会隐藏；这种情况下，请选择新的唯一 `meta.name` 并显式保存编辑后的脚本。在精简模式和非 TUI 客户端中，`/workflow runs` 会打印与单独 `/workflow` 相同的文本概览。

项目工作流位于 `.grok/workflows/*.rhai`；用户工作流位于 `~/.grok/workflows/*.rhai`。同进程暂停/恢复会根据已提交的主机调用结果，继续原始的不可变脚本、参数和 `agent_budget` 上限——要迭代，请编辑返回的脚本副本，并作为新运行启动。

预算受限的运行有所不同：只有通过模型/工具恢复请求才能恢复，该请求需要提供高于已接纳智能体数量的 `agent_budget`。单独的 `/workflow resume <name>` 无法提高上限，因此会拒绝预算受限的运行。进程重启中断的运行不会恢复，因为外部效果没有稳定的跨进程身份。恢复也不保证恰好一次：如果同进程暂停前外部效果的结果尚未提交，该效果可能再次运行。

### `/workflows`

在 **Workflows** 标签页打开扩展模态框，以只读方式浏览 Grok 发现的已保存工作流（内置、项目 `.grok/workflows/` 和用户 `~/.grok/workflows/`），每项会显示来源、描述和路径。模型也会在会话前言的技能列表下看到同一目录。使用 `/workflow <name>`（或工作流自己的斜杠命令）启动，再到 `/workflow runs` 中监控。

---

## 其他

### `/theme`

切换颜色主题。别名：`/t`。

### `/feedback [message]`

报告问题或发送反馈。命令会打开报告面板：`Enter` 发送，`Esc` 丢弃。带消息时会将其预填到面板中，发送前仍可编辑；在 `--minimal` 模式下，带消息仍会立即发送。

```
/feedback
/feedback 某项功能无法正常工作
```

### `/btw`

在不中断当前任务的情况下向智能体发送旁注。在精简模式（`--minimal`）中，答案会显示在提示框上方、可关闭的面板中：`Esc` 关闭面板，完成的答案会保存到原生回滚区，已经关闭的面板收到迟到回复时会丢弃该回复。旁问题及其答案不会成为主轮次的一部分。

```
/btw 还要检查错误处理
```

### `/mcps`

打开 MCP 服务器管理模态框。

### `/doctor`

检查当前会话的终端、剪贴板、颜色、输入、通知和沙箱问题。Doctor 会显示发现的问题以及每个问题的解决方法。运行 `/doctor fix` 可列出可用的自动修复；其他发现会包含手动步骤。`/terminal-setup`、`/terminal-check` 和 `/terminal-info` 仍是别名。

### `/release-notes`

查看当前版本的发行说明。别名：`/changelog`。

### `/docs`

浏览内置 How-to Guides，打开在线 Build 文档，或按标题直接跳到某篇指南。别名：`/howto`、`/guides`。

```
/docs
/docs web
/docs Getting Started
```

- 不带参数的 `/docs`（或 `/docs how-to`）打开 How-to Guides 选择器。
- `/docs web` 在浏览器中打开 https://docs.x.ai/build/overview。
- `/docs <title>` 根据不区分大小写的标题匹配打开指定指南。

### `/tutorial`

打开入门教程：一组简短主题（你的第一个提示、附加上下文、导航、斜杠命令、工作树、计划模式、自定义、从其他智能体工具切换）——每个主题阅读约 30 秒，按 `→` 可直接进入下一个主题。它不会自动显示；该命令（或命令面板）是进入教程的方式。

```
/tutorial
```

别名：`/tour`、`/onboarding`

### `/import-claude`

打开 Claude 导入模态框，导入 `~/.claude` 设置：权限、环境变量、MCP 服务器、hook 和路径。

---

## 智能体和 persona

### `/config-agents`

打开智能体模态框，查看和管理智能体定义、设置默认值并切换活动智能体。别名：`/agents`。

它不是实时多会话的[智能体 dashboard](23-dashboard.md)（`/dashboard` / `Ctrl+\`）。

### `/personas`

创建、编辑和删除 persona。子智能体可以应用 persona 来塑造自身行为。

---

## 账户和计费

### `/login`

登录或重新进行身份验证，无需离开当前会话。

### `/logout`

退出登录并返回登录界面。

### `/usage`

查看额度用量或管理计费。别名：`/cost`。

```
/usage
/usage manage
```

若要查看任一本地会话持久保存的逐轮令牌与费用总计，请在 shell 中运行 `grok-zh usage <session-id> [turn]`。详见[会话管理](17-sessions.md#the-grok-usage-subcommand)。

### `/privacy`

在设置中打开**编码数据、保留期限和训练**，在此选择**选择加入**或**选择退出**。不接受参数。

```
/privacy
```

此设置不会影响 `[features] telemetry`、`trace_upload` 或外部 OTEL 设置——参见[监控用量](24-monitoring-usage.md#related-settings)。在团队账户中，只有团队管理员可以更改它；管理员还可以为团队启用或禁用零数据保留（ZDR）（[如何启用 ZDR](https://docs.x.ai/developers/faq/security#how-to-enable-zdr)）。当选择权不在你手中时，该行会显示 `ZDR` 或 `· Admin Managed`，而不是打开选择器。ZDR 会锁定编码数据共享，但不会关闭外部 OTEL 或 `user.email`；详见[此数据流与 ZDR](24-monitoring-usage.md#zdr-and-this-stream)。

---

## 配置和界面

### `/settings`

打开设置模态框，以交互方式查看和更改配置。别名：`/config`、`/preferences`、`/prefs`。

### `/timestamps`

切换消息时间戳的显示。

---

## 作为斜杠命令的技能

在其 SKILL.md frontmatter 中启用 `user-invocable: true` 的技能会显示为斜杠命令。（通过 `/skills` 关闭技能后，它将不再被宣传。）例如，位于 `~/.grok/skills/commit/SKILL.md` 的技能可这样运行：

```
/commit 修复 README 中的拼写错误
```

来自插件的技能工作方式相同。当不同作用域中有两个同名技能时，请限定其名称：

```
/local:commit      # 项目作用域技能
/user:commit       # 用户作用域技能
```

内置命令始终优先使用未限定名称。将技能命名为 `compact` 后，`/compact` 仍会运行内置命令——该技能可作为 `/local:compact`（插件则为 `/acme:compact`）使用。两者都会出现在斜杠菜单中：内置命令带有 `built-in` 标签，技能带有 `skill · local` / `skill · acme` 标签。

---

## 自动补全

菜单支持模糊搜索：在 `/` 后开始输入即可筛选。每个条目显示命令名称、描述、需要参数时的参数提示以及来源（builtin、技能作用域或插件名称）。按 `Tab` 或 `Enter` 接受高亮的命令。
