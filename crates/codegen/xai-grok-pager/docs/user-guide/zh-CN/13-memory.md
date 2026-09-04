<a id="cross-session-memory"></a>
# 跨会话记忆

记忆让 Grok 能够回忆早期会话中的事实、决策和模式。Grok 会为你保存的信息建立索引并自动搜索，因此新会话可以复用相关上下文。

---

<a id="what-is-memory"></a>
## 什么是记忆？

没有记忆时，每个 Grok 会话都会从头开始：模型不了解之前的会话。启用记忆后，Grok 可以：

- 回忆你之前说明过的项目约定。
- 复用有效的调试步骤。
- 在后续会话中延续架构决策。
- 避免再次询问它已经知道答案的问题。

记忆目前处于实验阶段，默认禁用。

---

<a id="enabling-memory"></a>
## 启用记忆

<a id="per-session-flag"></a>
### 每会话标志

```bash
grok-zh --experimental-memory
```

<a id="environment-variable"></a>
### 环境变量

```bash
export GROK_MEMORY=1
grok-zh
```

<a id="config-file-persistent"></a>
### 配置文件（持久化）

```toml
# ~/.grok/config.toml
[memory]
enabled = true
```

<a id="force-disable"></a>
### 强制禁用

即使其他设置启用了记忆，也可以将其禁用：

```bash
grok-zh --no-memory
```

或者：

```bash
export GROK_MEMORY=0
```

`--no-memory` 标志的优先级绝对最高，始终会禁用记忆。

<a id="mid-session-toggle"></a>
### 会话中途切换

无需重启，即可在会话期间打开或关闭记忆：

```
/memory on
/memory off
```

该切换仅作用于当前会话，不会持久化到 `config.toml`。关闭后会移除对记忆工具的访问，但会保留磁盘上的现有文件。打开后会重新初始化记忆存储并注册记忆工具。

也可以在 `/memory` 模态窗口中按 `t` 切换。

<a id="priority-order"></a>
### 优先级顺序

1. `--no-memory` CLI 标志（始终禁用）
2. `--experimental-memory` CLI 标志（启用）
3. `GROK_MEMORY` 环境变量：`1`/`true` 启用，`0`/`false` 禁用
4. config.toml 中的 `[memory]` 节
5. 默认值：禁用

---

<a id="how-memory-is-stored"></a>
## 记忆如何存储

记忆以 Markdown 文件形式存储在 `~/.grok/memory/` 下：

| 位置 | 范围 | 说明 |
|----------|-------|-------------|
| `~/.grok/memory/MEMORY.md` | 全局 | 适用于所有项目的事实 |
| `~/.grok/memory/<project-slug>-<hash8>/MEMORY.md` | 工作区 | 项目专用的约定和上下文 |
| `~/.grok/memory/<project-slug>-<hash8>/sessions/` | 会话 | 每个会话的摘要和日志 |

Grok 会在每个工作区目录名后附加仓库身份的短哈希。若该目录是带有 `origin` 远程的 Git 仓库，身份采用 `org/repo` 形式的 `origin` 远程；否则使用目录路径。由于同一仓库的克隆和工作树共享 `origin` 远程，它们也会共享一个记忆目录。

SQLite 索引支持对全部记忆文件的混合搜索：
- **FTS5** 提供用于关键词匹配的全文搜索。
- **vec0** 提供用于语义相似度的向量搜索。向量搜索是可选的，需要嵌入。

---

<a id="automatic-saves"></a>
## 自动保存

会话结束时，Grok 会将结构化的元数据摘要保存到该会话的每日日志中。摘要包含：

- 消息计数（用户、助手和工具结果）。
- 主题：会话中最初几条实质性用户提示，最多五条。
- 会话日期和时间（UTC）。

Grok 从会话元数据生成摘要，不调用 LLM，也不会增加延迟。对于琐碎会话——实质性提示少于三条，或用户文本少于 50 字节——Grok 会跳过保存。

摘要不会记录工具使用情况、文件路径或 Shell 命令。会话 ID 是日志文件名的一部分。要关闭自动保存，请设置 `session.save_on_end = false`。如需更丰富地捕获决策、模式和推理，请使用 `/flush`。

---

<a id="saving-rich-knowledge-with-flush"></a>
## 使用 /flush 保存丰富知识

如需更丰富地捕获决策、模式、调试工作流和 API 发现，请在 TUI 中使用 `/flush`：

```
/flush
```

该命令会触发由 LLM 生成的摘要，提炼当前会话中最重要的内容，并将其写入带日期的会话日志。摘要会建立索引，以便未来会话搜索。

在以下情况下使用 `/flush` 保存重要上下文：
- 压缩之前（压缩会丢弃旧的会话轮次）
- 高效的调试会话结束时
- 发现重要模式或约定后

---

<a id="working-with-memory"></a>
## 使用记忆

<a id="remember"></a>
### Remember

让 Grok 记住某件事，它会将笔记追加到 `MEMORY.md` 文件中——项目专用内容写入工作区文件，跨项目偏好写入全局 `~/.grok/memory/MEMORY.md`：

```
> remember to always open PR links after pushing
```

Grok 会将条目以持久化陈述记录在有组织的标题下，例如 `## Preferences`、`## Project Context` 或 `## Debugging`。文件监视器会在下次记忆搜索时重新建立索引，因此新条目在当前会话中即可搜索。

也可以直接使用 `/remember` 命令保存笔记：

```
/remember always open PR links after pushing
```

不带文本运行 `/remember` 会进入记住模式，你输入的下一行将成为笔记。无论哪种方式，Grok 都会打开审阅面板显示笔记（可选的改写版本可用 `Tab` 切换）；只有确认后才会写入。保存后，Grok 会显示 `Memory saved to ~/.grok/memory/MEMORY.md`。

<a id="forget"></a>
### Forget

让 Grok 忘记某件事，它会查找并删除匹配的条目：

```
> forget the snake_case convention
```

Forget 采用尽力而为的方式：模型搜索记忆并移除匹配项。要保证删除，请直接编辑 `~/.grok/memory/` 下的文件并自行删除条目。要定位文件，请打开 `/memory` 浏览器并按 `y` 复制路径。

<a id="recall"></a>
### Recall

询问 Grok 记得什么：

```
> what do you remember?
```

Grok 会搜索全部记忆文件，并按来源汇总已知内容：全局偏好、项目专用知识和会话历史。使用 `/memory` 浏览原始文件。

<a id="direct-editing"></a>
### 直接编辑

可以直接编辑 `~/.grok/memory/` 下的记忆文件。文件监视器会在下次记忆搜索时重新建立索引。使用 `/flush` 立即保存当前会话，使用 `/dream` 将会话日志整理为有组织的主题。

---

<a id="browsing-memory-with-memory"></a>
## 使用 /memory 浏览记忆

`/memory` 命令会打开一个模态窗口，显示全部记忆文件：

```
/memory
```

文件按范围分组：
- **Global** —— 跨项目记忆（`MEMORY.md`）。
- **Workspace** —— 项目专用记忆（`MEMORY.md`）。
- **Sessions** —— 按时间倒序排列的每会话摘要。

模态窗口采用分栏布局：左侧是文件列表，右侧是只读内容预览。你在列表中移动时，预览会更新。

<a id="keyboard-shortcuts"></a>
### 键盘快捷键

| 键 | 操作 |
|-----|--------|
| `↑`/`↓` 或 `j`/`k` | 在文件列表中移动 |
| `PgUp`/`PgDn` | 跳转 10 个条目 |
| `/` | 过滤文件列表 |
| `y` | 将所选文件的路径复制到剪贴板 |
| `x` | 删除所选会话文件（再次按 `x` 确认） |
| `t` | 打开或关闭记忆 |
| `Ctrl+F` | 切换全屏 |
| `Esc` | 关闭模态窗口，或退出过滤模式 |

预览窗格为只读。使用鼠标滚轮或拖动滚动条滚动。只能删除会话文件，不能删除全局或工作区的 `MEMORY.md`。

当记忆模态窗口的内容区域少于 80 列时，窗口会隐藏预览窗格，仅显示文件列表。

也可以从命令面板打开 `/memory`。

---

<a id="memory-notifications"></a>
## 记忆通知

使用 `/remember` 保存笔记时，Grok 会在回滚区确认：

```
Memory saved to ~/.grok/memory/MEMORY.md
```

后台保存——flush、dream 和会话结束保存——会静默运行，不会发布回滚区消息。随时使用 `/memory` 浏览 Grok 已存储的内容。

---

<a id="dream-consolidation-with-dream"></a>
## 使用 /dream 整理记忆

`/dream` 命令会将零散的记忆片段整理为有组织的主题：

```
/dream
```

Dream 会把各个会话日志和记忆条目重组为连贯、去重的知识库，随时间推移减少噪声并提升搜索质量。`/dream` 要求已启用记忆。

<a id="auto-dream"></a>
### 自动 Dream

Dream 也会自动运行。默认情况下，Grok 会在启动时以及会话期间定期检查整理条件，并在经过足够时间且积累足够会话后运行一次 Dream：

```toml
[memory.dream]
enabled = true     # 运行自动整理（默认：true）
min_hours = 24     # 整理之间的最少小时数
min_sessions = 5   # 上次整理以来的最少会话数
# check_interval_secs 默认为 3600，因此每小时检查一次。
```

---

<a id="how-memory-affects-prompts"></a>
## 记忆如何影响提示

<a id="first-turn-injection"></a>
### 首轮注入

每个会话的第一轮中，Grok 会自动搜索与当前项目相关的记忆并将其注入上下文。这意味着 Grok 无需提醒即可带着早期会话的知识开始工作。

可以配置首轮注入：

```toml
[memory.initial_injection]
enabled = true     # 启用或禁用首轮注入
min_score = 0.0    # 可选的分数阈值；默认未设置，即不进行过滤
```

<a id="after-compaction"></a>
### 压缩之后

自动压缩后也会搜索记忆，以恢复可能被丢弃的相关上下文。

---

<a id="memory-search"></a>
## 记忆搜索

Grok 会自动搜索记忆，但你也可以在聊天中手动触发搜索：

```
Search memory for "auth middleware patterns"
Read my workspace MEMORY.md
```

模型可以使用两个记忆工具：
- `memory_search` —— 跨全部记忆的混合搜索（向量 + 全文）
- `memory_get` —— 按路径读取特定记忆文件

<a id="hybrid-scoring"></a>
### 混合评分

记忆搜索采用加权组合：
- **向量相似度**（语义）——权重：0.7
- **BM25 文本相似度**（关键词）——权重：0.3

结果会按最低分数阈值过滤（默认：0.35）。

<a id="source-weights"></a>
### 来源权重

每个记忆来源都有应用于其分数的权重乘数。所有来源默认均为 `1.0`，可在 `[memory.search.source_weights]` 下调整：

| 来源 | 权重 | 说明 |
|--------|--------|-------------|
| `workspace` | 1.0 | 项目专用记忆 |
| `session` | 1.0 | 会话日志 |
| `global` | 1.0 | 跨项目记忆 |

<a id="temporal-decay"></a>
### 时间衰减

会话记忆会随时间衰减，以便优先显示最近会话：

```toml
[memory.search.temporal_decay]
enabled = true           # 启用基于时间的衰减
half_life_days = 7.0     # 经过这么多天后分数减半
```

只有会话分块会衰减。全局和工作区记忆包含经过整理的长期知识，不受影响。

<a id="mmr-maximal-marginal-relevance"></a>
### MMR（最大边际相关性）

MMR 重排会惩罚重复结果，以提高多样性：

```toml
[memory.search.mmr]
enabled = false          # 选择启用多样性重排
lambda = 0.7             # 0.0 = 最大多样性，1.0 = 纯相关性
```

---

<a id="cli-commands"></a>
## CLI 命令

`grok memory` 命令从 Shell 管理记忆。它有一个子命令 `clear`：

```bash
# 清除工作区记忆（MEMORY.md、sessions/ 和 index.sqlite）。这是默认范围。
grok-zh memory clear

# 显式指定同一范围
grok-zh memory clear --workspace

# 清除全局 MEMORY.md
grok-zh memory clear --global

# 同时清除工作区和全局记忆
grok-zh memory clear --all

# 跳过确认提示（-y 是短形式）
grok-zh memory clear --yes
```

要从 Shell 编辑记忆，请直接在编辑器中打开文件，例如 `$EDITOR ~/.grok/memory/MEMORY.md`。

---

<a id="configuration-reference"></a>
## 配置参考

<a id="core-settings-memory"></a>
### 核心设置（`[memory]`）

| 键 | 默认值 | 说明 |
|-----|---------|-------------|
| `enabled` | `false` | 启用记忆 |
| `session.save_on_end` | `true` | 会话结束时写入元数据摘要 |
| `watcher.enabled` | `true` | 监视 `~/.grok/memory/` 的外部编辑并重新建立索引 |

<a id="index-settings-memory-index"></a>
### 索引设置（`[memory.index]`）

| 键 | 默认值 | 说明 |
|-----|---------|-------------|
| `max_chunk_chars` | `1600` | 分块的最大字符数 |
| `chunk_overlap_chars` | `320` | 分块之间重叠的字符数 |

<a id="embedding-settings-memory-embedding"></a>
### 嵌入设置（`[memory.embedding]`）

| 键 | 默认值 | 说明 |
|-----|---------|-------------|
| `provider` | `"api"` | 嵌入提供方（当前为 `"api"`） |
| `model` | *(provider default)* | 嵌入模型名称 |
| `dimensions` | `1024` | 嵌入向量维度 |

<a id="search-settings-memory-search"></a>
### 搜索设置（`[memory.search]`）

| 键 | 默认值 | 说明 |
|-----|---------|-------------|
| `max_results` | `6` | 最大搜索结果数 |
| `min_score` | `0.35` | 最低相关性分数 |
| `vector_weight` | `0.7` | 向量相似度权重 |
| `text_weight` | `0.3` | BM25 文本相似度权重 |

<a id="initial-injection-settings-memory-initial-injection"></a>
### 初始注入设置（`[memory.initial_injection]`）

| 键 | 默认值 | 说明 |
|-----|---------|-------------|
| `enabled` | `true` | 启用首轮记忆注入 |
| `min_score` | unset | 首轮结果的分数阈值。未设置时，Grok 不应用阈值，等同于 `0.0`。 |

<a id="dream-settings-memory-dream"></a>
### Dream 设置（`[memory.dream]`）

| 键 | 默认值 | 说明 |
|-----|---------|-------------|
| `enabled` | `true` | 启用自动 Dream 整理 |
| `min_hours` | `24` | 整理之间的最少小时数 |
| `min_sessions` | `5` | 上次整理以来的最少会话数 |
| `stale_lock_secs` | `3600` | 回收陈旧整理锁之前等待的秒数 |
| `check_interval_secs` | `3600` | 周期检查间隔（秒）。 |

<a id="flush-settings-compaction-memory-flush"></a>
### Flush 设置（`[compaction.memory_flush]`）

由于 Flush 是压缩行为，请在 `[compaction]` 而不是 `[memory]` 下配置它。

| 键 | 默认值 | 说明 |
|-----|---------|-------------|
| `enabled` | `true` | 启用压缩前的记忆 Flush |
| `soft_threshold_tokens` | `4000` | 触发 Flush 的压缩阈值前令牌余量 |
| `max_flush_write_chars` | `8000` | Flush 最多可写入记忆的字符数 |
| `flush_model` | unset | Flush 轮次使用的模型。未设置时，Grok 使用会话的主模型。 |
| `idle_timeout_secs` | unset | 后台 Flush 前的空闲秒数。未设置时，Flush 只在压缩前运行。 |
| `semantic_dedup_threshold` | unset | 对 Flush 内容去重的余弦相似度阈值。未设置时默认为 `0.92`。 |

<a id="pruning-settings-compaction-pruning"></a>
### 修剪设置（`[compaction.pruning]`）

由于修剪是压缩行为，请在 `[compaction]` 而不是 `[memory]` 下配置它。

| 键 | 默认值 | 说明 |
|-----|---------|-------------|
| `enabled` | `true` | 启用工具结果修剪 |
| `keep_last_n_turns` | `3` | 永不修剪其工具结果的最近轮次数 |
| `soft_trim_threshold` | `4000` | 旧工具结果进行软裁剪的字符阈值 |
| `soft_trim_head` | `1500` | 软裁剪结果开头保留的字符数 |
| `soft_trim_tail` | `1500` | 软裁剪结果末尾保留的字符数 |
| `hard_clear_age_turns` | `10` | 工具结果被占位符替换前的轮次年龄 |

---

<a id="memory-staleness"></a>
## 记忆陈旧度

会话记忆变旧后，Grok 会在搜索结果中为它附加陈旧提示。较旧的结果会得到更强的提醒，要求你依赖前先核实当前状态。这些提示可帮助你发现存储的事实可能已不再准确。全局和工作区记忆不会收到陈旧提示，因为其中保存的是经过整理的长期知识。

---

<a id="file-watcher"></a>
## 文件监视器

默认情况下，Grok 会监视 `~/.grok/memory/` 的外部文件变更。如果直接编辑记忆文件（例如在编辑器中），下次记忆搜索时会自动获取变更：

- 新建或修改的文件会重新建立索引。
- 删除的文件会从索引中移除其陈旧分块。

```toml
[memory.watcher]
enabled = true    # 默认值
```

---

<a id="troubleshooting"></a>
## 故障排除

<a id="memory-not-working"></a>
### 记忆不起作用

1. 确认记忆已启用：检查 `grok-zh inspect` 输出。
2. 检查标志：`grok-zh --experimental-memory` 或 `GROK_MEMORY=1`。
3. 检查是否有 `--no-memory` 或 `GROK_MEMORY=0` 覆盖你的配置。

<a id="memory-not-appearing-in-sessions"></a>
### 记忆未出现在会话中

记忆会在第一轮注入。如果你在启用记忆前已经启动会话，请使用 `/new` 开始新会话。

<a id="viewing-memory-files"></a>
### 查看记忆文件

在 TUI 中使用 `/memory` 浏览带预览的全部记忆文件。也可以直接访问：

```bash
ls ~/.grok/memory/
cat ~/.grok/memory/MEMORY.md
$EDITOR ~/.grok/memory/MEMORY.md
```

<a id="debug-logging"></a>
### 调试日志

```bash
RUST_LOG=debug GROK_LOG_FILE=/tmp/grok.log grok-zh
grep "memory" /tmp/grok.log
```
