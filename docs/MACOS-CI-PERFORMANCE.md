# macOS CI 构建耗时

macOS 复合 action 由预览和正式 Release 工作流共同调用，两者复用已经通过 CI 的编译配置。`release_build` 继续决定正式版本要求、构建信息中的模式标记及缓存写入权限；正式调用仍传入 `true`，平台、安装包布局和更新协议保持原约定。

| 调用 | 编译配置 | 缓存 |
| --- | --- | --- |
| 正式 Release（`release_build: 'true'`） | `release` + `release-dist` features；关闭跨 crate LTO 和 debug，codegen-units=16；shell 使用 opt-level=1/codegen-units=16 | 读取与预览相同的已验证配置缓存，不写缓存 |
| CI 预览（`release_build: 'false'`） | 与正式 Release 相同 | 保留已有 preview 缓存键及 `release` 目录，受信任事件可写缓存 |

正式 macOS 包也采用上述配置，实际参数和 Release/CI preview 模式写入包内 `BUILD-INFO.txt`；`release-dist` features 不等于同名 Cargo profile。Windows 产物现已单独采用上游 Thin LTO `release-dist` profile，具体见 [Windows 构建说明](WINDOWS-CI-PERFORMANCE.md)。macOS 工作流仍通过显式参数选择本页配置，根 Cargo.toml 中的 Thin LTO profile 保持原定义；两种 macOS 模式均保留 ARM64 Mach-O、版本、归档校验及安装器冒烟。

## 缓存与测量

- `zh-dev` 的 push/手动预览，以及 `sync/upstream-*` 的手动预览成功后，可保存依赖、目标产物和宿主构建缓存。PR、其他事件/分支和正式 Release 不写入这些缓存。
- 编译缓存继续按工具链、目标、配置、构建输入和 Cargo.lock 区分。沿用现有 `preview-release-lto0-debug0-cgu16-shellopt1-shellcgu16-v1` 键，使正式构建能读取已验证的同配置缓存；键中的 preview 是历史命名。是否实际恢复缓存仍以当次日志为准。
- 维持 3 路 Cargo 并行和 `CARGO_INCREMENTAL=0`。没有增加整个 debug/test 目录缓存，避免未经测量扩大缓存体积。
- `macos-build.py` 添加 `--timings`，以 10 秒为目标间隔记录 Cargo 及其子进程的 RSS 总和、rustc 数量和 runner swap 用量，CSV 保留实际采样时刻。采样 RSS 会重复计算共享页，也可能错过瞬时峰值；不能作为独占内存或精确峰值。
- 探测失败或进程已消失时保留为 `null` 并记录原因，不记作零。Cargo 的失败退出码直接传回工作流；取消或异常时清理独立进程组，覆盖 Cargo 和编译器后代。
- 独立的 `grok-zh-macos-build-monitor-<run>-<attempt>` artifact 包含 `runner.json`、`samples.csv`、`summary.json` 和 Cargo timings。诊断文件不加入安装归档。

## 优化前基线

基线为 [CI 34700710020](https://github.com/JoyElliot/grok-build-Chinese/actions/runs/34700710020)，提交 `7a2eb52e485d13c7b48c0a79a374eafe33703bdc`，版本 `1.0.24-zh.ci.88`。三平台及汇总成功，专用分支限定的 macOS 真账号冒烟按规则跳过。

| macOS 阶段 | 耗时 |
| --- | --- |
| 三类缓存恢复（均回退到已有缓存） | 2 分 04 秒 |
| 格式与基础本地化验证 | 9 分 29 秒 |
| Cargo `release-dist` 编译 | **132 分 02 秒** |
| 打包和安装器验证 | 19 秒 |
| 上传软件包 | 3 秒 |

优化收益须用后续同目标、同工具链 CI 的实际构建日志验证，并同时注明缓存命中情况。这里的基线是构建耗时，不是终端 UI 帧时间或运行时性能基准。

## 本地验证

```text
python .github/scripts/tests/test_macos_build.py
python .github/scripts/tests/test_package_protocol.py
```

macOS 的真实编译、资源采样、Mach-O、权限和安装器验证由 Apple Silicon CI 执行。测试覆盖正式与预览命令及缓存兼容性，并单独验证正式模式仍不可写缓存。流程更新不重发已有 Release，下一次正式标签运行才验证新的正式构建耗时。

## 包体积与显式对照配置

发布副本现在通过局部符号裁剪、运行信息等价检查和 ad-hoc 重签减小体积，诊断映射并入原有 build-monitor artifact。默认编译参数不因符号裁剪而变化。

手动 CI 的 `macos_build_variant=thin-lto` 仅用于构建对照：使用 `release-dist` profile、debug=0、独立且只读的编译缓存；正式 Release 拒绝该实验选项。默认值 `current` 保持上表配置。两种配置均保留原有测试、features、安装包和门禁，也不拆分 macOS 串行作业。验证方法与边界见 [Unix 包体积说明](UNIX-BINARY-SIZE.md)。
