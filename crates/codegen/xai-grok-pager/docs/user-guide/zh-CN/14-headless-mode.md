<a id="headless-mode-and-scripting"></a>
# 无头模式与脚本编程

无头模式从命令行以非交互方式运行 Grok。它接收单个提示，使用完整工具访问权限执行，然后返回结果。你可以用它自动化任务、编写工作流脚本、构建集成并以编程方式解析输出。

---

<a id="basic-usage"></a>
## 基本用法

以非交互方式传入提示会触发无头模式。最常用的是 `-p` 标志（`--single` 的简写）；`--prompt-json` 和 `--prompt-file` 也会触发该模式：

```bash
grok-zh -p "Your prompt here"
```

Grok 会处理提示，运行所需工具，并将结果打印到 stdout。响应完成后进程退出。

---

<a id="command-line-options"></a>
## 命令行选项

| 标志 | 说明 |
| ----------------------- | ----------------------------------------------------- |
| `-p, --single <PROMPT>` | 要发送的提示（或使用 `--prompt-json` / `--prompt-file`） |
| `-m, --model <MODEL>`   | 要使用的模型（例如 `grok-build`） |
| `-s, --session-id <ID>` | 使用此 **UUID** 创建**新**会话（若 UUID 无效或目标会话目录中已在使用则报错；不会恢复会话，请使用 `-r`/`-c`） |
| `--fork-session`        | 与 `-r`/`-c` 一起使用时，派生新的会话 ID，而不是追加到原会话 |
| `-r, --resume <ID_OR_TITLE>` | 按 ID 恢复现有会话；对于当前目录，也可按标题恢复并忽略大小写（重复项中只有一个手动重命名的匹配项时取它；其余重复项会报错并列出各自 ID；形似 UUID 的值始终走 ID 路径；脚本应优先使用 ID） |
| `-c, --continue`        | 继续当前目录中最近的会话 |
| `--cwd <PATH>`          | 设置工作目录 |
| `--output-format <FMT>` | 输出格式：`plain`、`json`、`streaming-json`、`streaming-messages-json` |
| `--include-partial-messages` | 发出原始 `stream_event` 增量。仅影响 `--output-format streaming-messages-json`；其他格式会忽略（并发出警告）。 |
| `--yolo`                | 自动批准所有工具执行 |
| `--rules <TEXT>`        | 系统提示的自定义规则 |
| `--tools <TOOLS>`       | 内置工具允许列表（逗号分隔）。除非拒绝，否则 MCP 元工具仍可用。仅限无头模式。 |
| `--disallowed-tools <TOOLS>` | 要移除的内置工具拒绝列表（逗号分隔）。支持 `Agent` 条目。仅限无头模式。 |
| `--max-turns <N>`       | 停止前允许的智能体轮次数。仅限无头模式。 |
| `--reasoning-effort` / `--effort <LEVEL>` | 推理模型的推理力度。规范级别：`none`、`minimal`、`low`、`medium`、`high`、`xhigh`、`max`（每个都是不同层级；模型只接受其菜单声明的级别）。也接受按模型提供的菜单选项 ID（例如 `deep` → 映射后的 wire 值），与 `/effort` 相同。在 TUI 和无头模式中都有效。 |
| `--permission-mode <MODE>` | 权限模式。`bypassPermissions` 启用始终批准（见 [权限与安全](22-permissions-and-safety.md#permission-modes)）；要默认拒绝，请在 `.claude/settings.json` 中使用 `defaultMode`。 |
| `--allow <RULE>`        | 带 glob 模式的权限允许规则（可重复）。在 TUI 和无头模式中都有效。 |
| `--deny <RULE>`         | 带 glob 模式的权限拒绝规则（可重复）。在 TUI 和无头模式中都有效。 |
| `--prompt-json <JSON>`  | 以 JSON 内容块提供提示 |
| `--prompt-file <PATH>`  | 从文件读取提示 |
| `--verbatim`            | 按给定内容原样发送提示 |
| `--no-auto-update`      | 禁用本会话的更新检查 |
| `--sandbox <PROFILE>`   | 文件系统/网络访问的沙箱配置 |

> **注意：** `--tools`、`--disallowed-tools`、`--max-turns` 和 `--agents` 是仅限无头模式的标志。在交互式 TUI 中使用时会打印警告并忽略标志。`--reasoning-effort`/`--effort`、`--permission-mode`、`--allow` 和 `--deny` 在两种模式中都有效。更多标志（智能体和工作树）见[其他无头标志](#additional-headless-flags)。

<a id="tool-filtering"></a>
### 工具过滤

使用 `--tools` 将智能体限制为明确的工具集合（允许列表），或使用 `--disallowed-tools` 从默认集合中移除特定工具（拒绝列表）。两者都接受逗号分隔的工具名称。

工具名称是内部工具 ID（例如 Shell 工具是 `run_terminal_cmd`，而不是 `bash`）。

```bash
# 只允许只读工具
grok-zh -p "Explain this codebase" --tools "read_file,grep,list_dir"

# 移除网页访问和文件编辑
grok-zh -p "Review this code" --disallowed-tools "web_search,web_fetch,search_replace"

# 移除 Shell 访问
grok-zh -p "Review this code" --disallowed-tools "run_terminal_cmd"
```

`--disallowed-tools` 还支持特殊的 `Agent` 条目，用来控制子智能体生成：

| 条目 | 效果 |
| ---------------------- | --------------------------------------- |
| `Agent`                | 阻止生成所有子智能体 |
| `Agent(explore)`       | 仅阻止 `explore` 子智能体类型 |
| `Agent(explore, plan)` | 阻止多个指定类型 |

```bash
# 阻止智能体生成任何子智能体
grok-zh -p "Fix this bug" --disallowed-tools "Agent"

# 只阻止 explore 子智能体
grok-zh -p "Refactor this module" --disallowed-tools "Agent(explore)"
```

`--tools` 保留所选智能体配置的注入策略：标准配置会在应用允许列表之前注入已启用的可选工具，而精选配置保持严格限制。最终工具集保留请求的工具以及始终启用的 MCP 元工具。同时提供两个标志时，以 `--disallowed-tools` 为准。

<a id="permission-rules-allow-deny"></a>
### 权限规则（`--allow` / `--deny`）

权限规则控制特定工具调用是自动批准、拒绝还是需要用户确认。与完全移除工具的 `--disallowed-tools` 不同，权限规则会保留工具，但限制其执行。

规则采用 `ToolPrefix(glob_pattern)` 语法：

| 前缀 | 控制内容 |
| ------------- | ---------------------------------- |
| `Bash(...)`   | Shell 命令执行 |
| `Edit(...)`   | 文件编辑（路径 glob） |
| `Write(...)`  | 文件写入（路径 glob） |
| `Read(...)`   | 文件读取（路径 glob） |
| `Grep(...)`   | 搜索操作（路径 glob） |
| `WebFetch(...)` | URL 获取（glob 或 `domain:host`） |
| `MCPTool(...)` | MCP 工具调用 |

对于路径规则（`Read`、`Edit`、`Write`、`Grep`），`*` 是单层通配符，`**` 是递归通配符。对于 `Bash` 规则，`*` 匹配包括空格在内的任意字符。不带括号的前缀会匹配该类型的所有调用，而 `Bash(cmd:*)` 等同于对 `cmd` 使用前缀匹配。完整匹配语义见 [22-permissions-and-safety.md](22-permissions-and-safety.md#rule-matching-reference)。

```bash
# 拒绝匹配 "rm*" 的 Shell 命令
grok-zh -p "Clean up this project" --deny "Bash(rm*)"

# 允许 npm 命令，拒绝 sudo
grok-zh -p "Set up the project" --allow "Bash(npm*)" --deny "Bash(sudo*)"

# 允许所有 bash 命令（自动批准而不提示）
grok-zh -p "Build the project" --allow "Bash"
```

`--allow` 和 `--deny` 可以重复使用。拒绝规则优先于允许规则。

---

<a id="output-formats"></a>
## 输出格式

无头模式支持四种输出格式，通过 `--output-format` 选择。

<a id="plain-default"></a>
### plain（默认）

适合直接显示或管道传输的人类可读文本：

```
Here's a summary of the codebase...
```

<a id="json"></a>
### json

响应完成后发出一个 JSON 对象：响应文本、停止原因、会话 ID、请求 ID（存在推理时还包括 `thought`）。提示到达模型后，同一对象还会携带开销字段（`usage`、`num_turns`、`modelUsage`、cost）。`stopReason` 是 snake_case 的 ACP/Messages 标记（`end_turn`、`max_tokens`、……）。

```json
{
  "text": "Here's a summary of the codebase...",
  "stopReason": "end_turn",
  "sessionId": "abc123",
  "requestId": "xyz789",
  "num_turns": 7,
  "usage": {
    "input_tokens": 7210,
    "cache_read_input_tokens": 41000,
    "cache_creation_input_tokens": 0,
    "output_tokens": 1893,
    "reasoning_tokens": 412,
    "total_tokens": 50103
  },
  "modelUsage": {
    "grok-build": {
      "inputTokens": 7210,
      "outputTokens": 1893,
      "cacheReadInputTokens": 41000,
      "modelCalls": 7,
      "costUSD": 0.01268905
    }
  },
  "total_cost_usd": 0.01268905,
  "total_cost_usd_ticks": 126890500
}
```

使用说明：

- `usage` 汇总提示所用的令牌，包括在轮次结束前完成的子智能体（也会出现在各自的 `modelUsage` 键下）。压缩和其他侧模型调用不计入。
- **令牌字段策略（无头结果 / `end` / 错误开销）：**
  - `usage.input_tokens` 和 `modelUsage.*.inputTokens` **仅包含未缓存的令牌**。
  - `cache_read_input_tokens` / `cacheReadInputTokens` 是缓存命中。
  - `total_tokens` 是完整输入 + 输出（包括两个缓存桶）：`total_tokens = input_tokens + cache_read_input_tokens + cache_creation_input_tokens + output_tokens`。
  - ACP `_meta.usage.inputTokens`（PromptUsage）仍是**完整**提示总和；只有无头投影器会减去缓存。自动化开销时应优先使用无头字段。
- `num_turns` 统计提示账本中记录的主智能体模型轮次（报告过用量的工具循环轮次）。子智能体采样调用不会增加此值。每模型调用数（包括子智能体）保存在 `modelUsage.*.modelCalls`。这与 `--max-turns` 使用同一计数族；当轮次缺少用量或触发门控时，不保证完全相等。
- 只有服务器报告了**完整**开销时才会出现 `total_cost_usd`。缺少该字段表示未报告或不完整，绝不表示免费。当前 API-key 流量会记录开销；池/OAuth 路径在服务器盖章开销前通常省略。当部分调用缺少开销时，`cost_is_partial` 为 true，并且会省略**所有**开销浮点数（`total_cost_usd` 和每个 `modelUsage.*.costUSD`），这样消费者无法将模型行相加伪造完整账单。
- `total_cost_usd_ticks` 是相同数值的精确整数 tick（1 USD = 10^10 tick），在相同条件下出现。账单对账应使用它：按调用求和的 tick 与服务器用量导出完全一致，而浮点美元无法保证这一点。
- 当无法应用子智能体用量、嵌套子智能体用量不完整，或成功路径的排空超时（轮次任务最多 120 秒）时，`usage_is_incomplete` 为 true，开销浮点数会同样省略（令牌总数可能少计子智能体）。取消快照不会进行长时间排空，在子智能体仍运行时即标记为不完整。若不完整且没有记录的令牌，只会发出 `usage_is_incomplete`（不会发出值为零的 `usage` 对象）。
- 从未到达模型的提示会省略开销字段。

`sessionId` 字段可用于稍后恢复会话。

失败时，Grok 会发出错误对象（进程以非零状态退出）。如果记录了用量，提示级失败也可能包含冻结的开销字段：

```json
{"type":"error","message":"Couldn't start session: ..."}
```

<a id="streaming-json"></a>
### streaming-json

每行一个 JSON 的换行分隔格式，每行是带 `type` 标签的对象，源自智能体 ACP 会话更新。叶字段名称（`toolCallId`、`kind`、`rawInput`、`rawOutput`）遵循 ACP；`toolName` 和 `usage` 行是 xAI 的扩展。通过按 `type` 分支来消费。

```json
{"type":"thought","data":"Analyzing the directory structure..."}
{"type":"tool_call","toolCallId":"call_1","title":"Read","kind":"read","status":"in_progress","toolName":"read_file","rawInput":{"path":"src/main.rs"},"content":[],"locations":[]}
{"type":"tool_call_update","toolCallId":"call_1","status":"completed","content":[],"rawOutput":{"lines":42},"locations":[]}
{"type":"text","data":"Here's a summary"}
{"type":"usage","messageId":"resp_1","stopReason":"end_turn","usage":{"input_tokens":812,"output_tokens":45,"cache_read_input_tokens":0,"cache_creation_input_tokens":0,"reasoning_tokens":0},"signature":"..."}
{"type":"end","stopReason":"end_turn","sessionId":"abc123","requestId":"xyz789","usage":{...},"num_turns":7,"modelUsage":{...}}
```

事件类型：

| 类型 | 说明 |
| ------------------ | ------------------------------------------------------------------------------------------- |
| `text`             | 智能体响应文本的一个分块 |
| `thought`          | 内部推理（思考令牌） |
| `tool_call`        | 智能体启动的工具调用（`toolCallId`、`toolName`、`kind`、`status`、`rawInput`、`content`、`locations`） |
| `tool_call_update` | 工具调用的进度或结果（`status`、`rawOutput`、`content`、`locations`） |
| `usage`            | 每个模型响应一个的响应边界（`messageId`、`stopReason`、`usage`、`signature`） |
| `plan`             | 智能体当前计划（`entries`） |
| `available_commands` | 工具和斜杠命令列表（`tools`、`commands`） |
| `end`              | 最终事件；可用时包含元数据和开销字段 |
| `error`            | 发生错误（携带 `message`，以及有记录时的开销字段） |

`end` 始终是最后一个事件。`end` 上的开销字段与 json 对象的形状一致（snake_case 的未缓存 `input_tokens`、安全的开销浮点数）。`end.stopReason` 是 snake_case 的轮次停止原因（`end_turn`、`max_tokens`、`max_turn_requests`、`refusal`、`cancelled`）；逐响应的提供方原因（例如 `tool_use`、`pause_turn`）位于 `usage` 行的 `stopReason` 中。Messages API 后端会填充每响应的 `message_id`/`stopReason`/`signature`；其他后端报告其自身携带的内容。

Grok 也可能发出 `max_turns_reached` 和 `auto_compact_*` 事件；应将列表视为非穷尽集合，并按 `type` 分支。

<a id="streaming-messages-json"></a>
### streaming-messages-json

每行一个 JSON 的换行分隔格式，采用 Messages API 的 `stream-json` wire 格式。承载数据的表面与 Messages 形状完全一致，包括 `assistant`/`user` 消息体、`usage`、`tool_use`/`tool_result`、内联网页搜索、`stop_reason` 以及 `--include-partial-messages` 事件封装。重建消息、读取开销或检测错误的消费者无需改动即可工作。

`system`/`init` 和终端 `result` 行携带元数据。Grok 会发出有真实数据的字段，省略无法填充的纯占位字段，而不是填零。因此这两行可能无法通过严格的 `init`/`result` 模式校验。下面列出各字段；将某个字段视为权威来源前请先阅读保真度说明。若要获得没有占位形状的干净 xAI 原生流，请使用 `streaming-json`。

流以 `system`/`init` 行开始，随后是 `assistant` 消息（其 `message.content[]` 保存 `text`、`thinking` 和 `tool_use` 块）、携带 `tool_result` 块的 `user` 消息，以及终端 `result`：

```json
{"type":"system","subtype":"init","session_id":"abc123","apiKeySource":"user","model":"grok-build","cwd":"/repo","permissionMode":"default","tools":["read_file","bash"],"slash_commands":["review"],"mcp_servers":[{"name":"linear","status":"connected"}],"skills":[],"uuid":"..."}
{"type":"assistant","message":{"id":"msg_0","type":"message","role":"assistant","model":"grok-build","content":[{"type":"text","text":"Let me read the file."},{"type":"tool_use","id":"call_1","name":"read_file","input":{"path":"src/main.rs"}}],"stop_reason":"tool_use","stop_sequence":null,"usage":{...}},"parent_tool_use_id":null,"session_id":"abc123","uuid":"..."}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call_1","content":"fn main() {}","is_error":false}]},"parent_tool_use_id":null,"session_id":"abc123","uuid":"..."}
{"type":"result","subtype":"success","is_error":false,"duration_ms":0,"duration_api_ms":0,"num_turns":7,"result":"Here's a summary...","stop_reason":"end_turn","total_cost_usd":0.0127,"usage":{"input_tokens":812,"output_tokens":210,"cache_read_input_tokens":0,"cache_creation_input_tokens":0,"server_tool_use":{"web_search_requests":0}},"modelUsage":{},"session_id":"abc123","uuid":"..."}
```

消息类型：

| 类型 | 说明 |
| ----------- | ---------------------------------------------------------------------- |
| `system`    | 会话前导（`subtype: "init"`），包含模型、cwd、权限模式、工具、斜杠命令和 MCP 服务器；`subtype: "compact_boundary"` 表示自动压缩 |
| `assistant` | 模型消息；`message.content[]` 保存 `text`/`thinking`/`tool_use`，以及内联后端网页搜索的 `server_tool_use`/`web_search_tool_result` |
| `user`      | 工具结果，作为 `message.content[]` 中的 `tool_result` 块 |
| `result`    | 终端消息，包含最终文本、停止原因和开销字段 |

`assistant` 和 `user` 消息携带 `session_id`、`uuid` 和 `parent_tool_use_id`（主会话为 `null`）。`system`/`init` 和终端 `result` 行携带 `session_id` 与 `uuid`，但没有 `parent_tool_use_id`。

每行的 `uuid` 都是为该行新生成的。它不是提供方、消息或事件 ID，也不是关联键。它与提供方的 `message.id` 不匹配（该值位于 `assistant.message.id`）。即使描述同一消息的行，`uuid` 也按行唯一，不携带跨行或跨运行身份。不要用它进行关联或去重。

文本和推理分块会按每次模型响应组合成一条 assistant 消息。并行的 `tool_result` 块会组合为一条 `user` 消息。`result.result` 是最终助手消息文本。默认模式下，不产生内容块的模型响应不会发出 `assistant` 行。只有 `--include-partial-messages` 才会显示这样的响应，即空的 `message_start` … `message_stop` 封装。

在 `init` 中，`skills` 是实时的。它列出会话可调用的技能名称，是会话公布命令中 `slash_commands` 的子集；当会话没有显示技能时为 `[]`。`init` 行只发出一次，并延迟到第一条输出行，以捕获会话公布的 `tools`、`slash_commands` 和 `skills`。Messages 模式没有第二个 `init`，因此流开始后命令列表发生变化不会再次公布。

其他 `init` 字段携带真实数据：

- `apiKeySource` 在 API-key 身份验证时为 `user`，否则为 `oauth`。Grok 不区分模式中的 `project`、`org` 和 `temporary` 来源。
- `permissionMode` 是映射到 Messages 枚举的有效无头模式：`--permission-mode` 的值，或 `--yolo` 下的 `bypassPermissions`，否则为 `default`。Grok 专有的 `auto` 等模式会折叠为 `default`。
- `mcp_servers[].status` 是 `x.ai/mcp/list` 的一次快照，仅在 `streaming-messages-json` 中输出：`connected`、`failed`、`needs-auth`、`pending` 或 `disabled`。握手中的服务器为 `pending`；只有会话报告 `sessionMcpResolved` 后才标记 `disabled`，未解析的行即使 `enabled` 暂为 false 也为 `pending`。快照不等待 Blocking 启动宽限期，但宽限期仍适用于提示的工具集。其他输出格式不包含该数组，也不会调用 `x.ai/mcp/list`。

Grok 会省略没有数据的模式纯占位 `init` 字段，而不是发出虚拟值：`claude_code_version`、`output_style` 和 `plugins`。

`result` 包含 `duration_ms`、`duration_api_ms`、`num_turns`、`stop_reason`、`total_cost_usd`、`usage`（Messages API 的 `message.usage` 形状）和 `modelUsage`。错误子类型还会包含 `errors[]`。Grok 会省略始终为空的模式 `permission_denials`，因为它不收集权限拒绝。`structured_output`（配合 `--json-schema`）使用 snake_case，与模式一致。

`model` 出现在 `init` 和每个 `assistant` 帧中。已知时是实际模型 ID；仅当发出时不知道模型才是字面量 `"unknown"`。

assistant 帧的 `stop_sequence` 端到端连通。当模型因匹配配置的停止序列而停止时（`stop_reason: "stop_sequence"`），它携带提供方匹配到的停止序列；其他停止原因和后端均为 `null`。在 `--include-partial-messages` 封装中，匹配序列同时位于刷新后的 `assistant` 帧和部分 `message_delta.stop_sequence`，因此部分重建与帧一致。只有部分的 `message_start.stop_sequence` 保持 `null`，因为消息打开时尚不知道匹配序列。

发出的错误子类型是 `error_max_turns`、`error_during_execution` 和 `error_max_structured_output_retries`。由于 grok 没有预算功能，模式中的 `error_max_budget_usd` 子类型永远不会发出。

`result.usage` 报告带三个互不重叠令牌桶的 Messages `message.usage` 形状：`input_tokens`（未缓存）、`cache_read_input_tokens` 和 `cache_creation_input_tokens`。Grok 从轮次的聚合账本派生这些桶，并重塑为上述形状。子智能体的缓存创建计入 `cache_creation_input_tokens`。聚合账本将其作为独立桶跟踪，因此不再折叠到 `input_tokens` 中。

`result.usage` 始终发出数值桶，即使数据缺失。当轮次用量账本不完整（与 `json` 格式出现 `usage_is_incomplete` 的条件相同），或根本没有聚合账本到达 reducer 时都会发生这种情况。由于 Messages API 模式没有表示不完整或缺失用量的标记，grok 无法核算的桶会退回 `0`。reducer 在两种情况下都会向 stderr 记录警告。这里读取全零的 `usage` 应理解为“未知”，而非“免费”。

嵌套的 `server_tool_use` 计数器会填充。`web_search_requests` 是本次运行发出的**成功**后端网页搜索数量。失败的搜索以及 `open_page` 等非搜索 `WebSearch` 操作会排除在外，这与不对出错搜索收费的 Messages API 一致。失败的后端搜索仍会以错误形状发出 `web_search_tool_result`（`content.type: "web_search_tool_result_error"`），但不会计数。其 `error_code` 是固定的 `"unavailable"` 占位符，不是后端转发的代码。由于 grok 没有服务端 `web_fetch`，不存在 `web_fetch_requests` 键，因此省略该占位符。

后端网页搜索是内联的。它会与周围文本折叠进同一个 `assistant` 帧。该帧携带 `server_tool_use` 块（`name: "web_search"`、`input.query`），紧接着是 `web_search_tool_result` 块。结果块的 `tool_use_id` 与 `server_tool_use.id` 匹配，其 `content` 是由 `{type, url, title}` 构成的 `web_search_result` 命中数组。这符合 Messages API 的内联服务端工具形状，而不是将响应拆分到多个帧中。

X 搜索和代码解释器是文档记录的差异。由于 Messages API 没有它们的内联块类型，它们保持通用形式，以客户端 `tool_use` 块加 `user` 的 `tool_result` 呈现。其他每个客户端工具同样保留 `tool_use`/`tool_result` 拆分。

`--include-partial-messages` 发出原始事件封装，使消费者能够使用 Messages 流式累加器重建每条消息。封装包含 `message_start`、`content_block_start`/`content_block_delta`/`content_block_stop`、`message_delta` 和 `message_stop`。它携带累加器所需的结构事件。增量比 Messages API 的令牌级流更粗：工具输入以单个 `input_json_delta` 到达，并且永远不会产生 `citations_delta`（见下文）。结果是忠实重建每条消息，而不是逐令牌回放。

在 Messages API 后端上，封装是忠实的。`message_start` 携带真实提供方 `message.id` 和输入侧 `usage`。思考块会按顺序发出其 `signature_delta`，然后才是块的 `content_block_stop`。`message_start.usage` 的输入侧报告消息打开时已知的全部三个提示侧桶：`input_tokens`（未缓存部分）、`cache_read_input_tokens` 和 `cache_creation_input_tokens`。因此缓存命中会在 `message_start` 可见，而不只是稍后的 `message_delta`/`result` 中出现。`output_tokens` 在此处初始化为 `0`，并在 `message_delta` 中最终确定。即使响应开始但没有内容，也会发出没有内容块的 `message_start` … `message_stop` 封装。

某些后端只在轮次结束时显示每响应元数据。这些后端会回退到合成的 `message_start.id` 和输入 `usage` 的零初始值，并将推理 `signature` 延迟到最终的 `assistant` 行；在这种情况下，最终行是权威来源。

工具调用输入作为携带完整参数 JSON 的单个 `input_json_delta` 发出，随后是 `content_block_stop`。它不是令牌级片段序列。这是有意偏离 Messages API 增量 `partial_json` 流的设计。Grok 的 ACP 工具调用路径会在参数完全解析后将每个工具调用作为一个经过验证的 JSON 对象交付，因此单个增量才是准确表示。无论哪种方式，拼接 `partial_json` 的消费者都能重新组装出相同对象。后端网页搜索 `server_tool_use` 块的 `input.query` 也以同样方式作为一个 `input_json_delta` 发出。

Messages API 的 `citations_delta` 携带被引用文本跨度（例如网页搜索结果）的内联引文。该流不会产生它。Grok 的 Messages 内容增量仅限文本、思考、签名和工具输入 JSON，因此没有可作为 `citations_delta` 展示的引文数据。后端网页搜索源 URL 会改为内联报告在已完成的 `web_search_tool_result` 块中（见上文），而不是作为逐跨度文本引文。

少数字段存在保真度注意事项。

`duration_ms` 是提示执行的挂钟时间。`duration_api_ms` 是每次模型调用报告的时间总和。未报告自身持续时间的模型调用贡献 `0`，因此 `duration_api_ms` 可能少计真实 API 时间。

已知时，`num_turns` 和 `total_cost_usd` 是权威值。未知时，`num_turns` 回退为本轮完成的模型响应数，`total_cost_usd` 回退为 `0`。已完成但没有内容的响应不会发出 `assistant` 行，但仍计为一轮。不会过量报告开销。

`modelUsage` 携带 grok 跟踪的每模型令牌和开销字段，以及归属于活动模型的 `webSearchRequests`。reducer 跟踪单个全局网页搜索计数，而不是按模型计数，因此总数会落到当前或最后一个模型，其他行保持 `0`。当某模型的开销未知或被隐藏时，其每模型 `modelUsage.*.costUSD` 为 `0`。这与顶层 `total_cost_usd` 的失败关闭为零行为相同。`json` 格式在部分情况下完全省略开销浮点数，但此流保留字段并设为 `0`。`contextWindow` 是当前模型真实的总上下文窗口（与 grok 用于自动压缩的值相同），只出现在当前模型行。其他行省略它；当前行在窗口未知时也省略。grok 没有 `maxOutputTokens` 目录，因此完全省略该键。没有每模型明细时，`modelUsage` 为 `{}`。

与 `streaming-json` 一样，该流是只读的。工具批准及其他双向流程使用 ACP 接口（`grok-zh agent`）。

---

<a id="session-management-in-headless-mode"></a>
## 无头模式下的会话管理

默认情况下，每次 `grok-zh -p` 调用都会创建全新会话。要在多次调用之间保持上下文，请使用会话标志。

<a id="named-sessions-s"></a>
### 命名会话（`-s`）

要在无头调用之间携带上下文，请使用 `-r/--resume` 或 `-c/--continue`。`-s/--session-id` 仅用于使用 **UUID** 创建**新**会话（不是 UUID 或目标目录中已存在时会报错）。旧的隐藏 `-s` upsert/恢复行为已移除。继续会话请使用 `-r`/`-c`。与 `-r`/`-c` 一起使用时，`-s` 需要 `--fork-session`：

```bash
# 启动无头会话并捕获其 ID
grok-zh -p "Review the changes in this PR" --output-format json | jq -r '.sessionId'

# 在同一会话中继续
grok-zh -p "Now check for security issues" --resume "<id>"

# 可选：使用客户端选择的 UUID 创建（不得已存在）
grok-zh -p "hello" --session-id "$(uuidgen | tr '[:upper:]' '[:lower:]')" --output-format json
```

> **注意：** `-s/--session-id` 只创建新会话（UUID 有效；已在使用时会报错）。要恢复会话，请使用 `-r`。

<a id="resume-r"></a>
### 恢复（`-r`）

`-r/--resume` 标志按 ID 恢复特定会话；当值不是 ID 时，对于当前目录也可按标题恢复并忽略大小写（重复项中只有一个手动重命名的匹配项时取它；其余重复项会报错并列出各自 ID；形似 UUID 的值始终走 ID 路径，因此脚本应优先使用 ID）。如果会话不存在则报错：

```bash
# 从之前的 JSON 响应获取会话 ID
grok-zh -p "Remember: the secret number is 42" --output-format json
# 输出包含 "sessionId": "abc123"

# 恢复该确切会话
grok-zh -p "What's the secret number?" --resume abc123
```

<a id="continue-c"></a>
### 继续（`-c`）

`-c/--continue` 标志会继续当前工作目录中最近的会话：

```bash
grok-zh -p "Continue where we left off" -c
```

<a id="extracting-session-ids"></a>
### 提取会话 ID

使用 `--output-format json` 并解析 `sessionId` 字段：

```bash
grok-zh -p "Hello" --output-format json | jq -r '.sessionId'
```

---

<a id="piping-input-and-output"></a>
## 管道传输输入和输出

无头模式可自然地配合 Unix 管道和重定向。

<a id="standard-output"></a>
### 标准输出

```bash
# 将输出写入文件
grok-zh -p "Generate a README" > README.md

# 使用 jq 解析 JSON 输出
grok-zh -p "List files" --output-format json | jq -r '.text'
```

<a id="standard-input"></a>
### 标准输入

无头模式不会将管道 stdin 读入提示。请通过命令替换或 `--prompt-file` 传入外部内容：

```bash
# 通过命令替换将 git diff 作为上下文
grok-zh -p "Write a concise commit message for these changes:

$(git diff --staged)"

# 或从文件读取提示
grok-zh --prompt-file ./prompt.txt
```

---

<a id="ci-cd-integration-examples"></a>
## CI/CD 集成示例

<a id="automated-code-review"></a>
### 自动代码审查

```bash
grok-zh -p "Review changes for bugs and security issues." \
  --output-format json --yolo | jq -r '.text' > review.md
```

<a id="pre-commit-hook"></a>
### 提交前钩子

```bash
grok-zh -p "Review staged changes for obvious bugs. Reply OK if fine, or list issues." \
  --yolo --output-format json | jq -r '.text' | grep -q "^OK" || exit 1
```

<a id="batch-processing"></a>
### 批处理

```bash
for file in src/*.js; do
  grok-zh -p "Migrate $file from CommonJS to ES modules." --yolo
done
```

---

<a id="scripting-patterns"></a>
## 脚本模式

<a id="python-wrapper"></a>
### Python 包装器

Grok 的无头模式可以包装成与 OpenAI 兼容的聊天补全 API：

```python
import asyncio
import json
import os

class GrokChat:
    """使用无头模式的简单 OpenAI 兼容包装器。"""

    def __init__(self, cwd="."):
        self.cwd = cwd
        self.env = {**os.environ}

    def _build_cmd(self, prompt, model, stream):
        return ["grok-zh", "-p", prompt, "-m", model, "--cwd", self.cwd,
                "--output-format", "streaming-json" if stream else "json",
                "--yolo"]

    async def create(self, messages, model="grok-build", stream=False):
        prompt = messages[-1]["content"] if len(messages) == 1 else "\n".join(
            f"{m['role']}: {m['content']}" for m in messages
        )
        cmd = self._build_cmd(prompt, model, stream)

        if stream:
            return self._stream(cmd)

        proc = await asyncio.create_subprocess_exec(
            *cmd, env=self.env, stdout=asyncio.subprocess.PIPE
        )
        stdout, _ = await proc.communicate()
        data = json.loads(stdout.decode()) if stdout else {"text": ""}
        return {
            "choices": [{
                "message": {"role": "assistant", "content": data.get("text", "")},
                "finish_reason": "stop"
            }]
        }

    async def _stream(self, cmd):
        proc = await asyncio.create_subprocess_exec(
            *cmd, env=self.env, stdout=asyncio.subprocess.PIPE
        )
        async for line in proc.stdout:
            if not line.strip():
                continue
            event = json.loads(line)
            if event.get("type") == "text":
                yield {"choices": [{"delta": {"content": event["data"]}}]}
            elif event.get("type") == "end":
                yield {"choices": [{"delta": {}, "finish_reason": "stop"}]}


async def main():
    client = GrokChat(cwd=".")
    response = await client.create(
        [{"role": "user", "content": "What files are here?"}]
    )
    print(response["choices"][0]["message"]["content"])

asyncio.run(main())
```

<a id="shell-script"></a>
### Shell 脚本

```bash
#!/bin/bash
# 运行代码审查；发现问题时以失败状态退出

RESULT=$(grok-zh -p "Review this PR for bugs. Output JSON with 'issues' array." \
  --output-format json --yolo | jq -r '.text')

ISSUE_COUNT=$(echo "$RESULT" | jq '.issues | length' 2>/dev/null || echo "0")

if [ "$ISSUE_COUNT" -gt 0 ]; then
  echo "Found $ISSUE_COUNT issues"
  echo "$RESULT" | jq '.issues[]'
  exit 1
fi

echo "No issues found"
```

---

<a id="always-approve-for-automation"></a>
## 自动化的始终批准

`--always-approve`（别名 `--yolo`，与 `--permission-mode bypassPermissions` 相同）会在没有交互式权限提示的情况下运行工具调用。拒绝规则、钩子和管理员锁仍然适用（见 [权限与安全](22-permissions-and-safety.md#permission-modes)）。

```bash
grok-zh -p "Format all files" --always-approve
grok-zh -p "Run the tests and fix any failures" --cwd ~/projects/my-app --always-approve
```

智能体服务器和 SDK 见 [智能体模式](15-agent-mode.md#automation-and-sdks)。

---

<a id="environment-variables-for-headless"></a>
## 无头模式的环境变量

影响无头模式的主要环境变量：

| 变量 | 说明 |
| ------------------------------- | ------------------------------------------------------------- |
| `XAI_API_KEY`        | 用于身份验证的 API 密钥（没有浏览器登录时必需） |
| `GROK_HOME`                    | 覆盖配置目录（默认：`~/.grok`） |
| `GROK_LOG_FILE`                | 日志文件路径（按原样用作路径；在无头模式和 TUI 中都有效，遵循 `RUST_LOG`） |
| `RUST_LOG`                     | 日志级别过滤器（例如 `debug`）。无头日志写入 stderr。 |

对于无法访问浏览器的 CI 环境，请使用 [console.x.ai](https://console.x.ai) 的 API 密钥设置 `XAI_API_KEY`：

```bash
export XAI_API_KEY="xai-..."
grok-zh -p "Run the test suite" --yolo
```

---

<a id="exit-codes"></a>
## 退出代码

| 代码 | 含义 |
| ---- | ------------------------------------ |
| `0`  | 成功。提示正常完成 |
| `1`  | 错误。身份验证失败、网络错误或运行时错误 |
| `130` | 被 SIGINT（Ctrl+C）中断 |
| `143` | 被 SIGTERM 终止 |

---

<a id="authentication-for-headless-environments"></a>
## 无头环境的身份验证

无头使用可通过以下方式之一进行身份验证：

- **`XAI_API_KEY`**：CI 中最简单的方式。见上面的[环境变量](#environment-variables-for-headless)。
- **`grok-zh login --device-auth`**（或 `--device-code`）：目标机器无需浏览器。见[身份验证 > 设备代码流程](02-authentication.md#device-code-flow)。
- **`grok-zh login`**：在有图形界面的机器上使用基于浏览器的 OAuth2。

如果之前登录过，会自动使用缓存的凭据。

---

<a id="tips"></a>
## 提示

- 无头模式默认启动**全新会话**。使用 `-r/--resume` 或 `-c/--continue` 在调用之间保持上下文。
- `--output-format json` 响应始终包含 `sessionId`，可将其用于后续调用的 `--resume`。
- 将 `--yolo` 与 `--rules` 组合以设置护栏：`grok-zh -p "..." --yolo --rules "Never delete files"`。
- 调试时提高日志级别并捕获 stderr：`RUST_LOG=debug grok-zh -p "..." 2> debug.log`。

---

<a id="project-root-discovery"></a>
## 项目根目录发现

Grok 启动时，会从 `--cwd`（或当前目录）向上遍历，直到找到 `.git` 目录，从而发现项目根目录。

注意：如果 `--cwd` 位于大型仓库（例如 monorepo）内部，Grok 会将该仓库识别为项目根目录，并将发现范围（AGENTS.md、技能、Git 历史）限定于此，这可能导致启动变慢。将 `--cwd` 指向希望处理的具体子项目，以缩小范围。

---

<a id="file-locations"></a>
## 文件位置

Grok 将数据存储在 `~/.grok`（使用 `GROK_HOME` 覆盖；见[无头模式的环境变量](#environment-variables-for-headless)）：

| 路径 | 内容 |
| ------------------------ | ------------------------------------- |
| `config.toml`            | 用户配置 |
| `auth.json`              | 缓存的 OAuth2/API 凭据 |
| `version.grok-build-zh.json` | 社区版更新检查的版本缓存（`version.json` 为上游/旧版回退） |
| `sessions/`              | 会话记录（SQLite） |
| `memory/`                | 跨会话记忆存储 |
| `logs/`                  | 内部日志文件（例如 `unified.jsonl`） |
| `logs/mcp/`              | MCP 服务器日志 |
| `skills/`                | 用户技能定义 |
| `personas/`              | 用户范围的智能体 persona |
| `crash/`                 | 崩溃报告 |
| `trace-exports/`         | 会话跟踪导出 |
| `worktrees/`             | Git 工作树元数据 |

<a id="read-only-grok"></a>
### 只读 `~/.grok`

对于容器或 CI，可以将 `~/.grok` 以只读方式挂载：

- 预先填充 `auth.json`，或使用 `XAI_API_KEY`
- 会话持久化会静默失败（使用临时会话）
- 更新检查记录警告并跳过

```bash
export XAI_API_KEY="xai-..."
export GROK_DISABLE_AUTOUPDATER=1
grok-zh -p "..." --no-auto-update
```

---

<a id="update-check-suppression"></a>
## 禁止更新检查

| 方法 | 范围 |
| ------------------------------- | --------- |
| `--no-auto-update`              | 会话 |
| `GROK_DISABLE_AUTOUPDATER=1`    | 进程 |
| 非 TTY stderr（自动检测）       | 自动 |
| `[cli] auto_update = false`     | 持久化 |

将 `GROK_DISABLE_AUTOUPDATER` 设置为假值（`0`、`false`、`off`、`no` 或空值，不区分大小写）等同于未设置。Agent SDK 会为其生成的非 leader 智能体注入 `GROK_DISABLE_AUTOUPDATER=1`（SDK 隔离环境中的假值会保持更新开启），而 stdio 智能体会跳过后台更新，除非它从受管理安装（`$GROK_HOME/bin/grok`）运行。

更新消息写入 **stderr**。对于 `--output-format json`，stdout 保持干净。另见[无头模式的环境变量](#environment-variables-for-headless)。

---

<a id="additional-headless-flags"></a>
## 其他无头标志

这些标志补充上面的[命令行选项](#command-line-options)表。已在表中列出的标志（`--prompt-json`、`--prompt-file`、`--verbatim`、`--sandbox`、`--no-auto-update`）不会在此重复。

| 标志 | 说明 |
| ----------------------------- | ------------------------------------------------- |
| `--agent <NAME>`              | 智能体名称或定义文件路径 |
| `--agents <JSON>`             | 以内联 JSON 提供的子智能体定义 |
| `--system-prompt-override`    | 覆盖智能体的系统提示 |
| `--no-plan`                   | 禁用计划模式 |
| `--no-subagents`              | 禁止生成子智能体 |
| `--no-memory`                 | 禁用跨会话记忆 |
| `--disable-web-search`        | 禁用网页搜索和获取工具 |
| `--no-alt-screen`             | 内联运行（不使用备用屏幕） |
| `--worktree [NAME]`           | 在新的 Git 工作树中启动会话 |
| `--ref <REF>` / `--worktree-ref <REF>` | 作为工作树基础的分支/标签/提交（与 `--worktree` 一起使用） |

---

<a id="interrupted-headless-runs"></a>
## 被中断的无头运行

收到 SIGINT/SIGTERM 时：

- 保存截至最后一次已完成工具调用的会话状态
- 工具所做的文件修改**不会回滚**
- SIGINT（`128 + 2`）的退出代码为 **130**，SIGTERM（`128 + 15`）的退出代码为 **143**；CI 流水线可以将它们与普通错误（退出代码 `1`）区分开
- 恢复：`grok-zh -p "continue" --resume "<id>"` 或 `grok-zh -p "continue" --continue`

有关命名会话以及 `-s`/`-r`/`-c` 标志的详情，请见[无头模式下的会话管理](#session-management-in-headless-mode)。
