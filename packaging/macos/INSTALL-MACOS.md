# macOS ARM64 安装说明

此软件包是 Grok Build 简体中文社区版的 Apple Silicon 构建，仅支持
`aarch64-apple-darwin`（M1 及后续 Apple Silicon）。它不是 SpaceXAI 官方发行版。

## 安全边界

- 软件包由 GitHub Actions 的 `macos-15` Apple Silicon runner 构建，并在 CI 中检查
  纯 ARM64 Mach-O 架构、包内文件、二次安装、入口链接和 `grok-zh --version`。
- 正式 Release 同时提供外层 `.sha256`，包内提供严格的 `SHA256SUMS.txt`。内置更新器
  只接受本仓库中不可变、资产集合完整且 GitHub SHA-256 元数据匹配的 Release。
- 当前构建未使用 Apple Developer ID 签名，也没有经过 Apple 公证。首次运行仍可能被
  Gatekeeper 阻止；安装器不会关闭 Gatekeeper、移除 quarantine 属性或修改系统安全设置。
- 默认与官方 `grok` 共用 `~/.grok` 数据目录，但程序入口与下载目录保持独立。

## 校验并安装

把 `.tar.gz` 和同名 `.sha256` 放在同一目录，把下面的版本替换为实际下载版本后再校验并解包。
`release-v*` 归档只含一个与归档同名（去掉 `.tar.gz`）的顶层目录：

```sh
archive='grok-zh-1.0.13-macos-aarch64.tar.gz'
package=${archive%.tar.gz}
test -f "$archive" && test -f "$archive.sha256"
shasum -a 256 -c "$archive.sha256"
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
安装完成后按提示把 `${GROK_HOME:-$HOME/.grok}/bin` 加入 `PATH`，重新打开终端并运行 `grok-zh`；启用兼容入口的用户也可以
运行 `grok`。

## 自动更新

- `grok-zh update` 和 TUI 更新入口只读取本仓库 Releases；不会访问官方 npm、x.ai、
  GCS 或官方 GitHub Release。
- macOS 更新不会覆盖当前进程正在使用的 Mach-O 文件。每次安装都会创建新的版本目标，
  校验并冒烟运行后，再原子切换 `grok-zh` 与 `agent-zh`。更新完成后的清理通常保留当前
  版本和一个上一版本；刚创建的目标还受保护时间窗约束，不会在切换过程中被提前删除。
- 后台自动下载默认关闭；用户可在设置中显式开启，或手动确认单次更新。
- 自动更新不等于 Apple 签名或公证。没有 Apple Developer ID 时仍能构建、校验、安装和
  更新，但 Gatekeeper 的首次运行提示不会因此消失。
- `release-v1.0.13` 起，macOS 与 Windows、Linux 共用统一稳定版的三平台六资产契约。
  `v1.0.8` 仍是旧 Windows 客户端专用的桥接版本，不含 macOS 资产。

Actions Artifact 只用于预览测试，不是正式更新源。正式自动更新只消费本仓库统一发布
工作流生成的 Immutable GitHub Release。

自定义 `GROK_HOME` 必须是绝对、非根路径；其父目录必须已存在，现有路径组件
不能是符号链接，只允许最终的 `GROK_HOME` 目录由安装器或更新器创建。安装器
另外拒绝含冒号的路径，避免生成含多个目录项的 `PATH`。这是防目录劫持的安全边界。
