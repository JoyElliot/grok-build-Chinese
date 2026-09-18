<a id="mcp-servers"></a>
# MCP 服务器

MCP（模型上下文协议）服务器通过外部工具集成扩展 Grok。它们让 Grok 能够与任何实现 MCP 标准的服务交互。

---

<a id="what-are-mcp-servers"></a>
## 什么是 MCP 服务器？

MCP 服务器是一个通过标准化协议向 Grok 暴露工具的进程。配置 MCP 服务器后，其工具会与 Grok 内置工具一起提供给模型。模型可以在会话中发现并调用这些工具。

例如，GitHub MCP 服务器可能暴露 `create_issue`、`list_pull_requests` 和 `search_code` 等工具。数据库服务器可能暴露 `query`、`list_tables` 和 `describe_schema`。

协议详情请参阅 [MCP 规范](https://modelcontextprotocol.io)。

---

<a id="configuration"></a>
## 配置

MCP 服务器在 `~/.grok/config.toml` 的 `[mcp_servers.<name>]` 节中配置。

若要向团队分发 MCP 服务器，或限制用户可运行的服务器，请参阅插件指南中的[在组织中分发](09-plugins.md#distribute-across-an-organization)。

<a id="stdio-transport-local-process"></a>
### stdio 传输（本地进程）

Grok 会启动本地进程，并通过 stdin/stdout 通信：

```toml
[mcp_servers.my-server]
command = "/path/to/server"           # 服务器可执行文件
args = ["--flag", "value"]            # 命令参数
env = { API_KEY = "sk-..." }          # 环境变量
enabled = true                        # 启用或禁用服务器（默认：true）
startup_timeout_sec = 30              # 服务器启动超时，单位为秒（默认：30）
tool_timeout_sec = 6000               # 每次工具调用的超时回退值，单位为秒（默认：6000）
tool_timeouts = { slow_op = 120 }     # 每个工具的超时覆盖值，单位为秒
```

> **全局启动超时覆盖：** 不必为每个服务器设置 `startup_timeout_sec`，可以通过环境变量 `MCP_TIMEOUT`（毫秒，与 Claude Code 兼容）或 `GROK_MCP_STARTUP_TIMEOUT_SECS`（秒）更改所有服务器的默认值。单服务器的 `startup_timeout_sec` 仍优先于两者。首次启动时需要下载包的冷启动 `npx`/`uvx` 服务器通常需要该设置；默认值为 30s。
>
> **MCP 工具结果大小上限：** 大型 MCP / `use_tool` 结果会在行内截断（完整载荷会转存到会话的 `mcp/` 文件夹）。默认值为 **20_000 字节**。可通过以下方式覆盖：
>
> - 环境变量 `GROK_MAX_MCP_OUTPUT_BYTES` 或 `MAX_MCP_OUTPUT_BYTES`（字节；两者同时设置时 Grok 原生变量优先；兼容 Claude 的名称，但限制单位是**字节**而不是 token）
> - `config.toml`——用户级（`~/.grok/config.toml`）**或仓库级**（cwd → git 根目录链上任意位置的 `.grok/config.toml`；最深层文件优先，且仓库值仅在文件夹受信任后生效）：
>
> ```toml
> [mcp]
> max_output_bytes = 40000
> ```
>
> 优先级：requirements.toml > 环境变量 > 仓库 `.grok/config.toml` > 用户/受管配置 > 默认值。该目录中运行的会话会通过配置热重载应用仓库编辑。

<a id="http-sse-transport-remote-server"></a>
### HTTP/SSE 传输（远程服务器）

对于可通过 HTTP 访问的远程 MCP 服务器：

```toml
[mcp_servers.remote-api]
url = "https://mcp.example.com/api"
headers = { "Authorization" = "Bearer token" }
```

MCP 数据面请求（JSON-RPC 和 SSE）以及匿名访问探测默认会携带 `User-Agent: grok-cli/<version>` 标头，其中 `<version>` 是 Grok 二进制版本。OAuth 发现、客户端注册和令牌请求由 rmcp OAuth 客户端发出，保持其自身行为（不添加默认 `User-Agent`）。服务器 `headers` 中有效的 `User-Agent` 会覆盖默认值；无效值会在解析标头时被丢弃并给出警告，因此服务器仍会收到默认值。Figma MCP 服务器例外：服务器名为 `figma`、旧版托管名为 `grok_com_figma`，或主机位于 `figma.com`（均不区分大小写）时，会发送不带版本的 `grok-cli`；配置中自定义的 `User-Agent` 仍然优先。

<a id="streamable-http-with-session-id"></a>
### 可流式 HTTP 与会话 ID

```toml
[mcp_servers.my-streamable-server]
url = "https://mcp.example.com/api/mcp"
headers = { "x-mcp-session-id" = "{{session_id}}" }
```

---

<a id="cli-management"></a>
## CLI 管理

无需编辑配置文件即可从命令行管理 MCP 服务器：

```bash
# 列出已配置的 MCP 服务器
grok-zh mcp list
grok-zh mcp list --json          # 机器可读输出

# 添加 stdio 服务器。-- 后的所有内容都是服务器命令，因此 -y 等标志
# 会传给服务器，而不会被 grok-zh 解析
grok-zh mcp add filesystem -- npx -y @modelcontextprotocol/server-filesystem /path/to/dir

# 添加带环境变量的 stdio 服务器（-e 可重复使用）
grok-zh mcp add postgres -e DATABASE_URL=postgres://localhost/mydb -- npx -y @modelcontextprotocol/server-postgres

# 添加远程 HTTP 服务器
grok-zh mcp add --transport http sentry https://mcp.sentry.dev/mcp

# 添加带身份验证标头的远程服务器（--header 可重复使用）
grok-zh mcp add --transport http api https://mcp.example.com/mcp --header "Authorization: Bearer YOUR_TOKEN"

# 添加远程 SSE 服务器
grok-zh mcp add --transport sse linear https://mcp.linear.app/sse

# 移除服务器
grok-zh mcp remove github

# 启用或禁用本地/TOML（或兼容来源）的服务器
grok-zh mcp enable github
grok-zh mcp disable github

# 诊断服务器的配置和连接性
grok-zh mcp doctor               # 检查每个已配置的服务器
grok-zh mcp doctor github        # 检查一个服务器
grok-zh mcp doctor --json        # 机器可读输出
```

传输方式默认为 `stdio`；对于远程服务器，请传入 `--transport http` 或 `--transport sse`。

默认情况下，`grok-zh mcp add` 会写入 `~/.grok/config.toml`（`--scope user`）。使用 `--scope project` 可改为写入当前目录的 `.grok/config.toml`，该文件可以提交并与团队共享（参阅[项目范围的 MCP 服务器](#project-scoped-mcp-servers)）。标头和环境变量值会原样存储，因此请使用 `${VAR}` 引用密钥，而不要将密钥直接粘贴到已提交的项目配置中（参阅[配置示例](#example-configurations)）。`grok-zh mcp list` 会显示两个作用域中的服务器，将项目范围的服务器标为 `(project)`，将禁用的服务器标为 `(disabled)`。

`grok-zh mcp remove` 会搜索两个作用域，移除服务器后退出码为 0。找不到名称，或名称同时在用户和项目作用域中定义时退出码为 1——请传入 `--scope` 指定要移除哪一个。

`grok-zh mcp enable` / `disable` 会将个人开关状态持久化到用户的 `~/.grok/config.toml`（`disabled_mcp_servers`，以及条目存在时的 `[mcp_servers.<name>].enabled`）。作用域包括：

- **已知名称：** 用户/项目 Grok TOML、已在禁用列表中的名称、兼容来源（`.mcp.json`、Claude、Cursor）以及**插件** MCP 服务器（与 doctor/`/mcps` 使用相同发现逻辑）。
- **仅启用操作：** 如果 cwd 最近的项目定义带有粘滞的 `enabled = false`，只清除该键（保留注释）；禁用操作不会重写项目配置。
- **不与 `/mcps` 完全对等：** 网关连接器（`managed_gateway:…`，存储在 `disabled_mcp_tools.__managed_gateway_connectors` 下）在 TUI 中仍仅能通过 Space 操作。操作幂等；未知名称退出码为 1。

相较早期版本的破坏性变更：`--env` 现在每个标志只接受一个 `KEY=value`（使用 `-e A=1 -e B=2`，不要使用 `--env A=1 B=2`），服务器名称只能包含字母、数字、连字符和下划线。

---

<a id="project-scoped-mcp-servers"></a>
## 项目范围的 MCP 服务器

在仓库中放置 `.grok/config.toml` 即可按项目配置 MCP 服务器：

```
my-project/
  .grok/
    config.toml
  src/
  ...
```

```toml
# .grok/config.toml
[mcp_servers.linear]
url = "https://mcp.linear.app/mcp"
enabled = true
```

当服务器暴露原生 HTTP/SSE 端点时，优先使用 `url` 形式，而不是用 `npx mcp-remote <url>` 之类的 stdio 代理包装它。Grok 会直接处理 HTTP/SSE 和 OAuth，因此原生形式可避免每个会话多启动一个子进程，同时还会向提供方注册 Grok 自己的 OAuth 客户端。

Grok 会从当前目录逐级向上走到 git 仓库根目录，加载每一级的 `.grok/config.toml`：

| 位置 | 作用域 | 优先级 |
|----------|-------|----------|
| `~/.grok/config.toml` | 所有项目 | 最低 |
| `<repo-root>/.grok/config.toml` | 此仓库 | 中 |
| `<cwd>/.grok/config.toml` | 当前目录 | 最高 |

如果项目定义了与全局服务器同名的服务器，项目版本会完整替换它（不会合并字段）。

项目范围文件可提供 `[mcp_servers]`、`[plugins]` 和 `[permission]` 条目。Grok 只从 `~/.grok/config.toml` 读取大多数其他配置节。

---

<a id="tool-naming"></a>
## 工具命名

MCP 工具使用服务器名称作为命名空间以避免冲突，目录键为 `server__tool`（两个下划线）：

- `filesystem` 的 `read_file` 变为 `filesystem__read_file`。
- `github` 的 `create_issue` 变为 `github__create_issue`。
- 工具部分可以以数字开头，例如 `auth__2fa_enable`。

### 工具进入目录的条件

`qualify_mcp_tool_name` 只接受满足以下条件的名称：

| 部分 | 规则 |
| --- | --- |
| 服务器名称 | 以字母或下划线开头；其余仅限 ASCII 字母、数字、下划线和连字符。 |
| 工具名称 | 非空；仅限 ASCII 字母、数字、下划线和连字符；可以以数字开头。 |
| 分隔符 | 必须恰好一个 `__`；包含第二个 `__` 或 `___` 的名称会跳过。 |
| 完整目录键 | `server` + `__` + `tool` 最多 **256** 个字符。 |

被拒绝的工具会单独跳过，日志记录 `Skipping MCP tool` 及原因，其他工具仍会加载。**64 字符**是提供商的函数名预算，适用于 `search_tool` 和 `use_tool` 本身，不适用于目录键；超过 64 字符的合格目录键仍能通过 `use_tool` 按完整名称调用。

`[mcp_servers.<name>]` 或 `grok-zh mcp add` 中的名称就是目录前缀。虽然数字开头的名称可以是合法 TOML 键，目录准入仍会以 `InvalidServerName` 拒绝。服务器名称以 `_` 结尾会产生含 `___` 的键，以 `InvalidOrAmbiguousQualifiedName` 跳过。

`search_tool` / `use_tool` 使用完整限定目录键（例如 `github__create_issue`），不是原始工具名 `create_issue`。

---

<a id="toggle-servers-at-runtime"></a>
## 在运行时切换服务器

无需重启 Grok 即可启用或禁用 MCP 服务器（TUI `/mcps` 或 CLI——参见 [CLI 管理](#cli-management)）。

<a id="the-mcps-modal"></a>
### `/mcps` 模态窗口

在 TUI 中打开 MCP 服务器模态窗口：

- 将 `/mcps` 作为斜杠命令运行；
- 或按 `Ctrl+L`（非 VS Code 系列）并导航到 MCP Servers 选项卡；VS Code 系列使用 `/plugins` 或 `/mcp`，再打开 MCP Servers 选项卡。

在模态窗口中可以：

- 查看每个服务器的来源、启用状态和工具数量；
- 使用 `Space` 启用或禁用服务器；
- 展开服务器查看它提供的工具；
- 编辑 `config.toml` 后按 `r` 刷新列表；
- 使用 `i` 验证 OAuth 服务器；
- 使用 `a` 添加服务器，或使用 `x` 移除本地服务器（模态窗口会请求确认；按小写 `y` 移除，按其他任意键取消）。

<a id="tool-discovery"></a>
### 工具发现

模型有两个用于处理 MCP 服务器的内置工具：

- `search_tool` —— 在所有已启用的 MCP 服务器中发现可用的集成工具。用它按名称或描述查找工具。
- `use_tool` —— 调用通过 `search_tool` 发现的集成工具。请指定完整限定的工具名称（例如 `github__create_issue`）。

---

<a id="compatibility"></a>
## 兼容性

为实现兼容性，Grok 会从多个来源加载 MCP 服务器配置：

| 来源 | 格式 | 位置 | 可配置 |
|--------|--------|----------|-------------|
| `config.toml` | 原生 Grok 配置 | `~/.grok/config.toml`、`.grok/config.toml` | 始终启用 |
| `.claude.json` | Claude Code 格式 | `~/.claude.json` | `[compat.claude] mcps` |
| `.cursor/mcp.json` | Cursor 格式 | `~/.cursor/mcp.json`、`<project>/.cursor/mcp.json` | `[compat.cursor] mcps` |
| `.mcp.json` | MCP 标准格式 | 项目根目录（cwd 到 git 根目录） | 除非已导入或关闭 Claude 导入提示（已设置导入标记），否则加载 |

所有来源按优先级合并：config.toml > Claude > Cursor > `.mcp.json`。来源优先级较高的服务器在名称冲突时优先。

默认会扫描 Claude 和 Cursor MCP 来源。若要停用某个厂商的扫描，请在 `~/.grok/config.toml` 设置 `[compat.<vendor>] mcps = false`，或设置相应环境变量（`GROK_CURSOR_MCPS_ENABLED`、`GROK_CLAUDE_MCPS_ENABLED`）。详情请参阅[配置](05-configuration.md#harness-compatibility)。使用 `grok-zh inspect` 可查看加载了哪些 MCP 服务器及其厂商来源（`[cursor]`、`[claude]`）。

---

<a id="mcp-oauth"></a>
## MCP OAuth

对于要求 OAuth 身份验证的 MCP 服务器，Grok 会自动处理凭据流程。当 MCP 服务器请求 OAuth 凭据时，Grok 会打开基于浏览器的授权流程，并保存生成的令牌供以后使用。

---

<a id="example-configurations"></a>
## 配置示例

托管 MCP 服务器使用 `url` 形式，本地 stdio 工具使用 `command` / `args` 形式。

<a id="native-http-hosted-services"></a>
### 原生 HTTP（托管服务）

必须先对基于 OAuth 的 MCP 服务器完成身份验证，才能使用它们。Grok 会将生成的令牌以本地明文存储在 `~/.grok/mcp_credentials.json`，并设置仅所有者可读写的文件权限（Unix 上为 `0600`）。建议在主机上使用全磁盘加密。编辑 `config.toml` 后，在 `/mcps` 模态窗口中按 `r` 刷新服务器列表。

```toml
[mcp_servers.linear]
url = "https://mcp.linear.app/mcp"
enabled = true

[mcp_servers.sentry]
url = "https://mcp.sentry.dev/mcp"
enabled = true

[mcp_servers.mixpanel]
url = "https://mcp.mixpanel.com/mcp"
enabled = true
```

对于使用静态 bearer token 而非 OAuth 进行身份验证的内部或自托管服务器，请显式设置 `Authorization` 标头：

```toml
[mcp_servers.internal-tools]
url = "https://mcp.internal.example.com/mcp"
enabled = true

[mcp_servers.internal-tools.headers]
Authorization = "Bearer <token>"
```

若不想将密钥放入配置文件，请使用 `${VAR}`（或 `${VAR:-default}`）引用环境变量。Grok 加载时会展开 `[mcp_servers.*]` 中的字符串字段——`url`、`command`、`args` 以及 `env` 和 `headers` 中的值：

```toml
[mcp_servers.internal-tools]
url = "https://mcp.internal.example.com/mcp"
enabled = true
headers = { "Authorization" = "Bearer ${INTERNAL_MCP_TOKEN}" }
```

<a id="local-stdio"></a>
### 本地 stdio

必须在本地运行的工具（文件系统访问、本地数据库、内部服务器）请使用 stdio。

```toml
# 将文件系统访问限制在一个目录
[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/allowed/directory"]

# 本地 Postgres
[mcp_servers.postgres]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-postgres", "postgresql://user:pass@localhost/db"]

# 使用更长的启动超时并调整每个工具超时的自定义服务器
[mcp_servers.my-tools]
command = "/usr/local/bin/my-mcp-server"
args = ["--config", "/etc/my-mcp.json"]
startup_timeout_sec = 30
tool_timeout_sec = 120
tool_timeouts = { slow_analysis = 300, quick_lookup = 10 }
```

在 Windows 上，npm 会将 `npx`、`npm`、`pnpm` 和 `yarn` 等启动器安装为 `.cmd` 批处理 shim（不存在 `npx.exe`）。Grok 会在启动前将 `PATH` 上的裸 `command`（例如 `npx`）解析为真实启动器路径（遵循 `PATHEXT`），因此无需手动用 `cmd /c` 包装即可工作。作为绝对路径给出或包含路径分隔符的 `command` 将按原样使用。

---

<a id="available-mcp-servers"></a>
## 可用的 MCP 服务器

下面是可用上述 `url` 或 `command` 形式配置的部分 MCP 服务器列表。使用前请向每个提供方确认当前端点或包名：

| 服务器 | 传输方式 | 端点 / 包 |
|---|-----------|--------------------|
| Linear | HTTP（OAuth） | `https://mcp.linear.app/mcp` |
| Sentry | HTTP（OAuth） | `https://mcp.sentry.dev/mcp` |
| Mixpanel | HTTP（OAuth） | `https://mcp.mixpanel.com/mcp` |
| Filesystem | stdio | `@modelcontextprotocol/server-filesystem` |
| Git | stdio | `@modelcontextprotocol/server-git` |
| GitHub | stdio | `@modelcontextprotocol/server-github` |
| GitLab | stdio | `@modelcontextprotocol/server-gitlab` |
| PostgreSQL | stdio | `@modelcontextprotocol/server-postgres` |
| SQLite | stdio | `@modelcontextprotocol/server-sqlite` |
| Puppeteer | stdio | `@modelcontextprotocol/server-puppeteer` |

完整社区服务器列表请参阅 [MCP 服务器注册表](https://github.com/modelcontextprotocol/servers)，协议详情请参阅 [MCP 规范](https://modelcontextprotocol.io)。

---

<a id="subagents-and-mcp"></a>
## 子智能体与 MCP

子智能体默认继承父会话已连接的 MCP 服务器，包括插件来源的智能体。使用智能体 frontmatter 的 `mcpInheritance` 可限制该集合（`all`、`none`、`named` 或 `except`）。详情请参阅[子智能体——MCP 继承](16-subagents.md#mcp-inheritance)。

如果子智能体列出 `search_tool` / `use_tool` 却返回空目录，请检查：

1. 父会话确实连接了该服务器（参阅扩展 / `grok-zh inspect`）；
2. 智能体的 `mcpInheritance` 不是 `none`，也不是排除该服务器的过滤器；
3. 插件智能体不能在 frontmatter 中声明自己的 `mcpServers`——它们只能看到父会话已连接的服务器。

---

<a id="troubleshooting"></a>
## 故障排查

<a id="server-not-starting"></a>
### 服务器未启动

```bash
# 手动测试服务器命令
npx -y @modelcontextprotocol/server-filesystem /path

# 增加启动超时
# 在 config.toml 中：
[mcp_servers.filesystem]
startup_timeout_sec = 30
```

对于 stdio 服务器，Grok 会将进程的标准错误捕获到 `~/.grok/logs/mcp/<server>.stderr.log`，并在每次启动时截断该文件。服务器启动但握手失败时，请检查此文件：

```bash
tail -f ~/.grok/logs/mcp/filesystem.stderr.log
```

### 服务器列出的工具没有出现

服务器已启动且 `tools/list` 返回工具，但 `/mcps` 和 `search_tool` 不显示时：检查 `GROK_LOG_FILE` / `--debug` 中的 `Skipping MCP tool`；确认服务器名以字母或下划线开头且不以下划线结尾；工具名只包含 `[A-Za-z0-9_-]`，不含点或冒号；完整目录键不超过 256 字符。无需为满足函数名限制而把目录键缩短到 64 字符。

这与首次提示时仍在握手的情况不同。握手未完成时，可在服务器就绪后发送第二条提示，或运行 `grok-zh mcp doctor`。

<a id="viewing-server-status"></a>
### 查看服务器状态

使用 `grok-zh inspect` 查看所有已加载的 MCP 服务器及其来源：

```bash
grok-zh inspect          # 人类可读
grok-zh inspect --json   # 机器可读
```

<a id="debug-logging"></a>
### 调试日志

```bash
RUST_LOG=debug GROK_LOG_FILE=/tmp/grok.log grok-zh
tail -f /tmp/grok.log
```

查找包含 `mcp` 的日志条目，以跟踪服务器启动、工具发现和工具调用执行。
