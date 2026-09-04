# 会话管理

Grok 会自动将每段对话保存到磁盘。无论你是在 TUI、无头模式中工作，还是通过
agent stdio 工作，Grok 都会将这次交互记录为一个会话。你可以恢复、回退或压缩
会话。本文件介绍如何管理会话。

---

## 什么是会话

会话是一个包含完整历史记录的持久化对话，其中包括：

- 所有用户提示和智能体回复
- 工具调用及其结果
- TODO/任务列表状态
- 用于撤销后续轮次的回退点
- Token 用量和轮次计数
- 子智能体会话（启用时）

会话由唯一的会话 ID 标识（Grok 生成时使用 UUIDv7；客户端也可以通过 `-s` 提供
自己的 ID），并存储在 `~/.grok/sessions/` 下。设置 `GROK_HOME` 可覆盖基础目录；
未设置时，Grok 使用 `~/.grok`。

---

## 存储布局

Grok 为每个会话使用单独的目录，并按工作目录分组。它会对工作目录进行 URL 编码，
用编码结果命名分组。当编码后的名称超过 255 字节时，它会改用 slug 加哈希，并将
原始路径记录在分组内的 `.cwd` 文件中。

```
~/.grok/sessions/<encoded-cwd>/<session-id>/
  summary.json            # 元数据：摘要/标题、时间戳、模型 ID、消息计数
  updates.jsonl           # ACP 会话更新流（对话 + 工具调用）
  chat_history.jsonl      # 发送给模型的原始聊天消息
  plan.json               # TODO/任务列表状态
  rewind_points.jsonl     # 用于 /rewind 撤销的回退点
  signals.json            # 会话信号（Token 用量、工具/轮次计数器）
  feedback.jsonl          # 用户反馈和评分
  compaction_checkpoints/ # 压缩时保存的状态（手动或自动）
  subagents/              # 每个子智能体的元数据（meta.json）；子会话位于常规会话树中
```

`summary.json` 是索引条目。它记录会话摘要和生成的标题、模型 ID、创建和更新时间
戳、消息计数，以及分叉或恢复会话的父会话引用。它还会记录最近一轮摘要和会话回顾，供列表界面展示。`updates.jsonl` 是驱动 `/resume` 和会话恢复的权威对话日志。逐轮令牌与费用总计可通过 `grok-zh usage` 查看。

### 会话标题

dashboard 和 `/resume` 中显示的会话标题会根据对话自动生成。提示框边框仅在手动执行 `/rename` 后显示标题；暂存草稿时，标题会与 `Stashed`（已暂存）标记并列显示。首次提示发送后系统会立即开始生成标题，确保会话始终有标题；在前几个轮次中，还会根据完整对话重新生成几次，之后便固定下来。这样标题能从含糊的首条提示逐渐反映会话的真实主题，同时在后续保持稳定，便于识别。手动执行 `/rename` 始终优先：一旦重命名，自动生成就不会覆盖它。使用 `/rename --auto` 可重新交由系统自动生成标题。

---

## 开始和结束会话

### 新建会话

每次启动时，TUI 都会创建一个新会话。要在当前会话中显式开始新会话：

```
/new
```

这会清除当前上下文并开始新对话。别名：`/clear`。

### 退出

结束会话并退出 Grok：

```
/quit
```

别名：`/exit`。若要离开当前会话但继续留在 Grok 中，请使用 `/home` 返回欢迎屏幕。

### 删除当前会话

```
/delete
```

确认后，会永久删除会话历史。它会返回欢迎屏幕；如果你是从 dashboard 打开会话，
则返回 dashboard。在 `/resume` 或欢迎屏幕的会话列表中，按 `d` 再按 `y`。在
[智能体 dashboard](23-dashboard.md) 中，按两次 `Ctrl+X`（或悬停在 `[✗]` 上）即可
永久删除。

---

## 恢复会话

### 从 TUI 恢复

使用 `/resume` 命令浏览并恢复以前的会话：

```
/resume
```

这会打开会话选择器，列出当前工作区的最近会话。选择一个会话即可恢复。该命令不
接受参数。

在选择器中输入内容会按标题筛选列表，同时搜索你输入时的对话内容；内容匹配项会
显示在“扩展搜索结果”标题下。按 `Ctrl+/` 可立即搜索，不必等待短暂的延迟。

对于此 pager 中实时的顶层会话（父会话和分叉会话），若要切换、重命名、查看、
派发或关闭，请使用[智能体 dashboard](23-dashboard.md)：`/dashboard`（别名
`/sessions`、`/agents-dashboard`）或 `Ctrl+\`。

### 从命令行恢复

按 ID 或标题恢复指定会话：

```bash
grok-zh --resume <session-id-or-title>
```

不是会话 ID 的值会与当前目录的会话标题匹配，忽略字母大小写（简单的小写比较），
这在使用 `/rename` 后很方便。如果多个会话共享该标题，单个手动重命名的会话会优先
于自动生成的重复项；否则命令会报错并列出匹配的 ID。UUID 形状的值始终按会话 ID
处理，从不按标题处理。脚本应优先使用 ID。

不带值运行 `grok-zh --resume`，会恢复当前目录最近的会话。

### 从欢迎屏幕恢复

启动 `grok-zh` 时，欢迎屏幕会列出当前目录的最近会话。选择一个即可恢复。

---

## 分叉和重命名会话

### 分叉

将当前会话分支为一个从对话副本开始的同级智能体：

```
/fork [--worktree|--no-worktree] [directive]
```

可选的 `directive` 会设置新会话的第一条提示。使用 `--worktree` 或 `--no-worktree`
选择分叉是否在新的 git worktree 中运行；省略两者则每次询问。此版本不支持
`--at <turn>` 标志。

### 重命名

重命名当前会话的标题：

```
/rename <title>
```

别名：`/title`。

---

## `/rewind` 命令

`/rewind`（别名 `/undo`）会将对话回退到较早轮次，并丢弃后续轮次。该轮次之后
产生的文件更改会原样留在磁盘上。

```
/rewind
/undo
```

运行 `/rewind` 或 `/undo` 时（或者在提示为空、有对话消息且空闲时，于 800 毫秒内
按 **Esc Esc**），Grok 会：

1. 显示回退点列表（每条用户提示一个）
2. 让你选择要回退到的点
3. 将对话历史截断到该点

开启**回退前确认**时（`/settings` 中的默认设置），每次选择都会要求确认（Yes /
Yes, and don't ask again / No）。“Yes, and don't ask again”会关闭该设置。关闭
设置后，选择会立即执行。

**重要：** `/rewind` 不会恢复磁盘上的文件，只会截断对话历史。

---

## `/compact` 命令

`/compact` 会压缩对话历史，以节省上下文窗口空间。在早期消息不再相关的长会话中
使用它。

```
/compact
/compact [context]
```

可选的 `context` 参数允许你提供关于压缩期间应保留哪些内容的附加说明。

### 自动压缩

当上下文窗口接近上限时，Grok 会自动压缩对话。自动压缩触发时你会看到通知。模型
配置中的 `context_window` 设置控制达到此阈值的时机。

---

## `/session-info` 命令

查看当前会话的详细信息：

```
/session-info
```

它会显示：

- 会话标题（设置后）
- Shell 版本
- 身份验证方式（OAuth 与 API key；API-key 会话还会建议使用 `grok-zh login` 登录 SuperGrok）
- 会话 ID
- 工作目录
- 模型（coding 模型还会显示模型哈希）
- API 后端和 sandbox 配置（设置后）
- 上下文窗口用量（已用和总 Token 数，以及使用百分比）

---

## 无头模式下的会话管理

在无头模式中，你通过命令行标志管理会话：

```bash
# 每次都新建会话（默认）
grok-zh -p "Hello"

# 按 ID 或标题恢复现有会话（不存在时出错）
grok-zh -p "Continue where we left off" -r <session-id-or-title>

# 继续当前目录中最近的会话
grok-zh -p "What were we doing?" -c
```

在无头模式中，使用 `-r`/`--resume` 恢复现有会话（会话不存在时出错），或使用
`-c`/`--continue` 继续当前目录最近的会话。非 ID 值会与当前目录的会话标题匹配，
忽略字母大小写（重复项中唯一的手动重命名匹配项优先；其余重复项会带 ID 报错；
UUID 形状的值始终走 ID 路径）——脚本应将 JSON 输出中的会话 ID（见下文）传给 `-r`。

`-s`/`--session-id` 仅用于使用 **UUID** 创建新会话（值不是 UUID，或该 ID 在目标
会话目录下已有会话时会出错）。它不会恢复现有会话——那是旧的隐藏 upsert 行为；
请改用 `-r`/`-c`。只有同时传入 `--fork-session` 时，才能将 `-s` 与 `-r`/`-c`
组合使用（将历史分叉到新的 ID；可选的 `-s` 用于命名子会话 UUID）。这与 Claude
Code 的防覆盖模型一致（客户端在写入 cwd 下预检；顺序使用可靠，并发使用相同 ID
则属于尽力而为）。

要读回会话 ID，请请求 JSON 输出：

```bash
grok-zh -p "Hello" --output-format json | jq -r '.sessionId'
```

---

## Agent stdio 会话管理

使用 ACP 构建时，会话通过协议方法管理：

```typescript
// 创建新会话
const { sessionId } = await connection.request("session/new", {
  cwd: "/path/to/project",
  mcpServers: [],
});

// 加载现有会话
await connection.request("session/load", {
  sessionId: "existing-session-id",
  cwd: "/path/to/project",
  mcpServers: [],
});

// 修改实时选项（model 或 reasoning_effort）。
// session/new 和 session/load 已返回带类型的 configOptions 列表。
await connection.request("session/set_config_option", {
  sessionId,
  configId: "model",
  value: { value: "grok-4.6" },
});
```

智能体会自动持久化所有会话更新。客户端可以重新连接，并按 ID 加载以前的会话。选项 ID、值结构和主导模式监听机制详见 [Agent 模式](15-agent-mode.md#session-config-options)。

---

## `grok-zh sessions` 子命令

从命令行列出或搜索会话。`grok-zh sessions` 需要一个子命令：

```bash
# 列出当前目录的最近会话
grok-zh sessions list

# 限制结果数量（默认 20）
grok-zh sessions list --limit 50

# 按关键词搜索会话（匹配标题和提示）
grok-zh sessions search "rate limit"
```

`grok-zh sessions list` 显示当前工作目录的会话，并按 worktree 标签分组。每行列出
会话 ID、创建和更新时间、来源状态以及摘要。`grok-zh sessions search` 会将本地
SQLite 索引与远程结果合并。

---

<a id="the-grok-usage-subcommand"></a>
## `grok-zh usage` 子命令

打印某个会话持久保存的令牌与费用用量。请使用该命令，不要直接读取会话文件：

```bash
# 会话总计和每个已记录轮次
grok-zh usage <session-id>

# 指定一个轮次
grok-zh usage <session-id> 3
```

输出为 JSON，包含 `sessionId`、`updatedAt`、`session` 和 `turns`。指定轮次时使用同一封装结构，但 `turns` 只有一个元素。会话总计覆盖完整对话，包括恢复或分叉继承的历史。`costUsdTicks` 以每美元 10¹⁰ tick 计（除以 `1e10` 即美元）；不存在的轮次号会报错。TUI 中的交互式额度与计费入口仍是 `/usage`。

---

## Worktree 会话

与子智能体或会话分叉协作时，Grok 可以为每个会话创建隔离的 git worktree。每个
worktree 都拥有工作目录的独立副本，因此一个会话中的文件更改不会影响另一个会话。

Worktree 会话在内部通过 `x.ai/git/worktree/*` 扩展方法管理。关键操作：

- **Create**：为隔离会话创建新的 worktree
- **Apply**：将 worktree 更改合并回主工作目录
- **Remove**：会话结束后清理 worktree

使用 `grok-zh -w -r <session-id>` 在新的 worktree 中恢复会话。

### 检查磁盘用量

`grok-zh du`（别名：`grok-zh disk-usage`）报告 grok 主目录（`~/.grok`）在磁盘上
占用的空间。它会按从大到小列出每个顶层目录，然后列出每个 worktree 的大小、类型、
年龄、标签和路径。注册表未跟踪的 worktree 会显示为 `untracked`。传入 `--json` 可
获取相同报告的机器可读形式。

```text
Disk usage for ~/.grok
    412.3 GB  worktrees
      1.2 GB  sessions
    412.0 MB  (top-level files)
    413.9 GB  total
  Worktree clones share storage with their source, so the total can exceed real disk use.

Worktrees
        SIZE  TYPE                AGE        LABEL  PATH
    380.0 GB  session             12d ago    my-fix ~/.grok/worktrees/xai/worktree-abc
     32.3 GB  untracked (session) 40d ago           ~/.grok/worktrees/xai/worktree-old

To reclaim space, run `grok-zh worktree gc --max-age 7d --dry-run`, then the same command without `--dry-run`. Without `--max-age`, gc expires nothing.
Untracked rows are not in the registry, so gc never visits them. Remove one with `grok-zh worktree rm --dry-run <path>`, then without `--dry-run`.
```

`AGE` 是 `grok-zh worktree gc` 衡量的值：从 worktree 上次访问起经过的时间，或者从
创建起经过的时间（以较近者为准）。会话和智能体活动会更新它；留在目录中的 Shell
或编辑器不会更新它。未跟踪的 worktree 没有注册表条目，因此其年龄取自其下方最新
文件。

大小在 Unix 上是物理块计数，在其他系统上是逻辑文件大小，与 `grok-zh worktree show`
报告的内容一致。Worktree 克隆与源共享存储，每个副本都完整计数，因此总量可能超过
`du -sh` 和实际正在使用的空间。当总量超过卷上的已用空间时，报告会说明这一点。
`--json` 会将相同数字放在 `volume_capacity_bytes` 和 `volume_available_bytes` 中。

报告只测量一个文件系统，即承载 grok 主目录的文件系统。其他文件系统上的目录不计入
总量，并计入 `other_filesystem_dirs`；其 worktree 行的大小显示 `-`（在 `--json` 中为
`null`）。指向目录的顶层符号链接（例如迁移后的 `worktrees`）会计入
`unfollowed_dir_symlinks`；其目标不计入总量，但其下方的行仍会计算大小。报告无法读取
的目录和无法获取状态的条目分别计入 `unreadable_dirs` 和 `unstatable_entries`。运行
`RUST_LOG=debug grok-zh du` 可列出它们的名称。

`--json` 中的每个 worktree 行还带有 Unix 秒表示的 `created_at`、`last_accessed_at`
和 `last_modified_at`，以及 `repo_name` 和 `git_ref`。未跟踪行的注册表字段为 `null`。
`git_ref` 是注册 worktree 时记录的分支，而不是当前检出的分支。

注册表不可用时，每一行都会显示为 `untracked`，报告会列出原因。`--json` 的 `registry`
字段携带相同的值：`read`、`absent`、`busy`、`unopenable` 或 `corrupt`。`busy` 表示
注册表被另一个进程占用，应重试。`unopenable` 表示权限或 I/O 问题，应检查文件。
`corrupt` 是唯一需要删除的情况：删除报告所列的文件，然后运行
`grok-zh worktree db rebuild`。

要回收空间，运行 `grok-zh worktree gc --max-age 7d`，它会删除超过指定年龄的已跟踪
worktree。不带 `--max-age` 时 gc 不会使任何内容过期，并且只访问注册表跟踪的 worktree。
使用 `grok-zh worktree rm <path>` 删除未跟踪的 worktree。两个命令都接受 `--dry-run`
并报告将执行的操作：gc 会统计将删除的 worktree 数量，`rm` 会列出路径。两者都不会
检查工作树中是否有未提交或未推送的工作，因此请先阅读预览。

---

## 会话存储详情

### 持久化格式

Grok 将对话存储为换行分隔的 JSON（JSONL）。`updates.jsonl` 中的每一行都是独立的
ACP 会话更新事件。这种格式支持：

- 增量写入（会话期间仅追加）
- 高效的流式读取（用于恢复会话）
- 易于调试（每一行都是有效 JSON）

较小的状态文件——`summary.json`、`plan.json` 和 `signals.json`——使用普通 JSON 而
不是 JSONL。JSONL 是会话内容的事实来源；`grok-zh sessions search` 还会在会话标题
和提示上维护本地 SQLite FTS5 索引，以便快速关键词搜索。

### 会话元数据

除其他字段外，`summary.json` 还记录：

- `info` —— 会话 ID 和工作目录
- `session_summary` 和 `generated_title` —— 会话摘要及模型生成的标题
- `created_at` 和 `updated_at` —— 创建和最后更新时间戳
- `num_messages` 和 `num_chat_messages` —— 更新和聊天消息计数
- `current_model_id` —— 当前使用的模型
- `parent_session_id` —— 分叉或恢复时的源会话
- `agent_name` —— 上次保存会话时处于活动状态的智能体定义

### 磁盘用量

长会话中，会话历史（`updates.jsonl`、`chat_history.jsonl`）占据主要磁盘空间。使用
`/compact` 可减小历史记录大小。

---

## 提示

- 当前上下文不再相关时，使用 `/new` 开始新会话。
- 在长会话中主动使用 `/compact`，保持上下文窗口高效。
- 使用 `/rewind` 撤销错误；它会将对话回退到较早轮次（被移除轮次产生的文件更改仍保留在磁盘上）。
- 在无头模式中，从 JSON 输出捕获 `sessionId` 并传给 `-r`，以构建能维持上下文的多步自动化。
- 检查 `/session-info`，了解上下文窗口的使用量。
