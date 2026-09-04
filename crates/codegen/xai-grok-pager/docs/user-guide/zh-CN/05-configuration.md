<a id="configuration"></a>
# 配置

Grok 会从配置文件、环境变量和 CLI 标志中读取设置。本页介绍常用选项。`config.toml`、`managed_config.toml` 和 `requirements.toml` 的完整字段列表见 [26-config-reference.md](26-config-reference.md)（启动时会提取到 `~/.grok/docs/user-guide/`）。

---

<a id="precedence"></a>
## 优先级

设置按优先级从高到低解析：

1. **CLI 标志**（例如 `--yolo`、`--model`、`--sandbox`）
2. **环境变量**（例如 `XAI_API_KEY`、`GROK_MEMORY`）
3. **config.toml**（`~/.grok/config.toml`）
4. **托管配置 / requirements 配置**（你的组织可能部署的文件，例如 `managed_config.toml` / `requirements.toml`）
5. **内置默认值**

---

<a id="configtoml-main-configuration"></a>
## config.toml（主配置）

位置：`~/.grok/config.toml`。如果文件不存在，Grok 会使用内置默认值，因此只需设置希望覆盖的值。

<a id="general-settings"></a>
### 常规设置

```toml
[cli]
auto_update = false                    # 默认：只提示；按 Ctrl+U 后才下载
channel = "stable"                    # stable（默认）| alpha（预发布）

[models]
default = "grok-4.5"                   # 新会话使用的模型
web_search = "grok-4.5"                # web_search 工具使用的模型
# 可选的模型选择器允许列表（匹配目录键或模型 ID 的 glob）；空列表表示不限制。
# 签名策略中的固定值会替换此列表（仅按模型 ID），本地配置无法放宽它。
# allowed_models = ["grok-4.5", "grok-4*"]

# 应用于每个模型的默认值；每模型的 [model.<id>] 值始终优先。
# 详见“自定义模型”中的每模型覆盖项和完整说明。
extra_headers = { "X-Request-Tags" = "team=example,env=prod" }
temperature = 0.7
top_p = 0.95
max_completion_tokens = 8192
max_retries = 8
inference_idle_timeout_secs = 600
subagent_rate_limit_max_attempts = 8
stream_tool_calls = true

[ui]
simple_mode = true                     # readline 风格的提示编辑（默认）；false = 在提示中使用 Vim 编辑
vim_mode = false                       # Vim 风格的回滚导航键（默认：false）
max_thoughts_width = 120               # 推理显示的最大列宽
default_selected_permission = "always_allow_all_sessions" # 首次审批提示中预先选中的行
remember_tool_approvals = true         # 在权限提示中显示每条命令的“始终允许”选项；
                                       # 按项目记住授权（默认：true）；见 22-permissions-and-safety.md
show_thinking_blocks = true            # 在 TUI 中显示智能体思考块（默认：true）
group_tool_verbs = true                # 将连续的 read/search/list 工具调用和子智能体行
                                       # ——以及其中已完成的思考——折叠成一行（默认：true）
collapsed_edit_blocks = false          # 将编辑显示为单行 +N/-M diffstat 摘要，并把
                                       # 同一文件连续编辑合并为一行；展开可查看
                                       # 差异（默认：false；pager.toml [scrollback.blocks.edit]
                                       # expanded_by_default/line_summary 可覆盖其折叠形状）
page_flip_on_send = true               # 将刚发送的提示固定在视口顶部，使
                                       # 回复从新页面开始（默认：true）；设为 false
                                       # 后发送不会移动滚动位置
follow_up_behavior = "queue"           # 轮次中途跟进："queue"（等待轮次结束，默认）或
                                       # "steer"（普通 Enter 仍先显示在队列，然后在下一个
                                       # 工具／模型安全间隙插话）。参见“键盘快捷键 → 活动轮次期间”。
screen_mode = "fullscreen"             # 默认渲染模式："fullscreen" | "minimal"
                                       #（未设置 → fullscreen）；可通过 /settings → 默认屏幕模式设置

[features]
telemetry = false                      # 匿名使用遥测
feedback = true                        # 反馈系统（默认：true）
lsp_tools = false                      # 暴露 lsp 工具
codebase_indexing = true               # 代码图索引（默认：true）
two_pass_compaction = false            # 预先执行两阶段压缩（默认：false，选择启用）
remote_fetch = true                    # 允许可选的在线模型目录获取（默认：true；
                                       # 防火墙/气隙部署可设为 false；后台
                                       # 托管配置同步有独立开关：managed_config）

[session]
auto_compact_threshold_percent = 85    # 在上下文窗口达到此百分比时自动压缩（默认：85）
load_envrc = true                      # 加载 .envrc 环境变量

[tools]
respect_gitignore = false              # 默认：false；设为 true 后每个工具都会跳过被 gitignore 的文件
```

<a id="input-mode"></a>
#### 输入模式

`[ui] simple_mode` 控制你如何在 **提示**（输入编辑器）中编辑文本。它与如何在回滚区中移动无关；后者由 [`vim_mode`](#vim-mode) 控制。

| 值 | 行为 |
|-------|----------|
| `true`（默认） | **Readline 编辑。** 普通的 readline 风格文本输入。 |
| `false` | **Vim 编辑（实验性）。** Vim 风格的模态编辑（普通模式和插入模式）。提示为空时从普通模式开始，焦点位于回滚区。 |

将提示切换为 Vim 风格编辑：

```toml
[ui]
simple_mode = false
```

也可以在设置窗格中切换（`/settings` → **禁用 Vim 输入模式**）；Grok 会将选择写入 `[ui] simple_mode`。`simple_mode` 与 `vim_mode` 相互独立——前者控制提示编辑器，后者控制回滚导航。完整按键绑定请参见[键盘快捷键](03-keyboard-shortcuts.md)。

<a id="default-selected-permission"></a>
#### 默认选中的权限

当智能体请求运行命令（或执行其他工具操作）时，审批菜单默认会突出显示一行。`[ui] default_selected_permission` 设置会话中**首次**提示时预先选中的行。

| 值 | 预选行 |
|-------|-----------------|
| `always_allow_all_sessions`（默认） | “在所有会话中始终允许”行。 |
| `allow_command_always` | “始终允许此命令”行。 |
| `allow_once` | “是”/仅允许一次行。 |
| `reject` | 拒绝行。 |

```toml
[ui]
default_selected_permission = "allow_once"
```

回答首次提示后，光标会变为**粘滞**状态：后续每次提示都会预选你上次确认的选项（例如只选一次“否”后，后续提示会从拒绝行开始），并跨越编辑 / bash / MCP 提示持续到重启。因此此设置只决定起始位置。

值不区分大小写；未设置或无法识别的值会回退到 `always_allow_all_sessions`。`allow_command_always` 行始终只针对正在审批的具体操作（命令 / 工具 / 域 / 编辑会话），绝不是全局允许一切——全局允许由 `always_allow_all_sessions` 提供。启用 `[ui] remember_tool_approvals` 时会显示每条命令的“始终允许”行；该设置默认启用，设为 `false` 可隐藏这些选项。参见 [22-permissions-and-safety.md](22-permissions-and-safety.md)。

也可以用 `GROK_DEFAULT_SELECTED_PERMISSION` 覆盖此设置，适用于不应修改 `config.toml` 的无头或智能体测试运行。优先级：环境变量 → `config.toml` → `always_allow_all_sessions`。

<a id="vim-mode"></a>
#### Vim 模式

`[ui] vim_mode` 控制 Vim 风格绑定是否在**回滚区**中启用，不影响提示。

| 值 | 行为 |
|-------|----------|
| `false`（默认） | 回滚区会抑制裸字母和 `Shift+letter` 按键（`j`/`k`、`h`/`l`、`g`/`G`、`y`/`Y`、`o`/`O`、`r`、`x`、`e`/`E`、`H`/`L`，以及 `i`）：按下其中任意键会聚焦提示并输入该字符。方向键、`Tab`、`Space`、`PageUp`/`PageDown` 以及所有 `Ctrl+letter` 快捷键仍可导航。`Esc` **不是**回滚键——它会取消正在运行的回合；空闲时遵循清除 / 回退策略（见[键盘快捷键](03-keyboard-shortcuts.md#escape)）。 |
| `true` | 所有 Vim 风格回滚绑定均启用，完全按照[键盘快捷键](03-keyboard-shortcuts.md)所列。回合进行中，`Esc` 在此模式下会被吞掉（用 `Ctrl+C` 取消）；精简模式无论如何都保留 Esc 取消行为。 |

可在运行时使用 `/vim-mode` 切换，或从 `/settings` → **Vim 回滚导航**切换。Grok 会立即将更改写入 `[ui] vim_mode`，并应用于该进程中所有未来的 pager 会话，包括新智能体和子智能体。不存在按会话覆盖——下次启动时 `config.toml` 才是事实来源。`vim_mode` 与 `simple_mode` 相互独立。

<a id="screen-mode"></a>
#### 屏幕模式

`[ui] screen_mode` 是直接运行 `grok-zh` 时的**默认渲染模式**。可在 `/settings` → **默认屏幕模式**中设置（需重启），或手动编辑 `config.toml`——两种方式都会写入该文件。CLI 标志（`--minimal` / `--fullscreen`）和斜杠命令（`/minimal` / `/fullscreen`）仅作用于当前会话，**不会**写入此键；使用斜杠命令切换后，反向命令只会在该会话中将你切回。

| 值 | 行为 |
|-------|----------|
| 未设置 | 设置中显示**全屏**。启动时没有粘滞偏好：旧版 `pager.toml` 的 `[terminal] minimal` 仍可强制精简模式；泄漏鼠标报告的终端（JediTerm/Windows）可能会自动打开精简模式，直到显式设置值。除此之外，备用屏幕策略会选择全屏或内联。 |
| `"fullscreen"` | 粘滞的非精简模式。全屏与内联仍遵循备用屏幕策略（`--no-alt-screen`、`[terminal] alt_screen`、终端自动检测）。 |
| `"minimal"` | 粘滞的精简（原生回滚区）模式。 |

CLI 标志在该次调用中始终优先于配置值。

<a id="snap-prompt-to-top-on-send"></a>
#### 发送时将提示吸附到顶部

默认情况下，发送提示会将其滚动到视口顶部，使回复从新页面开始。设置 `[ui] page_flip_on_send = false`（或在 `/settings` → 外观中切换**发送时将提示吸附到顶部**）可在发送时保持原滚动位置。该设置在下一次发送时生效，无需重启。

<a id="scrolling"></a>
#### 滚动

四个 `[ui]` 设置用于调整鼠标滚轮和触控板滚动。全部立即生效，并可在设置窗格（`/settings` → **滚动速度** / **滚动输入** / **滚动行数** / **反转滚动**）中编辑。

| 键 | 值（默认） | 行为 |
|-----|------------------|----------|
| `scroll_speed` | `1`–`100`（`50`） | 滚轮和触控板的速度倍数。`50` = 1.0x，`1` = 0.1x，`100` = 6.0x。 |
| `scroll_mode` | `auto` \| `wheel` \| `trackpad`（`auto`） | 滚轮与触控板检测采用启发式方法（终端滚动事件不携带幅度）；自动检测误判设备时可强制指定，例如滚轮一格跳得过远，或触控板滚动感觉有阶梯。 |
| `scroll_lines` | `1`–`10`（未设置） | 每个滚动刻度的行数，同时应用于**滚轮和触控板**。未设置时使用各终端自身配置（例如 tmux 下每个事件保守地滚动 1 行）。提交任意值——即使是设置窗格显示的数字 `3`——都会永久切换到显式覆盖。 |
| `invert_scroll` | `false` \| `true`（`false`） | 反转垂直滚动方向（“自然”滚动）。 |

```toml
[ui]
scroll_speed = 50
scroll_mode = "auto"     # auto | wheel | trackpad
invert_scroll = false
# 默认未设置 scroll_lines：由每个终端配置负责。
# scroll_lines = 3
```

每个设置也有环境变量覆盖，但只在首次加载时应用（同样适合无头 / 测试运行）：`GROK_SCROLL_SPEED`、`GROK_SCROLL_MODE`、`GROK_INVERT_SCROLL`（`1`/`true`/`0`/`false`）和 `GROK_SCROLL_LINES`。优先级：环境变量 → `config.toml` → 默认值。无法识别的值会回退到默认值，超出范围的数字会被限制在范围内。

<a id="tool-configuration"></a>
### 工具配置

```toml
[toolset.bash]
timeout_secs = 120.0                   # 前台命令超时秒数（默认：120）
output_byte_limit = 20000              # 捕获输出的最大字节数（默认：20000）

[toolset.ask_user_question]
timeout_enabled = true                 # false = 永久等待回答（默认：true）
timeout_secs = 1800                    # 启用超时时的等待秒数（默认：1800 / 30 分钟）

[toolset.web_fetch]
proxy_endpoint = "https://proxy.example.com"   # 出站代理 URL
allowed_domains = ["docs.rs", "x.ai"]          # 覆盖内置允许列表
allow_local = false                            # true = 仅允许 localhost / 127.0.0.0/8 / ::1
```

`allow_local` 默认关闭（SSRF 失败关闭）。启用它（或设置 `GROK_WEB_FETCH_ALLOW_LOCAL=1`）后，`web_fetch` 只能访问**明确指定**的环回主机——私有、链路本地和云元数据网段仍会被阻止。解析优先级：TOML > 环境变量 > 默认关闭。

`[toolset.ask_user_question]` 会在 **requirements.toml**、**托管配置**和用户的 **`config.toml`** 中生效。优先级：requirements → 环境变量（`GROK_ASK_USER_QUESTION_TIMEOUT_ENABLED` / `GROK_ASK_USER_QUESTION_TIMEOUT_SECS`）→ 用户配置 → 托管配置 → 默认值。在用户配置中设置 `timeout_enabled = false` 可为自己禁用自动问卷超时；`timeout_secs` 必须是正整数。也可在 `/settings` → **询问问题超时**（位于智能体与审批下）切换 `timeout_enabled`；更改会应用于新启动的会话。

<a id="authentication"></a>
### 身份验证

完整说明请参见[身份验证](02-authentication.md)。

```toml
[auth]
auth_provider_command = "/usr/local/bin/my-auth-provider"
auth_provider_label = "Acme Corp"
auth_token_ttl = 3600

[grok_com_config.oidc]
issuer = "https://acme.okta.com"
client_id = "0oa1b2c3d4e5f6g7h8i9"
# scopes = ["openid", "profile", "email", "offline_access", "api:access"]
# audience = "https://api.acme.com"
```

<a id="custom-models"></a>
### 自定义模型

添加自定义模型端点，以使用其他提供方或自行托管的模型。

```toml
[model.my-model]
model = "model-id"                    # 发送到 API 的模型标识符
base_url = "https://api.example.com/v1"  # OpenAI 兼容端点
name = "Display Name"                 # 模型选择器中显示的名称
description = "Model description"      # 可选说明
api_key = "sk-..."                    # 此提供方的 API 密钥
env_key = "XAI_API_KEY"               # 保存 API 密钥的环境变量；字符串或数组（取第一个已设置且非空的值）
temperature = 0.7                     # 采样温度（0.0-2.0）
top_p = 0.95                          # nucleus 采样参数
max_completion_tokens = 8192          # 每次响应的最大 token 数
context_window = 128000               # 上下文窗口大小（用于自动压缩）
query_params = { api-version = "2026-07-22" } # 追加到每个请求 URL 的查询参数
env_http_headers = { "X-Tenant" = "TENANT_TOKEN" }    # 从环境变量读取的请求标头，在客户端构建时解析
```

凭据解析顺序：`api_key` > `env_key` > 已登录的会话令牌 > `XAI_API_KEY`。有关 `query_params` 和 `env_http_headers`，参见[自定义模型](11-custom-models.md#request-query-parameters)；有关 `[shell_environment_policy]`（限制工具子进程继承的环境变量），参见[沙箱模式](18-sandbox.md#shell-environment-policy)。

要覆盖内置模型，请使用其名称作为节键，只设置需要的字段：

```toml
[model.grok-build]
api_key = "my-api-key"
```

<a id="mcp-servers"></a>
### MCP 服务器

通过模型上下文协议配置外部工具集成。

```toml
[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_PERSONAL_ACCESS_TOKEN = "ghp_xxx" }
enabled = true                        # 启用/禁用（默认：true）
startup_timeout_sec = 30              # 初始化超时秒数（默认：30）
tool_timeout_sec = 6000              # 工具调用超时秒数（默认：6000）
tool_timeouts = { create_issue = 120 }  # 按工具覆盖超时

[mcp_servers.postgres]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-postgres", "postgresql://user:pass@localhost/db"]

[mcp_servers.my-streamable-server]
url = "https://mcp.example.com/api/mcp"  # HTTP/SSE 传输
headers = { "x-mcp-session-id" = "{{session_id}}" }
```

远程（HTTP/SSE）服务器默认会收到 `User-Agent: grok-cli/<version>` 标头；`headers` 中有效的 `User-Agent` 会覆盖它（Figma 服务器使用不带版本的 `grok-cli`）。详情见 [MCP 服务器](07-mcp-servers.md)。

MCP 服务器也可以在 `.grok/config.toml` 中按项目设置。项目级配置会贡献 `[mcp_servers]`、`[plugins]` 和 `[permission]` 规则；其他所有节仅从 `~/.grok/config.toml` 加载。

`[mcp_servers]` 和 `[plugins]` 的优先级为：`.grok/config.toml`（当前目录）> `<repo-root>/.grok/config.toml` > `~/.grok/config.toml`。`[permission]` 规则不按优先级覆盖，而是在所有文件之间合并，顺序为 `deny` > `ask` > `allow`（见[22-permissions-and-safety.md](22-permissions-and-safety.md)）。

<a id="memory"></a>
### 记忆

跨会话持久化知识（需要 `--experimental-memory` 或 `GROK_MEMORY=1`）。

```toml
[memory]
enabled = false                       # 启用记忆

[memory.session]
save_on_end = true                    # 会话结束时写入元数据摘要

[memory.watcher]
enabled = true                        # 监视记忆文件的外部编辑

[memory.search]
max_results = 6                       # 结果默认数量
min_score = 0.35                      # 最低相关性分数

[memory.initial_injection]
enabled = true                        # 首回合自动注入记忆
min_score = 0.0                       # 首回合注入的分数阈值

[memory.embedding]
model = "embedding-model"             # 嵌入模型名称
dimensions = 1024                     # 向量维度
```

<a id="subagents"></a>
### 子智能体

```toml
[subagents]
enabled = true

[subagents.toggle]
explore = true                        # 启用/禁用特定类型
plan = false

[subagents.models]
explore = "grok-build"               # 路由到不同模型
```

要固定子智能体使用的模型，请在 `[subagents.models]` 下设置对应条目。

<a id="goal-mode-and-background-workflows"></a>
### 目标模式与后台工作流

`/goal` 有两个驱动程序，由后台工作流设置选择。启用工作流时，由主机拥有的工作流引擎评估各轮并驱动完成验证；禁用时，`/goal` 回退到面向模型的旧版 `update_goal` 工具。`/goal` 是否可用是另一个独立开关（目标功能设置）。

后台工作流——`workflow` 工具、命名的 `.grok/workflows/*.rhai` 脚本、`/deep-research` 以及 `/workflow` 启动——**默认开启**。可通过配置、环境变量或远程设置禁用。

```toml
[workflows]
enabled = false                       # 禁用后台工作流（或使用 GROK_WORKFLOWS=0）
```

项目工作流从 `<repo-root>/.grok/workflows/` 发现；用户工作流从 `~/.grok/workflows/` 发现。发现和调用依据脚本的 `meta.name`，因此每个文件名应与其 `meta.name` 保持一致。内置名称优先于项目名称，项目名称优先于用户名称，因此应在各作用域中保持名称唯一。

每次启动都会获得会话唯一的显示句柄，例如 `deep-research-2`。你可以在 `/workflow runs` 运行面板中看到该句柄，并将其传给 `/workflow pause`、`resume` 或 `stop`——内部运行 ID 不会出现在命令中。带编号的句柄不是可复用的定义名称，因此在选择新的唯一 `meta.name` 并自行保存编辑后的脚本之前，面板会禁用**保存**。示例请参见[斜杠命令](04-slash-commands.md)。

<a id="skills"></a>
### 技能

```toml
[skills]
paths = ["~/my-team-skills"]          # 要扫描的附加目录
ignore = ["~/my-team-skills/wip"]     # 要排除的路径
disabled = ["wip-skill"]              # 保持列出但不激活的技能名称
```

<a id="harness-compatibility"></a>
### Harness 兼容性

控制 Cursor、Claude 和 Codex 的供应商兼容性。每个单元格默认值均为 `true`。会话单元格会保持已暂存且不活动，直到外部会话扫描器使用它们；每个工具还需要同时启用其 `sessions` 单元格和匹配的 `resume-claude`、`resume-codex` 或 `resume-cursor` 技能——缺少技能意味着不会进行任何外部会话文件系统 I/O。

```toml
[compat.cursor]
skills = true     # 扫描 ~/.cursor/skills/ 和 <cwd>/.cursor/skills/
rules = true      # 扫描 ~/.cursor/rules/ 和 <dir>/.cursor/rules/
agents = true     # 扫描 ~/.cursor/ 下的命名指令文件
mcps = true       # 扫描 ~/.cursor/mcp.json 和 <cwd>/.cursor/mcp.json
hooks = true      # 扫描 ~/.cursor/hooks.json 和 <cwd>/.cursor/hooks.json
sessions = true   # 已暂存；目前没有扫描器使用者

[compat.claude]
skills = true     # 扫描 ~/.claude/skills/ 和 <cwd>/.claude/skills/
rules = true      # 扫描 ~/.claude/rules/ 和 <dir>/.claude/rules/
agents = true     # 扫描 ~/.claude/ 和 <dir>/.claude/CLAUDE*.md
mcps = true       # 扫描 ~/.claude.json 中的 MCP 服务器
hooks = true      # 扫描 ~/.claude/settings.json 中的钩子
sessions = true   # 已暂存；目前没有扫描器使用者

[compat.codex]
sessions = true   # 已暂存；目前没有扫描器使用者
```

Codex 的 `skills`、`rules`、`agents`、`mcps` 和 `hooks` 单元格是预留项，目前不活动——不会启用 `.codex` 发现。

对于 Claude 和 Cursor，`rules` 与 `agents` 相互独立：关闭命名指令文件不会禁用主目录或项目规则目录，关闭规则也不会禁用命名文件。Claude 的 `agents` 单元格控制主目录级 `~/.claude/` 命名文件以及项目 `<dir>/.claude/CLAUDE*.md`；顶层通用名称 `Claude.md`、`CLAUDE.md` 和 `CLAUDE.local.md` 仍会被识别。项目规则路径会从仓库根目录到当前目录的每一级目录扫描。

每个单元格都可以通过环境变量或 `config.toml` 设置；名称参见环境变量参考。解析优先级：环境变量 > config.toml > 默认值（开启）。

`grok-zh inspect` 会将仍需在会话启动时解析的单元格报告为 `?`，直到获得值；具有显式环境变量或 TOML 值的单元格则使用该值。受影响的发现条目在 JSON 中报告 `compatibilityStatus: "unresolved"`，在人类可读输出中报告 `[compat unresolved]`。

<a id="plugins"></a>
### 插件

```toml
[plugins]
paths = ["~/my-plugins/custom-tools"]
disabled = ["user/a1b2c3d4/noisy-plugin"]
```

<a id="hints"></a>
### 提示

`[hints]` 保存少量持久化的 UI 偏好：记住的答案和模态布局。Grok 会在你使用 TUI 时写入这些内容，但也可以手动编辑或删除；移除某个键会恢复默认值。

`[hints]` 从**有效配置合并结果**中读取，遵循通常的优先级：系统托管配置 → 用户 `managed_config.toml` → 用户 `config.toml` → 用户 `requirements.toml` → 系统 `requirements.toml`，高层级优先。TUI 只会将这些设置**写入**用户的 `~/.grok/config.toml`。

```toml
[hints]
memory_modal_fullscreen = false        # 记住记忆模态框的全屏状态
new_session_worktree_mode = "never"    # /new 工作树提示："ask" | "always" | "never"
fork_worktree_mode = "ask"             # /fork 工作树提示："ask" | "always" | "never"
```

| 键 | 类型 | 默认值 | 说明 |
|-----|------|---------|-------------|
| `memory_modal_fullscreen` | bool | `false` | 记住上次打开记忆模态框时是否为全屏。 |
| `new_session_worktree_mode` | string | `"never"` | `/new` 的工作树提示：`ask` 显示弹窗，`always` 创建工作树，`never` 跳过。 |
| `fork_worktree_mode` | string | `"ask"` | `/fork` 的工作树提示：`ask`、`always` 或 `never`。 |

<a id="notifications"></a>
### 通知

在智能体完成一回合或需要审批时发送终端通知。通知使用终端原生协议（OSC 9、OSC 99、OSC 777 或 BEL），默认受焦点控制，因此只有在你没有查看终端时才会触发。

```toml
[ui.notifications]
method = "auto"           # auto|osc9|osc99|osc777|bel|none
condition = "unfocused"   # unfocused|always|never
idle_threshold_secs = 3   # 失去焦点后等待多少秒才发送通知
events = ["turn_complete", "approval_required"]
sleep_prevention = true   # 在智能体回合期间防止显示器休眠
progress_bar = true       # 显示标签页进度条（OSC 9;4）

[ui.notifications.title]
enabled = true
items = ["action-required", "spinner", "activity", "session-name", "grok"]
```

| 选项 | 类型 | 默认值 | 说明 |
|--------|------|---------|-------------|
| `method` | string | `"auto"` | 通知协议。`auto` 会为终端选择最佳协议。 |
| `condition` | string | `"unfocused"` | 通知时机：`unfocused`（仅终端失去焦点时）、`always` 或 `never`。 |
| `idle_threshold_secs` | integer | `3` | 终端失去焦点后发送通知前的最少秒数。 |
| `events` | array | `["turn_complete", "approval_required"]` | 触发通知的事件。选项：`turn_complete`、`approval_required`、`session_ready`、`task_complete`、`agent_error`。 |
| `sleep_prevention` | bool | `true` | 智能体工作时保持显示器唤醒（macOS/Linux）。 |
| `progress_bar` | bool | `true` | 在终端标签页中显示进度指示器（OSC 9;4）。 |
| `title.enabled` | bool | `true` | 设置终端标题以反映智能体状态。 |
| `title.items` | array | （见上文） | 标题栏中显示的项目。选项：`action-required`、`spinner`、`activity`、`session-name`、`cwd`、`model`、`turn-timer`、`grok`。 |

<a id="terminal-support-matrix"></a>
#### 终端支持矩阵

| 终端 | 自动协议 | 焦点跟踪 | 进度条 |
|----------|---------------|----------------|--------------|
| iTerm2 | OSC 9 | Yes | Yes |
| Kitty | OSC 99 | Yes | No |
| Ghostty | OSC 777 | Yes | Yes |
| WezTerm | OSC 9 | Yes | Yes |
| Warp | OSC 9 | Yes | No |
| Alacritty | BEL | Yes | No |
| VS Code | BEL | Yes | No |
| Apple Terminal | BEL | No | No |
| VTE (GNOME Terminal) | OSC 777 | Yes | No |
| Grok Desktop | None (native) | N/A | N/A |
| Unknown | BEL | No | No |

使用 `method = "auto"` 时，Grok 会检测终端品牌并选择最佳协议。显式设置 `method` 可覆盖该行为。

<a id="notification-hooks"></a>
#### 通知钩子

在事件触发时运行自定义命令。钩子会在环境中接收 `$GROK_EVENT`、`$GROK_MESSAGE` 和 `$GROK_SESSION_ID`。

```toml
# macOS 原生通知
[[ui.notifications.hooks]]
command = "terminal-notifier -title 'Grok' -message '$GROK_MESSAGE'"
events = ["turn_complete", "approval_required"]
only_unfocused = true
timeout_secs = 10

# 推送到 ntfy 服务器
[[ui.notifications.hooks]]
command = "curl -s -d '$GROK_MESSAGE' ntfy.sh/my-grok-alerts"
events = ["turn_complete"]
only_unfocused = true
timeout_secs = 10

# 播放声音
[[ui.notifications.hooks]]
command = "afplay /System/Library/Sounds/Glass.aiff"
events = ["turn_complete"]
only_unfocused = true
timeout_secs = 5
```

| 钩子选项 | 类型 | 默认值 | 说明 |
|-------------|------|---------|-------------|
| `command` | string | （必需） | 要运行的 Shell 命令。 |
| `events` | array | `[]` | 触发此钩子的事件（为空 = 所有事件）。 |
| `only_unfocused` | bool | `true` | 仅在终端失去焦点时触发。 |
| `timeout_secs` | integer | `10` | 在指定秒数后终止钩子进程。 |

<a id="troubleshooting"></a>
#### 故障排除

在受影响的会话中运行 `/doctor`。它会显示检测到的通知和焦点问题、相关配置文件以及解决步骤。显式设置 `method = "bel"` 会被视为有意选择。`method = "none"` 会关闭通知和焦点检查结果。

**防止休眠未生效：**在 macOS 上，防休眠通过 CoreFoundation 使用 `IOPMAssertionCreateWithName`；在 Linux 上使用 `systemd-inhibit`（必须位于 `$PATH`）。请确认相关工具可用。防休眠仅在智能体回合期间启用，回合结束时会自动释放。

<a id="status-line"></a>
### 状态栏

全屏分页器底部可以显示一行可选状态栏；默认关闭。通过 `[ui.status_line]` 启用：

```toml
[ui.status_line]
type = "builtin"                # builtin | command | disabled
items = ["cwd", "model", "context"]
```

其他键包括 `items`（按顺序显示的内置分段）、`command`、`padding` 和 `refresh_interval`（秒；定时重新运行 `command` 状态栏，让告警页面或 CI 状态也能在会话空闲时更新）。[状态栏指南](25-status-line.md)列出了全部选项、命令脚本从标准输入读取的 JSON 约定以及示例脚本。

最小模式没有状态栏；它会改用终端标签页标题（参见[通知](#notifications)中的 `title.items`）。

<a id="keyboard-shortcuts"></a>
### 键盘快捷键

键盘快捷键**不可配置**——所有绑定均为内置。完整参考请参见[键盘快捷键](03-keyboard-shortcuts.md)。

<a id="telemetry"></a>
### 遥测

这些是相互独立的开关（见[监控使用情况](24-monitoring-usage.md#related-settings)）：

- **`[features] telemetry`** / `GROK_TELEMETRY_ENABLED` —— 产品分析总开关。`/privacy` 不会改变它。
- **编码数据、保留期限和训练** —— 由设置行 `/privacy` 打开；编码数据共享与遥测彼此独立。
- **`[telemetry] trace_upload`** / `GROK_TELEMETRY_TRACE_UPLOAD` —— 会话跟踪；未设置时遵循遥测开关。
- **`[telemetry] otel_*`** / `GROK_EXTERNAL_OTEL` —— 发送到你自己的收集器的外部 OTEL（见下文）。

启用遥测后，运行自有收集器的企业可以在 `[telemetry]` 下重定向遥测或关闭其中部分功能：

```toml
[telemetry]
events_url = "https://telemetry.your-company.com/events"  # 将事件发送到你自己的收集器
events_api_key = "your-collector-token"                   # 收集器所需的身份验证
mixpanel_enabled = false                                  # 禁用 Mixpanel 产品分析
trace_upload = false                                      # 禁用会话/跟踪上传（未设置时继承遥测开关）
```

这些设置仅用于将遥测指向你自己的基础设施或关闭其中部分功能。内置端点和凭据由 Grok 管理——留空即可使用默认值。

同一个 `[telemetry]` 表还配置**外部 OpenTelemetry 流**：这是独立的选择启用项（不要求打开上面的遥测开关），会将经过筛选且不含内容的使用情况架构发送到你自己的 *OTLP* 收集器。收集器身份验证来自 `OTEL_EXPORTER_OTLP_HEADERS`，绝不会写入磁盘。完整架构、环境变量和隐私模型请参见[监控与使用情况](24-monitoring-usage.md)。

```toml
[telemetry]
otel_enabled = true                                       # 外部 OTEL 总开关（= GROK_EXTERNAL_OTEL）
otel_metrics_exporter = "otlp"                            # otlp | console | none
otel_logs_exporter = "otlp"                               # otlp | console | none
otel_endpoint = "https://collector.corp.example:4318"     # OTLP 基础端点
otel_protocol = "http/protobuf"                           # http/protobuf | grpc
otel_certificate = "/etc/ssl/corp-ca.pem"                 # 可选：信任私有 CA（只接受路径）
otel_client_certificate = "/etc/ssl/client.crt"           # 可选：mTLS 客户端证书（只接受路径）
otel_client_key = "/etc/ssl/client.key"                   # 可选：mTLS 客户端私钥（只接受路径）
otel_log_user_prompts = false                             # 内容开关（管理员通过 requirements 固定）
otel_log_assistant_responses = false                      # 未设置时跟随 prompts；设为 false 可仅记录提示词
otel_log_tool_details = true                              # 元数据/预览；企业默认开启，便于关联 SIEM
otel_log_tool_content = false                             # 完整正文开关；与 details 独立，不代表名称/路径
```

签名 `requirements.toml` 中列出的 `[telemetry] otel_*` 键会固定其值，并覆盖进程环境变量（锁定目标）；`managed_config.toml` 不会这样做。这里没有 `headers` 键——收集器令牌仍应放在 `OTEL_EXPORTER_OTLP_HEADERS` 中。详见[监控与使用情况](24-monitoring-usage.md)。

<a id="version-pinning"></a>
### 固定版本

控制 CLI 可以自动更新到哪些版本以及允许运行哪些版本。在 `[cli]` 中设置这些值，或在托管层中设置以实施全局策略。每个设置都有只能收紧限制的环境变量覆盖，适用于 CI 和测试。

> **已更改：**`minimum_version` 不再阻止启动，而是作为更新器的软防降级下限。若要设置阻止旧版本启动的硬下限，请使用 `required_minimum_version`。

```toml
[cli]
minimum_version = "0.2.109"          # 更新器不会降级到低于此版本
maximum_version = "0.2.180"          # 更新器不会安装高于此版本
required_minimum_version = "0.2.100" # 低于此版本时拒绝启动
required_maximum_version = "0.2.200" # 高于此版本时拒绝启动
```

- `minimum_version`（`GROK_MINIMUM_VERSION`）是软性防降级下限。更新器会跳过低于它的目标并保留当前版本；它从不阻止启动。
- `maximum_version`（`GROK_MAXIMUM_VERSION`）是软性上限。更新器会将目标限制在此版本，绝不安装更高版本。
- `required_minimum_version`（`GROK_REQUIRED_MINIMUM_VERSION`）和 `required_maximum_version`（`GROK_REQUIRED_MAXIMUM_VERSION`）是硬性边界。如果正在运行的版本超出范围，CLI 会在启动时退出，并指示用户安装获批准的版本。`grok-zh update` 和 `grok-zh --version` 仍可运行，使超出范围的安装能够恢复。
- 各配置层的边界只会收紧：下限取最高值，上限取最低值，因此无法放宽托管边界，用户或环境边界也无法取消托管硬边界。无效值会被忽略，避免错误策略阻止启动。
- 显式执行 `grok-zh update --version X` 时允许高于上限，以便从过新安装恢复；低于硬下限则会被拒绝。

<a id="enterprise-deployment"></a>
### 企业部署

企业使用的完整配置示例：

```toml
[cli]
auto_update = false

[auth]
auth_provider_command = "/usr/local/bin/my-company-auth-provider"
auth_provider_label = "Acme Corp"
auth_token_ttl = 3600

[models]
default = "company-grok"

[model.company-grok]
model = "grok-build"
base_url = "https://grok-proxy.acme.com/"
name = "Grok Build Latest (Proxy)"
context_window = 128000

[features]
telemetry = false
```

---

<a id="pagertoml-appearance-configuration"></a>
## pager.toml（外观配置）

位置：`~/.grok/pager.toml`。它控制 TUI 的外观和感觉。更改会在重启后生效。

<a id="terminal"></a>
### 终端

```toml
[terminal]
alt_screen = "auto"                   # 全屏模式："auto"、"always"、"never"
```

- `auto`（默认）：终端支持时使用备用屏幕。
- `always`：始终使用备用屏幕。
- `never`：在终端主回滚缓冲区中以内联方式运行。

<a id="animation"></a>
### 动画

```toml
[animation]
fps = 30                              # 动画帧率（每秒 tick 数）
wave_rows = 32                        # 强调色动画每个波形周期的行数
```

<a id="prompt"></a>
### 提示

```toml
[prompt]
collapse_unfocused = true             # 回滚区获得焦点时折叠提示
mouse_hover = true                    # 在提示组件上显示悬停高亮
show_prefix = true                    # 显示提示前缀字符
```

紧凑模式不会在此处持久化——请通过 `[ui] compact_mode` 或 `/compact-mode` 命令在运行时控制。

<a id="scrollback"></a>
### 回滚区

```toml
[scrollback.layout]
outer_vpad = 1                        # 垂直内边距
outer_hpad_left = 2                   # 左侧水平内边距
outer_hpad_right = 2                  # 右侧水平内边距
block_pad_left = 2                    # 块内、内容左侧的内边距
block_pad_right = 2                   # 块内、内容右侧的内边距

[scrollback.scrollbar]
enabled = true                        # 显示滚动条
gap_left = 0                          # 内容与滚动条之间的间隔
gap_right = 0                         # 滚动条与屏幕边缘之间的间隔

[scrollback.scroll]
margin = 0                            # 选中项上方/下方的最小上下文行数
min_page_fraction = 0                 # 视口最小滚动百分比（0-100）
follow_indicator = "center"           # ▼/▲ 滚动指示器："center" 或 "none"
follow_auto_select = true             # 跟随模式下自动选择最新条目
follow_by_overscroll = true           # 滚过底部时进入跟随模式
anchor_on_fold = true                 # 折叠时保持块位置
respect_manual_folds = true           # 选择启用（默认：false）：流式传输/完成期间保留手动折叠的块；跟随时展开会停止自动滚动

[scrollback.display]
sticky_headers = true                 # 将用户提示固定为粘滞标题
tab_width = 4                         # 每个制表符的空格数
expandable_indicator = true           # 在可折叠条目上显示展开指示器
expandable_indicator_running = true   # 在运行中的条目上显示指示器
expandable_indicator_char = "›"       # 展开指示器字符（默认：“›”）
selection_buttons = false             # 在选中项上显示复制/查看按钮
line_under_last_entry = false         # 在最后一个条目下显示水平线
group_selection_split = true          # 为展开的块拆分选框
highlight_overlays_border = false     # 高亮延伸到选框边框之上
dim_accent = 0.5                      # 折叠强调色的变暗因子（0.0-1.0）
```

`respect_manual_folds` 默认关闭。开启后，手动折叠的块会被固定：流式更新和完成事件（例如思考块结束）不会改变其折叠状态；跟随模式追踪新内容时展开块会停止自动滚动，让视图保持不动。可通过 `Shift+G`、在最后一个条目处按 `j`、滚过底部或发送新提示恢复跟随。`Shift+E` 清除所有固定；`Ctrl+E` 清除思考块上的固定。

<a id="block-configuration"></a>
### 块配置

```toml
[scrollback.blocks.edit]
indent = true                         # 缩进差异内容
vpad = false                          # 垂直内边距
# expanded_by_default = true          # 未设置：遵循 config.toml 中 [ui] collapsed_edit_blocks
                                      #（该标志开启时为折叠单行）；取消注释可固定任一形状
dual_line_numbers = false             # 双列行号（旧 + 新）
# line_summary = false                # 在折叠标题中显示 +N/-M；未设置时遵循相同标志
hunk_separator = "…"                  # 差异块之间的分隔符（默认：“…”）

[scrollback.blocks.prompt]
vpad = true                           # 垂直内边距
show_prefix = true                    # 显示提示前缀字符
min_lines = 2                         # 粘滞模式下的最少内容行数

[scrollback.blocks.thinking]
animate = true                        # 思考时显示动画强调色
truncated_lines = 3                   # 截断模式下的行数
```

<a id="todo"></a>
### Todo

```toml
[todo]
badge_format = "default"              # "default"、"colon" 或 "comma"
```

徽章格式示例：

- `default`：`2/5` —— `done/total` 进度分数（done = 已完成，total = 除已取消任务外的全部任务）。
- `colon`：`[>:1 [ ]:4 ok:3 x:2]` —— 图标:计数。
- `comma`：`[1 >, 4 [ ], 3 ok, 2 x]` —— 计数 图标，以逗号分隔。

<a id="plugins-1"></a>
### 插件

```toml
disable_plugins = false               # 完全隐藏钩子/插件 UI
```

---

<a id="environment-variables"></a>
## 环境变量

以下是主要变量。完整列表请参见 README。

<a id="authentication-1"></a>
### 身份验证

| 变量 | 说明 |
|----------|-------------|
| `XAI_API_KEY` | 来自 console.x.ai 的 API 密钥 |
| `GROK_AUTH_PROVIDER_COMMAND` | 外部身份验证二进制文件路径 |
| `GROK_AUTH_PROVIDER_LABEL` | TUI 登录屏幕上的显示名称 |
| `GROK_AUTH_TOKEN_TTL` | 令牌有效期（秒） |
| `GROK_AUTH_EARLY_INVALIDATION_SECS` | 过期前多少秒刷新（默认：300） |
| `GROK_OIDC_ISSUER` | OIDC 颁发者 URL |
| `GROK_OIDC_CLIENT_ID` | OIDC 客户端 ID |

<a id="endpoints"></a>
### 端点

| 变量 | 说明 |
|----------|-------------|
| `GROK_CLI_CHAT_PROXY_BASE_URL` | 覆盖 API 代理基础 URL |

<a id="features"></a>
### 功能

| 变量 | 说明 |
|----------|-------------|
| `GROK_MEMORY` | 启用（`1`）或禁用（`0`）跨会话记忆 |
| `GROK_SUBAGENTS` | 启用（`1`）或禁用（`0`）子智能体 |
| `GROK_WORKFLOWS` | 启用（`1`）或禁用（`0`）后台工作流并选择 `/goal` 驱动程序（默认开启：主机拥有的工作流驱动；关闭：旧版 `update_goal`） |
| `GROK_WEB_FETCH` | 启用（`1`）或禁用（`0`）`web_fetch` 工具 |
| `GROK_WEB_FETCH_ALLOW_LOCAL` | 仅允许 `web_fetch` 访问明确指定的环回主机（`localhost` / `127.0.0.0/8` / `::1`）。等同于 `[toolset.web_fetch] allow_local`。默认关闭；私有/元数据网段仍会被阻止。 |
| `GROK_AGENT` | 自定义智能体定义路径或名称 |
| `GROK_SANDBOX` | 沙箱配置文件（off、workspace、devbox、read-only、strict；或自定义配置文件名） |
| `GROK_EXIT_TIMEOUT_SECS` | 请求退出后若清理卡住，经过多少秒强制退出（默认：20；设为 `0` 禁用；再过 5 秒仍未退出则执行硬退出） |

<a id="logging"></a>
### 日志

| 变量 | 说明 |
|----------|-------------|
| `GROK_LOG_FILE` | 将日志写入此文件路径（路径按原样使用） |
| `RUST_LOG` | 日志级别过滤器（例如 `debug`）；控制 `GROK_LOG_FILE` 日志和无头模式 stderr 输出 |

<a id="paths"></a>
### 路径

| 变量 | 说明 |
|----------|-------------|
| `GROK_HOME` | 覆盖配置目录（默认：`~/.grok`） |
| `GROK_RESPECT_GITIGNORE` | 强制开启（`1`）或关闭（`0`） gitignore 过滤；覆盖 `[tools] respect_gitignore` |

<a id="telemetry-1"></a>
### 遥测

| 变量 | 说明 |
|----------|-------------|
| `GROK_TELEMETRY_ENABLED` | 启用/禁用遥测 |
| `GROK_TELEMETRY_TRACE_UPLOAD` | 启用/禁用会话跟踪上传 |
| `GROK_TELEMETRY_MIXPANEL_ENABLED` | 专门启用/禁用 Mixpanel |
| `GROK_EXTERNAL_OTEL` | 发送到你的收集器的外部 OTEL（见[24-monitoring-usage.md](24-monitoring-usage.md)） |
| `GROK_FEEDBACK_ENABLED` | 启用/禁用反馈系统 |
| `GROK_DEPLOYMENT_KEY` | 企业管理 API 密钥 |

---

<a id="file-locations"></a>
## 文件位置

| 路径 | 说明 |
|------|-------------|
| `~/.grok/config.toml` | 主配置文件 |
| `~/.grok/pager.toml` | TUI 外观配置 |
| `~/.grok/auth.json` | 身份验证凭据（自动管理） |
| `~/.grok/sessions/` | 持久化会话（按工作目录组织） |
| `~/.grok/memory/` | 跨会话记忆文件和索引 |
| `~/.grok/skills/` | 用户级技能定义 |
| `~/.grok/plugins/` | 用户级插件 |
| `~/.grok/agents/` | 用户级智能体定义 |
| `~/.grok/lsp.json` | LSP 服务器配置（用户级） |
| `~/.grok/logs/` | 内部日志文件（例如 `unified.jsonl`、MCP 服务器日志） |
| `.grok/config.toml` | 项目级 MCP 服务器、插件和权限规则 |
| `.grok/skills/` | 项目级技能定义 |
| `.grok/plugins/` | 项目级插件 |
| `.grok/agents/` | 项目级智能体定义 |
| `.grok/hooks/` | 项目级钩子 |
| `.grok/lsp.json` | LSP 服务器配置 |

---

<a id="project-scoped-configuration"></a>
## 项目级配置

可以将部分设置按项目配置：将文件放在仓库内的 `.grok/` 中：

| 文件 | 配置内容 |
|--------------------|--------------------|
| `.grok/config.toml` | MCP 服务器、插件、权限规则以及 `[mcp] max_output_bytes` 工具结果上限（其他节仅从 `~/.grok/config.toml` 加载） |
| `.grok/skills/` | 项目专用技能 |
| `.grok/hooks/` | 项目专用生命周期钩子 |
| `.grok/agents/` | 项目专用智能体定义 |
| `.grok/lsp.json` | LSP 服务器配置 |
| `.grok/sandbox.toml` | 自定义沙箱配置文件 |
| `AGENTS.md` | 项目指令（系统提示） |

项目级 MCP 服务器会覆盖同名全局服务器（完全替换，而不是合并）。

---

<a id="lsp-servers"></a>
## LSP 服务器

语言服务器提供被动诊断和可选的 `lsp` 工具（见 [`lsp_tools`](#general-settings) 功能标志）。定义来自三个来源，并按服务器名称合并：

| 来源 | 位置 | 范围 |
|--------|----------|-------|
| 用户 | `~/.grok/lsp.json` | 所有项目 |
| 项目 | `.grok/lsp.json` | 当前仓库 |
| 插件 | 受信任插件的 `.lsp.json` 文件，或其 `plugin.json` 中的内联 `lspServers` 块 | 插件启用的所有位置 |

同一服务器名称来自多个来源时，按优先级从高到低解析：

1. **项目**——`.grok/lsp.json`
2. **用户**——`~/.grok/lsp.json`
3. **插件**——基于文件的 `.lsp.json`，然后是内联 `lspServers`，按插件加载顺序

项目和用户条目会替换同名的低优先级条目。插件条目只会添加本地文件尚未定义的名称，因此本地 `lsp.json` 始终优先于插件。只有在插件受信任后才会加载插件 LSP 服务器（见[插件](09-plugins.md)）。
