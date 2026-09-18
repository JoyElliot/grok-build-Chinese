# 项目规则（AGENTS.md）

项目规则让你可以按项目或目录配置 Grok。在仓库中放置 `AGENTS.md` 文件，即可设置编码约定、构建说明、样式指南，以及 Grok 在处理该代码库时应遵循的其他指令。

---

<a id="what-are-project-rules"></a>
## 什么是项目规则？

项目规则是 Grok 读取并加入其上下文的 Markdown 文件。对于这棵目录树中的每次交互，Grok 都会遵循其中的内容。

这是向 Grok 传授项目约定的主要机制，因此你无需在每个会话中重复说明这些约定。

---

<a id="supported-file-names"></a>
## 支持的文件名

Grok 会在每个目录中按以下顺序检查这些文件名：

- `Agents.md`
- `Claude.md`
- `CLAUDE.md`
- `CLAUDE.local.md`
- `AGENT.md`
- `AGENTS.md`

Grok 会加载目录中每个匹配的文件，因此同时包含 `AGENTS.md` 和 `CLAUDE.md` 的目录会贡献两份文件。在不区分大小写的文件系统上，解析到同一个文件的名称（例如 `Agents.md` 和 `AGENTS.md`）会去重，只计一次。为兼容 Claude Code 工作流，支持 `Claude.md`、`CLAUDE.md` 和 `CLAUDE.local.md`。启用 Claude 兼容性（默认启用）时，Grok 还会扫描主目录级别的 `~/.claude/` 目录中的这些文件名，并在每一级目录检查 `.claude/CLAUDE.md` 和 `.claude/CLAUDE.local.md`——这些正是 Claude Code 用于项目记忆的位置。启用 Cursor 兼容性时，也会以同样方式扫描主目录级别的 `~/.cursor/` 目录。

<a id="rules-directories"></a>
### 规则目录

除 AGENTS.md 文件外，Grok 还会在从仓库根目录到当前工作目录的每一级（`<dir>`）扫描规则目录中的 `*.md` 文件：

| 位置 | 说明 |
|------|------|
| `<dir>/.grok/rules/` | 始终扫描 |
| `<dir>/.claude/rules/` | Claude 兼容性（可配置） |
| `<dir>/.cursor/rules/` | Cursor 兼容性（可配置） |

无论从哪里启动，Grok 也会扫描主目录级别的规则。这些根目录已经按厂商区分，因此规则直接放在 `rules/` 下：

| 位置 | 说明 |
|------|------|
| `$GROK_HOME/rules/`（默认 `~/.grok/rules/`） | 始终扫描；适用于所有项目 |
| `~/.claude/rules/` | 由 `compat.claude.rules` 控制 |
| `~/.cursor/rules/` | 由 `compat.cursor.rules` 控制 |
| `[paths] extra_rule_dirs` 的每个条目 | 配置中列出的绝对目录；支持 `~` 展开 |

主目录规则按表中顺序先加载，随后加载从仓库根目录到当前目录的项目文件。每个规则目录中的文件按字母顺序排列。厂商的 `rules` 配置项独立控制主目录规则和项目规则，不受相应 `agents` 配置项影响。Claude 的 `agents` 配置项控制 `~/.claude/` 下的命名文件以及项目 `<dir>/.claude/CLAUDE*.md`；顶层通用名称（如 `Claude.md`、`CLAUDE.md` 和 `CLAUDE.local.md`）仍会被识别。请参阅[配置](05-configuration.md#harness-compatibility)。

---

### 额外规则目录

主目录规则按表中顺序加载，随后是从仓库根目录到当前目录的项目文件。每个规则目录内按文件名字母顺序加载。额外目录的配置示例：

```toml
[paths]
extra_rule_dirs = ["~/team-rules", "/opt/company/grok-rules"]
```

每个目录下直接包含的 `*.md` 都会作为用户规则在所有项目中加载；不会递归扫描子目录，也不受文件夹信任、仓库 `.gitignore` 或兼容性开关影响。`grok-zh inspect` 将其标为 `global`。条目必须是绝对路径或以 `~/` 开头；相对路径或不存在的路径不会加载内容。`/import-claude` 会把现有 `~/.claude/rules/` 写到这里，使关闭 Claude 兼容扫描后仍能加载。

---

<a id="how-discovery-works"></a>
## 发现机制

Grok 按以下顺序扫描项目规则：

1. **主目录规则**：先扫描 `$GROK_HOME`，再扫描已启用的 `~/.claude/` 和 `~/.cursor/` 来源，最后扫描 `[paths] extra_rule_dirs`
2. **仓库规则**：如果位于 git 仓库内，则从仓库根目录向下扫描到当前工作目录（含当前目录）的每一级
3. **仅当前工作目录**：如果不在 git 仓库内，只扫描当前工作目录

<a id="example"></a>
### 示例

给定如下项目结构：

```
~/projects/my-app/
  AGENTS.md              # “使用 TypeScript。遵循 ESLint 规则。”
  src/
    AGENTS.md            # “优先使用函数式组件。”
    components/
      AGENTS.md          # “使用 CSS 模块进行样式设计。”
```

当 Grok 在 `~/projects/my-app/src/components/` 中运行时，它会加载这三个文件。指令会累积，因此 Grok 能看到全部内容。

<a id="deeper-files-take-precedence"></a>
### 更深层文件优先

Grok 会按仓库根目录到当前工作目录的顺序排列文件，因此更深目录中的文件会在上下文中靠后出现；指令冲突时，靠后的指令优先。在上面的示例中，如果根目录写着“使用 styled-components”，而 `components/AGENTS.md` 写着“使用 CSS 模块”，则 CSS 模块指令会胜出，因为它出现得更靠后。

<a id="auto-loading-behavior"></a>
### 自动加载行为

- Grok 会在会话开始时自动加载从仓库根目录到当前工作目录的文件。
- 当 Grok 读取、列出或编辑初始集合之外目录中的文件时，它会检测那里是否存在项目指令文件，记录其路径，并在这些文件适用于任务时读取它们。

---

<a id="what-to-put-in-project-rules"></a>
## 项目规则中应写什么

<a id="coding-conventions"></a>
### 编码约定

```markdown
# 编码规范

- 所有新代码使用 TypeScript
- 优先使用带 hooks 的函数式组件，而不是类组件
- 默认使用 `const`；只有需要重新赋值时才使用 `let`
- 最大行长：100 个字符
```

<a id="build-and-test-instructions"></a>
### 构建和测试说明

```markdown
# 构建与测试

- 提交前运行 `npm test`
- 使用 `npm run lint` 检查代码样式
- 使用 `npm run build` 构建——确保没有 TypeScript 错误
- 集成测试：`npm run test:e2e`（需要 Docker）
```

<a id="style-guides"></a>
### 样式指南

```markdown
# 样式指南

- 遵循 Airbnb JavaScript Style Guide
- 使用 2 个空格缩进
- 多行数组/对象始终保留尾随逗号
- 优先使用模板字面量，而不是字符串拼接
```

<a id="pr-and-commit-requirements"></a>
### PR 和提交要求

```markdown
# 版本控制

- 使用 conventional commits 格式编写提交消息
- 分支名称以 `feature/`、`fix/` 或 `chore/` 开头
- 所有 PR 合并前至少需要一人批准
- 对功能分支使用 squash merge
```

<a id="architecture-notes"></a>
### 架构说明

```markdown
# 架构

- API 路由放在 `src/routes/`，每个资源使用一个文件
- 业务逻辑放在 `src/services/`
- 数据库查询放在 `src/repositories/`
- 禁止在 `src/services/` 中导入 `src/routes/`
```

---

<a id="scoping-rules-to-subdirectories"></a>
## 将规则限定到子目录

AGENTS.md 文件的作用域是从其所在目录开始的整棵目录树。你可以用它为代码库的不同部分提供不同指令：

```
my-monorepo/
  AGENTS.md                    # Monorepo 全局规则
  packages/
    frontend/
      AGENTS.md                # “使用 React。优先使用 CSS 模块。”
    backend/
      AGENTS.md                # “使用 Express。遵循 REST 约定。”
    shared/
      AGENTS.md                # “此包中不使用特定框架的代码。”
```

---

<a id="session-rules-flags"></a>
## 会话规则标志

无需编辑文件即可为单个会话添加规则，传递 `--rules`（别名为 `--append-system-prompt`）：

```bash
grok-zh --rules "始终使用 TypeScript。优先使用函数式组件。"
```

Grok 会将此文本附加到会话的系统提示中。若要使用 `--system-prompt-override`（别名为 `--system-prompt`）完全替换系统提示，Grok 会逐字使用该文本，并跳过默认系统提示和 `--rules`。（相较之下，使用 `--rules` 传入的文本会包在 `<human_rules>` 块中，然后附加到默认提示。）

---

<a id="file-size"></a>
## 文件大小

Grok 会完整加载每个项目指令文件；没有字符上限，也不会截断。不过，仍应让指令简洁而聚焦。相较于冗长指令，简短而具体的规则更容易被 Grok 遵循，而且加载的每个文件都会消耗上下文。

---

<a id="gitignore-filtering"></a>
## Gitignore 过滤

发现过程中会跳过被 `.gitignore` 忽略的文件。若要将个人覆盖规则排除在共享仓库之外，可以将已识别的文件名（例如 `CLAUDE.local.md`）加入 gitignore：

```gitignore
# .gitignore
CLAUDE.local.md
```

作为顶层指令文件，Grok 只会发现[支持的文件名](#supported-file-names)中列出的名称——不会发现 `AGENTS.local.md` 或 `notes.md` 等自定义名称。（在 `.grok/rules/` 这样的规则目录中，无论名称是什么，所有 `*.md` 文件都会加载。）

---

<a id="the-grok-project-directory"></a>
## .grok/ 项目目录

除 AGENTS.md 文件外，项目根目录中的 `.grok/` 目录还可以包含其他项目级配置：

| 路径 | 用途 |
|------|------|
| `.grok/config.toml` | 项目作用域的 MCP 服务器、插件和权限规则（其他设置只从 `~/.grok/config.toml` 加载） |
| `.grok/skills/` | 项目作用域的技能定义 |
| `.grok/plugins/` | 项目作用域的插件 |
| `.grok/agents/` | 项目作用域的智能体定义 |
| `.grok/hooks/` | 项目作用域的生命周期钩子 |
| `.grok/lsp.json` | LSP 服务器配置 |

这些内容都可选。各项的详细信息请参阅相应指南。

---

<a id="inspecting-loaded-rules"></a>
## 检查已加载的规则

使用 `grok-zh inspect` 查看所有已加载的项目指令：

```bash
grok-zh inspect
```

此命令会显示它找到的每个项目指令文件、文件路径以及大致 token 数。用它确认 Grok 是否加载了你的规则。

---

<a id="best-practices"></a>
## 最佳实践

1. **从根目录开始。** 将最重要的全项目规则放在仓库根目录的 AGENTS.md 中。

2. **具体明确。** “使用 TypeScript”优于“使用现代 JavaScript”。“提交前运行 `cargo fmt`”优于“格式化代码”。

3. **保持简短。** 简洁指令比冗长指令更可能被遵循。

4. **大型仓库使用子目录作用域。** Monorepo 的不同部分可能有不同约定。使用每个目录的 AGENTS.md 来限定规则作用域。

5. **对规则进行版本控制。** 将 AGENTS.md 提交到仓库，让整个团队受益。用户专属覆盖规则应放在 `~/.grok/`（全局规则）中。

6. **不要重复文档。** AGENTS.md 应包含可执行的指令，而不是项目 README 的副本。如有需要，请链接到外部文档。

7. **定期审查。** 随着项目演进，更新规则以匹配当前约定。
