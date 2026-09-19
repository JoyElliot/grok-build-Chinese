<a id="terminal-support-and-troubleshooting"></a>
# 终端支持与故障排查

Grok Build 以全屏 TUI 运行。它依赖终端提供颜色、剪贴板、键盘输入、鼠标输入和全屏显示支持。不同终端、多路复用器、容器和 SSH 会话对这些功能的处理方式可能不同。

<a id="diagnose-and-fix-terminal-problems"></a>
## 诊断和修复终端问题

在 Grok 中运行 `/doctor`，检查当前会话并查看可用修复。如果 Grok 无法启动，请在 Shell 中运行 `grok-zh doctor`。使用 `grok-zh doctor --json` 获取机器可读的报告。

Doctor 会检查终端、多路复用器、颜色支持、键盘与换行行为、剪贴板路径，以及在包含音频捕获时的麦克风可用性。应用内命令还可以检查实时会话详情，例如通知焦点跟踪和沙箱配置文件冲突。

报告可能包含问题或建议，但仍会成功退出。通过管道传输时，`grok-zh doctor --json` 报告的颜色能力不变。麦克风检查不会开始录音，因此 Doctor 无法检测只有在捕获过程中表现为静音的 macOS 权限失败。

`/terminal-setup`、`/terminal-check` 和 `/terminal-info` 仍是 `/doctor` 的别名。

当 Doctor 发现明确不健康的 tmux 设置时，`/doctor fix` 会列出可用的自动修复。一次应用一个指定的修复，例如 `/doctor fix tmux-clipboard` 或 `grok-zh doctor fix dcs-passthrough --yes`。

Doctor 可以持久化以下四个 tmux 选项：

- `terminal.tmux-clipboard` —— `set -g set-clipboard on`
- `terminal.dcs-passthrough` —— `set -wg allow-passthrough on`
- `terminal.tmux-extended-keys` —— `set -g extended-keys on`
- `terminal.tmux-truecolor` —— `set -as terminal-features ",*:RGB"`

tmux 修复只会编辑托管受影响 tmux 服务器的计算机上的持久配置，包括远程会话。普通 tmux 使用真实的 `$HOME/.tmux.conf`；Byobu-tmux 使用其生效的 `BYOBU_CONFIG_DIR`，如果该目录不可用或不安全则拒绝猜测。Grok 会保留文件的换行符和模式，修改已有文件时创建备份，并拒绝有冲突或含糊不清的直接赋值。

Grok 特意**不会**运行 `tmux source-file`，也不会更改正在运行的 tmux 服务器。应用后使用显示的确切命令重新加载，或分离后重新连接，然后再次运行 `/doctor`。在重新加载前，实时检查项仍保持原状是预期行为。保守的配置扫描只检查直接的全局赋值；请自行检查被 source 的文件、条件语句、插件和生成的 tmux 设置。

---

<a id="detected-terminals"></a>
## 检测到的终端

Grok 从环境变量中检测以下终端模拟器：

- **Apple Terminal**
- **Ghostty**
- **iTerm2**
- **Warp**
- **WezTerm**
- **Kitty**
- **Alacritty**
- **Rio**
- **foot**（原生 Wayland，Linux）
- **VS Code**、**Cursor**、**Windsurf** 和 **Zed** 集成终端
- **JetBrains** IDE 终端
- **Grok Desktop**
- 基于 **VTE** 的终端，例如 GNOME Terminal、GNOME Console 和 Tilix
- **Windows Terminal**

检测有以下限制：

- 在 tmux 内部，标识外层终端的变量可能无法传递到 Grok。
- 通过 SSH 时，许多终端变量不会转发。
- tmux 的全局环境反映连接到服务器的第一个客户端，不一定是当前终端。

---

<a id="common-problems-and-fixes"></a>
## 常见问题与修复

<a id="colors-look-wrong-or-lack-truecolor"></a>
### 颜色显示错误或缺少 truecolor

运行 `/doctor`。完全支持的设置会显示 `color truecolor` 和 `themes all`。如果没有，Doctor 会显示检测到的限制及相关修复。

在 tmux 内部有两个不同的问题：Grok 发出的颜色是什么，以及颜色经过多路复用器后还能保留什么。`color` 行回答第一个问题。对于第二个问题，当连接的客户端没有标记为 `RGB` 时，tmux 会将每个 24 位颜色重写为外部终端 terminfo 声明的最近颜色，可能少至八种。即使 `color` 读取为 `truecolor`，主题仍会显得褪色。Doctor 会将此报告为 `terminal.tmux-truecolor`。重新加载 tmux 配置，然后分离并重新连接：服务器只会在重新加载时读取新选项，而客户端只会在连接时修正颜色深度，因此单独执行任何一步都不会改变结果。

<a id="clipboard-problems"></a>
### 剪贴板问题

Grok 最多通过三个路径写入剪贴板，这些路径会显示在 `/doctor` 的 **Clipboard** 下：

- **native** ——本地操作系统剪贴板。
- **tmux** ——Grok 在 tmux 内运行时的 tmux 粘贴缓冲区。
- **OSC 52** ——可穿过 tmux、容器或 SSH 的转义序列。

<a id="wayland"></a>
#### Wayland

现代 Wayland 合成器可以在终端未保持焦点时更新剪贴板。较旧的合成器可能要求 Grok 一直保持焦点，直到复制消息出现。适用时 Grok 会在启动时发出警告；运行 `/doctor` 查看检测状态和步骤。

`GROK_CLIPBOARD_NO_DATA_CONTROL=1` 是一种高级回退选项，会禁用 data-control 路径。复制随后使用命令行剪贴板工具。

<a id="osc-52-kill-switch"></a>
#### OSC 52 紧急开关

当该路径启用时，Grok 会在 Linux 上以及跨 tmux、SSH 或无显示容器时发出 OSC 52。不实现 OSC 52 的终端可能会把编码后的载荷显示为文本。在启动 Grok 前设置 `GROK_CLIPBOARD_NO_OSC52=1` 可禁用该路径。此时 `/doctor` 显示 `osc 52 off`；native 和 tmux 路径不变。

<a id="linux-x11-selections"></a>
#### Linux X11 选区

X11 的 **PRIMARY** 和 **CLIPBOARD** 是分开的：

- 未修改的中键点击只有在设置了 `DISPLAY` 时才读取 PRIMARY。在 XWayland 下，`xclip` 或 `xsel` 必须位于 `PATH` 中。
- `Ctrl+V` 读取 CLIPBOARD，绝不会回退到 PRIMARY。
- `Shift+Insert` 仍是终端的选中文本粘贴操作。

<a id="ssh-and-selected-text"></a>
#### SSH 与选中文本

远程 Grok 进程通常无法读取本地终端的选区。请使用终端原生的 `Shift+Insert`；如果终端用该手势绕过鼠标报告，也可以在中键点击时按住 `Shift`。

当 Grok 无法通过 SSH 识别外层终端时，它会预测将发送 OSC 52，但将该路径标记为未经验证。复制提示会命名备份文件，以便你取回文本。运行 `/doctor` 查看其他复制选项。

<a id="apple-terminal-over-ssh"></a>
#### Apple Terminal 通过 SSH

Apple Terminal 不支持 OSC 52，因此远程复制无法到达本地剪贴板。每次复制仍会保存到备份文件（默认是 `~/.grok/last-copy.txt`；使用 `GROK_COPY_FILE` 覆盖）；传送未经验证或剪贴板不可达时，提示会显示该路径。你也可以使用 `/copy <file>` 或 `/minimal`。

若要直接转发剪贴板，请在本地计算机上通过 `grok-zh wrap` 运行 SSH 命令，例如 `grok-zh wrap ssh user@host`。同一命令也可以包装容器和 pod Shell。连接意外断开后，它还会恢复终端模式。

不使用 `grok-zh wrap` 的 SSH 会话会显示一次性提示“运行 `/doctor` 查看详情和修复方法。”通过 wrap 启动会话后，该提示不再出现。可通过 `/settings` → **显示上下文提示** → **SSH wrap** 关闭，或在 `$GROK_HOME/config.toml` 的 `[ui.contextual_hints]` 下设置 `ssh_wrap = false`。此设置不会隐藏 Doctor 建议。

对于重复使用 SSH 的场景，Doctor 提供 `grok-zh doctor fix ssh-wrap`。它还会显示一次性命令、将要修改的文件，以及应绕过别名的情况。ID `terminal.ssh-wrap` 仍被接受，并会出现在 JSON 中。

> **警告**：`grok-zh wrap` 处于实验阶段，可能无法在每种设置中工作。

<a id="iterm2"></a>
#### iTerm2

iTerm2 可能需要授予 OSC 52 剪贴板访问权限。运行 `/doctor`；其中的 `terminal.iterm2-clipboard-permission` 建议会显示需要检查的设置。

<a id="fullscreen-or-alternate-screen-does-not-activate"></a>
### 全屏或备用屏幕未激活

Zellij 和 tmux 控制模式可能限制备用屏幕。Grok 通常会在这些环境中使用内联模式。运行 `/doctor` 查看检测到的情况。
你可以在 `~/.grok/pager.toml` 中配置 `[terminal] alt_screen`，或运行 `grok-zh --no-alt-screen` 确认内联模式可用。

<a id="zellij-keybindings-interfere-with-grok"></a>
### Zellij 快捷键干扰 Grok

Zellij 可能在 Ctrl/Alt 键到达 Grok 前拦截它们。在 Zellij 0.41 或更高版本中，使用 **Unlock-First (non-colliding)** 预设：

1. 按 `Ctrl+o`，然后按 `c`。
2. 打开 **Change Mode Behavior**。
3. 选择 **Unlock-First (non-colliding)**。
4. 按 `Enter` 应用。

需要 Zellij 自己的窗格或会话控制时，按 `Ctrl+g`。在精简模式中，如果 `Ctrl+G` 仍然无法到达 Grok，请打开命令面板并选择 **Edit Prompt in External Editor**。这样会保留当前草稿；输入 `/edit-prompt` 会开始一个空的编辑器草稿，因为命令本身占用了编辑框。

<a id="problem-ctrlenter-doesnt-interject-in-wezterm"></a>
<a id="ctrlenter-does-not-interject-in-wezterm"></a>
### Ctrl+Enter 在 WezTerm 中无法打断

WezTerm 默认禁用 Kitty 键盘协议。在 Grok 中运行 `/doctor`。`terminal.wezterm-kitty` 检查项会显示设置和重启步骤。通过 SSH 时，Doctor 只显示能在当前会话中工作的变通方法。
Apple Terminal 使用 `Ctrl+O` 进行打断，因为它无法区分带修饰键的 Enter 组合键。

<a id="shiftenter-does-not-insert-a-newline-in-vs-code"></a>
### Shift+Enter 在 VS Code 中无法插入换行

VS Code、Cursor、Windsurf 和 Zed 终端使用 xterm.js；它只部分实现 Kitty 键盘协议，并会错误编码某些带 Shift 的可打印键。因此 Grok 不会在那里协商该协议，Shift+Enter 可能会以与 Enter 相同的 `CR` 到达。使用 `Alt+Enter` 插入换行；`/doctor` 会报告 `terminal.newline-fallback`，并给出检测到的解释和变通方法。

<a id="mouse-scrolling-stops-working"></a>
### 鼠标滚动停止工作

如果 Grok 停止接收鼠标输入，请在终端中重新启用鼠标报告：

- **Apple Terminal**：**View → Allow Mouse Reporting**（`Cmd+R`）。
- **iTerm2**：**Settings → Profiles → Terminal → Enable mouse reporting**。

<a id="voice-dictation-records-nothing"></a>
### 语音听写没有录音

大约 10 秒没有转录内容后，Grok 会停止捕获，并显示 **“未检测到语音。语音已停止。”** 以及麦克风修复步骤。在 macOS 上，被拒绝的麦克风授权可能看起来和静音一样，因为权限属于承载 Grok 的终端。打开 **System Settings → Privacy & Security → Microphone**，启用该终端并重新启动它。如果访问权限已开启，请在 **System Settings → Sound → Input** 下检查输入设备和音量，然后重试。

运行 `grok-zh doctor`，或在语音模式开启时运行 `/doctor`。**Voice** 部分会显示 Grok 将使用的麦克风。如果没有可用输入设备，Doctor 会显示 `voice.no-input-device` 及后续步骤。当 macOS 被动提供静音时，Doctor 无法检测被拒绝的 macOS 麦克风访问权限。

在 macOS 上，每次听写都会使用一个短生命周期的捕获辅助进程，因此捕获结束时音频栈的内存会释放。如果辅助进程本身可能有问题，可设置 `GROK_VOICE_CAPTURE=inprocess`，使用进程内回退进行对比。

<a id="byobu-with-gnu-screen"></a>
### Byobu 与 GNU screen

GNU screen 上的 Byobu 支持有限。`/doctor` 会报告 `terminal.byobu-screen`，并说明如何切换到 Byobu 的 tmux 后端。

---

<a id="still-stuck"></a>
## 仍然卡住？

运行 `/feedback` 进行报告。

## Cmd+Enter 不作为约定的发送或换行快捷键

Grok 只将 `Shift+Enter` 和 `Alt+Enter` 作为约定的换行键。很多终端把 Cmd+Enter 用于全屏，因此换行匹配器排除 SUPER，带 SUPER 的 Enter 也不匹配普通 Enter 发送绑定。当 Kitty 等终端实际传来 SUPER+Enter 时，按键会落到把 Enter 视为换行的文本区，因而仍插入换行。Apple Terminal 则有独立的本机 CoreGraphics 修饰键补救路径，按住 Cmd 时可把看似普通 Enter 的事件处理为换行。

SSH 不传递 Cmd 修饰键，远程端收到的可能是普通 Enter，从而发送消息。草稿非空时底栏显示可用换行组合；SSH 优先显示 Alt+Enter。也可以输入 `\` 再按 Enter，或启用 `/ml`。不要依赖远程 Cmd+Enter 换行。
