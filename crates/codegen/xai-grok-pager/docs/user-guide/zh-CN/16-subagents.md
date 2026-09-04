<a id="subagents-and-personas"></a>
# 子智能体与 Persona

子智能体是独立的子会话，可以并行处理任务。每个子智能体都有自己的上下文窗口，因此主智能体可以委派研究、实现、测试和代码审查，而不消耗自己的上下文。子智能体完成后，会向父级报告摘要。

默认启用子智能体。

---

<a id="agents-vs-personas"></a>
## 智能体与 Persona 的区别

智能体和 Persona 都可以定制行为，但作用层级不同：

| | **智能体（Agents）** | **Persona** |
|---|---|---|
| **配置内容** | 整个会话：模型、工具、提示模式、系统提示 | 添加到子智能体提示中的行为覆盖层 |
| **作用范围** | 主会话或子智能体 | 仅子智能体 |
| **设置方式** | 启动时设置，或通过智能体定义（`.md` 文件，位于 `.grok/agents/` 或 `~/.grok/agents/`）设置 | 在 `config.toml`（`[subagents.personas]`）中设置，或在 `.grok/personas/` 下的 `.toml` 文件中设置；在解析子智能体时应用 |
| **控制内容** | 模型、工具可用性、提示正文、技能 | 语气、输出格式、任务重点以及输入/输出契约 |
| **编辑者** | 你——在智能体模态框中，或通过编辑文件来创建、删除或切换 | 你——在配置或文件中定义自定义 Persona；内置 Persona 只读 |
| **示例** | `grok-build`、`explore`、`plan` | `researcher`、`concise` |

智能体定义会话本身；Persona 决定子智能体在会话中的行为。子智能体始终以某种智能体类型（例如 `general-purpose`）运行，解析时还可以在其上叠加 Persona。

在智能体模态框中管理二者。使用 `/config-agents`（别名 `/agents`）打开模态框，或直接用 `/personas` 打开 Personas 标签页。模态框有两个标签页：**Agents** 和 **Personas**。

---

<a id="disabling-subagents"></a>
## 禁用子智能体

可通过环境变量或配置文件禁用子智能体：

```bash
export GROK_SUBAGENTS=0              # 环境变量
```

```toml
# ~/.grok/config.toml
[subagents]
enabled = false
```

---

<a id="how-subagents-work"></a>
## 子智能体的工作方式

当主智能体确定要委派工作时，会调用 `spawn_subagent` 工具启动子会话。子会话具有：

- 独立于父级的上下文窗口
- 由其智能体类型和可选能力模式决定的工具集
- 在解析过程中应用的可选 Persona 指令

子智能体完成后，父级会收到子智能体的输出（通常是摘要）。

---

<a id="built-in-agent-types"></a>
## 内置智能体类型

`spawn_subagent` 工具接受 `subagent_type` 参数，用于选择子智能体的角色：

| 类型              | 说明                                          |
| ----------------- | -------------------------------------------- |
| `general-purpose` | 默认类型。可处理任何任务的全能力智能体。    |
| `explore`         | 研究智能体。搜索、读取、grep 并运行 shell 命令，但不编辑文件。用于代码库调查。 |
| `plan`            | 规划智能体。探索代码库并生成结构化实现计划；不编辑文件。 |

项目或用户定义的智能体可以添加新的类型，或按名称覆盖这些内置类型。

---

<a id="personas"></a>
## Persona

Persona 是一种命名的行为覆盖层。它的指令会作为 `<system-reminder>` 注入子智能体对话，从而塑造语气、输出格式和任务重点，但不会改变子智能体的智能体类型、模型或工具。

可在 `config.toml` 或 `.toml` 文件中定义 Persona：

```toml
[subagents.personas.researcher]
instructions = "You are a thorough researcher. Always cite specific file paths."
description = "Deep investigator."
```

Grok Build 会按优先级顺序从以下位置发现基于文件的 Persona：

- `.grok/personas/*.toml`（项目）
- `~/.grok/personas/*.toml`（用户）
- 内置 Persona 目录（最低优先级）

每个文件定义一个 Persona，文件名（不含扩展名）成为 Persona 名称。内联 `config.toml` Persona 的优先级高于文件。只发现 `.toml` 文件。

在智能体模态框的 Personas 标签页（`/personas`）中管理 Persona。内置 Persona 为只读；你定义的 Persona 可编辑。

> **注意：** Grok Build 通过子智能体解析和角色应用 Persona，而不是通过 `spawn_subagent` 参数应用。主智能体生成子级时不会传递 Persona 名称。

<a id="persona-fields"></a>
### Persona 字段

| 字段               | 说明                                                          |
| ------------------- | ------------------------------------------------------------ |
| `instructions`      | 以内联指令文本作为 Persona 层应用。               |
| `instructions_file` | 指令文件的路径；在生成时加载，并在 `instructions` 之后合并。 |
| `description`       | Persona 目录中显示的简短摘要。若未提供，则回退为 `instructions` 的第一段。 |
| `inputs` / `outputs`| 声明的输入和输出契约（见下文）。                     |
| `model`             | 使用 Persona 时应用的模型覆盖。                    |
| `reasoning_effort`  | 使用 Persona 时应用的推理力度。                  |
| `default_isolation` | 默认隔离模式（`none` 或 `worktree`）。                      |

<a id="input-output-contracts"></a>
### 输入/输出契约

Persona 可以声明它所需的输入和它产生的输出。父智能体读取这些声明来了解应提供哪些上下文以及应期待哪些工件。这样就可以串联 Persona：一个 Persona 的输出文件成为下一个 Persona 的输入：

```toml
[[subagents.personas.reviewer.inputs]]
name = "review_file"
io_type = "file"
required = true
description = "Path to the code under review"

[[subagents.personas.reviewer.outputs]]
name = "summary_file"
io_type = "file"
required = false
description = "Path to write review notes"
```

每个字段都有 `name`、`io_type`（默认值为 `file`）、`required` 标志和 `description`。

<a id="persona-resolution"></a>
### Persona 解析

应用 Persona 时，Grok Build 按以下顺序解析生效的模型和推理力度，优先级从高到低：

1. 生成时的显式覆盖
2. 角色默认值
3. Persona 默认值
4. 父会话

隔离遵循前三步的相同顺序，但默认值为 `none`（无工作树），而不是继承父会话。

如果请求了 Persona 但无法解析——找不到、没有指令，或其 `instructions_file` 无法读取——生成会失败。

---

<a id="spawning-subagents"></a>
## 生成子智能体

主智能体调用 `spawn_subagent` 工具。参数如下：

| 参数         | 说明                                                       |
| ---------------- | ------------------------------------------------------------ |
| `prompt`          | 给子智能体的完整任务提示。                           |
| `description`     | 任务的简短标签（3–5 个单词）。                          |
| `subagent_type`   | 要启动的智能体类型。默认为 `general-purpose`。         |
| `background`      | 在后台运行子智能体并立即返回子智能体 ID。默认为 `false`。 |
| `isolation`       | `none`（共享工作区，默认）或 `worktree`（隔离的 Git 工作树）。 |
| `resume_from`     | 继续已完成的子智能体对话。传入其子智能体 ID。 |
| `cwd`             | 子智能体的工作目录。与 `isolation: worktree` 互斥；设置 `resume_from` 时忽略（恢复的子级继承源目录）。 |

后台运行子智能体时，稍后使用 `get_command_or_subagent_output` 获取其结果。

### 向活动中的子智能体发送消息

`send_subagent_message` 工具目前仅根会话可用，而且只能向该会话拥有的活动子智能体发送消息。可选参数 `queue` 控制投递方式：

- 省略或设为 `false` 时使用 **Steer**。若子智能体空闲，该消息会成为一个受保护的排队轮次；若其正在运行，消息会在下一个安全点注入当前轮次。
- 设为 `true` 时使用 **Queue**，保留排队轮次行为：消息会作为受保护轮次等待，而不会进入活动轮次。

---

<a id="capability-modes"></a>
## 能力模式

能力模式不是创建子智能体时的参数。子智能体可用的工具由其**智能体类型**以及**角色 / 定义的默认值**决定。`general-purpose` 不受限制（`all`）；内置 `explore` 和 `plan` 类型可以读取、搜索并运行 shell 命令，但不能编辑文件。

| 模式         | 读取 | 写入 | 执行 | 说明                                  |
| ------------ | ---- | ---- | -------- | -------------------------------------------- |
| `read-only`  | 是   | 否   | 否       | 读取、搜索和检查（也包括网页搜索和 LSP）；不允许文件编辑或 shell。 |
| `read-write` | 是   | 是   | 否       | 读取，以及创建、编辑、删除和移动文件。不允许 shell。 |
| `execute`    | 是   | 否   | 是       | 读取，以及运行 shell 命令和后台任务。不允许文件编辑。 |
| `all`        | 是   | 是   | 是       | 不受限的工具访问；`general-purpose` 的默认值。 |

---

<a id="context-inheritance"></a>
## 上下文继承

<a id="resume-from"></a>
### `resume_from`

`resume_from` 参数允许新的子智能体从已完成的子智能体继续，这对多阶段工作流很有用：

1. 生成一个研究子智能体来调查问题。
2. 生成第二个子智能体，并将第一个子智能体的 ID 设置为 `resume_from`，使其获得完整的研究上下文。

新的子智能体继承源子智能体的记录、工具状态和模型；其系统提示和工具会根据当前智能体定义重新渲染。源子智能体必须已完成（不能仍在运行）、属于当前会话，并使用相同的智能体类型。

<a id="mcp-inheritance"></a>
### MCP 继承

默认情况下，子智能体继承父会话中**已经连接**的 MCP 服务器。这包括本地 stdio/HTTP 服务器和插件提供的智能体（例如 `my-plugin:reviewer`）。子级通过 `search_tool` / `use_tool` 以与父级相同的方式发现和调用这些工具。

通过智能体 frontmatter 的 `mcpInheritance` 控制继承：

| 值 | 效果 |
| ----- | ------ |
| `all`（省略时的默认值） | 继承父级已连接的每个 MCP 服务器 |
| `none` | 不继承父级 MCP 服务器 |
| `named: [server, …]` | 仅继承列出的服务器名称 |
| `except: [server, …]` | 继承父级除列出名称外的所有服务器 |

示例：

```yaml
---
name: research-only
description: Read MCP tools but not internal connectors
tools: search_tool, use_tool, Read
mcpInheritance:
  except:
    - internal-tools
---
```

**插件智能体**也会以相同方式继承父级 MCP。出于安全原因，它们仍不能：

- 在智能体 frontmatter 中声明自己的 `mcpServers`（会带警告忽略）
- 在智能体 frontmatter 中声明钩子
- 设置 `permissionMode: bypassPermissions`

插件捆绑的 MCP 服务器（插件 `.mcp.json`）在插件受信任后仍附加到**父级/会话**，而不是仅在子级 frontmatter 中声明。参见[插件](09-plugins.md)和[MCP 服务器](07-mcp-servers.md)。

---

<a id="isolation-worktree-mode"></a>
## 隔离：工作树模式

对于会修改文件的任务，请使用 `isolation: worktree` 在隔离的 Git 工作树中运行子智能体。这可避免子级编辑与父级冲突：

- 子智能体在自己的工作树副本中工作。
- 其变更在合并前都与父级隔离。
- 子智能体的结果包含工作树路径。

Grok Build 通过 `x.ai/git/worktree/*` 扩展方法管理工作树，其中包括将变更合并回主工作目录的 apply 操作。

---

<a id="configuration"></a>
## 配置

<a id="per-type-toggles-and-model-overrides"></a>
### 按类型切换和模型覆盖

禁用特定智能体类型，或将其路由到其他模型：

```toml
[subagents.toggle]
explore = true                       # 默认——省略以保持启用
plan = false                         # 禁用 plan 子智能体

[subagents.models]
explore = "grok-build"               # 将 explore 路由到特定模型
```

按类型的模型覆盖适用于任何父级。没有覆盖时，子智能体继承父级模型。

<a id="custom-roles-and-personas"></a>
### 自定义角色和 Persona

定义带有自身能力和模型默认值的自定义角色：

```toml
[subagents.roles.researcher]
description = "Deep research agent"
default_capability_mode = "read-only"
model = "grok-build"
prompt_file = ".grok/prompts/researcher.md"
```

定义带行为指令的自定义 Persona：

```toml
[subagents.personas.concise]
instructions = "Be concise. No filler words."
# instructions_file = ".grok/personas/concise.md"  # 或从文件加载
```

Grok Build 还会从 `.grok/roles/*.toml` 发现角色，从 `.grok/personas/*.toml` 发现 Persona。内联 `config.toml` 定义的优先级高于文件。

---

<a id="the-tasks-pane-tui"></a>
## 任务窗格（TUI）

Grok Build 会在智能体屏幕的侧窗格中显示运行中和已完成的工作：

- 按 `Ctrl+G` 切换任务窗格，其中列出活动和已完成的子智能体以及带状态的后台命令。
- 按 `Ctrl+T` 切换独立的待办窗格。

要查看可用的智能体类型和 Persona，请用 `Ctrl+P` 打开命令面板，然后选择 **Manage Agents**（`/config-agents`）。

子智能体会出现在任务窗格顶部自己可折叠的“Subagents”分组中。

---

<a id="viewing-subagents-in-the-tui"></a>
## 在 TUI 中查看子智能体

交互式 TUI 的多个位置都会显示子智能体：

<a id="scrollback-parent-conversation-history"></a>
### 回滚区（父级对话历史）

生成子智能体时，父级回滚区会添加一个紧凑的生命周期块：

- `Subagent running: "do the thing" (Implementer · grok-3) · Thinking`
- 对于后台子智能体：`Subagent started: "..."`

运行中时，该块显示实时活动后缀（例如“Running: cargo test”“Compacting”“Retrying (2/3)”），取自子级的轮次跟踪器。项目符号会根据状态动画显示（或着色）。

按 **Enter**（或 Ctrl-F）可打开子智能体的完整记录。

对于阻塞式子智能体，子级完成时同一条记录会更新项目符号颜色。对于后台子智能体，还会追加 `Subagent completed/failed/cancelled in Xs: "..."` 块。

<a id="tasks-pane-ctrl-g"></a>
### 任务窗格（Ctrl+G）

如上所述——任务窗格按“Subagents”分组显示子智能体，并提供旋转指示器、耗时以及快速终止或检查的入口。

<a id="fullscreen-framed-view-the-child-transcript"></a>
### 全屏框架视图（子级记录）

从回滚区块或任务窗格打开子智能体时，父级视图会被替换为一个带边框的框架，其中包含子级的完整记录：

- 框架内的标题栏：状态图标（旋转指示器 / ✓ / ✗）、标签 + 粗体描述 + 模型、可选的“resumed”/“forked”徽章、实时活动 · 已耗时，以及 [✗] 关闭按钮。
- 子智能体自己的回滚、思考、工具调用和（有限的）提示区域会在框架内渲染。
- 子智能体视图主要用于观察——通常不能像对父级会话那样，直接向子级发送新的顶层提示。

使用 `q`、`Esc` 或点击关闭按钮返回父级视图。父级回滚区会继续显示子智能体的状态。

---

<a id="depth-limits"></a>
## 深度限制

只有顶层会话可以生成子智能体。子智能体不能再生成自己的子智能体：最大嵌套深度为一层。如果子级调用 `spawn_subagent`，调用会失败并返回深度限制错误。这样可以保持智能体树扁平，避免无限生成。

---

<a id="when-to-use-subagents"></a>
## 何时使用子智能体

**适合的场景：**

- 父级继续其他工作时研究代码库
- 父级实现功能时并行运行测试
- 提交前审查生成的变更
- 委派彼此独立、互不依赖的任务

**不适合的场景：**

- 父级可以直接处理的简单任务
- 需要与用户紧密往返的任务，因为子智能体自主运行，不适合交互式交流
- 上下文准备成本超过并行收益的任务
