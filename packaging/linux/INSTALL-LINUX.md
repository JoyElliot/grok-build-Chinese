# Linux GNU 使用说明

此软件包是 Grok Build 简体中文社区版的 Linux GNU 构建，按设备选用
`x86_64-unknown-linux-gnu` 或 `aarch64-unknown-linux-gnu`。它不是 SpaceXAI 官方发行版。

## 校验与安装

下载 `tar.gz` 后，先运行 `sha256sum <归档文件名>`，与 GitHub Release 对应附件旁显示的
SHA-256 核对一致。兼容期内也可下载同名 `.sha256` 并运行 `sha256sum -c <归档文件名>.sha256`。
归档只含一个与归档同名（去掉 `.tar.gz`）的顶层目录。完成外层校验后，在归档
所在目录运行以下命令（把示例版本替换为实际版本）：

安装器依赖 GNU coreutils、findutils、grep、sed、gawk、util-linux（`flock`）、
binutils、`file` 与 `sha256sum`；Ubuntu/WSL 可先运行：

    sudo apt-get install coreutils findutils grep sed gawk util-linux binutils file

    archive='grok-zh-<版本>-linux-<x86_64 或 aarch64>-gnu.tar.gz'
    package=${archive%.tar.gz}
    test -f "$archive"
    test ! -e "$package"
    tar -xzf "$archive"
    cd "$package"
    sha256sum -c SHA256SUMS.txt
    ./grok-zh --version
    ./Install-GrokZh.sh

安装器会把每个已验证版本保存为 `~/.grok/grok-zh-downloads` 下的不可变目标，
并原子切换 `~/.grok/bin/grok-zh` 与 `agent-zh`。它不会修改 shell 配置、不会
使用 `sudo`，也不会写入 `/usr/local/bin`。请按安装器最后的提示把
`~/.grok/bin` 加入 `PATH`。

如果希望与官方命令名一致，可在首次安装或重装时显式启用兼容入口：

    ./Install-GrokZh.sh --with-compat-aliases

这会在同一私有目录中额外建立 `grok -> grok-zh` 与 `agent -> agent-zh`。
后续自动更新只切换公共的 `grok-zh` 版本目标，因此四个入口始终收敛到同一版本。
以后不带参数重装时，安装器会保留已经启用且状态完整的兼容入口。

## 自动更新与发布通道

- CI 预览包可用于安装验收，但 Actions Artifact 本身不是自动更新目标。
- 更新器校验本仓库不可变 GitHub Release 中当前平台归档的名称、URL、GitHub digest、
  包内声明协议、USTAR 结构、权限和 `SHA256SUMS.txt`；新版不要求独立 sidecar 或其他平台附件。
- `release-v1.0.13` 是首个 Linux 统一稳定版。其发布二进制会移除调试信息，并在 CI 中
  同时检查旧版更新器的 512 MiB 单文件和 768 MiB 总解包上限。`release-v1.0.12` 因
  Linux 归档超过该上限而保留为预发布历史记录；稳定通道会跳过它。
- 稳定版 `v1.0.8` 是旧 Windows 客户端专用的两资产桥接版本。历史三平台
  `release-v*` 使用六附件契约；Linux ARM64 从六平台正式版开始提供。
- 社区版默认不自动下载；可在明确接受相应通道后使用更新命令启用或执行更新。

每个平台保持一个安装包。独立 `.sha256` 在约两个月兼容期内保留，之后的新 Release
只公开各平台归档；旧客户端先升级到永久保留的三平台六附件过渡版，再升级后续版本。
维护约定见 [单包更新协议](https://github.com/JoyElliot/grok-build-Chinese/blob/zh-dev/docs/COMMUNITY-UPDATE-PROTOCOL.md)。

## 安全与 WSL 边界

- `GROK_HOME` 必须是当前用户拥有的绝对非根路径；其父目录必须预先存在，且
  路径组件、安装目录和 `grok-zh`/`agent-zh` 入口不能是外部符号链接或未受管文件。
  默认安装不会触碰已有的官方 `grok`/`agent`；显式启用兼容入口时才会校验并管理它们。
- `GROK_HOME`、`bin`、`grok-zh-downloads` 必须能落实所有者检查及 `0700`
  权限。WSL 建议使用发行版 ext4 中的 `$HOME`；若 `/mnt/c`、`/mnt/e` 等 DrvFS
  挂载无法严格落实这些不变量，安装与自动更新会拒绝继续。
- 安装器使用不可变版本目标，激活失败时回滚入口。内置更新成功及后续启动会清理受管旧目标，
  保留当前入口、仍被进程使用的文件和一小时内创建的目标；不会删除用户文件、修改
  shell 启动文件，或覆盖官方 `grok`/`agent`，除非它们正是本安装器创建的兼容链接。
- 手动安装与程序内自动更新共用内核文件锁；同一 `GROK_HOME` 同时只能提交一次
  入口切换，进程崩溃后锁会由内核自动释放。
- 语音输入需要系统中可用的 PipeWire、PulseAudio 或 ALSA 录音工具；构建与
  基础命令运行不依赖这些工具。
