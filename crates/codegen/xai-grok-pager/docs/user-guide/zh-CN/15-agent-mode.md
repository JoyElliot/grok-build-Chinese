<a id="agent-mode-acp-and-ide-integration"></a>
# 智能体模式（ACP）与 IDE 集成

智能体模式将 Grok 作为长期运行的服务器，客户端通过 [ACP](https://agentclientprotocol.com)（JSON-RPC）与其通信。可从 IDE、SDK、评测 harness 和自定义应用使用。若要发送一次性提示并打印结果后退出，请改用 `grok-zh -p`（[无头模式](14-headless-mode.md)）。

---

<a id="automation-and-sdks"></a>
## 自动化与 SDK

对于脚本、CI、评测和智能体服务器，先使用始终批准模式，使工具无需交互式权限提示即可运行。拒绝规则和钩子仍然适用。

```bash
# stdio（本地进程 / 许多 SDK）
grok-zh agent --always-approve stdio

# WebSocket 服务器
grok-zh agent --always-approve serve --bind 127.0.0.1:2419 --secret <token>
```

也可以在 `session/new` 上为每个会话设置始终批准：

```json
{
  "cwd": "/path/to/project",
  "mcpServers": [],
  "_meta": { "yoloMode": true }
}
```

交互式 TUI 用户通常保留默认模式（或显式选择 ask 或 auto）。参见[权限与安全](22-permissions-and-safety.md)。

---

<a id="what-is-acp"></a>
## 什么是 ACP？

[智能体客户端协议（Agent Client Protocol，ACP）](https://agentclientprotocol.com) 定义客户端如何通过 JSON-RPC 与编码智能体通信。Grok 的 ACP 覆盖：

- 会话（创建、加载、恢复）
- 提示和流式回复
- 工具调用更新
- 推理/思考流
- 会话未处于始终批准模式时的权限提示

---

<a id="stdio-transport"></a>
## stdio 传输

stdio 是常见的本地集成路径。智能体在 stdin 和 stdout 上使用 JSON-RPC 通信：

```bash
grok-zh agent --always-approve stdio
```

典型客户端包括 IDE 扩展（Zed、Neovim、Emacs）、自定义工具和 ACP SDK。

<a id="options"></a>
### 选项

智能体选项适用于每种传输（`stdio`、`serve`、`headless`、`leader`）。它们放在 `agent` 之后、模式名称之前。模式专用标志放在模式之后（例如 `serve --bind`）。

```bash
grok-zh agent --always-approve --model grok-build stdio
grok-zh agent --always-approve serve --bind 127.0.0.1:2419 --secret <token>
```

| 标志 | 说明 |
| ---- | ----------- |
| `-m, --model <MODEL>` | 模型 ID（例如 `grok-build`）。 |
| `--always-approve` | 运行时不显示交互式工具权限提示。别名：`--yolo`。 |
| `--reauth` | 智能体启动前进行身份验证。 |
| `--agent-profile <PATH>` | 从文件加载智能体配置。 |
| `--leader` / `--no-leader` | 连接共享 leader 进程，或强制使用本地智能体。当请求非 `off` 沙箱配置时会拒绝 leader 模式，从而让工具留在进程内（参见[沙箱模式](18-sandbox.md)）。 |

---

<a id="server-mode"></a>
## 服务器模式

```bash
grok-zh agent --always-approve serve --bind 127.0.0.1:2419 --secret <token>
```

客户端通过 WebSocket 连接，并使用 secret token 进行身份验证。如果省略 `--secret`，智能体会在启动时打印生成的 token；也可以设置 `GROK_AGENT_SECRET`。进程会在客户端重新连接之间保留状态。权限与其他入口一致；参见[权限与安全](22-permissions-and-safety.md)。

---

<a id="websocket-relay"></a>
## WebSocket 中继

要通过互联网访问智能体，请将智能体连接到中继，并让浏览器指向同一个中继：

```bash
grok-zh agent --always-approve headless --grok-ws-url wss://your-relay.example.com/ws
```

---

<a id="acp-protocol-basics"></a>
## ACP 协议基础

通信遵循 JSON-RPC 2.0 格式。典型的会话生命周期如下：

1. **初始化**——客户端发送带有能力的 `initialize`
2. **创建会话**——客户端发送带工作目录的 `session/new`
3. **发送提示**——客户端发送包含用户消息的 `session/prompt`
4. **接收更新**——智能体通过 `session/update` 通知发送流式内容
5. **处理权限**——智能体可能请求工具执行批准（或根据权限模式允许或拒绝）

<a id="architecture"></a>
### 架构

```
+------------------------------------------+
|              ACP 客户端                  |
|       （IDE、编辑器、自定义应用）        |
+-------------------+----------------------+
                    | 通过 stdio 的 JSON-RPC
+-------------------v----------------------+
|          grok-zh agent stdio             |
|                                          |
|  +---------+  +---------+  +---------+   |
|  | 会话管理器 |  | 工具注册表 |  | MCP 服务器 |   |
|  +---------+  +---------+  +---------+   |
+------------------------------------------+
```

---

<a id="streaming-updates"></a>
## 流式更新

ACP 会流式传输结构化事件。每条 `session/update` 通知都带有 `sessionUpdate` 字段，用来标识更新类型：

| `sessionUpdate` 值 | 说明 |
| --------------------- | ----------------------------------------------------- |
| `agent_message_chunk` | 智能体响应文本的一个分块。 |
| `agent_thought_chunk` | 智能体内部推理的一个分块。 |
| `tool_call`           | 新的工具调用（标题、种类、状态、输入）。 |
| `tool_call_update`    | 正在执行的工具调用的状态或结果更新。 |
| `plan`                | 智能体的执行计划。 |

每次更新都会命名其类型，因此客户端可以为推理、工具调用和响应文本渲染不同面板。

---

<a id="extension-methods"></a>
## 扩展方法

除基础 ACP 协议外，Grok 还在 `x.ai/` 前缀下定义了用于 SpaceXAI 特定功能的扩展方法，包括：

| 类别 | 前缀 | 示例 |
| -------------------------- | -------------------- | ------------------------------------------------ |
| **文件系统**             | `x.ai/fs/*`          | `list`、`exists`、`read_file`、`write_file`      |
| **Git**                    | `x.ai/git/*`         | `status`、`stage`、`commit`、`diffs`、`discard`  |
| **Git 工作树**           | `x.ai/git/worktree/*`| `create`、`remove`、`apply`、`list`、`gc`        |
| **搜索**                 | `x.ai/search/*`      | `fuzzy/open`、`fuzzy/change`、`content`          |
| **终端**               | `x.ai/terminal/*`    | `create`、`kill`、`output`、`wait_for_exit`      |
| **会话管理**     | `x.ai/session/*`     | `fork`、`resolve_local_for_worktree_resume`      |
| **对话与历史记录** | `x.ai/*`             | `prompt_history`、`rewind/*`、`compact_conversation` |
| **身份验证**         | `x.ai/auth/*`        | `get_url`、`submit_code`                         |
| **反馈与遥测**   | `x.ai/*`             | `feedback`、`telemetry/*`                        |

此处的表格展示了每个类别中的代表性方法。`x.ai/*` 集合是 SpaceXAI 特有的，可能会随版本发布扩展，因此并不完整；请从智能体的 `initialize` 响应中发现可用方法。

<a id="notifications-agent-to-client"></a>
### 通知（智能体到客户端）

智能体会向客户端发送推送通知，以提供实时更新：

| 通知 | 说明 |
| -------------------------- | ------------------------------------ |
| `x.ai/search/fuzzy/status` | 模糊搜索结果更新 |
| `x.ai/git/worktree/status` | 工作树创建进度 |
| `x.ai/fs_notify`           | 文件系统变更通知 |
| `x.ai/fs/index`            | 完整文件索引更新 |
| `x.ai/fs/index/delta`      | 增量文件索引更新 |
| `x.ai/session_notification`| 会话专属更新（差异审查、重试状态、自动压缩） |
| `x.ai/session/update`      | 会话更新（工具调用、内容） |

---

<a id="session-config-options"></a>
## 会话配置选项

`session/new` 和 `session/load` 的响应包含一个带类型的 `configOptions` 列表（这是标准 ACP，而不是 `x.ai/` 扩展）。使用 `session/set_config_option` 可修改实时会话选项。

| `configId` | 类别 | 效果 |
|------------|------|------|
| `model` | `model` | 切换会话模型（受 `allowed_models` 与聊天网关路由约束）。值必须是字符串 ID。 |
| `reasoning_effort` | `thought_level` | 在不切换模型的情况下调整当前模型的推理强度（不重写提示词，也不经过 `allowed_models`）。值必须是字符串 ID（`minimal`、`low`、`medium`、`high`、`xhigh`）。若模型未声明 `supportsReasoningEffort`，则会发出警告并忽略。 |

```json
{
  "sessionId": "…",
  "configId": "reasoning_effort",
  "value": { "value": "high" }
}
```

响应会返回**完整且已更新的**选项列表；`config_option_update` 会话通知也会将其同步给所有订阅客户端。在主导模式下，代理会监听 `configId: model`，以保持每个客户端的 `default_model` 同步。布尔值会被拒绝；目前尚未实现布尔选项。

---

<a id="session-meta-options"></a>
## 会话 `_meta` 选项

`session/new` 上的可选字段：

| 字段 | 说明 |
| ----- | ----------- |
| `rules` | 追加到系统提示的额外规则。 |
| `systemPromptOverride` | 替换系统提示。 |
| `agentProfile` | 智能体配置名称或 JSON 对象。 |
| `yoloMode` | 为 `true` 时，对本会话始终批准。 |
| `autoMode` | 为 `true` 时，对本会话使用自动权限模式。如果已启用始终批准，则此项被其取代。 |

```json
{
  "cwd": "/path/to/project",
  "mcpServers": [],
  "_meta": { "yoloMode": true }
}
```

---

<a id="acp-sdks"></a>
## ACP SDK

官方 SDK 库支持多种语言：

| 语言   | 包 |
| ---------- | ---------------------------------------------------------------------------------------- |
| TypeScript | [`@agentclientprotocol/sdk`](https://www.npmjs.com/package/@agentclientprotocol/sdk)     |
| Rust       | [`agent-client-protocol`](https://crates.io/crates/agent-client-protocol)                |
| Python     | [`agent-client-protocol-python`](https://github.com/PsiACE/agent-client-protocol-python) |
| Go         | [`acp-go-sdk`](https://github.com/coder/acp-go-sdk)                                     |
| Kotlin     | [`acp`](https://github.com/agentclientprotocol/kotlin-sdk)                               |

---

<a id="compatible-clients"></a>
## 兼容的客户端

| 客户端                                                   | 状态      |
| -------------------------------------------------------- | ----------- |
| [Zed](https://zed.dev/docs/ai/external-agents)           | 支持   |
| [Neovim](https://neovim.io)（CodeCompanion、avante.nvim） | 支持   |
| [Emacs](https://github.com/xenodium/agent-shell)         | 支持   |
| [marimo notebook](https://github.com/marimo-team/marimo) | 支持   |
| JetBrains                                                | 即将推出 |

---

<a id="integration-example-a-typescript-acp-client"></a>
## 集成示例：TypeScript ACP 客户端

```typescript
import { spawn, ChildProcess } from "child_process";
import * as readline from "readline";

class GrokACPChat {
  private proc!: ChildProcess;
  private sessionId!: string;
  private rl!: readline.Interface;

  constructor(private cwd = ".") {}

  async init() {
    this.proc = spawn("grok-zh", ["agent", "--always-approve", "stdio"]);
    this.rl = readline.createInterface({ input: this.proc.stdout! });

    await this.request("initialize", {
      protocolVersion: 1,
      clientCapabilities: {
        fs: { readTextFile: true, writeTextFile: true },
        terminal: true,
      },
    });

    const { sessionId } = await this.request("session/new", {
      cwd: this.cwd,
      mcpServers: [],
      _meta: { yoloMode: true },
    });
    this.sessionId = sessionId;
    return this;
  }

  private async request(method: string, params: any): Promise<any> {
    return new Promise((resolve) => {
      const msg = JSON.stringify({ jsonrpc: "2.0", id: 1, method, params });
      this.proc.stdin!.write(msg + "\n");

      this.rl.once("line", (line) => {
        resolve(JSON.parse(line).result || {});
      });
    });
  }

  async *streamPrompt(text: string) {
    const msg = JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "session/prompt",
      params: {
        sessionId: this.sessionId,
        prompt: [{ type: "text", text }],
      },
    });
    this.proc.stdin!.write(msg + "\n");

    for await (const line of this.rl) {
      const data = JSON.parse(line);

      if (data.method === "session/update") {
        const update = data.params.update;
        yield update; // { sessionUpdate, content, title, ... }
      } else if (data.result) {
        break; // Final response
      }
    }
  }
}

// 用法
const client = await new GrokACPChat(".").init();

for await (const update of client.streamPrompt("List the files in this project")) {
  switch (update.sessionUpdate) {
    case "agent_message_chunk":
      process.stdout.write(update.content?.text || "");
      break;
    case "agent_thought_chunk":
      console.log(`\n[Thinking: ${update.content?.text}]`);
      break;
    case "tool_call":
      console.log(`\n[Tool: ${update.title}]`);
      break;
  }
}
```

---

<a id="resources"></a>
## 资源

- [ACP 规范](https://agentclientprotocol.com/protocol/prompt-turn)
- [协议介绍](https://agentclientprotocol.com/overview/introduction)
