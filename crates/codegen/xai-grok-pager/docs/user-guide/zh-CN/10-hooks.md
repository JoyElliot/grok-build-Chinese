# 钩子

钩子可以在 Grok 会话的关键时刻运行脚本或发送 HTTP 请求。你可以用它们自动化任务、执行安全检查、记录活动、发送通知并集成自己的工具。

---

<a id="what-are-hooks"></a>
## 什么是钩子？

钩子是 Grok 在特定生命周期事件发生时调用的 Shell 命令或 HTTP 端点。钩子可以：

- **阻止操作**——`PreToolUse` 钩子可以在危险命令运行前拒绝它。
- **让智能体继续工作**——`Stop` 钩子可以在某个条件满足前阻止智能体结束本轮（例如测试套件通过），并将原因反馈给模型。
- **响应事件**——`PostToolUse` 钩子可以将每次工具执行记录到文件中。
- **在调用后纠正结果**——`PostToolUse` 钩子可以向模型解释工具结果，或替换模型将读取的输出（例如隐藏密钥或裁剪超长日志）；真实结果仍保留在记录中。
- **设置上下文**——`SessionStart` 钩子可以导出环境变量或运行设置脚本。

---

<a id="common-use-cases"></a>
## 常见用例

- **安全防护**：在命令运行前阻止 `rm -rf /` 等命令。
- **审计日志**：将工具使用和会话记录到文件或外部服务。
- **通知**：任务完成时发送消息。
- **自动格式化**：编辑后运行 `cargo fmt` 或 `prettier`。
- **环境设置**：在会话开始时导出变量。
- **自定义工作流**：在特定事件上触发构建、测试或部署。

---

<a id="quick-start"></a>
## 快速开始

1. 创建钩子目录：

   ```sh
   mkdir -p ~/.grok/hooks
   ```

2. 创建钩子文件，例如 `~/.grok/hooks/session-start.json`：

   ```json
   {
     "hooks": {
       "SessionStart": [
         {
           "hooks": [
             { "type": "command", "command": "echo 'Grok 会话已在 '$(pwd)' 中启动'" }
           ]
         }
       ]
     }
   }
   ```

3. 启动（或重新启动）Grok 会话。钩子会在 `SessionStart` 上自动运行。

4. 在非 VS Code 系列终端按 `Ctrl+L`（或在任何地方运行 `/hooks`——在 VS Code 系列终端中推荐），检查 Hooks 选项卡以确认已加载。

---

<a id="hook-locations"></a>
## 钩子位置

钩子会从多个位置发现（全部合并）：

| 作用域 | 路径 | 可信？ | 说明 |
|-------|------|-------|------|
| 全局 | `~/.grok/hooks/*.json` | 始终 | 个人钩子 |
| 全局 | `~/.claude/settings.json`（以及 `settings.local.json`） | 始终 | Claude Code 兼容性（可配置） |
| 全局 | `~/.cursor/hooks.json` | 始终 | Cursor 兼容性（可配置） |
| 项目 | `<project>/.grok/hooks/*.json` | 需要信任 | 每个仓库的自动化 |
| 项目 | `<project>/.claude/settings.json`（以及 `settings.local.json`） | 需要信任 | Claude 兼容性（可配置） |
| 项目 | `<project>/.cursor/hooks.json` | 需要信任 | Cursor 兼容性（可配置） |
| 配置 | `~/.grok/config.toml` | 始终 | 将你的钩子与其他配置放在一起 |
| 配置 | `managed_config.toml`（`$GROK_HOME` 和 `/etc/grok`） | 始终 | 组织分发的钩子（服务器同步和设备本地） |
| 配置 | `requirements.toml`（用户和系统） | 始终 | requirements 层中的组织分发钩子 |
| 插件 | 已安装插件内部捆绑 | 按插件 | 团队共享钩子 |

配置文件钩子位于组织已经控制的同一个 TOML 中；格式请参阅[配置文件中的钩子](#hooks-in-config-files)。默认情况下会扫描兼容的厂商钩子来源。若要停用某个厂商的扫描，在 `~/.grok/config.toml` 中设置 `[compat.<vendor>] hooks = false`，或设置对应的环境变量。详情请参阅[配置](05-configuration.md#harness-compatibility)。

**信任项目**：第一次打开含有钩子的项目时，必须先信任它，项目钩子才会运行——在此之前它们会被静默跳过。运行 `/hooks-trust`（或使用 `--trust` 启动）授予信任；决定会记录在统一的文件夹信任存储（`~/.grok/trusted_folders.toml`）中，该存储与控制仓库本地 MCP/LSP 服务器的门禁相同。`~/.grok/hooks/` 中的全局钩子始终可信，无需条目。这样可以防止不受信任的仓库运行任意代码。

由于钩子统一受文件夹信任控制，`--trust` / `/hooks-trust` 授权会同时信任整个文件夹中的 **MCP、LSP 和钩子**，并级联到子目录。反过来，停用文件夹信任（`GROK_FOLDER_TRUST=0` 或 `[folder_trust] enabled = false`）也会解除项目钩子以及 MCP/LSP 的门禁。

---

<a id="hook-events"></a>
## 钩子事件

| 事件 | 触发时机 | 阻塞？ |
|-------|---------------|-----------|
| `SessionStart` | 会话开始。 | 否 |
| `UserPromptSubmit` | 你提交提示。 | 是——可以阻止提示 |
| `PreToolUse` | 工具即将运行。 | 是——可以拒绝 |
| `PostToolUse` | 工具已经运行完毕，包括内置工具返回的逻辑错误（例如 `run_terminal_command` 非零退出）；分发失败或 MCP 错误结果改触发 `PostToolUseFailure`。 | 不阻止调用，但可向模型提供反馈并替换模型看到的输出 |
| `PostToolUseFailure` | 工具分发失败，或 MCP 工具返回错误结果。 | 否，但可通过 `additionalContext` 向模型提供上下文 |
| `PermissionDenied` | 权限系统拒绝工具调用。 | 否 |
| `Stop` | 智能体在真正完成时结束一轮（不是用户中断）。 | 是——可以阻止停止 |
| `StopFailure` | 由于 API 错误而结束一轮。 | 否 |
| `Notification` | 智能体发送通知。 | 否 |
| `SubagentStart` | 子智能体启动。 | 否 |
| `SubagentStop` | 子智能体的一轮结束（在子智能体中触发一次，并带有停止决策控制）。 | 是——可以阻止停止 |
| `PreCompact` | 即将运行会话压缩。 | 否 |
| `PostCompact` | 会话压缩完成。 | 否 |
| `SessionEnd` | 会话结束。 | 否 |

`SubagentEnd` 被接受为 `SubagentStop` 的别名。`PreToolUse` 可以阻止工具调用，`Stop`/`SubagentStop` 可以阻止智能体停止（请参阅[停止决策控制](#stop-decision-control)）。`PostToolUse` 触发时工具已经运行，不能阻止调用，但会读取 stdout，并可向模型提供反馈或替换模型看到的工具输出（请参阅 [PostToolUse 输出](#posttooluse-output)）。其他事件都是被动事件。

<a id="cursor-hook-compatibility"></a>
### Cursor 钩子兼容性

Grok 接受 Cursor 的 camelCase 钩子事件名称，因此 `~/.cursor/hooks.json` 无需修改即可加载：

| Cursor 事件 | 映射到 |
|---|---|
| `sessionStart`, `sessionEnd` | `SessionStart`, `SessionEnd` |
| `preToolUse`, `postToolUse`, `postToolUseFailure` | `PreToolUse`, `PostToolUse`, `PostToolUseFailure` |
| `beforeShellExecution`, `beforeMCPExecution`, `beforeReadFile` | `PreToolUse` |
| `afterShellExecution`, `afterMCPExecution`, `afterFileEdit` | `PostToolUse` |
| `afterAgentResponse`, `afterAgentThought` | `PostToolUse` |
| `beforeSubmitPrompt` | `UserPromptSubmit` |
| `subagentStart`, `subagentStop` | `SubagentStart`, `SubagentStop` |
| `preCompact`, `stop` | `PreCompact`, `Stop` |

Cursor 的按操作钩子（`beforeShellExecution`、`afterFileEdit` 等）会映射到通用的 `PreToolUse`/`PostToolUse` 事件。钩子脚本会在 JSON 输入中收到工具名称，因此可以据此过滤，或者使用 `matcher` 字段。

---

<a id="the-hook-json-format"></a>
## 钩子 JSON 格式

每个 `.json` 文件可以为多个事件定义钩子：

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command", "command": "bin/safety-check.sh", "timeout": 10 }
        ]
      }
    ],
    "PostToolUse": [
      {
        "hooks": [
          { "type": "command", "command": "bin/log-activity.sh" }
        ]
      }
    ]
  }
}
```

<a id="key-fields"></a>
### 关键字段

- **事件名称**（顶层键）：[钩子事件](#hook-events)中列出的任意事件。Grok 会跳过无法识别的事件名称，因此共享的 Claude 或 Cursor 设置文件仍能加载。
- **matcher**（可选）：选择哪些调用会触发钩子的正则表达式。它测试的内容取决于事件：工具事件（`PreToolUse`、`PostToolUse`、`PostToolUseFailure`、`PermissionDenied`）测试工具名称，`Notification` 测试通知类型，`SubagentStart`/`SubagentStop` 测试子智能体类型（例如 `explore`），`SessionStart` 测试启动来源（`startup`、`resume`、……），`SessionEnd` 测试结束原因，`PreCompact`/`PostCompact` 测试压缩触发方式（`manual` 或 `auto`），`StopFailure` 测试错误类型（`rate_limit`、`authentication_failed`、`invalid_request`、`server_error`、`max_output_tokens` 或 `unknown`）。`Stop` 或 `UserPromptSubmit` 上的 matcher 会被忽略并给出警告（这些事件始终触发）。matcher 为空或省略时匹配所有内容。matcher 测试真实工具名称；经内部 `use_tool` 分发器路由的 MCP 调用显示为限定名称 `server__tool`（例如 `linear__save_issue`），因此应匹配该名称，而不是分发器名称。
- **type**：`"command"`（运行脚本或 Shell 单行命令）或 `"http"`（将事件 POST 到 URL）。
- **command**：可执行文件路径（相对于 JSON 文件）或内联 Shell 命令。
- **timeout**：终止钩子前的秒数（默认 5 秒；`Stop`/`SubagentStop`/`PostToolUse` 门禁默认 600 秒）。所有钩子失败（超时、崩溃、输出格式错误、缺少必需环境变量）都采用故障开放：失败会记录到 UI 回滚区，但不会阻止工具调用。只有钩子返回的显式 `deny` 决策会阻止工具调用。

<a id="tool-name-aliases"></a>
### 工具名称别名

在 `matcher` 中，Grok 会将 Claude 风格的工具名称映射到自己的名称，使从 Claude 迁移的钩子能够正确触发。常见别名包括：

- `Bash` → `run_terminal_command`
- `Read` → `read_file`
- `Edit`、`Write` 和 `MultiEdit` → `search_replace`
- `Grep` → `grep`
- `Glob` 和 `ListDir` → `list_dir`
- `WebSearch` → `web_search`
- `Task` → `spawn_subagent`

matcher 也会保留原名称，因此 `Bash` 同时匹配 `Bash` 和 `run_terminal_command`。

---

<a id="hooks-in-config-files"></a>
## 配置文件中的钩子

钩子也可以直接放在 Grok 配置中，让团队与其他配置一起分发钩子，而无需单独提供 JSON 文件。三个 TOML 文件都会读取同一个 `hooks` 对象：

| 文件 | 层级 | 设置者 |
|------|------|-------------|
| `~/.grok/config.toml` | 用户 | 你 |
| `managed_config.toml`（`$GROK_HOME`、`/etc/grok`） | 托管/系统 | 你的组织 |
| `requirements.toml`（用户和系统） | Requirements | 你的组织 |

TOML 的结构与 JSON 钩子对象相同，因此现有钩子可以直接转写：

```toml
[[hooks.PreToolUse]]
matcher = "Bash|Write|Edit"
hooks = [
  { type = "command", command = "/opt/guard/pretooluse.sh", timeout = 10 },
]
```

每个 matcher 组都是一个 `[[hooks.<Event>]]` 条目，包含可选的 `matcher` 和内层 `hooks` 处理器数组。处理器字段（`type`、`command`、`url`、`timeout`、`env`）和事件名称与 [JSON 格式](#the-hook-json-format)完全相同。

TOML 为内层处理器提供两种等价写法，两者解析后结构完全相同。上面展示的内联表数组是推荐写法：对于常见的单处理器情况，它可读性最佳。也接受嵌套的表数组写法：

```toml
[[hooks.PreToolUse]]
matcher = "Bash|Write|Edit"
[[hooks.PreToolUse.hooks]]
type = "command"
command = "/opt/guard/pretooluse.sh"
timeout = 10
```

推荐使用内联形式，以免为每个处理器重复 `[[hooks.<Event>.hooks]]` 标头。

- **各层叠加。** 每一层的钩子都会运行；低优先级层添加钩子，但永远不会替换另一层的块。在多个层中完全相同的钩子会去重，并保留权限最高的副本。
- **来源标签。** 配置钩子会在 `/hooks` 中按来源标记（`managed:`、`requirements/user:`、`user:` 等），因此可以看到每个钩子来自哪一层。
- **读取时不展开。** `command` 或 `url` 中的字面量 `${VAR}` 会原样到达钩子运行器，与 JSON 钩子文件语义一致；运行器负责唯一一次展开。

---

<a id="writing-hook-scripts"></a>
## 编写钩子脚本

<a id="input"></a>
### 输入

事件会以 JSON 形式发送到 **stdin**（例如一个 `PreToolUse` 事件；负载始终还包含 `toolUseId` 和 `toolInputTruncated`）：

```json
{
  "hookEventName": "pre_tool_use",
  "hook_event_name": "PreToolUse",
  "sessionId": "abc-123",
  "cwd": "/Users/you/project",
  "workspaceRoot": "/Users/you/project",
  "permissionMode": "default",
  "toolName": "run_terminal_command",
  "toolInput": { "command": "npm test" },
  "timestamp": "2026-04-14T12:00:00Z"
}
```

每个事件都携带相同的公共字段：`hookEventName`、`sessionId`、`cwd`、`workspaceRoot`、`timestamp`、`permissionMode`（`default`、`auto`、`plan` 或 `bypassPermissions`）以及 `promptId`（事件所属轮次；会话级事件可缺省），另有上面 `toolName` 等事件专用字段。snake_case 键 `hook_event_name` 使用 Claude 的 PascalCase 值；camelCase 键 `hookEventName` 使用 grok 的 snake_case 值。

<a id="output-blocking-hooks"></a>
### 输出（阻塞钩子）

对于 `PreToolUse` 钩子，将 JSON 写入 **stdout**：

- **允许**：`{"decision": "allow"}`
- **拒绝**：`{"decision": "deny", "reason": "检测到不安全命令"}`

<a id="posttooluse-output"></a>
### PostToolUse 输出

`PostToolUse` 在工具运行后触发，因此不会阻止调用；但它的 stdout 会决定模型接下来看到什么。将 JSON 写入 **stdout**：

```json
{
  "decision": "block",
  "reason": "差异中仍有调试输出",
  "hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "additionalContext": "这是生成文件，请改模板",
    "updatedToolOutput": { "type": "Bash", "command": "…", "exit_code": 0, "output_for_prompt": "[已隐藏]" }
  }
}
```

| 字段 | 效果 |
|------|------|
| `decision: "block"` + `reason` | 将原因连同工具结果交给模型；这里的“block”表示告诉模型结果有问题，不会停止已经完成的调用。 |
| `additionalContext` | 在工具结果旁附加给模型的说明。 |
| `updatedToolOutput` | 替换模型看到的结果，适用于所有工具。 |
| `updatedMCPToolOutput` | 仅用于 MCP 的 `updatedToolOutput` 别名；用于内置工具时会被忽略。 |

- 多个钩子的原因和 `additionalContext` 会按运行顺序，在工具结果之后用当前 harness 的 reminder 标签封装并注明钩子名；输出替换采用最后写入者优先，被覆盖的替换会记入日志。
- 对内置工具，`updatedToolOutput` 必须保持该工具在事件 `toolResult` 中的原始输出结构，否则替换会被忽略并记录失败；拼错 `decision`（仅接受 `"block"`）也按失败记录。应先检查 `toolResultTruncated`：超大负载会以普通字符串交给钩子，无法按原结构回传。
- MCP 输出不做结构校验；JSON 字符串直接成为模型可见文本，其他值会被序列化。`updatedToolOutput` 和 `updatedMCPToolOutput` 两个键之间同样由最后一个钩子写入者获胜。
- 原因和 `additionalContext` 上限为 10,000 字符，替换后的模型可见文本在渲染后限制为 64K 字符；只有结构不匹配才会丢弃。非零退出（包括退出 2）会保留 block 原因，但丢弃 `additionalContext` 和替换内容。
- 替换只影响模型的副本；回滚区、会话记录和遥测仍保留真实输出。替换后的截图或 PDF 读取不会再向模型传递原图。钩子发送的说明、原因和替换内容都会转义，不能闭合 reminder 标签并伪装成 harness 或用户指令。
- 命令和 HTTP 配置钩子可替换输出；通过 grok-agent-sdk 注册的 `PostToolUse` 只能提供 block 原因和 `additionalContext`，不能替换工具输出。
- 实际运行过的工具（包括内置逻辑错误）触发 `PostToolUse`；分发失败和 MCP 错误结果触发仅支持上下文的 `PostToolUseFailure`。默认超时为 600 秒。

<a id="exit-codes"></a>
### 退出代码

| 退出代码 | 含义 |
|-----------|---------|
| `0` | 成功/允许（针对阻塞钩子） |
| `2` | 显式拒绝（`PreToolUse`）、使用 stderr 作为反馈阻止停止（`Stop`/`SubagentStop`），或向模型反馈（`PostToolUse`）。JSON 中的 `reason` 优先于 stderr。 |
| 其他 | 故障开放——会记录失败但不会阻止任何操作。对于 `PreToolUse`，无论退出代码如何，stdout JSON 中的 `deny` 决策都会生效。对于 `Stop`/`SubagentStop`，stdout 上的有效决策 JSON 优先；没有可用 JSON 时退出 2 才会阻止并使用 stderr。对于 `PostToolUse`，任何退出代码都不会阻止已经完成的调用；失败会被记录，block 原因保留，而 `additionalContext` 与输出替换会被丢弃。 |

**`PostToolUse` 的退出 2 语义已经变化。** 它现在会把 stderr 反馈给模型。若日志钩子写成 `run_checker; exit $?`，而 `mypy`、`grep`、`pytest` 或 `argparse` 以 2 退出，输出也会交给模型；希望保持静默时请显式 `exit 0`。

<a id="stop-decision-control"></a>
### 停止决策控制

`Stop` 和 `SubagentStop` 钩子会在智能体即将结束本轮时运行，可以让它继续工作（兼容 Claude Code）。将 JSON 写入 **stdout**：

- **阻止停止**：`{"decision": "block", "reason": "尚未运行测试套件"}`。该原因会作为用户消息反馈给模型，智能体在同一轮中再次运行。
- **非错误反馈**：`{"hookSpecificOutput": {"hookEventName": "Stop", "additionalContext": "结束前运行 linter"}}`。这也会让智能体继续工作，但会作为钩子反馈而不是钩子错误显示。
- **强制停止**：`{"continue": false, "stopReason": "预算已耗尽"}`。结束本轮并覆盖所有阻止决定。
- **允许停止**：退出 0 且无输出（或输出任何非 JSON 内容）。

退出代码为 `2` 也会阻止停止，此时 **stderr** 是反馈。

钩子输入包含 `stopHookActive` 和 `lastAssistantMessage`。当智能体因本轮先前的停止钩子阻止而正在继续时，`stopHookActive` 为 true；请检查它或 transcript，避免在永远无法解决的条件上继续阻止。`lastAssistantMessage` 携带智能体本轮最终响应的文本，因此钩子无需解析 transcript。单轮中经过 **8 次继续**（阻止或非错误反馈）后，门禁会被覆盖，本轮结束；最后一次强制停止不会咨询钩子。计数器按轮次计算：下一个用户提示会重新开始，因此长时间运行的目标可以跨轮。钩子失败采用故障开放：智能体正常停止。

`Stop`、`SubagentStop` 和 `PostToolUse` 钩子默认超时 600 秒，因为这些门禁通常会运行构建或测试套件；超时采用故障开放，不会阻止完成。其他事件仍使用 5 秒默认值。门禁需要更长时间时显式设置 `timeout`：`{ "type": "command", "command": "bin/verify.sh", "timeout": 1200 }`。

会话关闭时，排队的轮次结束钩子最多等待半秒；随后每个 `SessionEnd` 钩子默认最多运行 1.5 秒。可用 `GROK_SESSION_END_HOOKS_TIMEOUT_MS`（毫秒，最大 60 秒）调整后者。

门禁只在真正完成时运行。被中断（Esc / Ctrl+C）、被拒绝和达到最大轮数的轮次会完全跳过 Stop 钩子，API 错误轮次则触发 `StopFailure`。会话结束时还会单独触发 Stop（`reason: "channel_closed"` 或 `"shutdown"`）；其决策输出会被解析但忽略，因为已经没有可继续的轮次。统计或门控 Stop 触发次数的脚本应检查 `reason == "end_turn"`，以免会话结束触发影响统计。

`StopFailure` 仅用于观察（可用它记录失败或发送警报；输出和退出代码都会被忽略）。其输入包含 `error`（匹配器测试的分类类型，使用 Claude Code 词汇：`rate_limit`、`authentication_failed`、`invalid_request`、`server_error`、`max_output_tokens` 或无法区分时的 `unknown`；容量错误归入 `rate_limit`，没有 `billing_error` 信号）、`errorDetails`（可用时的原始错误详情）和 `lastAssistantMessage`（会话中显示的渲染错误文本；对于此事件它是错误字符串而不是智能体输出）。

`Stop` 输入还包含 `backgroundTasks` 和 `sessionCrons`，因此钩子可以区分“会话已完成”和“会话暂停，正在等待后台工作唤醒”。没有正在运行或已调度的任务时，两个数组都为空。每个 `backgroundTasks` 条目描述一个正在运行的任务：`id`、`type`（`shell`、`monitor` 或 `subagent`）、`status`，以及按类型而定的 `command`（仅 Shell 任务）、`description`（监视器所监视的命令行，或子智能体的任务描述）和 `agentType`（子智能体）。每个 `sessionCrons` 条目描述一个已调度的唤醒（`scheduler_create` 或 `/loop`）：`id`、`schedule`、`recurring` 和 `prompt`。`schedule` 的值是人类可读的间隔（如 `every 5 minutes`）；grok 调度的是间隔，不是 cron 表达式。自由文本条目字段上限为 1000 个字符，超出部分会在字符串内使用 `… [+N chars]` 标记。

在子智能体内部，门禁以 `SubagentStop` 触发（智能体 frontmatter 中的 `Stop` 钩子会自动重映射）。`Stop` 钩子只控制主智能体。

`SubagentStop` 按每个子智能体触发一次，在子智能体自己的轮次结束时触发，与 Claude Code 一致。其输入包含 `phase` 字段（当前始终为 `"gate"`），为未来兼容性保留。

**移植 Claude Code 停止钩子**：输出词汇（`decision`、`reason`、`continue`、`stopReason`、`additionalContext`）可以原样使用。以下事项与 Claude 不同：

- **camelCase 输入**：grok 的 stdin 外壳各处使用 camelCase 键，而 Claude 使用 snake_case。读取 `.stop_hook_active` 或 `.background_tasks[].agent_type` 的脚本必须切换为 `.stopHookActive` 和 `.backgroundTasks[].agentType`。snake_case 的 `hook_event_name` 携带 Claude 的 PascalCase 值（如 `"Stop"`），camelCase 的 `hookEventName` 携带 grok 的 snake_case 值（如 `"stop"`）。通过 grok-agent-sdk 注册的钩子仍会把顶层键及数组条目键转换为 snake_case。
- **`toolResult` 字段**：`PostToolUse` 工具输出是 `toolResult`（SDK：`tool_result`）；grok 同时发出复制该值的 `tool_response` snake_case 别名，因此读取 Claude `.tool_response` 的钩子无需修改。
- **`updatedToolOutput` 使用 grok 自己的输出结构**：内置工具的替换会按事件中 `toolResult` 的结构验证；沿用其他运行时字段名会被忽略。MCP 工具没有固定结构，因此 `updatedToolOutput` 与 `updatedMCPToolOutput` 都会原样通过。
- **会话结束触发**：会话结束时会额外触发仅观察的 Stop；按 `reason == "end_turn"` 过滤（见上文）。
- **间隔调度**：`sessionCrons[].schedule` 是人类可读的间隔，永远不是 cron 表达式。
- **任务类型**：`backgroundTasks[].type` 只有 `shell`、`monitor` 或 `subagent`；不会发出 Claude 的其他标签（`workflow`、`teammate` 等）。
- **StopFailure 类别**：发出的集合使用 Claude Code 词汇——`rate_limit`、`authentication_failed`、`invalid_request`、`server_error`、`max_output_tokens`、`unknown`。grok 只发出其中一部分：容量错误（503/529）像 Claude 一样归入 `rate_limit`，永远不会发出 `billing_error`（没有信号），因此 `billing_error` matcher 不会触发。
- **permission_mode 值**：grok 发出 `default`、`auto`、`plan` 或 `bypassPermissions`。Claude 的 `acceptEdits`/`dontAsk` 在 grok 中没有对应值（grok 的 `auto` 最接近），因此 `permission_mode === "acceptEdits"` 之类的检查永远不匹配。
- **客户端（SDK）门禁超时**：SDK `Stop`/`SubagentStop` 门禁默认 600 秒，与文件钩子相同；`PreToolUse` 客户端门禁默认 30 秒（交互式热路径）。可以通过每个 matcher 组的 `timeoutS` 覆盖，最大 600 秒。
- **`/goal`**：grok 的目标循环是停止门禁之前运行的独立功能，不是提示类型的 Stop 钩子。

一个完整的“保持工作”策略脚本：

```bash
#!/bin/bash
input=$(cat)
# 只门控真正的轮次结束，不处理会话结束时的观察触发。
if [ "$(echo "$input" | jq -r '.reason')" != "end_turn" ]; then exit 0; fi
if ! bin/verify.sh >/dev/null 2>&1; then
  echo '{"decision": "block", "reason": "verify.sh 失败；请在结束前修复失败项"}'
fi
```

以 `{ "type": "command", "command": "bin/stop-gate.sh", "timeout": 300 }` 注册，并将 `timeout` 设置为验证步骤所需的时长。每次继续后钩子都会再次触发，内置上限在 8 次后结束本轮；检查 `stopHookActive`，可以在智能体明显无法采取行动的反馈上更早放弃。

<a id="passive-hooks"></a>
### 被动钩子

对于 `SessionStart` 或 `Notification` 等事件，stdout 会被忽略。例外是 `PreToolUse`、`Stop`/`SubagentStop`，以及虽不阻止调用但会读取 stdout 的 `PostToolUse`（见 [PostToolUse 输出](#posttooluse-output)）。

<a id="environment-variables"></a>
### 环境变量

Grok 会为每个钩子进程设置多个环境变量。编写需要感知上下文或插件的钩子脚本时，这些变量很有用。

<a id="runner-injected-variables-always-available"></a>
#### 运行器注入的变量（始终可用）

这些变量由钩子运行器为**每个**钩子设置：

| 变量 | 说明 |
|-----------------------|-------------|
| `GROK_HOOK_EVENT` | 触发钩子的事件名称（例如 `pre_tool_use`、`session_start`、`post_tool_use`、`session_end`、`stop`、`notification`）。 |
| `GROK_HOOK_NAME` | 此特定钩子的配置名称（插件提供的钩子包含插件前缀）。 |
| `GROK_SESSION_ID` | 当前 Grok 会话的唯一标识符。 |
| `GROK_WORKSPACE_ROOT` | 当前工作区根目录的绝对路径。 |
| `CLAUDE_PROJECT_DIR` | 工作区根目录的绝对路径。与 Claude Code 兼容的 `GROK_WORKSPACE_ROOT` 别名，为每个钩子设置。 |

这些变量是**保留变量**。你尝试通过钩子 JSON 中的 `env` 字段设置它们的任何值都会在加载时被剥离（并记录警告），运行器总会在生成进程时注入真实值。

<a id="plugin-hook-variables"></a>
#### 插件钩子变量

钩子来自插件时，Grok 还会注入以下变量：

| 变量 | 说明 |
|-----------------------|-------------|
| `GROK_PLUGIN_ROOT` | 插件安装目录的绝对路径。 |
| `GROK_PLUGIN_DATA` | 插件可写数据目录的绝对路径（用于保存插件状态、缓存等）。 |

这些值由插件系统提供。对于四个插件相关键（`GROK_PLUGIN_ROOT`、`GROK_PLUGIN_DATA` 及其 Claude 别名），插件适配器确保官方插件值始终优先于钩子 `env` 映射中的用户声明值。

<a id="user-defined-environment-variables"></a>
#### 用户定义的环境变量

你可以使用 `env` 字段为单个钩子处理器提供额外环境变量：

```json
{
  "type": "command",
  "command": "bin/my-hook.sh",
  "env": {
    "MY_SECRET": "value",
    "LOG_LEVEL": "debug"
  }
}
```

这些变量会传递给钩子进程，但不能覆盖上面列出的运行器变量或插件变量。

<a id="using-variables-in-command-and-url-fields"></a>
#### 在 `command` 和 `url` 字段中使用变量

`command` 和 `url` 都支持 `${VAR}` 与 `$VAR` 展开。在 Windows PowerShell 中，已知的 `$VAR` 引用会改写为 `$env:VAR`，以便从子进程环境读取。完整细节（包括加载时与运行时展开、`env` 映射查找顺序，以及参数展开修饰符（如 `${VAR:-default}`）的处理）请参阅 custom-hooks 参考。

---

<a id="http-hooks"></a>
## HTTP 钩子

你可以调用远程端点，而不是运行本地脚本：

```json
{ "type": "http", "url": "https://hooks.example.com/grok-event", "timeout": 15 }
```

完整事件外壳会作为 JSON 通过 POST 发送。

---

<a id="managing-hooks-in-the-tui"></a>
## 在 TUI 中管理钩子

<a id="the-hooks-tab"></a>
### Hooks 选项卡

在非 VS Code 系列终端按 `Ctrl+L` 打开 Extensions 模态框（Plugins 选项卡），或运行 `/hooks`（任意终端；在 `Ctrl+L` 会插入文本的 VS Code 系列终端中必须运行 `/hooks`）在 Hooks 选项卡中打开。在 **Hooks** 选项卡中：

| 按键 | 操作 |
|-----|--------|
| `r` | 从磁盘重新加载所有钩子 |
| `a` | 按路径添加自定义钩子 |
| `x` | 移除选中的钩子来源（会请求确认；按小写 `y` 确认） |
| `Space` | 启用或停用选中的钩子 |
| `f` | 循环切换状态筛选（All / Enabled / Disabled） |

钩子按来源分组：**Global**、**Project**、**Plugin** 和 **Custom**。

每个钩子显示：
- 触发它的**事件**
- 运行的**命令**或 **URL**
- **超时**时长
- **状态**——已启用或 `[disabled]`

<a id="slash-commands"></a>
### 斜杠命令

```
/hooks-list           # 显示本会话中加载的钩子
/hooks-trust          # 信任此项目以执行钩子
/hooks-add <path>     # 添加自定义钩子文件或目录
/hooks-remove <path>  # 移除自定义钩子
/hooks-untrust        # 撤销对此项目的信任
```

在 TUI pager 中，单独的 `/hooks-*` 命令不会出现在斜杠命令列表中。`/hooks` 模态框负责列出、添加、移除以及启用或停用钩子；项目信任通过 `/hooks-trust`（或模态框的 Trust 操作）管理，它会写入上文所述的统一文件夹信任存储。

<a id="per-hook-enable-disable"></a>
### 单个钩子的启用/停用

在 Hooks 选项卡中按 `Space`，可以在运行时启用或停用单个钩子。更改会立即生效，无需重新启动会话。

<a id="mid-session-reload"></a>
### 会话中途重新加载

在 Hooks 选项卡中按 `r`，从磁盘重新加载所有钩子。Grok 会重新读取每个钩子来源，因此会拾取你在会话期间对钩子文件所做的更改。

---

<a id="hook-annotations-in-scrollback"></a>
## 回滚区中的钩子注释

钩子执行时，其结果会作为注释出现在 TUI 回滚区。你可以看到哪些钩子运行了、操作被允许还是拒绝，以及它们产生的任何输出。只有启用 plugins UI（默认启用）时才会显示这些注释。

---

<a id="example-safe-shell-guard"></a>
## 示例：安全 Shell 防护

阻止危险的 Shell 命令：

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command", "command": "bin/safe-shell.sh", "timeout": 5 }
        ]
      }
    ]
  }
}
```

其中 `bin/safe-shell.sh` 的内容为：

```bash
#!/bin/sh
INPUT=$(cat)
CMD=$(echo "$INPUT" | jq -r '.toolInput.command // empty')

# 阻止破坏性模式
if echo "$CMD" | grep -qE '(rm -rf /|mkfs|dd if=|:(){ :|& };:)'; then
  echo '{"decision": "deny", "reason": "已阻止可能具有破坏性的命令"}'
  exit 2
fi

echo '{"decision": "allow"}'
```

---

<a id="security-notes"></a>
## 安全说明

- 全局钩子（`~/.grok/hooks/`）以你的用户权限运行——请像对待 Shell 脚本一样对待它们。
- 项目钩子需要文件夹信任（`/hooks-trust` 或 `--trust`，与仓库本地 MCP/LSP 使用同一门禁），以防止恶意仓库发起供应链攻击。
- HTTP 钩子会发送会话数据——只使用可信端点。
- `PostToolUse` 钩子可以向模型加入指令或替换模型看到的工具输出，因此应像信任 `PreToolUse` 门禁一样信任它。真实输出仍保留在回滚区和会话记录中。

---

<a id="best-practices"></a>
## 最佳实践

1. **保持钩子快速**——运行时间很长的钩子会阻塞 UI。尽可能使用后台进程（`&`）或异步方式。
2. **使用显式 `deny` 进行阻止**——钩子在发生任何错误时都会故障开放，因此崩溃的钩子不会阻止工具。要执行策略，钩子必须运行完成，并在 stdout 输出 `{"decision":"deny","reason":"..."}`。始终在脚本内部处理错误，使其能返回显式决策。
3. **使用绝对路径或相对于钩子文件的路径**——JSON 文件旁 `bin/` 中的脚本具有可移植性。
4. **使用模态框测试**——按 `Ctrl+L`（非 VS Code 系列）或运行 `/hooks`，确认钩子已加载并能匹配，再依赖它们。
5. **对项目钩子进行版本控制**——提交 `.grok/hooks/`（但绝不要提交机密）。

---

<a id="troubleshooting"></a>
## 故障排除

- **钩子没有运行？** 在非 VS Code 系列终端按 `Ctrl+L`（或在任何地方运行 `/hooks`），查看钩子是否已加载并匹配。
- **项目钩子被忽略？** 文件夹可能不受信任。运行 `/hooks-trust`（或使用 `--trust` 重新启动）。
- **找不到脚本？** 检查路径是否相对于 `.json` 文件，并且脚本可执行（`chmod +x`）。
- **查看错误？** 使用 `RUST_LOG=debug GROK_LOG_FILE=/tmp/grok.log grok-zh` 启动以捕获日志，然后检查 `/tmp/grok.log`。
