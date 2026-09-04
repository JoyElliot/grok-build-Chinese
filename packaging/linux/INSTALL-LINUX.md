# Linux x86_64 GNU 使用说明

此软件包是 Grok Build 简体中文社区版的 Linux x86_64 GNU 构建，目标为
`x86_64-unknown-linux-gnu`。它不是 SpaceXAI 官方发行版。

## 校验与安装

GitHub Actions Artifact 或 Release 内含 `tar.gz` 和同名 `.sha256` 文件。
归档只含一个与归档同名（去掉 `.tar.gz`）的顶层目录。下载外层制品后，在这两个文件
所在目录运行（把示例版本替换为实际版本）：

安装器依赖 GNU coreutils、findutils、grep、sed、gawk、util-linux（`flock`）、
binutils、`file` 与 `sha256sum`；Ubuntu/WSL 可先运行：

    sudo apt-get install coreutils findutils grep sed gawk util-linux binutils file

    archive='grok-zh-1.0.13-linux-x86_64-gnu.tar.gz'
    package=${archive%.tar.gz}
    test -f "$archive" && test -f "$archive.sha256"
    sha256sum -c "$archive.sha256"
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
- 发布工作流会核验外层 `.sha256` 内容；更新器只接受本仓库不可变 GitHub Release 中
  名称、数量、URL、sidecar 元数据、GitHub digest、USTAR 结构、权限和内层
  `SHA256SUMS.txt` 全部匹配的 Linux 包。
- `release-v1.0.13` 是首个 Linux 统一稳定版。其发布二进制会移除调试信息，并在 CI 中
  同时检查旧版更新器的 512 MiB 单文件和 768 MiB 总解包上限。`release-v1.0.12` 因
  Linux 归档超过该上限而保留为预发布历史记录；稳定通道会跳过它。
- 稳定版 `v1.0.8` 是旧 Windows 客户端专用的两资产桥接版本。现代稳定
  `release-v*` 使用三平台六资产契约。
- 社区版默认不自动下载；可在明确接受相应通道后使用更新命令启用或执行更新。

## 安全与 WSL 边界

- `GROK_HOME` 必须是当前用户拥有的绝对非根路径；其父目录必须预先存在，且
  路径组件、安装目录和 `grok-zh`/`agent-zh` 入口不能是外部符号链接或未受管文件。
  默认安装不会触碰已有的官方 `grok`/`agent`；显式启用兼容入口时才会校验并管理它们。
- `GROK_HOME`、`bin`、`grok-zh-downloads` 必须能落实所有者检查及 `0700`
  权限。WSL 建议使用发行版 ext4 中的 `$HOME`；若 `/mnt/c`、`/mnt/e` 等 DrvFS
  挂载无法严格落实这些不变量，安装与自动更新会拒绝继续。
- 安装器保留旧的不可变版本目标，激活失败时回滚入口；不会删除用户文件、修改
  shell 启动文件，或覆盖官方 `grok`/`agent`，除非它们正是本安装器创建的兼容链接。
- 手动安装与程序内自动更新共用内核文件锁；同一 `GROK_HOME` 同时只能提交一次
  入口切换，进程崩溃后锁会由内核自动释放。
- 语音输入需要系统中可用的 PipeWire、PulseAudio 或 ALSA 录音工具；构建与
  基础命令运行不依赖这些工具。
