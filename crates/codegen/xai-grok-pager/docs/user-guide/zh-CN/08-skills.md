<a id="skills"></a>
# 技能

技能是可复用的提示包，可用针对任务的指令扩展 Grok。它们让你只需记录一次可重复流程，而不必在每个会话中重新解释。

---

<a id="what-are-skills"></a>
## 什么是技能？

技能是包含 `SKILL.md` 文件的目录。其 Markdown 正文告诉 Grok 如何处理某一类任务：包括分步指令、约定和工具使用模式。

对于过于具体、不适合放进 AGENTS.md、又长到不想重新输入的可重复流程，请使用技能。只有在适用于当前任务时，Grok 才会激活技能。

---

<a id="skill-locations"></a>
## 技能位置

Grok 按以下优先级从这些目录发现技能：

| 位置 | 作用域 | 优先级 | 备注 |
|----------|-------|-------|-------|
| `./.grok/skills/`、`./.grok/commands/` | 本地（CWD） | 最高 | 当前目录技能 / 旧版命令 Markdown |
| `<repo_root>/.grok/skills/`、`…/commands/` | 仓库 | 中 | 仓库共享 |
| `~/.grok/skills/`、`~/.grok/commands/` | 用户 | 最低 | 所有项目的个人技能 |
| `~/.claude/skills/`、`~/.claude/commands/` | 用户 | 最低 | Claude Code 兼容性（可配置） |
| `./.claude/skills/`、`./.claude/commands/` | 本地 / 仓库 | 高 | 项目 Claude 技能和旧版自定义斜杠命令 |
| `~/.cursor/skills/` | 用户 | 最低 | Cursor 兼容性（可配置） |
| `./.cursor/skills/` | 本地 / 仓库 | 高 | 项目 Cursor 技能（启用 Cursor 兼容技能时） |

Grok 会按名称对技能去重——高优先级位置会覆盖低优先级位置。Grok 还会在每个层级扫描 `.agents/skills/`（以及 `commands/`，与 `.grok/` 并列），并遍历当前工作目录到仓库根目录之间的每个目录。

`commands/` 目录下的扁平 `*.md` 文件会成为用户可调用的斜杠命令（文件名主干 = 命令名称），与 Claude Code 的旧版自定义命令布局一致。

技能和命令发现**不**使用 `.gitignore`。已知技能根目录（`.grok/`、`.agents/`、`.claude/`、`.cursor/`）下的路径只要存在于磁盘上就始终加载——团队经常将 `.claude/**` 忽略为仅本地配置，同时仍希望 `/frontend` 风格的项目命令能够工作。若要隐藏技能，请在配置中使用 `[skills] ignore`（而不是仓库忽略规则）。

Grok 默认扫描 Claude 和 Cursor 技能目录。若要停止扫描某个厂商，请在 `~/.grok/config.toml` 的 `[compat.cursor]` 或 `[compat.claude]` 下将其 `skills` 单元设为 `false`，或者将 `GROK_CURSOR_SKILLS_ENABLED` 或 `GROK_CLAUDE_SKILLS_ENABLED` 环境变量设为 `false`。详情请参阅[配置](05-configuration.md#harness-compatibility)。无论这些设置如何，Grok 都会始终过滤掉已知的厂商随附默认技能（例如 Cursor 的 `shell`、`canvas` 和 `statusline`）。

<a id="additional-skill-directories"></a>
### 其他技能目录

通过 `~/.grok/config.toml` 中的 `[skills]` 添加目录、排除路径或禁用单个技能：

```toml
[skills]
paths = ["~/my-team-skills"]          # 要扫描的其他目录
ignore = ["~/my-team-skills/wip"]     # 要排除的路径（完全隐藏）
disabled = ["wip-skill"]              # 保留在列表中但不激活的技能名称
```

`paths` 中的每个条目可以是 `SKILL.md` 文件，或 Grok 会递归遍历的目录。`ignore` 会完全隐藏技能；`disabled` 会将技能保留在列表中，但从系统提示和调用中排除。`paths` 和 `ignore` 接受文件系统路径并支持 `~` 展开；`disabled` 接受技能名称。

`[paths] extra_skill_dirs` 由 `/import-claude` 写入，但不会注入技能。需要额外目录时，请使用 `[skills] paths`。

---

<a id="creating-a-skill"></a>
## 创建技能

<a id="directory-structure"></a>
### 目录结构

每项技能都位于包含 `SKILL.md` 文件的独立目录中：

```
~/.grok/skills/
  commit/
    SKILL.md
  review-pr/
    SKILL.md
  deploy/
    SKILL.md
```

<a id="skillmd-format"></a>
### `SKILL.md` 格式

技能文件包含 YAML frontmatter，后跟 Markdown 指令：

```markdown
---
name: commit
description: Create well-formatted git commits following conventional commit standards. Use when the user wants to commit changes or asks for /commit.
---

# Git Commit Skill

Review staged changes and create a commit with a clear, conventional message.

## Steps

1. Run `git diff --staged` to see changes
2. Summarize what changed and why
3. Create commit message following conventional commits format
4. Run `git commit -m "..."` with the message
```

<a id="core-frontmatter-fields"></a>
### 核心 frontmatter 字段

| 字段 | 说明 |
|-------|-------------|
| `name` | 技能标识符。使用小写字母、数字和连字符，最多 64 个字符。Grok 会将空格和下划线规范化为连字符。省略 `name` 时，Grok 使用技能目录名称。 |
| `description` | 技能的作用及使用时机。Grok 根据它决定是否调用技能。省略时，Grok 使用正文的第一段。 |

请编写具体的 `description`。它决定 Grok 何时自动调用技能。写出触发短语和使用场景。

<a id="optional-frontmatter-fields"></a>
### 可选 frontmatter 字段

多词 frontmatter 键使用 kebab-case（像 `model` 这样的单词键按原样书写）。

| 字段 | 说明 |
|-------|-------------|
| `when-to-use` | 自动调用的触发短语，与 `description` 分开保存。 |
| `allowed-tools` | 技能使用的工具，可写成 YAML 列表，也可写成逗号分隔或空格分隔的字符串。 |
| `argument-hint` | 斜杠命令自动补全中显示的提示文本（例如 `commit message`）。 |
| `user-invocable` | 是否可以作为斜杠命令运行。默认为 `true`；设为 `false` 可从斜杠命令中隐藏。（若要阻止模型调用技能，请改用 `disable-model-invocation`。） |
| `disable-model-invocation` | 为 `true` 时，只有你的斜杠命令会运行技能——模型不能自动调用。默认为 `false`。 |
| `model` | 运行技能时覆盖模型。 |
| `effort` | 推理 effort 覆盖值。 |
| `license` | 许可证标识符（例如 `Apache-2.0`）。 |
| `compatibility` | 环境要求（例如 `Requires git, docker, jq`）。 |
| `metadata` | 任意字符串键值对。Grok 会提升 `metadata.author` 和 `metadata.short-description` 以供显示。 |

---

<a id="creating-skills-with-create-skill"></a>
## 使用 /create-skill 创建技能

`/create-skill` 命令会交互式引导你构建新技能。Grok 会询问你的需求、起草文件并写入磁盘。

<a id="how-it-works"></a>
### 工作原理

运行 `/create-skill` 时，Grok 会：

1. **收集需求。** Grok 会询问技能名称、保存作用域以及你想要记录的工作流描述。名称使用小写字母、数字和连字符（2–64 个字符，首尾为字母或数字）。

2. **起草描述。** Grok 会编写 `description`，说明技能作用、触发它的短语和斜杠命令名称。继续前你可以批准或编辑草稿。

3. **创建技能目录。** Grok 会创建 `<scope>/.grok/skills/<name>/` 目录；如果技能需要，还会创建 `scripts/` 或 `references/` 子目录。

4. **写入 SKILL.md。** Grok 会写入 frontmatter（`name` 和 `description`）及 Markdown 指令正文，以及任何支持文件。

5. **验证并确认。** Grok 会重新读取文件，确认写入正确，并告诉你如何运行技能。

<a id="choosing-a-scope"></a>
### 选择作用域

Grok 会询问保存技能的位置：

- **项目**（`<repo_root>/.grok/skills/<name>/`）——仅在此仓库中可用，可通过版本控制与队友共享。Grok 建议在 git 仓库内使用此作用域。
- **用户**（`~/.grok/skills/<name>/`）——在所有项目中可用。

若要向整个团队或组织分发技能，请将其打包进插件，并通过市场发布。参阅[创建自己的市场](09-plugins.md#create-your-own-marketplace)和[在组织中分发](09-plugins.md#distribute-across-an-organization)。

新技能会在几秒内出现在斜杠菜单中，因为 Grok 会在磁盘文件发生变化时重新加载技能。

---

<a id="using-skills"></a>
## 使用技能

<a id="run-a-skill-by-name"></a>
### 按名称运行技能

每项技能都是以技能名称命名的斜杠命令。输入其名称即可运行：

```
/commit              # 运行“commit”技能
/review-pr           # 运行“review-pr”技能
```

运行技能会将其指令加载到对话中，并引导模型遵循这些指令。要传入参数，请将参数写在名称后：

```
/commit fix the build
```

要浏览技能，请输入 `/` 打开斜杠命令菜单。Grok 会列出每个内置命令和技能，并在输入时筛选。若要改用命令行列出技能，请运行 `grok-zh inspect`（参阅[查看技能详情](#viewing-skill-details)）。

<a id="qualified-names"></a>
### 限定名称

当技能名称与其他技能或内置命令冲突时，Grok 会保留两者供调用。内置命令保留裸名称（`/login`、`/compact`、…），技能则使用带作用域前缀的限定名称——`local:`、`repo:`、`user:` 或插件名称：

```
/local:commit        # 来自 ./.grok/skills/ 的“commit”技能
/user:commit         # 来自 ~/.grok/skills/ 的“commit”技能
/acme:login          # 名为“login”的插件技能（内置 /login 不变）
```

在斜杠菜单输入 `/login` 时会显示两行，右对齐的标记分别是 **built-in** 或 **skill · plugin-name**，以便区分。若希望技能使用裸 `/name`，请重命名技能（或其目录）。

`grok-zh inspect` 会用 `[collides with /login → /acme:login]` 标记冲突技能。

<a id="automatic-invocation"></a>
### 自动调用

Grok 能在识别到相关任务时自行调用技能。Grok 会将提示与技能的 `description` 和 `when-to-use` 字段进行匹配，因此应在两者中说明触发场景。

例如，如果技能描述写着 “Use when the user wants to commit changes”，那么说 “commit my changes” 就可能自动触发该技能。若要求必须显式使用斜杠命令并阻止自动调用，请在 frontmatter 中设置 `disable-model-invocation: true`。

---

<a id="viewing-skill-details"></a>
## 查看技能详情

运行 `grok-zh inspect` 查看 Grok 发现的每项技能以及其他配置：

```bash
grok-zh inspect          # 人类可读摘要
grok-zh inspect --json   # 机器可读报告
```

在人类可读输出中，Skills 部分会列出每项技能的名称及来源——`project`、`user`、`bundled`、`config`（`[skills].paths` 条目）、`server`（从受管工作区的技能存储同步的技能）或 `plugin: <name>`。通过 `[skills].disabled` 或禁用的厂商界面停用的技能会标记为 `[disabled]`。

报告会以与实时会话相同的方式遵循 `[skills]` 配置：列出来自 `paths` 的技能，隐藏 `ignore` 前缀下的技能，并保留 `disabled` 中的技能但标记 `[disabled]`。

`--json` 报告会为每项技能包含完整详情：其 `name`、`description`、`source`（以及 SKILL.md 文件路径）和 `userInvocable` 标志。裸斜杠名称若被内置命令或另一项技能占用，还会包含 `collidesWith`（冲突名称）和 `invocableAs`（要输入的限定命令）。

---

<a id="bundled-and-plugin-skills"></a>
## 捆绑技能和插件技能

Grok 将平台技能与个人技能分开分发。捆绑技能缓存于 `~/.grok/bundled/skills/` 下；Grok 不会将它们写入 `~/.grok/skills/`。同名的本地、仓库或用户技能会覆盖捆绑副本。`grok-zh inspect` 会按实际来源标记每个定义。（同名插件技能不会覆盖原生技能，而是以限定的 `plugin:name` 形式继续可用。）

技能也可以来自插件。安装包含技能的插件后，它们会与用户和项目技能一起出现。`grok-zh inspect` 会将插件提供的每项技能标为 `plugin: <name>` 来源。

更多关于安装提供技能的插件的信息，请参阅[插件指南](09-plugins.md)。

---

<a id="best-practices"></a>
## 最佳实践

1. **编写具体描述。** 描述驱动自动调用。“Create git commits”过于笼统；“Create well-formatted git commits following conventional commit standards. Use when the user wants to commit changes or asks for /commit.” 更有效。

2. **包含具体步骤。** 技能最好提供 Grok 可以遵循的清晰、有序流程。

3. **按名称引用工具。** 如果技能依赖特定工具（例如 `run_terminal_cmd` 或 `search_replace`），请写出工具名称，让模型知道使用什么。

4. **保持技能专注。** 每个工作流编写一项技能。“deploy”技能和“rollback”技能比分别合并为一个“deploy-and-rollback”技能更好。

5. **对项目技能进行版本控制。** 将 `.grok/skills/` 提交到仓库，让整个团队受益。`~/.grok/skills/` 中的用户技能仍是个人且不会共享。

6. **通过运行进行测试。** 调用 `/name` 并确认技能工作正常，然后再依赖自动调用。

### 技能正文大小上限

Grok 最多内联技能正文的前 25,000 个 Token，与 `read_file` 的读取上限相同。将较长的参考资料放在同级文件，并在技能中指示使用行偏移和行数限制读取。
