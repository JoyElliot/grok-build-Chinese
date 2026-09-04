# Grok Build 简体中文社区版

`grok-zh` 是基于 xAI 官方 Grok Build 源码维护的非官方简体中文社区版本。它尽量保持官方命令、协议和配置格式兼容，使用独立程序名提供中文 TUI，并与官方 `grok` 共用用户数据。

> 这是社区构建，不代表 xAI 官方发布。登录、模型、计费、云端会话、共享链接和服务可用性仍由官方服务端决定。

> 完整中文用户指南见 [GitHub 在线文档](https://github.com/JoyElliot/grok-build-Chinese/blob/zh-dev/crates/codegen/xai-grok-pager/docs/user-guide/zh-CN/README.md)；可下载版本见 [Releases](https://github.com/JoyElliot/grok-build-Chinese/releases)。

## 快速开始

Windows 云构建包下载后只需解压一次。要自动安装并把 `grok-zh`、`agent-zh`
加入当前用户 `Path`，请参阅包内的 `INSTALL-WINDOWS.md`，或在
[仓库中在线查看](https://github.com/JoyElliot/grok-build-Chinese/blob/zh-dev/packaging/windows/INSTALL-WINDOWS.md)。

```powershell
# 交互式 TUI
.\grok-zh.exe

# 指定项目目录
.\grok-zh.exe --cwd C:\path\to\project

# 查看完整 CLI 帮助
.\grok-zh.exe --help

# 查看子命令帮助
.\grok-zh.exe doctor --help
```

首次启动会引导登录和文件夹信任。请只信任你了解的工作目录；自动批准或宽松权限模式可能允许模型读写文件、执行命令或访问网络。

## 与官方版并行使用

两个程序入口使用以下约定：

- 可执行文件：`grok-zh.exe`
- 与官方版共用的用户数据目录：`~/.grok`
- 两个程序共同使用的目录覆盖：`GROK_HOME`
- 运行时内置帮助：`~/.grok/README.grok-build-zh.md`

会话、删除操作、登录状态、用户配置、第三方 API、模型、MCP、插件、技能和缓存都直接使用这套共享目录，不需要复制或同步。不要把社区测试包中的可执行文件覆盖到官方程序安装位置；官方安装脚本也不能用于安装或更新 `grok-zh`。

项目内的 `.grok/` 仍是上游兼容的项目级配置目录，名称不会翻译或改名。这样，同一项目可继续与官方版、团队配置和现有协议工具互操作。

## 语言与兼容边界

界面标题、设置、菜单、常见错误、权限提示、登录提示和内置教程会使用简体中文。以下标识必须保持原样：

- CLI 参数和子命令，如 `--cwd`、`--model`、`doctor`、`mcp`
- TOML/JSON 配置键和协议字段
- 工具名、MCP server 名、模型名、会话 ID 和日志字段
- 文件名与项目约定，如 `AGENTS.md`、`.grok/config.toml`

模型、工具、MCP server、Shell 命令或官方服务端返回的动态文本可能仍是英文；社区版不会擅自翻译协议负载。

## 登录与凭据

默认使用浏览器登录。界面中的 `/login` 可重新认证，`/logout` 可退出当前账号。API Key、OIDC 或外部认证提供程序仍沿用官方配置格式；配置键和值不要翻译。

凭据保存在共享的 `~/.grok/auth.json`。在 `grok` 或 `grok-zh` 中登录、退出或设置 API Key，都会影响另一入口。不要把凭据文件提交到 Git，也不要在问题报告中粘贴 token、API Key 或完整认证日志。

登录、订阅状态、模型权限和速率限制由官方或你配置的认证服务决定，Fork 无法保证这些服务始终可用。

## 交互式 TUI

常用操作：

- 输入任务后按 `Enter` 发送。
- 使用 `/help` 打开命令面板。
- 使用 `/docs` 打开内置用户指南。
- 使用 `/tutorial` 打开中文上手教程。
- 使用 `/settings` 调整界面与行为设置。
- 使用 `/model` 选择可用模型。
- 使用 `/sessions` 浏览或恢复会话。
- 使用 `/new` 开始新会话。
- 使用 `/quit` 退出。

快捷键会根据当前界面显示。终端、输入法和键盘布局不同，组合键行为也可能不同；遇到问题可在 `/help` 或快捷键帮助中核对。

## 权限与安全

权限提示中的“允许一次”“始终允许”和“拒绝”会影响工具能否继续。批准前请检查：

- 命令是否可能删除、覆盖或移动重要文件；
- 写入路径是否位于预期项目内；
- 网络请求是否会上传源码、日志或隐私数据；
- MCP server 或插件是否来自可信来源；
- 自动批准规则是否过宽。

配置键、工具调用参数和 Shell 命令不会被翻译。中文标签只帮助理解，不改变底层协议含义。

## 配置

用户级配置默认位于：

```text
~/.grok/config.toml
```

项目级配置仍位于：

```text
<project>/.grok/config.toml
```

项目规则通常使用仓库中的 `AGENTS.md`。上层目录和更靠近当前文件的规则可能共同生效；修改前应确认作用域。

配置键沿用官方格式。升级上游版本后，配置能力可能变化；以 `grok-zh --help`、设置界面和当前源码为准。

## 会话与数据

会话、日志、调试信息、缓存和内置文档都写入共享的 `~/.grok`。如需为一次开发测试显式使用临时目录，`GROK_HOME` 会同时改变官方版与中文版在该进程环境中的数据根：

```powershell
$env:GROK_HOME = 'C:\temp\grok-test-home'
.\grok-zh.exe
```

测试结束后先退出程序，再决定是否保留该目录。不要在 Grok 任务运行期间移动、删除或编辑会话文件。

会话恢复、云端同步、共享和跨设备能力可能依赖官方服务端；本地 Fork 直接复用官方客户端的数据布局与协议。

## MCP、技能、插件与 Hooks

MCP server、技能、插件和 Hooks 继续使用官方兼容格式：

- 用户级资源放在 `~/.grok` 对应子目录，供两个程序入口共同使用；
- 项目级资源继续放在项目 `.grok/` 或官方约定位置；
- `mcp`、工具名、server ID、插件 ID 和配置键保持英文原值；
- 第三方扩展的界面和错误文本由其作者提供，社区版不保证中文覆盖。

安装或启用第三方扩展前，请审查其来源、命令、网络权限和文件访问范围。

## Headless 与 Agent 模式

脚本和 IDE 集成应继续使用官方兼容的命令参数与 ACP 协议字段。不要翻译 JSON 字段、事件名、枚举值或工具 schema。

查看当前构建支持的精确参数：

```powershell
.\grok-zh.exe --help
.\grok-zh.exe agent --help
```

自动化脚本应显式检查退出码和结构化输出，不要依赖已经翻译的自然语言提示做解析。

## 诊断

遇到问题时，先运行：

```powershell
.\grok-zh.exe doctor
.\grok-zh.exe doctor --help
```

报告问题时建议提供：

- 社区版版本号与构建来源提交；
- Windows 版本、终端和 Shell；
- 可复现步骤；
- 已脱敏的错误文本；
- 是否设置了自定义 `GROK_HOME`；
- 是否能在官方 Grok Build 同版本复现。

请删除 token、API Key、账号信息、私有仓库路径、源码片段和会话内容后再分享日志。

## 更新策略

正式社区更新只读取 `JoyElliot/grok-build-Chinese` 的不可变 GitHub Releases；不会回退
到官方 npm、x.ai、GCS 或官方 GitHub 更新源。默认 `stable` 通道只选择严格三段版本的
非预发布包，`grok-zh update --alpha` 可显式选择预发布通道。后台自动下载默认关闭；
欢迎页按 `Ctrl+U` 或显式运行更新命令后，才会下载并安装本次更新。

发布工作流会核验三平台完整归档、`.sha256`、GitHub digest 和构建来源证明；更新器还会
核验固定资产集合、安全归档布局、包内清单与候选程序版本。不要运行官方安装脚本来
“更新”社区版，否则可能安装或覆盖官方程序而不是 `grok-zh`。

Windows、macOS 和 Linux 的完整包均附带社区安装器，默认安装为 `grok-zh`、`agent-zh`；
只有用户显式启用兼容入口时才会接管 `grok`、`agent`。CI 预览 Artifact 仅用于测试，
不是自动更新源。

## 许可证

本社区版本继续受仓库中的原始 `LICENSE` 约束。中文说明用于帮助理解，不替代许可证原文，也不改变 xAI、Grok 或第三方项目的商标和权利归属。
