# Windows x64 MSVC 迁移验收包（测试分支）

这是供下载后自行验收的**单版本测试包**，不是正式发布或默认安装程序。无需安装 Python、Rust、MinGW、gcc 或开发工具。使用 Windows x64 的 64 位 Windows PowerShell 5.1 或 PowerShell 7；无需管理员权限。建议至少保留 3 GB 空闲磁盘。

## 文件和来源

- `grok-zh-<版本>-windows-x86_64-gnu.zip`：本轮真实 GNU 兼容启动器，内部包含完整原生 MSVC ZIP。
- `grok-zh-<版本>-windows-x86_64-msvc.zip`：与上面内嵌内容逐字节一致的原生 MSVC 完整包。
- `SHA256SUMS.txt`：上述两个 ZIP、全部验收脚本、本文及来源记录的 SHA-256。
- `ACCEPTANCE-PROVENANCE.json`：版本、源码提交、仓库、CI run 链接、两个 ZIP 大小/摘要和验收范围。
- `Run-WindowsAcceptance.ps1`：独立自动验收入口。
- `Start-IsolatedTui.ps1`：显式选择的交互 TUI 入口，依赖同目录的自动验收脚本。

请只从维护者提供的 GitHub Actions run 下载此包，并核对来源记录中的提交和 run 链接。SHA-256 清单能发现内容损坏，不是发布签名，也不能单独证明来源可信。测试脚本会重新校验清单、ZIP、包协议、安装标记和实际程序。产品安装器来自经过校验的原生 ZIP；没有外部脚本下载或隐藏的源码仓库依赖。

## 运行自动验收

1. 将下载的 artifact ZIP 完整解压到一个全新目录，例如 `C:\grok-acceptance`。不要只取出 EXE，也不要直接在 ZIP 预览中运行。这个目录中应只有上面列出的 7 个文件；不要在其中保存输出日志。
2. 在该目录打开 **64 位** Windows PowerShell，然后运行：

```powershell
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File .\Run-WindowsAcceptance.ps1
```

或使用已安装的 PowerShell 7：

```powershell
pwsh.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File .\Run-WindowsAcceptance.ps1
```

`-ExecutionPolicy Bypass` 只用于这个新进程，不保存或更改系统执行策略。如果组织策略或杀毒软件阻止执行，请保留错误信息；不要关闭防护或更改组织策略。

脚本会在系统临时目录下生成新的 `grok-windows-acceptance-<随机编号>` 沙盒，并显示逐步进度。若希望放到另一块磁盘，先自行创建一个空的结果父目录，再指定：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\Run-WindowsAcceptance.ps1 -ResultsDirectory D:\acceptance-results
```

`ResultsDirectory` 必须已存在，且不能位于验收包目录内。脚本只在里面新建一个随机子目录，不覆盖之前的结果。不要双击产品 ZIP 中的“一键安装”或“替换原始启动方式”来替代本验收命令；那些是产品安装入口，采用不同的默认路径及选项。

## 自动验证的范围

1. 校验本轮 GNU/MSVC ZIP、脚本依赖和来源记录，拒绝额外文件、损坏清单及不安全的 ZIP 路径。
2. 执行真实 MSVC 的 `--version`、`--help`。
3. 在全新沙盒仅复制 GNU 启动器 EXE，模拟历史更新器**已经完成下载和提交之后**的 EXE-only 边界。确认迁移前 `--version` 不解包，首次 `--help` 完成离线迁移，生成的 MSVC EXE 与本轮原生包一致。
4. 在不存在的 TEMP 路径下再次执行 `--help`、`--version`、`agent --help`、`update --help`，验证后续启动不重复迁移，原 GNU 入口和现有运行时保持不变。这里的 `update --help` 只显示帮助，**不会运行更新**。
5. 使用完整 GNU 包在另一新沙盒目录运行真正的产品安装器，显式传入安装目录、隔离 GROK_HOME、`-NoPathUpdate` 和关闭官方命令接管，验证便携安装及安装归属标记。
6. 检查沙盒中合成的账号、配置、会话和个人文件保持原样；核对用户/系统 PATH 没有改变。

所有自动产品命令都只用新建的工作目录和数据目录，并清除继承的 provider 密钥及用户配置环境。脚本临时缩减当前进程 PATH、设置隔离 GROK_HOME/TEMP/用户目录，结束时恢复。不会读取或复制你的真实登录数据，不会调用登录、联网更新、默认目录安装、卸载或全局命令接管。沙盒是文件布局隔离，**不是操作系统安全隔离机制**；请只执行可信来源的包。

## 结果、失败和清理

- 成功时显示“自动验收通过”，`RESULT.json` 的 `status` 为 `passed`。
- 失败时返回非零退出码，`RESULT.json` 记录已完成检查及失败原因。`logs/progress.log` 有进度，`logs/command-*.log` 有非交互 CLI 输出、退出码及耗时。单个自动命令最多等待 180 秒。
- 不论通过还是失败，脚本都会保留沙盒及日志，便于检查；不会自动删除整个沙盒。产品安装器会按自身既有逻辑清理本次沙盒内部的临时文件。
- 向维护者反馈时，先检查并分享 `RESULT.json` 和 `logs` 即可。不要上传整个沙盒；可选交互登录后，`interactive-home` 可能含私人凭据。
- 确认无需保留后，可自行删除屏幕显示的**那个完整随机沙盒目录**。不要删除原来的 `.grok` 或其他安装目录。

## 可选：手动查看 TUI

自动验收不启动交互界面。若你明确要手动检查显示和操作，在普通终端中运行：

```powershell
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File .\Start-IsolatedTui.ps1
```

它会先在新沙盒重新完成自动验收，再启动已安装的真实 MSVC 程序。TUI 使用额外新建的空 `interactive-home` 和空工作目录，不继承现有 API 密钥，也不复制你的真实账户。它可能提示登录；是否登录、连接 provider 或发送请求由你自行决定，这些操作可能联网或产生服务费用。脚本不自动填写、授权或发送请求，也不录制交互屏幕。按程序正常退出方式关闭；返回的退出码仅作记录，不能作为 TUI/账号/provider 已验收的结论。

## 仍然未覆盖，不能据此启用正式迁移

本包只有一个真实版本。以下项目需要后续专门安排及额外真实资产/环境：

- 真实历史客户端从实际发布服务发现、下载并校验迁移版本
- 实际发布服务上的连续两次版本更新、第二版本及指定版本降级
- 真实账号、provider、TUI 中文显示和日常操作的人工验收
- 多 Windows 版本/机器、真实低磁盘空间、文件锁、杀毒拦截和中断恢复

没有使用替身 EXE 来冒充真实第二版本，也没有把编译器依赖的仓库 fixture 测试当成可独立运行的用户验收脚本。测试通过不表示这些待办已完成，不改变正式迁移默认关闭的策略，不会创建 tag 或 Release。
