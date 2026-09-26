<div align="center">

<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://media.x.ai/v1/website/spacexai-symbol-white-transparent-0c31957f.png">
    <source media="(prefers-color-scheme: light)" srcset="https://media.x.ai/v1/website/spacexai-symbol-black-transparent-6435cf42.png">
    <img alt="SpaceXAI logo" src="https://media.x.ai/v1/website/spacexai-symbol-black-transparent-6435cf42.png" width="96">
  </picture>
  <br>
  Grok Build 简体中文社区版（<code>grok-zh</code>）
</h1>

这是基于官方 [xai-org/grok-build](https://github.com/xai-org/grok-build) Fork 的非官方简体中文社区版。

本项目在尽量保持原有功能、命令行参数、配置格式和协议兼容性的前提下，为 Grok Build 的 CLI、TUI、设置、提示信息和用户文档提供简体中文支持。它以独立程序名 `grok-zh` 与官方版并行使用，但有意共用 `~/.grok` 数据目录：会话、登录状态、配置、第三方 API、插件与本地状态在两个入口之间保持一致。

[项目定位](#项目定位) · [当前状态](#当前状态) · [Windows-安装](#windows-安装) · [macOS-安装](#macos-安装) · [Linux-GNU-安装](#linux-gnu-安装) · [从源码构建](#从源码构建) · [共享数据与兼容约定](#共享数据与兼容约定) · [文档](#文档) · [开发](#开发) · [Releases](https://github.com/JoyElliot/grok-build-Chinese/releases) · [上游与发布策略](#上游与发布策略) · [许可证](#许可证)

![grok-zh 中文 TUI 工具链体检](docs/screenshots/grok-zh-toolchain-check.png)

</div>

---

## 项目定位

- 官方 Grok Build 的产品介绍与服务说明见 [x.ai/cli](https://x.ai/cli)。
- 本仓库不是 SpaceXAI 官方发行版，也不代表官方翻译或服务承诺。
- `SOURCE_REV` 记录本仓库源码所对应的官方 monorepo 提交；发布时还会在构建信息中记录 Fork 的 Git 提交。
- 模型可用性、账号权限、订阅、远程会话、搜索、语音及其他在线能力依赖官方服务端，社区 Fork 无法保证。

## 当前状态

统一稳定版见[最新正式 Release](https://github.com/JoyElliot/grok-build-Chinese/releases/latest)，以该页面的实际附件为准。新构建工作流覆盖以下六种系统与架构：

| 系统 | 架构 | 社区包目标 |
| --- | --- | --- |
| Windows | x64 | `x86_64-pc-windows-gnu` |
| Windows | ARM64 | `aarch64-pc-windows-msvc` |
| macOS | Apple Silicon | `aarch64-apple-darwin` |
| macOS | Intel | `x86_64-apple-darwin` |
| Linux | x64 | `x86_64-unknown-linux-gnu` |
| Linux | ARM64 | `aarch64-unknown-linux-gnu` |

新平台的完整包须通过各自的原生 CI 构建和安装验收后，才会随正式版发布。`release-v1.0.12` 因 Linux
发布二进制未移除调试信息、超过旧版更新器的解包安全上限，已保留为预发布历史记录，
稳定通道不会再选择它。已发布的 Windows `v1.0.3`、`v1.0.5` 客户端会先自动升级到
`v1.0.8` 桥接版；`v1.0.8` 与早期 `release-v*` 客户端会先选择保留六附件的三平台正式版，再升级到六平台新版。更早写死旧仓库
地址的 `v1.0.0-zh.preview.3` 仍需手工安装一次现代完整包。Windows 产物尚未经过
Authenticode 签名，首次运行可能触发 SmartScreen；请只从本仓库
[Releases](https://github.com/JoyElliot/grok-build-Chinese/releases) 下载。

`zh-dev` 的统一 [CI 工作流](https://github.com/JoyElliot/grok-build-Chinese/actions/workflows/zh-dev-windows-preview.yml)
构建上述六种目标的预览 Artifact。预览产物只用于
构建和设备验收，不会独立创建 Release；正式 Tag 由统一发布工作流按版本契约汇总、核验并
证明各平台资产。macOS 产物尚未使用 Apple Developer ID 签名或公证。安装与安全边界见
[macOS 安装说明](packaging/macos/INSTALL-MACOS.md)和
[Linux GNU 安装说明](packaging/linux/INSTALL-LINUX.md)。

已建立的产品与数据边界：

- 可执行文件：Windows 为 `grok-zh.exe`，macOS/Linux 为 `grok-zh`
- 与官方版共用的默认数据目录：`~/.grok`
- 两个程序共同使用的目录覆盖：`GROK_HOME`
- 默认界面语言：`zh-CN`，可用 `--locale en-US` 切换英文
- 内置更新器只读取本仓库的 Immutable GitHub Releases；官方 npm、GitHub、x.ai 和 GCS 更新源始终禁用

### 中文标题与计划

官方原版的相关提示没有中文语言约束，中文对话中的会话标题和计划容易被生成为英文。社区版没有重写整套上游提示词，只在会话标题和主要计划入口加入少量、按条件生效的语言规则：中文请求优先生成简洁的中文标题，并以简体中文创建计划和任务步骤；命令、路径、工具名、配置键、协议字段、任务 ID 以及 `pending`、`in_progress`、`completed`、`cancelled` 等规范状态仍保持原样。标题为空或中文请求生成纯英文标题时，会回退到用户输入。

![grok-zh 中文计划与工具链测试摘要](docs/screenshots/grok-zh-chinese-plan.png)

> [!WARNING]
> `crates/codegen/xai-grok-pager/scripts/` 下的安装脚本及同模块内的 npm 包装仍来自官方上游，可能安装或覆盖官方 `grok`。安装社区版时只使用本仓库 Release 或 CI 产物中的平台完整包及其社区安装器。

## Windows 安装

推荐在 PowerShell 中粘贴下面一行，按中文菜单安装或更新最新正式版，也可创建便携版。在线入口按系统架构选择 Windows x64 GNU 或 ARM64 MSVC 完整包，支持 Windows PowerShell 5.1 和 PowerShell 7，无需管理员权限。ARM64 包须先在 Release 发布。

```powershell
$p=Join-Path $env:TEMP ('grok-zh-install-'+[guid]::NewGuid().ToString('N')+'.ps1'); $tls=[Net.ServicePointManager]::SecurityProtocol; try { [Net.ServicePointManager]::SecurityProtocol=$tls -bor [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -UseBasicParsing 'https://raw.githubusercontent.com/JoyElliot/grok-build-Chinese/zh-dev/packaging/windows/Install-GrokZhOnline.ps1' -OutFile $p; & "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File $p; if ($LASTEXITCODE -ne 0) { throw "安装未完成，退出码：$LASTEXITCODE" } } finally { [Net.ServicePointManager]::SecurityProtocol=$tls; Remove-Item -LiteralPath $p -Force -ErrorAction SilentlyContinue }
```

菜单提供共存安装、设置 `grok` / `agent` 命令、自定义目录和便携版。默认保留官方版；程序下载、完整性校验和安装进度均以中文显示。便携目录顶层只有 `启动.cmd`、`使用说明.md` 和 `app/`，不修改 PATH。便携版仍与官方版共用 `~/.grok` / `GROK_HOME` 数据。

在线入口只选择本仓库可验证的最新正式 Release，不自动降级；下载与安装临时文件在结束后清理。安装完成后重新打开终端运行 `grok-zh`。详细参数和便携更新方式见 [Windows 自动安装说明](packaging/windows/INSTALL-WINDOWS.md)。

### 手动下载安装

正式 Tag 工作流会在 [Releases](https://github.com/JoyElliot/grok-build-Chinese/releases)
中发布完整 Windows ZIP；`CI` 工作流仍会上传短期 Actions Artifact。
解压完整包后，`release-v*` 包会得到唯一的
`grok-zh-<version>-windows-x86_64-gnu` 或 `grok-zh-<version>-windows-aarch64-msvc` 目录；进入该目录再双击下列入口。
旧版与 `v1.0.8` 桥接包仍是兼容所需的扁平结构，可在解压目录直接双击：

```text
一键安装.cmd
```

安装窗口会显示完整性校验和文件复制进度，完成后提示在新终端中输入
`grok-zh` 或 `agent-zh`。默认安装与官方命令共存；如需直接使用 `grok`、`agent`
启动中文版，再双击 `[可选]替换原始启动方式.cmd`。未检测到官方版时直接安装并
接管命令；检测到官方版时，可选择保留或卸载官方程序。卸载不创建备份，共享的
聊天记录、登录状态和配置会保留。高级参数、重新安装官方版的方法
和共享数据边界见 [Windows 自动安装说明](packaging/windows/INSTALL-WINDOWS.md)。

### 自动更新

- 默认使用 `stable` 通道，只接受本仓库非 Draft、非 prerelease 的 Immutable Release；如需预览版，可显式运行 `grok-zh update --alpha`。
- 更新器验证当前平台完整 ZIP 的固定下载地址、大小、GitHub SHA-256、包内协议/清单和候选程序版本；新版不依赖独立 `.sha256` 或其他平台附件。
- 后台自动更新默认关闭。按 `Ctrl+U` 才会下载并安装本次更新；也可以在设置中显式开启后台更新。
- `grok-zh update`、`Ctrl+U` 和 Windows 在线安装完成后显示实际安装版本的更新条目、兼容性提醒和已知问题，并保留 Release 页面链接；终端不展示网页末尾的比较链接和折叠安装区。后台更新的日志在返回终端时显示一次，不进入模型对话。
- 更新成功后清理受管旧版本备份；Windows 被运行进程占用的旧 EXE 在后续启动重试，完整安装器的旧目录在后续更新重试。失败回滚、个人文件和历史官方程序恢复数据仍受保护。
- 激活失败时保留当前版本；需要同步 `agent-zh.cmd`、`rg.exe`、安装器或文档时，重新运行新 ZIP 中的安装器。

旧版迁移、高级参数和恢复方式见 [Windows 自动安装说明](packaging/windows/INSTALL-WINDOWS.md)。正式 Release 同时提供 SHA-256 与 GitHub Artifact Attestation，用于核对文件完整性和云端构建来源；它们不等同于 Windows Authenticode 签名。

每个平台保持一个安装包。约两个月兼容期内，仅原 Windows x64 GNU、macOS ARM64、Linux x64 GNU 保留三个独立 `.sha256`，后续停止公开发布；新增 Windows ARM64、macOS Intel、Linux ARM64 使用 GitHub Release 的 SHA-256 digest，不公开独立校验附件。所有平台的包内 `SHA256SUMS.txt` 继续用于文件校验。迁移与维护约定见 [单包更新协议](docs/COMMUNITY-UPDATE-PROTOCOL.md)。

## macOS 安装

macOS 按 CPU 架构选择 `macos-aarch64`（Apple Silicon）或 `macos-x86_64`（Intel）归档。先在归档旁完成外层 SHA-256 校验；
`release-v*` 包解压后只会得到唯一的 `grok-zh-<version>-macos-<arch>` 目录，
进入该目录完成包内 SHA-256 校验后运行：

```sh
./Install-GrokZh.sh
```

默认只创建 `grok-zh`、`agent-zh`；明确需要兼容官方命令名时，才使用
`./Install-GrokZh.sh --with-compat-aliases`。安装器只写入 `${GROK_HOME:-$HOME/.grok}`，
不会改 shell 配置、`/usr/local/bin` 或 macOS 安全设置。

从 `release-v1.0.13` 统一稳定版起，内置更新器会校验本仓库的不可变 Release、
外层 GitHub SHA-256、严格 USTAR 布局、包内清单和候选程序版本，再把新的不可变目标原子
切换到 `grok-zh`/`agent-zh`。这不要求本地拥有 Xcode 或 Apple Developer ID，但当前未签名、
未公证的构建仍可能触发 Gatekeeper。完整步骤与安全边界见
[macOS 安装说明](packaging/macos/INSTALL-MACOS.md)。

## Linux GNU 安装

Linux 包面向 `x86_64-unknown-linux-gnu` 和 `aarch64-unknown-linux-gnu`；按 `uname -m` 选择 `linux-x86_64-gnu` 或 `linux-aarch64-gnu` 归档。先在归档旁完成外层 SHA-256 校验；
`release-v*` 包解压后只会得到唯一的对应平台目录，
进入该目录完成包内 SHA-256 校验后运行：

```sh
./Install-GrokZh.sh
```

默认只创建 `grok-zh`、`agent-zh`；需要 `grok`、`agent` 兼容入口时使用
`./Install-GrokZh.sh --with-compat-aliases`。安装器只写入
`${GROK_HOME:-$HOME/.grok}`，不会使用 `sudo`、修改 shell 配置或写入
`/usr/local/bin`。

Linux 自动更新从 `release-v1.0.13` 起进入统一稳定通道；该版本会在打包前移除 Linux
二进制的调试信息，并按旧版更新器的 512 MiB 单文件、768 MiB 总解包上限执行 CI
门禁。稳定 `v1.0.8` 是专供旧 Windows 客户端迁移的两资产桥接版本，不包含 macOS 或
Linux 资产。更新器会严格校验不可变 Release、GitHub digest、USTAR 结构、权限、包内
清单和候选版本，再把新的不可变目标原子切换到入口。WSL 应安装到发行版 ext4 的 `$HOME`，
不要把受管目录放到无法落实所有者或 `0700` 权限的 DrvFS 挂载。完整说明见
[Linux GNU 安装说明](packaging/linux/INSTALL-LINUX.md)。

### 反馈

- 当遇到汉化不全等任何问题时，欢迎提出 [issue](https://github.com/JoyElliot/grok-build-Chinese/issues)
- Linux Do 社区讨论地址：[点此进入](https://linux.do/t/topic/2770188)

## 从源码构建

### 通用要求

- Rust：版本由 `rust-toolchain.toml` 固定。
- [DotSlash](https://dotslash-cli.com)：用于下载并运行 `bin/` 下的密封工具，尤其是 `bin/protoc`。构建前请确保 `dotslash` 已加入 `PATH`：

  ```sh
  cargo install dotslash
  # 或使用预编译软件包：https://dotslash-cli.com/docs/installation/
  dotslash --help
  ```

- `protoc`：构建脚本优先通过 DotSlash 解析仓库内的 `bin/protoc`，也会回退到 `PATH` 或 `PROTOC` 指定的程序。
- 官方安装器发布 macOS、Linux、Windows 的 x64/ARM64 二进制；本 Fork 按上述六种目标独立构建、打包和验证。官方 Windows 安装包的目标架构不决定本 Fork 已发布的旧 Windows x64 GNU 包所用 ABI。

常用检查：

```sh
cargo run -p xai-grok-pager-bin
cargo build --locked -p xai-grok-pager-bin --release
cargo check --locked -p xai-grok-pager-bin --bin grok-zh --features release-dist
cargo test --locked -p xai-grok-locale
cargo fmt --all --check
```

普通 release 构建的产物为 `target/release/grok-zh`（Windows 为 `grok-zh.exe`）。首次启动会打开浏览器完成身份验证；详见[身份验证指南](crates/codegen/xai-grok-pager/docs/user-guide/zh-CN/02-authentication.md)。

### Windows 绿色测试构建

下面的命令把 Cargo 缓存、构建输出和测试数据放在仓库忽略的
`.codex-local` 目录中。它仅用于本地开发测试，不属于正式安装器。

```powershell
$localRoot = Join-Path $PWD '.codex-local'
$env:CARGO_HOME = Join-Path $localRoot 'cargo-home'
$env:CARGO_TARGET_DIR = Join-Path $localRoot 'target'
$env:GROK_HOME = Join-Path $localRoot 'test-home'
$env:GROK_VERSION = "1.0.3-zh.preview.1"
cargo build --frozen --target x86_64-pc-windows-gnu `
  -p xai-grok-pager-bin --profile release-dist --features release-dist
```

预期产物：

```text
.codex-local/target/x86_64-pc-windows-gnu/release-dist/grok-zh.exe
```

绿色测试包还会在 `grok-zh.exe` 同目录携带 `rg.exe`。社区版搜索入口优先使用该旁载工具，缺失时再回退到系统 `PATH`；这只隔离程序安装文件，不改变两个程序共用 `~/.grok` 数据的约定。

Windows x64 社区包采用 GNU 工具链，ARM64 包采用 MSVC 工具链；二者均未做代码签名，使用完整 ZIP 和包内安装器。正式工作流检查
归档布局、哈希和候选程序版本。社区自动更新链只消费本仓库
Releases 中通过这些门禁的平台资产。

## 动态公告翻译

从 `release-v1.0.16` 起，客户端支持从本仓库独立更新公告中文映射。用户首次安装包含此功能的客户端后，后续公告翻译无需再更新程序；已发布的 `release-v1.0.13` 及更早稳定版尚不支持。软件开始加载官方公告时并行检查译文版本，无独立轮询定时器；GitHub 不可用时沿用缓存或内置译文，未收录的内容显示官方英文原文。译文更新后会自动刷新界面。

维护者只需发布新版本的公告 JSON 和清单，流程见[动态公告翻译维护说明](community/announcements/README.md)。

模型说明与推理选项、远端提示和命令标签、订阅提示、官方 MCP、官方技能和插件市场也支持独立更新中文显示映射，维护方式见[动态界面译文说明](community/display-translations/README.md)。需要先安装包含此功能的客户端；之后只更新对应 JSON 即可。各类译文跟随官方目录的加载和刷新检查，不另设定时器；模型 ID、能力、请求参数和用户内容不受影响。

## 共享数据与兼容约定

`grok` 与 `grok-zh` 直接读写同一个 `~/.grok`（或 `GROK_HOME`）目录，不使用复制或双向同步层。因此在任一入口创建、恢复、重命名或删除会话，登录或退出账号，修改模型、第三方 API、MCP、插件和用户配置，另一入口都会看到相同结果。若两个程序同时运行，它们也遵循上游已有的文件锁与并发规则。

以下名称必须保持稳定，不做翻译：

- CLI 子命令、参数与取值，例如 `agent`、`--resume`、`--output-format json`
- 配置键、环境变量和序列化字段，例如 `[ui] screen_mode`、`GROK_HOME`、JSON key
- MCP、ACP、OAuth、OIDC、OSC 52 等协议名
- 工具名、模型 ID、会话 ID、路径、URL、日志字段和服务端原始错误

协议身份、遥测字段或兼容性所需的内部 `grok-pager` 名称可能继续保留；中文版的程序名使用 `grok-zh`，用户数据路径与官方版共同使用 `.grok`。

## 文档

- Windows 自动安装：[`packaging/windows/INSTALL-WINDOWS.md`](packaging/windows/INSTALL-WINDOWS.md)
- macOS Apple Silicon / Intel 安装与自动更新：[`packaging/macos/INSTALL-MACOS.md`](packaging/macos/INSTALL-MACOS.md)
- Linux x64 / ARM64 GNU 安装与自动更新：[`packaging/linux/INSTALL-LINUX.md`](packaging/linux/INSTALL-LINUX.md)
- 中文用户指南：[`crates/codegen/xai-grok-pager/docs/user-guide/zh-CN/README.md`](crates/codegen/xai-grok-pager/docs/user-guide/zh-CN/README.md)
- 中文入门教程：[`crates/codegen/xai-grok-pager/docs/tutorial/zh-CN/`](crates/codegen/xai-grok-pager/docs/tutorial/zh-CN/)
- 英文上游用户指南：[`crates/codegen/xai-grok-pager/docs/user-guide/README.md`](crates/codegen/xai-grok-pager/docs/user-guide/README.md)
- 贡献说明：[`CONTRIBUTING.zh-CN.md`](CONTRIBUTING.zh-CN.md)
- 安全策略：[`SECURITY.zh-CN.md`](SECURITY.zh-CN.md)
- 1.0.35 简体中文更新说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.35.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.35.zh-CN.md)
- 1.0.24 简体中文更新说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.24.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.24.zh-CN.md)
- 1.0.16 简体中文更新说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.16.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.16.zh-CN.md)
- 1.0.13 简体中文更新说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.13.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.13.zh-CN.md)
- 1.0.12 简体中文更新说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.12.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.12.zh-CN.md)
- 1.0.11 简体中文更新说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.11.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.11.zh-CN.md)
- 1.0.10 简体中文更新说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.10.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.10.zh-CN.md)
- 1.0.9 简体中文更新说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.9.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.9.zh-CN.md)
- 1.0.8 简体中文更新说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.8.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.8.zh-CN.md)
- 1.0.7 简体中文更新说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.7.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.7.zh-CN.md)
- 1.0.6 简体中文更新说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.6.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.6.zh-CN.md)
- 1.0.5 简体中文更新说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.5.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.5.zh-CN.md)
- 1.0.3 简体中文发行说明：[`crates/codegen/xai-grok-shell/changelogs/1.0.3.zh-CN.md`](crates/codegen/xai-grok-shell/changelogs/1.0.3.zh-CN.md)
- 版本发布：[`Releases`](https://github.com/JoyElliot/grok-build-Chinese/releases)
- 官方在线文档：[docs.x.ai/build/overview](https://docs.x.ai/build/overview)

中文文档将使用稳定文档 ID 和 `zh-CN` 平行目录，不直接改变英文标题所承担的查找身份，以降低合并上游更新时的冲突。

## 仓库结构

| 路径 | 内容 |
|---|---|
| `crates/codegen/xai-grok-locale` | 集中式语言目录、locale 解析与回退 |
| `crates/codegen/xai-grok-product` | 社区版程序名、共享数据目录与更新安全策略 |
| `crates/codegen/xai-grok-pager-bin` | 组合入口，生成 `grok-zh` |
| `crates/codegen/xai-grok-pager` | TUI、回滚区、提示输入、模态框和渲染 |
| `crates/codegen/xai-grok-shell` | 智能体运行时及 leader/stdio/headless 入口 |
| `crates/codegen/xai-grok-tools` | 终端、文件编辑、搜索等工具实现 |
| `crates/codegen/xai-grok-workspace` | 文件系统、版本控制、执行和检查点 |
| `crates/codegen/...` | CLI 依赖闭包中的其他配置、MCP、Markdown、沙箱等 crate |
| `crates/common/`、`crates/build/`、`prod/mc/` | 依赖闭包中少量共享与构建辅助 crate |
| `third_party/` | 仓库内 vendored 的 Mermaid 相关源码；归属见其中的 `NOTICE` |

> [!IMPORTANT]
> 根 `Cargo.toml`（工作区成员、依赖版本、lint 和 profile）由上游生成，应视为只读。新增社区功能应优先放在独立 crate 或局部适配层中，避免对上游文件进行大范围结构改写。

## 开发

工作区很大，日常检查应优先指定具体 crate：

```sh
cargo check -p <crate>
cargo test -p xai-grok-config
cargo clippy -p <crate>
cargo fmt --all
```

上游仓库不接受外部拉取请求；本社区 Fork 尚未公布独立贡献流程。开始修改前请先阅读[中文贡献说明](CONTRIBUTING.zh-CN.md)，并通过本仓库 Issue 与维护者沟通。

## 上游与发布策略

- `main`：尽量保持官方上游镜像，只用于同步和审查。
- `zh-dev`：汉化开发、上游合并、构建和测试。
- 稳定版不另建长期 `zh-stable` 分支；只从已经审核并通过 CI 的 `zh-dev` 精确提交创建
  受保护 Tag。`v1.0.8` 是最后一个旧通道桥接 Tag，后续稳定版使用
  `release-vA.B.C`，程序自身仍保持与上游一致的严格三段 SemVer。
- 仓库 Ruleset 必须同时限制 `v*` 与 `release-v*` Tag 的创建、更新和删除权限，只允许维护者给已审核分支提交
  打 Tag；工作流内的 SHA 复核不能替代 GitHub 服务端的 Tag 保护。
- 上游 `main` 更新只能触发审查和测试，不能直接进入用户更新源。
- GitHub 发布页直接列出本 Fork 更新，另设“上游更新”，按用户可见的功能与修复归并，不逐条列出
  提交、SHA、CI 调整或合并过程。每版先在 `.github/release-notes/versions/<Tag>.json`
  整理中文重点及来源；缺少版本说明会在构建前失败，不自动回退为提交清单。
- 上游重点依据本次实际引入的官方提交正文 `Changes` 编写，并用官方版本日志核对适用范围。
  正式版比较本 Fork 上一个正式版，重新纳入期间 Preview/RC 的重点；若跳过上游中间版本，
  重点仍须覆盖完整版本区间。预发布则比较上次实际发布。
  来源 SHA 仅供校验，正文末尾保留完整变更链接；安装命令置于折叠区。
- 生成器按 GitHub 发布类型核对比较基线、来源提交与上游范围，防止混入未来版本或重复基线已有内容。
  具体编写规则见 [Release 说明维护](.github/release-notes/README.md)。
- 正式更新日志、协议兼容检查、本 Fork 的 Windows 测试结果、Immutable Releases 开关和
  精确资产摘要共同构成发布门槛；官方 stable 指针不参与社区更新。

## 贡献

本项目当前处于社区维护准备阶段。提交翻译时请保留命令、配置键、协议字段、代码块、占位符和 URL，并优先修改集中式 locale 目录；不要在业务代码中逐处硬编码中文。

上游仓库的外部贡献政策见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。

## 许可证

本仓库第一方代码采用 **Apache License, Version 2.0**，详见 [`LICENSE`](LICENSE)。本 Fork 的修改继续遵守相同许可证，并保留上游版权和归属说明。

第三方及 vendored 代码保持各自原许可证，详见：

- [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES)
- [`crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md`](crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md)
- [`third_party/NOTICE`](third_party/NOTICE)
