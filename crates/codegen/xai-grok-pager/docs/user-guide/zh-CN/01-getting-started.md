# 入门指南

> **社区构建说明：** 这是非官方简体中文发行版。它使用命令 `grok-zh`，
> 并有意与官方程序共用 `~/.grok` 和 `GROK_HOME`，因此会话、凭据和配置
> 保持一致。xAI 官方安装器不会安装或更新 `grok-zh` 可执行文件。

Grok Build 是 SpaceXAI 推出的终端 AI 编程助手。它以 TUI（终端用户界面）
运行，能够理解代码库、执行 Shell 命令、编辑文件、搜索网页并管理任务。

你既可以在全屏 TUI 中交互使用，也可以用无头模式运行脚本和 CI/CD，或通过
Agent Client Protocol（ACP）集成到编辑器中。

---

## 安装

社区 Windows 包由本仓库 Releases 和 `CI` 工作流生成。
Release ZIP 下载后只需解压一次；所有 `release-v*` 包（包括
`release-v1.0.13`）需先打开唯一的
`grok-zh-<version>-windows-x86_64-gnu` 目录，旧版与 `v1.0.8` 桥接包则直接使用
解压目录。包内的 `Install-GrokZh.ps1` 可自动加入用户 `Path`，默认提供
`grok-zh`、`agent-zh`，并支持由用户显式接管 `grok`、`agent`。完整说明也会
作为包内的 `INSTALL-WINDOWS.md` 提供，并可在
[仓库中在线查看](https://github.com/JoyElliot/grok-build-Chinese/blob/zh-dev/packaging/windows/INSTALL-WINDOWS.md)。

macOS ARM64 与 Linux x86_64 GNU 的 `release-v*` 归档同样只含一个与归档同名
（去掉 `.tar.gz`）的顶层目录；解压后先进入该目录，再执行包内校验与
`Install-GrokZh.sh`。

上游的 `install.sh`、`install.ps1` 和 `@xai-official/grok` 包不能用于安装此
发行版。

验证安装：

```bash
grok-zh --version
```

带更新器的版本只读取本仓库的 Immutable GitHub Releases，只接受当前平台的完整归档及其
`.sha256` sidecar 元数据，并核对 GitHub SHA-256、安全归档布局和包内
`SHA256SUMS.txt`；绝不会回退到 xAI 官方发布渠道。所有 `release-v*` 包均使用单一
顶层目录；单独的 `v1.0.8` 是 Windows-only 扁平 ZIP 桥接版。Windows `v1.0.3`、
`v1.0.5` 会自动经过该桥接版再选择现代 `release-v*`；写死旧仓库地址的
`v1.0.0-zh.preview.3` 仍需手工安装一次现代完整包。

社区版默认关闭后台自动更新：程序启动时只检查版本并显示提示，不下载文件。欢迎页按
`Ctrl+U` 才会退出旧 TUI、下载并安装；显式开启设置中的“自动更新”后才允许后台预下载。
交互式下载会显示已下载大小、百分比、速度和预计剩余时间；后台更新或输出重定向时进度条
自动隐藏。版本严格使用与上游一致的三段 SemVer，不定义第四段社区修订号。
随后重新运行 `grok-zh`。默认通道为稳定版；`grok-zh update --alpha` 可显式选择预发布版，
`grok-zh update --stable` 可切回稳定版。

在 Grove 配置中启用 `[clone] enabled = true` 后，可以通过 Grove 提取仓库
（macOS 使用 NFS，Linux 使用 FUSE）：

```bash
grok-zh clone <url> [dir]
```

默认会对所选分支进行深度为 1 的检出。需要完整历史时请传入
`--full-history`。详情见 [grok clone](27-grok-clone.md)。

---

## 首次启动

运行以下命令启动 Grok：

```bash
grok-zh
```

首次启动时，Grok 会打开浏览器并引导你通过 grok.com 完成身份验证。登录后，
凭据会保存在 `~/.grok/auth.json`，可跨会话持续使用，并由 `grok` 与
`grok-zh` 共享。Grok 会自动刷新凭据；无法继续续期时，会提示你重新登录。

如果更希望使用 API 密钥（例如 CI/CD 或无法打开浏览器的环境），请设置
`XAI_API_KEY` 环境变量：

```bash
export XAI_API_KEY="xai-..."
grok-zh
```

浏览器登录、API 密钥、OIDC、外部身份提供方和设备代码流程的完整说明见
[身份验证](02-authentication.md)。

---

## 基本交互

完成身份验证后，Grok 会显示全屏 TUI，主要包含两个区域：

- **回滚区** —— 显示你的提示、Grok 回复、工具调用、文件编辑等会话历史。
- **提示输入框** —— 位于底部，用来输入消息。

输入消息并按 `Enter` 发送。Grok 会按需读取文件、运行命令和编辑代码；每次工具
运行都会实时流式显示在回滚区中。

按 `Tab` 在提示输入框与回滚区之间切换焦点。任务运行时，按 `Esc` 可取消任务
（全屏 Vim 回滚模式例外：任务运行期间 `Esc` 不执行取消；精简模式即使启用
Vim 也会取消）。输入框为空时，`Ctrl+C` 可取消任务；若仍有草稿，第一次按下
只会清空草稿。空闲时，在 800 毫秒内连续按两次 `Esc`：输入框非空时会清空
内容；输入框为空且已有会话消息时会打开回退界面，详见
[键盘快捷键](03-keyboard-shortcuts.md#escape)。回滚区获得焦点后，可用方向键
选择条目，并折叠或展开内容。若希望用 `j`/`k` 导航、`h`/`l` 折叠，请启用
Vim 模式。

### 引用文件

在提示中输入 `@` 可附加文件：

```
@src/main.rs              # 附加文件
@src/main.rs:10-50        # 附加第 10 到 50 行
@src/                     # 浏览目录
```

`@` 会打开模糊文件选择器。默认情况下，它遵循 `.gitignore` 并隐藏点文件。
在查询前加 `!` 可搜索隐藏文件：

```
@!.github                 # 搜索隐藏目录
@!.env                    # 附加隐藏文件
```

### 权限

默认情况下，Grok 在执行 Shell 命令或编辑文件前会请求许可。你可以逐次批准，
也可以开启始终批准模式：

- 按 `Ctrl+O` 切换始终批准模式。
- 启动时使用 `--yolo`：`grok-zh --yolo`。
- 在提示输入框中输入 `/always-approve` 切换该模式。

---

## 核心概念

### 会话

每段对话都是一个**会话**。会话会自动保存到 `~/.grok/sessions/`，由 `grok`
与 `grok-zh` 共享，并可稍后恢复。每个会话都会记录完整对话历史、工具调用、
文件编辑和任务状态。

- 新建会话：`Ctrl+N` 或 `/new`。
- 恢复旧会话：在 TUI 中输入 `/resume`，或从 CLI 使用 `--resume <ID>`。
- 继续最近的会话：`grok-zh -c`。

### 回滚区

回滚区是主要显示区域，其中包括：

- **用户提示** —— 你的消息，以固定标题形式显示。
- **智能体消息** —— Grok 的回复，支持完整 Markdown 渲染和语法高亮。
- **思考块** —— Grok 的推理过程，可折叠。
- **工具调用** —— 文件编辑（含行内差异）、命令执行、搜索结果等。
- **任务列表** —— 用于跟踪进度的 TODO 项目。

使用 `Left`/`Right` 方向键折叠或展开当前条目（Vim 模式下使用 `h`/`l`
和 `e`）。Vim 模式下，按 `y` 复制内容，按 `Y` 复制元数据（例如执行过的
命令）。任何模式下都可按 `Enter` 在全屏查看器中打开条目。

### 工具

Grok 内置以下工具：

| 工具 | 说明 |
|------|------|
| `read_file` / `search_replace` | 按精确行读取和编辑文件 |
| `grep` | 使用 ripgrep 在代码库中执行正则搜索 |
| `list_dir` | 列出目录内容 |
| `run_terminal_command` | 执行 Shell 命令 |
| `web_search` / `web_fetch` | 搜索网页并获取 URL 内容 |
| `todo_write` | 创建和管理任务列表 |
| `spawn_subagent` | 生成并行子智能体会话 |
| `memory_search` | 搜索跨会话记忆 |

还可通过 [MCP 服务器](05-configuration.md#mcp-servers) 扩展工具，以集成 GitHub、
数据库等服务。

### 斜杠命令

在提示输入框中输入 `/` 可访问命令，不必编写完整提示即可快速执行操作：

```
/model grok-build                 # 切换模型
/compact                          # 压缩会话历史
/always-approve                   # 切换始终批准模式
/new                              # 新建会话
```

完整命令参考见[斜杠命令](04-slash-commands.md)。

---

## 常用启动选项

```bash
# 启动交互式 TUI，并将初始提示作为第一轮任务提交
grok-zh "fix the failing auth test and run it"

# 在新 Git 工作树中提交初始提示。请使用带等号的 --worktree=<name>，
# 避免提示被当作工作树名称；例如 `grok-zh -w "refactor module X"`
# 会把 "refactor module X" 当作工作树标签，而不是提示。
grok-zh --worktree=feat "refactor module X"

# 让工作树基于指定分支（例如 main），而不是当前 HEAD
grok-zh -w --ref main "implement feature from main"

# 在指定项目目录中启动
grok-zh --cwd ~/projects/my-app

# 添加项目专用规则
grok-zh --rules "Always use TypeScript. Prefer functional components."

# 自动批准所有工具执行
grok-zh --yolo

# 使用指定模型
grok-zh -m grok-build

# 恢复旧会话
grok-zh --resume <session-id>

# 继续最近的会话
grok-zh -c

# 实验性回滚区原生渲染模式。该选择会被记住：通过 --minimal 或
# --fullscreen（也可用 /minimal 或 /fullscreen）选择后，普通 `grok-zh`
# 下次会继续使用上次选择的模式。
grok-zh --minimal

# 返回标准全屏 TUI，并将该模式设为默认
grok-zh --fullscreen

# 无头模式（供脚本使用）
grok-zh -p "Explain this codebase"
```

---

## 无头模式

使用无头模式以非交互方式运行 Grok，适用于脚本、CI/CD 和自动化：

```bash
grok-zh -p "Your prompt here"
```

输出格式：

| 格式 | 参数 | 说明 |
|------|------|------|
| `plain` | 默认 | 人类可读文本 |
| `json` | `--output-format json` | 含 `text`、`stopReason`、`sessionId` 和 `requestId` 的单个 JSON 对象 |
| `streaming-json` | `--output-format streaming-json` | 用于实时处理的 NDJSON 事件流 |

CI/CD 示例：

```bash
grok-zh -p "Review changes for bugs" --output-format json --yolo | jq -r '.text'
```

---

## 项目规则（AGENTS.md）

在仓库中创建 `AGENTS.md` 可添加项目专用指令。Grok 会读取这些文件，并在
会话开始时将内容作为项目指令消息注入：

```
~/.grok/AGENTS.md           # 全局规则（适用于所有项目）
<repo-root>/AGENTS.md       # 仓库级规则
<cwd>/AGENTS.md             # 目录级规则（优先级最高）
```

越靠近当前目录的文件优先级越高。为兼容现有项目，Grok 也会读取 `CLAUDE.md`。

---

## 后续阅读

| 文档 | 内容 |
|------|------|
| [身份验证](02-authentication.md) | 浏览器登录、API 密钥、OIDC、外部身份提供方和设备代码流程 |
| [键盘快捷键](03-keyboard-shortcuts.md) | 所有按键绑定的完整参考 |
| [斜杠命令](04-slash-commands.md) | 全部 `/` 命令 |
| [配置](05-configuration.md) | config.toml、pager.toml 和环境变量 |
