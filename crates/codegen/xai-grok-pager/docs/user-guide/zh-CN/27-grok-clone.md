# grok clone

`grok-zh clone` 将 Git 仓库提取到 Grove 内容存储并挂载投影工作树（macOS 使用 NFS，Linux 使用 FUSE）。每次调用先读取当前进程的 `GROK_CLONE` / `GROVE_CLONE`，再检查统一启用设置 `GROK_GROVE` 或 Grok `~/.grok/config.toml` 的 `[cli] grove`，最后检查 Grove `~/.config/grove/config.toml` 的 `[clone] enabled`。

克隆的专用开关不启用会话 / `-w` 的 Grove 工作树；后者使用 `GROK_WORKTREE_TYPE` 和 `[cli] grove_worktree`，也不会反过来启用克隆。未设置专用开关时，可使用 `GROK_GROVE=1` 或 `[cli] grove = true` 同时启用两者。专用开关优先：`GROK_WORKTREE_TYPE=copy` 保持会话使用复制工作树而克隆仍可启用；`GROK_CLONE=0` 关闭克隆而不关闭会话工作树。

```bash
grok-zh clone <url> [dir] [--branch NAME] [--cone PATH]... [--full-history]
```

## 历史记录

默认会对所选分支执行**深度为 1 的初始克隆**（`blob:none` +
`--depth=1`）。只有该分支会被公布为远程跟踪引用。

如果克隆时需要完整提交历史、标签或全部远程分支，请使用
`--full-history`（这是之前的默认行为）。

完成深度为 1 的克隆后，以下命令只会加深**所选分支**：

```bash
git fetch --deepen=N origin
git fetch --unshallow origin
```

提取其他分支时，需要显式指定限制深度的 refspec。普通的
`git fetch origin` 或 `git fetch origin other` 不会通过默认 refspec
拉取该分支的完整历史：

```bash
git fetch --depth=1 origin refs/heads/NAME:refs/remotes/origin/NAME
```

默认浅克隆需要 Grove 守护进程支持 `clone_shallow` RPC。如果客户端拒绝，
请重启或更新守护进程（也可以改用 `--full-history`）。
