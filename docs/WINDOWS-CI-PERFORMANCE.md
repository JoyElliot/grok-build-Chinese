# Windows CI 并行验证与编译

Windows 预览 CI 和正式 Release 都将验证与产物编译分成两个独立的 `windows-2022` 作业。每个作业分配自己的 GitHub 托管 runner、CPU、内存和临时目录，各自使用 `-j4`；验证与编译之间没有 `needs` 依赖。

```mermaid
flowchart LR
    V[Windows 验证 J4] --> G[Windows x64 GNU 预览版]
    B[Windows 编译打包 J4] --> G
    G --> S[三端构建汇总]
    L[Linux 构建] --> S
    M[macOS 构建] --> S
```

- `windows-gnu-validation` 保留 Windows PowerShell 5.1 / PowerShell 7 安装器测试、包协议、发布策略、发布说明、本地化检查及全部 12 条 Cargo 测试命令。
- `windows-gnu-build` 保留预览编译参数、资源监控、包内协议与文件清单、打包和制品上传。编译与验证使用相同的 CI 版本号和工具链。
- 原来的 `windows-gnu-preview` ID 和“Windows x64 GNU 预览版”名称作为聚合检查保留。它在两个作业结束后执行；任一结果为失败、取消或跳过时，该检查失败。三端汇总继续依赖它。
- 编译制品可能先于验证作业完成上传；完整验收以 Windows 聚合检查和三端汇总结果为准。

正式 Release 使用 `windows-x64-gnu-validation` 与 `windows-x64-gnu`，两者都只依赖 `release-plan`。验证作业保留原有 15 条 Cargo 测试命令、发布说明、格式、包协议、发布策略及 PowerShell 5.1/7 安装器检查；编译作业保留正式 ZIP 打包、哈希和版本冒烟。`release-attestations` 与 `release-publisher` 均显式要求验证、编译成功，验证失败、取消或跳过时不能生成发布证明或发布 Release。三端仍并行构建。

预览与正式验证都运行 `python .github/scripts/tests/test_release_workflow.py`，覆盖发布依赖失败、取消、跳过及旧版桥接例外，防止验证与编译拆分后绕过发布门禁。

## 共用准备与隔离边界

预览与正式的 Windows 作业共同调用 `.github/actions/setup-windows-gnu`，统一选择版本、准备固定 Rust / MinGW / protoc、恢复 Cargo registry/git，并各自在自己的 runner 获取依赖，以满足后续 `--frozen` 调用。正式作业检出 `release-plan` 校验的提交，并显式传入 Tag 对应版本，不生成 CI 版本号。

验证作业只读取依赖缓存，不恢复或写入 release 编译缓存。编译作业复用已有 target / host 缓存键与恢复规则；正式作业在摘要中记录精确命中和实际恢复的键，仅受信任预览作业保存缓存，避免并行写入竞争。测试 feature、增量开关和 Windows 产物编译 profile 保持原配置。

两个作业的 `-j4` 不竞争同一台 runner 的 CPU。将编译降到 `-j2` 不会给独立的验证 runner 增加资源；应先根据两组实际耗时评估，并计入 Linux、macOS、准备和排队时间。并行缩短的是等待时间，并不等于减少总计算量。

手动触发工作流时，可启用 `parallel_run`，让本轮使用包含运行 ID 的独立并发组，保留同分支仍在执行的其他 CI。该开关默认关闭，日常推送和普通手动触发继续沿用原来的同分支取消规则。不同运行的制品带各自运行 ID，验收时仍需核对对应提交。

## Windows 验证编译配置

预览和正式 Release 的验证作业都设置 `CARGO_PROFILE_TEST_DEBUG=0`，省去 workspace 测试编译原本继承自 dev profile 的 `line-tables-only` 调试信息；第三方依赖原本已关闭该信息。这会减少测试产物生成与链接的数据量，具体耗时收益以 CI 对比为准。测试的优化级别、debug assertions 和溢出检查保持原配置，运行时回溯中的源码行号信息可能减少；本地开发和产物编译不受此作业环境变量影响。

原有 12 条 Cargo 测试命令的 package、feature、过滤条件和顺序全部保留，按本地化运行库、更新器、Shell、完整界面、精简界面及免费账号选项拆分为独立步骤。每条命令增加 `--timings`，运行结束后上传 `grok-zh-windows-validation-<run_id>-<run_attempt>`，包含 Cargo 生成的 HTML 编译报告和 `summary.json`。后者记录提交、运行次数、验证配置、debug 目录文件数及未压缩总字节数；测试中途失败时也会尽力收集，目录不存在或统计失败时体积保持 `null`。

正式 Release 的 15 条 Cargo 测试保持原顺序并增加 `--timings`，验证诊断为 `grok-zh-windows-release-validation-<run_id>-<run_attempt>`；产物编译 timings 单独上传为 `grok-zh-windows-release-build-<run_id>-<run_attempt>`。这些诊断不进入安装包。正式流程改动的实际提速以下次正常发布为准，不为验证耗时重发已有版本。

当前没有 Windows debug 编译缓存。先通过诊断制品确认编译热点及目录体积，再评估缓存容量与恢复收益；未压缩目录体积不能直接作为实际缓存占用。对比时将拆分后的元数据与测试步骤耗时合计，与原来的单一步骤比较，并分别记录整个验证作业和 CI 汇总耗时。

## 串行基线

基线为 [CI 34709545996](https://github.com/JoyElliot/grok-build-Chinese/actions/runs/34709545996)，提交 `e84eb5af2ed8410458f205d0cecff4ad4d63a1d5`。该轮三端构建和汇总全部成功。

| Windows 阶段 | 耗时 |
| --- | --- |
| 本地化验证及测试编译 | 43 分 45 秒 |
| 产物编译及资源监控 | 33 分 17 秒 |
| 整个 Windows 作业 | 81 分 44 秒 |

并行收益须以新 CI 中两个 Windows 作业的开始/结束时间、同阶段日志和最终汇总时间验证；两个作业各自进行准备和依赖恢复，不能直接把基线的两个阶段相减当作实际收益。
