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

基于官方 [Grok Build](https://github.com/xai-org/grok-build) 的非官方简体中文社区版，为终端界面、设置、提示和用户文档提供中文支持。

[安装](#安装) · [使用与更新](#使用与更新) · [文档](#文档) · [下载与更新记录](https://github.com/JoyElliot/grok-build-Chinese/releases) · [反馈](#反馈)

![grok-zh 中文 TUI 工具链体检](docs/screenshots/grok-zh-toolchain-check.png)

</div>

## 特点

- **中文体验**：默认简体中文，中文请求优先生成中文会话标题和计划；可切换英文界面。
- **兼容官方版**：使用独立命令 `grok-zh`，保留原有命令参数、配置格式和协议。
- **译文独立更新**：公告、模型说明等中文映射可独立更新；未收录的内容回退到官方原文。
- **关闭辅助上传**：当前源码关闭遥测、反馈、trace 和自动云端会话上传。正常模型请求、主动分享和显式远程控制仍会联网，详见[隐私策略](docs/COMMUNITY-PRIVACY.md)。

> `grok-zh` 与官方 `grok` 共用 `~/.grok`（或 `GROK_HOME`），包括登录、会话和配置；在一方修改或删除数据会影响另一方。在线能力及账号权限仍由所用服务决定。

## 安装

正式版从[最新 Release](https://github.com/JoyElliot/grok-build-Chinese/releases/latest) 下载。请使用社区版完整包及其安装器；仓库内保留的上游安装脚本和 npm 包装面向官方 `grok`。

### Windows

支持 Windows x64、Windows PowerShell 5.1 和 PowerShell 7，无需管理员权限。在 PowerShell 中运行，按中文菜单安装或更新，也可创建便携版：

```powershell
$p=Join-Path $env:TEMP ('grok-zh-install-'+[guid]::NewGuid().ToString('N')+'.ps1'); $tls=[Net.ServicePointManager]::SecurityProtocol; try { [Net.ServicePointManager]::SecurityProtocol=$tls -bor [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -UseBasicParsing 'https://raw.githubusercontent.com/JoyElliot/grok-build-Chinese/zh-dev/packaging/windows/Install-GrokZhOnline.ps1' -OutFile $p; & "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File $p; if ($LASTEXITCODE -ne 0) { throw "安装未完成，退出码：$LASTEXITCODE" } } finally { [Net.ServicePointManager]::SecurityProtocol=$tls; Remove-Item -LiteralPath $p -Force -ErrorAction SilentlyContinue }
```

也可手动下载 Windows ZIP，解压后进入包目录，双击 `一键安装.cmd`。默认与官方版共存；安装完成后重新打开终端。

自定义目录、命令别名、便携版、旧版迁移及卸载见 [Windows 安装说明](packaging/windows/INSTALL-WINDOWS.md)。

### macOS 与 Linux

下载对应平台的完整包，按安装说明完成校验，解压并进入包目录后运行：

```sh
./Install-GrokZh.sh
```

| 平台 | 支持范围与详细步骤 |
| --- | --- |
| macOS | [Apple Silicon（M1 及后续机型）](packaging/macos/INSTALL-MACOS.md) |
| Linux | [x86_64 GNU](packaging/linux/INSTALL-LINUX.md) |

Windows 包未签名，macOS 包未签名或公证，首次运行可能出现系统安全提示；处理方式见对应安装说明。

## 使用与更新

```sh
grok-zh                 # 启动中文版
grok-zh --locale en-US  # 切换英文界面
grok-zh update          # 更新到社区版稳定版本
```

首次使用按提示登录，已有官方版登录状态可共用。后台自动更新默认关闭，可按 `Ctrl+U` 更新或在设置中开启；更新源仅为本仓库 Releases。

## 文档

- [中文用户指南](crates/codegen/xai-grok-pager/docs/user-guide/zh-CN/README.md) · [入门教程](crates/codegen/xai-grok-pager/docs/tutorial/zh-CN/)
- [社区版隐私策略](docs/COMMUNITY-PRIVACY.md)
- [源码构建与开发维护](docs/DEVELOPMENT.md)
- [贡献说明](CONTRIBUTING.zh-CN.md) · [安全报告](SECURITY.zh-CN.md)
- [官方在线文档](https://docs.x.ai/build/overview)

## 反馈

问题与汉化遗漏请提交 [Issue](https://github.com/JoyElliot/grok-build-Chinese/issues)，也欢迎参与 [Linux Do 社区讨论](https://linux.do/t/topic/2770188)。

## 许可证

第一方代码采用 [Apache License 2.0](LICENSE)，并保留上游版权与归属。第三方代码遵循各自许可证，见 [第三方声明](THIRD-PARTY-NOTICES)、[工具依赖声明](crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md)及 [vendored 代码声明](third_party/NOTICE)。
