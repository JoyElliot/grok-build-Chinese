# macOS ARM64 安装说明

此软件包是 Grok Build 简体中文社区版的 Apple Silicon 构建，仅支持
`aarch64-apple-darwin`（M1 及后续 Apple Silicon）。它不是 SpaceXAI 官方发行版。

## 安全边界

- 软件包由 GitHub Actions 的 `macos-15` Apple Silicon runner 构建，并在 CI 中检查
  纯 ARM64 Mach-O 架构、包内文件、二次安装、入口链接和 `grok-zh --version`。
- 正式 Release 的归档由 GitHub 提供 SHA-256，包内保留 `SHA256SUMS.txt`。新版内置更新器
  校验本仓库不可变 Release 中当前平台归档的 digest，不依赖独立 `.sha256` 或其他平台附件。
- 当前构建未使用 Apple Developer ID 签名，也没有经过 Apple 公证。首次运行仍可能被
  Gatekeeper 阻止；安装器不会关闭 Gatekeeper、移除 quarantine 属性或修改系统安全设置。
- 默认与官方 `grok` 共用 `~/.grok` 数据目录，但程序入口与下载目录保持独立。

## 校验并安装

先运行 `shasum -a 256 <归档文件名>`，与 GitHub Release 对应附件旁显示的 SHA-256 核对一致。
兼容期内也可以下载同名 `.sha256` 并运行 `shasum -a 256 -c <归档文件名>.sha256`。
把下面的版本替换为实际下载版本，完成外层校验后再解包。
`release-v*` 归档只含一个与归档同名（去掉 `.tar.gz`）的顶层目录：

```sh
archive='grok-zh-1.0.13-macos-aarch64.tar.gz'
package=${archive%.tar.gz}
test -f "$archive"
test ! -e "$package"
tar -xzf "$archive"
cd "$package"
shasum -a 256 -c SHA256SUMS.txt
./grok-zh --version
./Install-GrokZh.sh
```

默认只在 `${GROK_HOME:-$HOME/.grok}/bin` 建立 `grok-zh`、`agent-zh` 两个入口，
不会修改 `.zshrc`、`.bashrc`、`/usr/local/bin` 或官方命令。若你明确希望在同一用户目录
中使用 `grok`、`agent` 兼容入口，可以运行：

```sh
./Install-GrokZh.sh --with-compat-aliases
```

完成外层校验后，安装器会验证精确的包内文件集合、内层清单、版本、架构和目标目录，
再把程序复制到新的不可变版本文件，最后
原子切换入口链接。在没有同用户并发篡改的情况下，已有普通文件或不属于本安装器的链接不会被覆盖。
手动安装与内置更新器共用 `bin/.grok-zh-install.lock` 内核锁；若另一项安装或更新正在进行，
安装器会停止并提示重试。锁在进程退出后自动释放，不应手工删除锁文件。
安装器通过系统 `/usr/bin/perl` 调用原生锁；缺少该系统组件时会停止，不会继续无锁安装。
安装完成后按提示把 `${GROK_HOME:-$HOME/.grok}/bin` 加入 `PATH`，重新打开终端并运行 `grok-zh`；启用兼容入口的用户也可以
运行 `grok`。

## 自动更新

- `grok-zh update` 和 TUI 更新入口只读取本仓库 Releases；不会访问官方 npm、x.ai、
  GCS 或官方 GitHub Release。
- macOS 更新不会覆盖当前进程正在使用的 Mach-O 文件。每次安装都会创建新的版本目标，
  校验并冒烟运行后，再原子切换 `grok-zh` 与 `agent-zh`。更新完成及后续启动会清理受管旧目标，
  保留当前入口、仍被进程使用的文件和一小时内创建的目标，避免干扰运行中的会话或并发安装。
- 手动更新完成后显示实际安装版本的 Release 正文；后台更新的正文在返回终端时显示一次。
- 后台自动下载默认关闭；用户可在设置中显式开启，或手动确认单次更新。
- 自动更新不等于 Apple 签名或公证。没有 Apple Developer ID 时仍能构建、校验、安装和
  更新，但 Gatekeeper 的首次运行提示不会因此消失。
- `release-v1.0.13` 起，macOS 与 Windows、Linux 共用统一稳定版的三平台六资产契约。
  `v1.0.8` 仍是旧 Windows 客户端专用的桥接版本，不含 macOS 资产。

每个平台保持一个安装包。独立 `.sha256` 在约两个月兼容期内保留，之后的新 Release
只公开三个平台归档；旧客户端先升级到永久保留的最后一个六资产过渡版，再升级后续版本。
维护约定见 [单包更新协议](https://github.com/JoyElliot/grok-build-Chinese/blob/zh-dev/docs/COMMUNITY-UPDATE-PROTOCOL.md)。

Actions Artifact 只用于预览测试，不是正式更新源。正式自动更新只消费本仓库统一发布
工作流生成的 Immutable GitHub Release。

自定义 `GROK_HOME` 必须是绝对、非根路径；其父目录必须已存在，现有路径组件
不能是符号链接，只允许最终的 `GROK_HOME` 目录由安装器或更新器创建。安装器
另外拒绝含冒号的路径，避免生成含多个目录项的 `PATH`。这是防目录劫持的安全边界。
