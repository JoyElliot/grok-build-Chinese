# CI 轻量门禁与预览制品保留

## 工作流入口

- 主入口仍是 [CI](../.github/workflows/zh-dev-windows-preview.yml)。原“同步资料完整性”工作流已并入最早的 `localization` job，保留原有 39 项 Python 测试，并增加工作流与固定保留期限回归测试。
- “等待闲置 runner”（`zh-dev-priority-gate`）、“同步资料完整性”（`localization`）和“Rust 格式预检”（`rust-format-preflight`）并行运行。10 个重型入口（矩阵展开为 15 个作业）必须等待三项全部成功，再由 GitHub 按 runner 可用情况调度；原生打包与产物验证仍等待对应构建完成。最终汇总显式检查三项结果。
- `zh-dev`、原 macOS 冒烟分支、面向 `zh-dev` 的 PR、每五天的缓存预热和手动构建语义保持原样。
- `sync/**` 的 push 只运行轻量检查，不启动六平台编译，也不运行会将被跳过构建误报为失败的汇总 job。同步分支现在覆盖所有非公告/动态译文路径，比旧入口的指定路径范围更广。
- 不再运行自定义制品清理。制品按上传时设置的固定期限由 GitHub 原生到期机制处理；新构建成功不会提前删除旧预览。

## 固定保留期限

六平台预览包上传时设置 90 天：

1. Windows x64 GNU：`grok-zh-windows-{run_id}-{attempt}`
2. Windows ARM64 MSVC：`grok-zh-{version}-windows-aarch64-msvc-{run_id}-{attempt}`
3. Linux x64 GNU：`grok-zh-{version}-linux-x86_64-gnu-{run_id}-{attempt}`
4. Linux ARM64 GNU：`grok-zh-{version}-linux-aarch64-gnu-{run_id}-{attempt}`
5. macOS ARM64：`grok-zh-{version}-macos-aarch64-{run_id}-{attempt}`
6. macOS Intel：`grok-zh-{version}-macos-x86_64-{run_id}-{attempt}`

对应 Windows 调试符号 `grok-zh-windows-symbols-{run_id}-{attempt}` 上传时设置 30 天。GNU/MSVC 交叉编译中间包保留 7 天。

预览诊断仅在相应构建/验证遇到失败时上传，不因手动触发或正常成功而上传；纯取消不作为失败上传的理由。最终失败处理位于监控结束、timings、打包、验证与缓存处理之后，覆盖这些后期步骤的失败，并保留已产生的证据。既有诊断上传期限保持原样（通常 7 天，Windows 验证 14 天）。Linux 预览的长期产品包不混入短期诊断文本。

独立 Windows MSVC 迁移实验产品仍为 14 天。共享构建 action 的产品保留天数参数默认仍为 14 天，预览诊断处理与正式 Release 隔离；Release 调用与上传条件保持原样。Cargo 缓存保存前清理本仓 crate 产物的既有逻辑继续保留。

已有制品继续按原有到期时间处理，包括此前只设置 7/14 天的旧预览。移除自动化不会删除已有制品、历史工作流运行、日志、缓存、Tag 或 Release，也不会延长已上传制品的期限。上传仍须符合仓库的保留设置和 GitHub 的服务限制；固定保留天数并非总存储容量承诺。

## 验证

本地验证覆盖三项预检并行、任一预检未通过时阻止重型任务、`sync/**` 轻量检查、六平台产品及符号的固定期限、失败诊断、Release 隔离，以及已移除自动化不存在且无悬空引用：

```sh
python3 -B -m unittest discover -s .github/scripts/tests -p 'test_*.py'
python3 -B packaging/macos/tests/test_install_lock.py
```

这些验证不会调用 GitHub API，也不会删除在线制品。代码修改需要发布后才会改变默认分支的工作流；本地验证不代表已经发布。

参考：[Actions 限额](https://docs.github.com/en/actions/reference/limits)。
