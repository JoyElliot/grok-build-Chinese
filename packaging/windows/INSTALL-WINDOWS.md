# Windows 自动安装说明

本说明适用于本仓库 GitHub Releases 或 `zh-dev` Actions 生成的未签名 Windows x64 GNU 包。
它不是 xAI 官方安装器，也不是 Authenticode 签名安装包。

## 下载与解压

1. 推荐从本仓库 [Releases](https://github.com/JoyElliot/grok-build-Chinese/releases)
   下载 `grok-zh-<version>-windows-x86_64-gnu.zip`。开发测试也可从
   **Actions → CI** 下载短期 Artifact。
2. 解压一次；后续 `release-v*` 版本（含预发布）只会得到一个
   `grok-zh-<version>-windows-x86_64-gnu` 顶层目录。打开该目录后再运行安装入口。
   仅用于让 `v1.0.5` 升级的 `v1.0.8` 桥接包因旧更新器兼容要求仍是扁平结构。
3. 确认包目录中恰好包含以下受管文件和目录（`SHA256SUMS.txt` 是包内清单，不列入它自身的哈希项）：

   ```text
   grok-zh.exe
   agent-zh.cmd
   rg.exe
   一键安装.cmd
   [可选]替换原始启动方式.cmd
   Install-GrokZh.ps1
   INSTALL-WINDOWS.md
   LICENSE-grok-build.txt
   BUILD-INFO.txt
   licenses/ripgrep/COPYING
   licenses/ripgrep/LICENSE-MIT
   licenses/ripgrep/UNLICENSE
   licenses/project/THIRD-PARTY-NOTICES
   licenses/project/THIRD_PARTY_NOTICES.md
   licenses/project/NOTICE
   SHA256SUMS.txt
   ```

`Install-GrokZh.ps1` 会在写入任何安装目录前，自动核对 `SHA256SUMS.txt` 中的
文件哈希。Release 只提供 ZIP 的 `.sha256` 侧车文件，GitHub API 也会记录 ZIP 资产 digest；
不会再额外发布一份版本化裸 EXE。

`v1.0.8` 桥接包的内层清单只列旧更新器认识的 7 个执行与安装入口；ZIP 仍物理包含上述
许可证和构建信息，完整 ZIP 继续受 GitHub digest 与外层 `.sha256` 保护。从
首个 `release-v*` 版本起，内层清单覆盖全部 15 个受管文件。

两个双击入口、安装脚本和本说明只在上述包目录中使用，不会复制到程序运行目录；
需要升级或调整安装方式时，请使用新下载并解压后的完整包。

正式 Tag 工作流会为 ZIP 与 `.sha256` 自动生成 GitHub Actions 构建来源证明。下载后可用
GitHub CLI 核对不可变 Release、资产和构建工作流；以下命令已使用当前仓库
`JoyElliot/grok-build-Chinese`；以下命令以现代稳定版 `1.0.13` 为例，发布后执行。
旧桥接版 `1.0.8` 使用 `v1.0.8` Tag，其余命令结构相同：

```powershell
$repo = 'JoyElliot/grok-build-Chinese'
$version = '1.0.13'
$tag = "release-v$version"
$zip = ".\grok-zh-$version-windows-x86_64-gnu.zip"
$assets = @($zip, "$zip.sha256")

gh release verify $tag --repo $repo
foreach ($asset in $assets) {
  gh release verify-asset $tag $asset --repo $repo
  gh attestation verify $asset --repo $repo `
    --signer-workflow "$repo/.github/workflows/zh-release-windows.yml" `
    --source-ref "refs/tags/$tag"
}
```

Artifact Attestation 不是 Windows Authenticode；未签名 EXE 仍可能触发 SmartScreen 提示。

## 一键安装：与官方版共存

在上述包目录中，直接双击：

```text
一键安装.cmd
```

它会从脚本所在目录启动 Windows PowerShell，只对这一次子进程使用
`ExecutionPolicy Bypass`，不会修改用户或计算机的永久执行策略。安装窗口会显示
SHA-256 完整性校验和程序复制百分比；成功或失败后都会等待按键再关闭，避免一闪而过。

默认安装位置是：

```text
%LOCALAPPDATA%\Programs\grok-zh\bin
```

安装器会把该目录放到当前用户的 `Path` 最前方，并同步当前 PowerShell 进程。
这不会写入 Machine 级 `Path`，不需要管理员权限。其他已经打开的终端需要重新
打开一次。

默认提供两个不会占用官方名称的命令：

```powershell
grok-zh --version
agent-zh --help
```

- `grok-zh` 启动中文版 TUI/CLI。
- `agent-zh` 是包装命令，等价于 `grok-zh agent ...`；例如
  `agent-zh stdio`、`agent-zh headless`。
- `rg.exe` 与 `grok-zh.exe` 保持在同一目录，供内置搜索使用。

安装完成后，按任意键关闭安装窗口。完全关闭已有的 PowerShell / Windows Terminal，
再打开一个新终端并输入：

```powershell
grok-zh
# 或
agent-zh
```

这里修改的是用户 `Path`，不是创建名为 `grok-zh` 或 `agent-zh` 的环境变量。

## 可选：替换原始启动方式

如果希望在终端中直接输入 `grok` 和 `agent` 时使用中文版，请回到上述包目录，再双击：

```text
[可选]替换原始启动方式.cmd
```

菜单提供以下选择：

1. **保留官方版（推荐）**：只在中文版安装目录创建 `grok.cmd`、`agent.cmd`，
   并把该目录置于当前用户 `Path` 首位；官方程序文件保持原样。
2. **备份并停用官方程序入口**：只处理 `%GROK_HOME%\bin` 中通过
   X.AI LLC Authenticode 签名验证的 `grok.exe`、`agent.exe`，先备份再移动，
   然后创建中文版兼容命令。来源无法验证的文件会被拒绝自动移动。
3. **取消**：不修改程序、Path 或用户数据。

两个安装方案都不会覆盖官方 `grok.exe` 或 `agent.exe`。如果使用方案 1，删除中文版
安装目录中的两个 shim，或重新运行不带接管选项的高级安装命令，即可停止命令接管。
如果使用方案 2，官方程序入口已经移入可恢复备份；停止命令接管后，还需按下方
“恢复官方命令”步骤手工移回，安装器不会擅自覆盖后来安装的官方版本。

这里只调整用户级 `Path`。如果同名程序来自更靠前的 Machine 级 `Path`，Windows
仍可能优先解析该程序；安装后必须用下面的命令核对实际结果。此时可卸载对应的
Machine 级安装，或继续显式使用 `grok-zh`、`agent-zh`，安装器不会擅自修改
Machine 级环境变量。

可用以下命令确认当前解析到哪个文件：

```powershell
Get-Command grok-zh, agent-zh, grok, agent -All
```

## 高级命令行选项

双击入口已经覆盖常用安装方式。需要自动化或自定义目录时，仍可在 PowerShell 中运行：

```powershell
Set-ExecutionPolicy -Scope Process Bypass
& .\Install-GrokZh.ps1

# 创建 grok、agent 兼容命令，但保留官方程序
& .\Install-GrokZh.ps1 -OverrideOfficialCommands
```

### 高级：备份并移走官方命令

如果除接管命令名外，还希望移除官方安装器放在共享目录中的两个入口，运行：

```powershell
& .\Install-GrokZh.ps1 -UninstallOfficial
```

`-UninstallOfficial` 会自动启用 `-OverrideOfficialCommands`，并且只检查：

```text
%GROK_HOME%\bin\grok.exe
%GROK_HOME%\bin\agent.exe
```

未设置 `GROK_HOME` 时，`%GROK_HOME%` 按 `%USERPROFILE%\.grok` 处理。找到的文件
不会直接删除，而会连同 SHA-256 记录一起移动到：

```text
%LOCALAPPDATA%\Programs\grok-zh\bin\official-backup\<timestamp>\
```

这项操作不会删除或修改共享的 `auth.json`、`config.toml`、会话、第三方 API、
MCP、插件、缓存或其他 `~/.grok` 数据。若文件正被占用，安装器会保留原文件并
提示关闭相关进程后重试，不会强行结束进程。

> `-UninstallOfficial` 是供高级用户使用的低级兼容参数：它按指定路径备份并移动
> 两个命令文件，不等同于 Windows“应用和功能”中的完整卸载。双击可选入口会额外
> 验证 X.AI LLC 签名，安全性更高，普通用户应优先使用菜单。

如果官方版由 npm 或其他包管理器安装，它们在其他目录中的 shim/包记录不会被
这个开关猜测性删除。可先检查：

```powershell
npm list -g --depth=0
Get-Command grok, agent -All
```

确认确实通过 npm 安装后，才单独执行：

```powershell
npm uninstall -g @xai-official/grok
```

## 恢复官方命令

备份目录中的 `official-backup.json` 记录了原路径和哈希。关闭相关进程后，可将
对应的 `grok.exe`、`agent.exe` 移回记录的 `original_path`。恢复前请先用
`Get-Command grok, agent -All` 检查是否已有同名文件，避免覆盖后来安装的版本。

## 自定义选项

```powershell
# 自定义程序安装目录
& .\Install-GrokZh.ps1 -InstallDir 'D:\Apps\grok-zh\bin'

# 指定要检查官方命令的共享数据根
& .\Install-GrokZh.ps1 -GrokHome 'D:\GrokData' -UninstallOfficial

# 绿色复制，不修改用户 Path
& .\Install-GrokZh.ps1 -InstallDir 'D:\Apps\grok-zh\bin' -NoPathUpdate

# 仅预览将执行的安装，不写文件
& .\Install-GrokZh.ps1 -WhatIf
```

如果目标目录已存在但不是本安装器创建的，安装器会拒绝覆盖。只有在你已经检查
该目录并确认可以整体替换时，才使用 `-Force`。升级已有社区安装时，旧目录会被
重命名为同级的 `bin.previous.<timestamp>-<id>`，便于回滚。

`-GrokHome`（以及环境变量 `GROK_HOME`）必须解析为绝对路径。安装器会展开已经
定义的 `%USERPROFILE%` 这类 Windows 环境变量；无法展开的 `%VAR%` 或字面的
`$env:VAR` 会被拒绝，请先在 PowerShell 中解析后再传入。
安装器只用该参数定位可选的官方 `grok.exe`、`agent.exe`，不会替你持久化
`GROK_HOME`；若程序运行时也要使用自定义数据根，请另行设置同值的环境变量。

## 通过 GitHub Releases 自动更新

安装带社区更新器的版本后，程序启动时会查询固定仓库
`JoyElliot/grok-build-Chinese`：

- 默认 `stable` 只接受 immutable、非 Draft、非 prerelease 的严格三段版本。`v1.0.8`
  是旧更新器可见的最后一个桥接 Tag；后续版本使用 `release-vA.B.C`，二进制报告的版本仍是
  `A.B.C`，不会增加第四段社区修订号；
- `grok-zh update --alpha` 可选择预发布通道，`grok-zh update --stable` 可切回稳定通道；
- Release 只接受精确命名的完整
  `grok-zh-<version>-windows-x86_64-gnu.zip` 及其 `.sha256` sidecar；更新器验证固定 URL、
  大小、GitHub SHA-256、安全 ZIP 布局和包内 `SHA256SUMS.txt`；
- 社区版的“自动更新”设置默认关闭。此时启动只查询版本并显示提示，不下载；欢迎页按
  `Ctrl+U` 后才退出旧 TUI、下载 ZIP 并执行更新。交互式下载会显示大小、百分比、速度和
  预计剩余时间；输出重定向或后台更新时进度条自动隐藏。显式开启该设置后才允许后台预下载；
- ZIP 中的候选 EXE 必须通过 `--version`，之后才使用 Windows 的重命名旁置和失败回滚逻辑
  替换当前 `grok-zh.exe`；不会强制结束其他会话；完成后重新运行 `grok-zh`；
- `v1.0.0-zh.preview.3` 的旧更新器只认识裸 EXE，需要手工下载完整 ZIP 迁移；已撤回的
  `v1.0.0.1` 使用了不兼容代理版本门禁的四段版本号，不应继续安装或分发。
- 已发布的 `v1.0.3`、`v1.0.5` 会先更新到仅含 Windows 两个资产、扁平 ZIP 和 7 项兼容清单的
  `v1.0.8`。从首个 `release-v*` 版本起，Windows、macOS、Linux 共用六资产 Release；旧客户端
  无法识别 `release-v*`，因此不会跳过桥接版本。

自动激活仍沿用原版的单 EXE 替换语义，但传输与验证只使用完整 ZIP；不会在运行中的安装
目录内逐个改写 `agent-zh.cmd`、`rg.exe`、安装文档、Path 或官方 `grok`/`agent`。若某个版本
要求同步升级这些旁载文件，应重新运行已下载 ZIP 内的安装器。

更新器不会读取 `GROK_INSTALLER` 来切换来源，也不会回退到 xAI npm、官方 GitHub、x.ai
或 GCS。网络、元数据、digest、解压、内层清单、候选运行或文件替换任一步失败都会保持
当前 EXE。

## 数据共享与安全边界

程序安装目录和用户数据目录是两件事：

- 中文版程序默认安装到 `%LOCALAPPDATA%\Programs\grok-zh\bin`；
- 官方版与中文版有意共用 `~/.grok`（或 `GROK_HOME`）；
- 登录状态、会话删除、配置、第三方 API、MCP、插件和本地状态会在两个入口之间
  立即同步，不存在复制或双向同步层；
- 不要为了卸载任一程序而删除整个 `~/.grok`。

当前 Windows 包没有 Authenticode 签名，CI 只构建、剥离、校验和打包，不启动完整 TUI。
Immutable Release 与 SHA-256 不能替代代码签名；首次运行前请查看 `BUILD-INFO.txt`、
Release Tag、构建提交和哈希记录。

> 仓库中的 `crates/codegen/xai-grok-pager/scripts/install.ps1`、`install.sh` 与
> `@xai-official/grok` 属于官方上游安装链，不能用于安装或更新本社区版。
