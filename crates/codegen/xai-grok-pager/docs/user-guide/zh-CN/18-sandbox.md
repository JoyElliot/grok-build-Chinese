# 沙箱模式

沙箱模式使用操作系统级内核原语（Linux 上的 Landlock，macOS 上的 Seatbelt）限制
智能体进程及其派生命令能够访问的文件系统和网络。内核会在进程整个生命周期内强制
执行这些限制。

沙箱模式默认关闭。

---

## 快速开始

```bash
# 使用 workspace 沙箱运行（到处可读，仅可写 CWD + 临时目录 + ~/.grok/）
grok-zh --sandbox workspace

# 只读模式（到处可读，仅可写 ~/.grok/ + 临时目录）
grok-zh --sandbox read-only

# 限制最严格的配置（可读 CWD + 系统路径 + ~/.grok，可写 CWD + ~/.grok/sessions + 临时目录，不允许子进程联网）
grok-zh --sandbox strict
```

---

## 内置配置

| 配置                  | FS Read            | FS Write                                       | Child Network | Use Case                          |
| --------------------- | ------------------ | ---------------------------------------------- | ------------- | --------------------------------- |
| `off` (default)       | Unrestricted       | Unrestricted                                   | Unrestricted  | 无沙箱                            |
| `workspace`           | Everywhere         | CWD + `~/.grok/` + `/tmp` + `/var/tmp`         | Allowed       | 常规开发                          |
| `devbox`              | Everywhere         | 除 `/data` 外的所有顶层目录                    | Allowed       | 一次性开发 VM                     |
| `read-only`           | Everywhere         | `~/.grok/` + `/tmp` + `/var/tmp`               | Blocked¹      | 探索、代码审查                    |
| `strict`              | CWD + system paths + `~/.grok` | CWD + `~/.grok/sessions` + `/tmp` + `/var/tmp` | Blocked¹      | 不受信任的代码                    |

¹ 仅 **Linux** 通过 seccomp 强制阻止子进程网络。在 macOS 上这是空操作——这些配置不
会限制子进程网络。

要在配置之上阻止特定文件（例如 `.env` 或凭据路径），请定义带有 `deny` 列表的
[自定义配置](#custom-profiles)——它由内核强制执行（读 + 写/重命名），并支持
`**/*.pem` 这样的 glob 模式。

### 配置详情

**workspace** —— 日常开发推荐使用的配置。智能体可以读取系统上的任意文件（以
了解依赖项、系统库等），但只能写入当前工作目录、`~/.grok/` 和临时目录
（`/tmp`、`/var/tmp` 以及 macOS 临时目录）。网络访问对 `web_search` 和 MCP 服务器
等工具开放。

**devbox** —— 为一次性开发 VM 保留的内置配置。智能体可以到处读取，并可写入除
`/data` 和虚拟文件系统（`/proc`、`/sys`、`/dev`）之外的所有顶层目录，包括主目录。
网络访问开放。`--sandbox devbox` 运行内置的 `devbox` 配置，会遮蔽你在
`sandbox.toml` 中定义的任何 `[profiles.devbox]`。

**read-only** —— 需要在不修改项目文件的情况下分析代码时使用。智能体可以读取所有
内容，但只能写入 `~/.grok/`（会话持久化所需）和临时目录。Linux 上会阻止子进程网络
（macOS 上为空操作）。

**strict** —— 用于审查不受信任代码的最严格配置。智能体可以读取当前工作目录、
必要的系统路径和 `~/.grok`。写入范围限制为 CWD、`~/.grok/sessions` 和临时目录，
而不是整个 `~/.grok` 树。Linux 上会阻止子进程网络（macOS 上为空操作）。

### 全局 hook 的直接写入保护

在 `workspace`、`read-only` 和 `strict`（以及扩展这些基础配置的自定义配置）下，
内核会**拒绝写入** Grok 用作用户全局 hook 源的直接磁盘路径（在已授予读取权限时
仍可读取）。内置 `strict` 可以读取 `~/.grok`，但只能写入 CWD、
`~/.grok/sessions` 和临时目录；即使配置原本授予写入，下列路径仍受拒绝写入保护：

- `~/.grok/hooks/`（hook 目录）
- `~/.grok/hooks-paths`（注册表文件；不会作为 hook JSON 加载，只加载其中的绝对目标）
- `hooks-paths` 中列出的绝对目标（相对路径行会忽略；缺少目标会拒绝启动沙箱）

在这些配置下首次启动时，如果 `hooks/` 目录和 `hooks-paths` 文件缺失，Grok 会创建
真正的空 `hooks/` 目录和空 `hooks-paths` 文件（绝不会创建符号链接或错误类型）。
Claude/Cursor 全局设置**不**受此写入拒绝保护；是否发现这些厂商仍由兼容性设置单独
控制。

符号链接形式的 `$GROK_HOME`，或 `hooks-paths` 中带有符号链接组件的条目，会在沙箱
启动时被拒绝（防止重新指向其他目标）。受保护路径的现有父目录会被固定，使其无法
在拒绝规则下被重命名；同级目录仍可写入。在 Linux 上，bubblewrap 内会禁用嵌套用户
命名空间，因此无法重新排列挂载绑定。项目 hook 仍受文件夹信任控制。`devbox` 配置
不应用此保护（一次性 VM）。需要此保护的配置在内核策略无法应用时会拒绝启动（包括
Linux 上没有经过验证的只读挂载）。

---

<a id="custom-profiles"></a>
## 自定义配置

在 `~/.grok/sandbox.toml`（全局）或 `.grok/sandbox.toml`（按项目）中创建自定义沙箱
配置：

```toml
[profiles.project]
# 从内置配置开始，然后添加覆盖项
extends = "workspace"
restrict_network = true

# 智能体可读但不可写入/删除的路径
read_only = ["/data"]

# 额外可写路径
read_write = ["/tmp/scratch"]

# 由内核拒绝的路径或 glob（强制执行；见下方说明）
deny = ["/data/shared-secrets", "**/.env", "**/*.pem"]
```

使用自定义配置：

```bash
grok-zh --sandbox project
```

自定义配置不能复用内置名称。`--sandbox devbox` 始终运行内置的 `devbox` 配置，会
遮蔽你定义的任何 `[profiles.devbox]`。

如果用户文件和项目文件以不同方式定义同一个自定义配置，Grok 会使用用户配置并显示
启动警告。运行 `/doctor` 可查看两个文件的位置以及解决冲突的方法。定义完全相同则
不会产生警告。

### 自定义配置字段

| 字段               | 类型     | 说明                                                   |
| ------------------ | -------- | ------------------------------------------------------ |
| `extends`          | String   | 要继承的内置配置（`workspace`、`devbox`、`read-only`、`strict`）。省略时默认为 `workspace` |
| `restrict_network` | Boolean  | 阻止子进程访问网络                                     |
| `read_only`        | String[] | 额外的只读路径                                         |
| `read_write`       | String[] | 额外的读写路径                                         |
| `deny`             | String[] | 由内核拒绝的路径或 glob（见说明）。含 `*`、`?` 或 `[` 的条目是 glob |

> **关于 `deny` 的说明：** 非空的 `deny` 列表由**内核强制执行**。被拒绝的路径会
> 通过 macOS 上的 Seatbelt 和 Linux 上的 bwrap 覆盖绑定，同时被**拒绝读取和写入/重命名**，
> 因此既不能（通过 `bash`、`grep` 或子智能体）读取被拒绝路径，也不能将其移出拒绝集后
> 在别处读取（`mv secret x && cat x` 绕过方式已关闭）。在 **Linux** 上，拒绝读取需要
> `bubblewrap`：如果它缺失（或任何一个拒绝路径无法绑定），Grok 会拒绝启动，而不是
> 暴露被拒绝的路径（仅拒绝写入 `/data` 的 `devbox` 仍会回退到 Landlock）。对不在
> `deny` 中的路径写入权限由你在 `read_write` 中授予的内容控制。

> **`deny` 中的 glob：** 如果条目包含 `*`、`?` 或 `[`，它就是 **glob**。
> 这些字符**始终**表示 glob——若要拒绝名称中含这些字符的字面文件，请改为指定其
> 父目录。支持的、类似 gitignore 的子集如下：
>
> - `*` —— 路径段内任意长度的字符（遇到 `/` 停止）
> - `?` —— 路径段内恰好一个字符
> - `**` —— 跨越目录（作为完整路径段，例如 `**/`、`a/**`）；`**/` 还会匹配零个
>   目录，因此 `**/.env` 会匹配 `.env` 和 `sub/.env`
> - `[abc]` / `[a-z]` —— 字符类；开头的 `!` **或** `^` 表示否定
>   （`[!a]` 和 `[^a]` 都表示“不是 `a`”）
>
> 花括号交替（`{a,b}`）、反斜杠转义、空路径段（双写 `//` 或末尾 `/`）、`.` 或 `..`
> 段，以及不常见的字符类形式 `[]…]`（先出现字面 `]`）和 POSIX `[[:…:]]` **均不受
> 支持**，因此两个平台不可能以不同方式解释同一个 glob。使用不受支持的元字符，或格式
> 错误的 glob，会使 Grok 在**两个平台上都拒绝启动**（故障安全关闭）——请将 `*.pem`
> 和 `*.key` 写成单独条目，而不是 `*.{pem,key}`。
>
> 相对 glob 以工作区为锚点；绝对 glob（例如 `/home/**/.ssh`）以其字面前缀为锚点。
> 非 glob 条目保持精确路径匹配。相对 glob **仅**匹配工作区内部。要拒绝其他位置的文件，
> 请将条目写成绝对路径。除此之外，各平台的强制方式不同：
>
> - **macOS 密不透风：** 每个 glob 会变成运行时应用的 Seatbelt 正则表达式，因此即使
>   文件在 Grok 启动后创建，只要匹配也会被拒绝。
> - **Linux 尽力而为：** 挂载命名空间无法在运行时处理 glob，因此每个 glob 会展开为
>   启动时**已经存在**的文件，并对这些文件覆盖绑定。之后创建的匹配文件**不**在保护范围
>   内——在 Linux 上，任何必须密不透风保护的路径都应写成精确路径。匹配到的符号链接会
>   与其解析后的目标一同屏蔽。若 glob 匹配的文件过多，或其目录树太深、太宽而无法扫描，
>   Grok 会拒绝启动，而不是降低强制程度；错误信息会列出 glob 以及扫描停止的目录。启动
>   扫描从每个 glob 的字面前缀开始，并包含 gitignored 和隐藏文件，因此在大型工作区中，
>   优先使用锚定 glob（`certs/**/*.pem` 只扫描 `certs/`），而不是裸的 `**` 模式。

---

## 工作原理

沙箱在启动时使用内核原语应用于**整个 `grok-zh` 进程**，而不是按命令包装。这样所有
工具操作都会受到覆盖：

- `read_file`、`search_replace`、`list_dir` —— 在进程内受 Landlock/Seatbelt 限制
- `bash` 命令、`grep`（rg）—— 子进程自动继承文件系统限制
- 网络 —— 在 Linux 上可通过 seccomp 阻止子进程；在 macOS 上为空操作

当请求非 `off` 沙箱配置时（CLI、`GROK_SANDBOX`、配置或托管要求）：

- 智能体在**进程内**运行，不经过共享 leader，因此强制应用配置时工具调用留在此进程
  中。如果原本会启用 leader 模式，启动时会显示一行说明
- 如果内置配置应用失败，Grok 会发出警告并在不强制执行的情况下继续（见
  [平台支持](#platform-support)），但仍会拒绝 leader，工具不会被委派到其他位置
- `grok-zh workspace start`、`restart` 和 `resume` 不可用；`pause`、`stop` 和 `status`
  仍可用

要使用被拒绝的命令，请在选择该配置的来源处禁用它。

沙箱一旦应用便**不可逆**。智能体无法在运行时放宽限制。

---

## 恢复会话

会话启动时使用的配置会随会话保存，并在会话生命周期内**固定不变**。恢复会话时
（`grok-zh --resume <id>`、`grok-zh --continue` 或 `grok-zh -r`），Grok 会自动恢复
同一配置——因此使用 `--sandbox workspace` 启动的会话不会悄然以更严格的默认配置回来，
导致原本有效的命令失效。

恢复不会更改会话的沙箱：

- 恢复时省略 `--sandbox` 会使用会话保存的配置。
- 传入与保存配置**匹配**的 `--sandbox <profile>` 是允许的。
- 传入与保存配置**不同**的 `--sandbox <profile>` 会**报错拒绝**——更改恢复会话的
  沙箱是安全陷阱（可能扩大原本应受限会话的访问范围，或破坏依赖更宽访问范围的会话）。
  要使用不同配置，请新建会话。

新会话的配置解析顺序：

1. 显式的 `--sandbox <profile>` 标志或 `GROK_SANDBOX` 环境变量
2. 配置中的 `[sandbox] profile`
3. `off`（无沙箱）

---

<a id="platform-support"></a>
## 平台支持

| 平台    | 机制     | 最低版本                  |
| ------- | -------- | ------------------------- |
| Linux   | Landlock | Kernel 5.13 or later      |
| macOS   | Seatbelt | macOS (all versions)      |

如果沙箱无法应用（例如内核不受支持或缺少 entitlement），Grok 会记录警告并在不强制
执行的情况下继续。例外是显式请求的**自定义配置**：在 **macOS 和 Linux 两个平台**上，
如果它无法应用（未知配置、格式错误的 `sandbox.toml`，或——在 Linux 上——非空 `deny`
所需的 `bubblewrap` 不可用），Grok 会拒绝启动，而不是暴露被拒绝的路径。

---

## 网络限制

在 Linux 上，带有 `restrict_network` 的配置通过 seccomp 阻止**子进程**（bash 命令、
脚本）访问网络。在 macOS 上，网络阻止为空操作。在进程内发起 HTTP 请求的内置工具
（网页搜索、LLM API 调用）始终不受影响——智能体需要网络才能运行。

实际效果在 Linux 上意味着：

- `web_search`、`web_fetch` 和 LLM API 始终可以访问网络
- 启用 `restrict_network` 时，`curl`、`wget` 和 `npm install` 等 `bash` 命令会被阻止

---

<a id="shell-environment-policy"></a>
## Shell 环境策略

沙箱控制子进程能够访问的文件和网络。顶层 `[shell_environment_policy]` 表控制它继承
哪些环境变量，因此模型运行的工具命令无法读取碰巧存在于 Shell 环境中的秘密。

```toml
[shell_environment_policy]
inherit = "core"                 # all (default) | core | none
ignore_default_excludes = false  # also drop *KEY* / *SECRET* / *TOKEN*
exclude = ["ACME_*", "CI_*"]     # drop these names
include_only = ["PATH", "HOME"]  # if set, keep only these names
set = { MY_FLAG = "1" }          # force these values
```

Grok 按以下顺序构建子进程环境：从 `inherit` 开始（`all` 保留全部内容，`core` 保留
`PATH` 和 `HOME` 等一小组平台变量，`none` 从空环境开始）；除非
`ignore_default_excludes = true`，否则删除内置秘密模式 `*KEY*`、`*SECRET*` 和
`*TOKEN*`；删除匹配 `exclude` 的变量；应用 `set`；当 `include_only` 非空时，仅保留
匹配的名称。模式是不区分大小写的 glob（`*`、`?`）。

默认值（`inherit = "all"`、`ignore_default_excludes = true`）保持环境不变，因此在
配置策略前不会有任何变化。在非持久后端中，策略也会筛选从登录 Shell 捕获的变量，
因此 `.rc` 文件中的导出无法绕过 `exclude` 或 `include_only` 偷渡秘密。持久 Shell
是一个例外：它会将策略应用于基础环境，但 `.rc` 文件在登录期间导出的变量会从快照
重放而不会重新筛选，因此请避免在那里的 Shell 启动文件中放置秘密。此强制机制覆盖
macOS、Linux 和 Windows 上的 bash 工具及终端。

---

## 事件日志

沙箱事件会记录到 `~/.grok/sessions`，以便调试。事件包括：

- 已应用的配置（配置名称、时间戳）
- 违规行为（尝试访问被拒绝的路径）

---

## 何时使用沙箱模式

**以下情况使用 `workspace`：**

- 在自己的项目上工作，并希望获得基本的写入保护
- 在共享环境中运行，并希望限制更改范围

**以下情况定义带 `deny` 列表的自定义配置：**

- 需要在基础配置之上阻止特定文件（例如 `.env` 或凭据路径）
- 需要覆盖 `bash`、`grep` 和子智能体的内核强制，而不仅是 `read_file` 工具

**以下情况使用 `read-only`：**

- 审查不信任的代码
- 无意外修改风险地探索代码库
- 运行代码分析或审计

**以下情况使用 `strict`：**

- 分析不受信任或第三方代码
- 在安全敏感环境中运行
- 需要最大隔离

**以下情况跳过沙箱：**

- 智能体需要安装依赖（`npm install`、`pip install`）
- 智能体需要修改工作目录之外的文件
- 你正在受信任的环境中工作，并希望获得最大灵活性

---

## 权衡

| 方面     | 不使用沙箱              | 使用沙箱                    |
| -------- | ----------------------- | --------------------------- |
| 安全性   | 智能体拥有完整系统访问权 | 智能体受配置规则限制         |
| 能力     | 可以执行任何操作         | 能力受配置限制               |
| 性能     | 无额外开销               | 开销可忽略                   |
| 恢复     | 必须信任智能体           | 内核强制执行边界             |

沙箱在操作系统级别强制限制——Linux 上通过 Landlock 或挂载命名空间，macOS 上通过
Seatbelt——而不是使用独立 VM。
