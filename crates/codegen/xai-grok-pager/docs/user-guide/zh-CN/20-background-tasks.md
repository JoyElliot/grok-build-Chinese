<a id="background-tasks-and-monitoring"></a>
# 后台任务与监控

Grok 可以运行长时间进程而不会阻塞对话。本文介绍后台命令、`/loop` 命令、`monitor` 工具以及调度器。

---

<a id="background-commands"></a>
## 后台命令

在 `run_terminal_command` 工具中设置 `background: true`，即可在后台运行命令。工具会立即返回任务 ID；使用 `get_command_or_subagent_output` 获取输出。

<a id="how-it-works"></a>
### 工作原理

1. 代理调用 `run_terminal_command`，并设置 `background: true`。
2. 命令在后台启动。
3. 代理收到供后续引用的 `task_id`。
4. 命令完成时，对话中会出现通知。

<a id="getting-output"></a>
### 获取输出

使用 `get_command_or_subagent_output` 检查后台命令或子代理。`task_ids` 必须是列表；
单个 ID 也要放在仅含一项的数组中，最多可传 20 个：

- 省略 `timeout_ms` 或传入 `0`，会立即返回非阻塞快照。
- 传入正数会等待任务完成；多个 ID 会等到**全部**完成。

正数 `timeout_ms` 最长限制为 **1 小时**（`3600000` 毫秒）。传输层超时时间更短的
宿主可以设置 `GROK_MAX_WAIT_BLOCK_MS`（纯毫秒数；无法解析时保留默认值）。

等待返回时如果子任务仍在运行，请让它继续，不要终止它或要求它停止。任务完成后会
自动唤醒父代理；只有需要新快照时才再次查询。

<a id="killing-background-tasks"></a>
### 终止后台任务

使用 `kill_command_or_subagent(task_id)` 终止正在运行的后台任务或子代理。该工具向 Shell 进程发送 SIGTERM，然后发送 SIGKILL；向子代理发送 Cancel 和 Shutdown。如果任务已被终止或已经退出，工具都会报告成功。

<a id="common-use-cases"></a>
### 常见用例

- **开发服务器**：启动开发服务器后继续编写代码
- **测试套件**：在后台运行测试，同时处理修复
- **构建进程**：启动构建，稍后再检查结果
- **长时间编译**：开始编译，同时继续其他工作

---

<a id="send-a-running-task-to-the-background"></a>
## 将正在运行的任务发送到后台

在交互式 TUI 中，按 `Ctrl+B` 可将正在前台运行的命令发送到后台。这是唯一用于后台化的快捷键。以下情况可以使用：

- 命令耗时超出预期。
- 你想在命令运行时向代理询问其他事情。
- 进程启动后才发现它会长时间运行。

任务会继续运行，完成时你会收到通知。

---

<a id="the-loop-command"></a>
## `/loop` 命令

`/loop` 按固定间隔重复运行提示。它适合轮询任务、周期性检查和持续监控。

<a id="syntax"></a>
### 语法

```
/loop [interval] <prompt>
```

间隔格式支持：

| 格式 | 示例 | 描述        |
| ------ | ------- | ------------------ |
| `Ns`   | `60s`   | 每 N 秒（最短 60 秒） |
| `Nm`   | `5m`    | 每 N 分钟    |
| `Nh`   | `2h`    | 每 N 小时      |
| `Nd`   | `1d`    | 每 N 天       |

<a id="examples"></a>
### 示例

```
/loop 5m 检查测试套件是否通过，并报告任何失败
/loop 2h 汇总自上次检查以来的新提交
/loop 60s 检查 localhost:3000 上的开发服务器是否响应
```

<a id="behavior"></a>
### 行为

- 创建时提示会立即触发一次，然后按指定间隔重复
- 每次触发都会创建一个新的代理回合
- 周期性任务在 7 天后自动过期
- 同时最多可激活 50 个已调度任务

---

<a id="the-monitor-tool"></a>
## `monitor` 工具

`monitor` 工具会从长时间运行的脚本中流式传输事件。每一行输出都会成为对话中的通知。`monitor` 是 `/loop` 的流式对应工具：周期性检查使用 `/loop`，实时事件流使用 `monitor`。

<a id="how-it-works-1"></a>
### 工作原理

1. 提供 Shell 命令（`command`）以及会显示在每条通知中的简短 `description`。
2. Grok 将命令的 stdout 和 stderr 合并到同一个输出文件。
3. 该文件中的每一新行都会成为发送到对话的通知。
4. 监控会持续运行，直到命令退出或你停止它。

<a id="script-guidelines"></a>
### 脚本指南

- **管道中始终使用 `grep --line-buffered`。** 否则管道缓冲会让事件延迟数分钟。
- **在轮询循环中处理瞬时失败**（`curl ... || true`）。一次请求失败不应停止监控。
- **使用选择性过滤器。** 每一行都会变成消息，因此绝不要直接传递原始日志。
- **让轮询间隔匹配来源。** 远程 API 使用 30 秒或更长的间隔以遵守速率限制，本地检查使用 0.5 到 1 秒。
- **stdout 和 stderr 都会生成事件。** 将不想作为事件的输出重定向（例如追加 `2>/dev/null`），或将其过滤掉。

<a id="examples-1"></a>
### 示例

```bash
# 监视日志文件中的错误
tail -f /var/log/app.log | grep --line-buffered "ERROR"

# 监视目录中的文件更改
inotifywait -m --format '%e %f' /watched/dir

# 轮询 GitHub 获取新的 PR 评论
last=$(date -u +%Y-%m-%dT%H:%M:%SZ)
while true; do
  now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  gh api "repos/owner/repo/issues/123/comments?since=$last" \
    --jq '.[] | "\(.user.login): \(.body)"'
  last=$now; sleep 30
done
```

<a id="persistent-monitors"></a>
### 持久监控

对需要在整个会话生命周期内运行的监控设置 `persistent: true`：

- PR 监控
- 日志跟踪
- CI 状态监视

使用 `kill_command_or_subagent(task_id)` 停止持久监控。

<a id="volume-control"></a>
### 音量控制

如果监控生成的事件过多，Grok 会自动停止它。发生这种情况时，用更严格的过滤器重新启动监控。优先使用 `grep --line-buffered`、`awk`，或只发出你关心的事件的包装脚本。

---

<a id="the-scheduler"></a>
## 调度器

调度器提供用于创建周期性任务的底层 API。`/loop` 是调度器的便捷封装。

<a id="scheduler_create"></a>
### scheduler_create

创建已调度任务：

| 参数        | 描述                                              |
| ---------------- | -------------------------------------------------------- |
| `interval`       | 运行频率：`"5m"`、`"2h"`、`"1d"`、`"60s"`       |
| `prompt`         | 每次触发时执行的提示文本                  |
| `fire_immediately`| 创建时除按间隔触发外立即触发（默认：`false`） |
| `recurring`      | 重复（默认：`true`）或只触发一次（`false`）          |
| `durable`        | 跨会话持久化（默认：`false`）               |

<a id="scheduler_list"></a>
### scheduler_list

列出所有活动的已调度任务，以及各自的 ID、提示、间隔和下次触发时间。

<a id="scheduler_delete"></a>
### scheduler_delete

按 ID 取消已调度任务。如果找到并移除了该任务则返回成功。

---

<a id="the-tasks-pane"></a>
## 任务窗格

在交互式 TUI 中，按 `Ctrl+G` 切换任务窗格。该窗格在单一视图中列出：

- 正在运行的子代理及其进度
- 活动的后台任务及其状态
- 监控和 `/loop` 任务，每项都有实时行数徽标
- 每个条目的任务 ID

若要切换提示队列，请按 `Ctrl+;`。

---

<a id="the-still-running-status-line"></a>
## 仍在运行状态行

只要代理看起来处于空闲状态而后台工作仍在运行——回合之间，或回合因等待用户可中断的操作而阻塞时——提示框上方就会显示持久状态行：

```
◎ 1 command · 2 monitors · 1 loop · 1 subagent still running
```

它会统计正在运行的后台命令、监控、已调度的 `/loop` 任务和后台子代理，并在各项完成时实时更新。任何一项都可能唤醒代理开始新的回合（命令和子代理在完成时，监控在事件发生时，循环在计时器到点时），因此只要仍有未完成项，提示就会保留。运行计数只存在于这条状态行：完成会以一枚单独的 “Task completed” 芯片出现在转录中，而 “Worked for” 标记保持普通样式——转录不会重复或再次陈述运行计数。

当回合正在等待后台工作（阻塞在 `get_command_or_subagent_output` 调用中）时，状态行会添加提示，说明输入会立即接管：

```
◎ 1 command still running · send a message to interrupt
```

当代理正在等待某项没有活动计数器的事情（休眠，或工作已经完成）时，也会显示相同提示 `◎ waiting · send a message to interrupt`。发送消息会中断等待，并立即运行你的消息。整个过程中转录保持通常形态：回合结束时只有一个 “Worked for” 标记。完成事件唤醒代理并得到回复时，该回复会有自己的 “Worked for” 标记；若代理静默回应唤醒，则转录中不会留下痕迹——但静默唤醒失败时会出现 “Turn failed” 行，因此常驻指令不会在无形中停止执行。

---

<a id="use-cases-and-patterns"></a>
## 用例与模式

<a id="dev-server--coding"></a>
### 开发服务器 + 编码

在后台启动开发服务器并继续编码：

```
在后台用 `npm run dev` 启动开发服务器，然后实现登录表单。
```

代理会使用 `background: true` 运行开发服务器，并继续编写代码。服务器启动后，你会看到通知。

<a id="continuous-test-monitoring"></a>
### 持续测试监控

```
/loop 5m 运行测试套件，并报告自上次运行以来的新失败
```

代理每 5 分钟运行测试，并只报告新出现的失败。

<a id="log-monitoring"></a>
### 日志监控

使用 `monitor` 监视特定事件：

```
监视应用日志中的 ERROR 和 WARN 条目。使用：
tail -f /var/log/app.log | grep --line-buffered -E "ERROR|WARN"
```

每个错误或警告都会作为通知出现在对话中。

<a id="ci-pipeline-watching"></a>
### CI 管道监视

```
/loop 2m 检查此 PR 的 GitHub Actions 运行状态，并在完成时报告。
```

---

<a id="best-practices"></a>
## 最佳实践

- **对一次性长命令使用 `background`**（构建、测试套件、启动服务器）
- **对周期性检查使用 `/loop`**（CI 状态、测试运行、健康检查）
- **对实时事件流使用 `monitor`**（跟踪日志、监视文件）
- **对延迟的一次性任务使用 `scheduler_create` 和 `recurring: false`**
- **保持监控过滤器严格**——原始日志流优先使用 `grep --line-buffered`
- **不要在普通命令中使用 sleep 循环**进行轮询——改用带 `timeout_ms` 的 `get_command_or_subagent_output`
- **设置合理的轮询间隔**——远程 API 使用 30 秒以上以避免速率限制，本地检查可使用更短间隔
