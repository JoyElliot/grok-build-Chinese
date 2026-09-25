# macOS CI 构建耗时

macOS 复合 action 由预览和正式 Release 工作流共同调用，两者复用已经通过 CI 的编译配置。`release_build` 继续决定正式版本要求、构建信息中的模式标记及缓存写入权限；正式调用仍传入 `true`，平台、安装包布局和更新协议保持原约定。

| 调用 | 编译配置 | 缓存 |
| --- | --- | --- |
| 正式 Release（`release_build: 'true'`） | `release` + `release-dist` features；关闭跨 crate LTO 和 debug，codegen-units=16；shell 使用 opt-level=1/codegen-units=16 | 读取与预览相同的已验证配置缓存，不写缓存 |
| CI 预览（`release_build: 'false'`） | 与正式 Release 相同 | 保留已有 preview 缓存键及 `release` 目录，受信任事件可写缓存 |

正式 macOS 包也采用上述配置，实际参数和 Release/CI preview 模式写入包内 `BUILD-INFO.txt`；`release-dist` features 不等于同名 Cargo profile。Windows 产物现已单独采用上游 Thin LTO `release-dist` profile，具体见 [Windows 构建说明](WINDOWS-CI-PERFORMANCE.md)。macOS 工作流仍通过显式参数选择本页配置，根 Cargo.toml 中的 Thin LTO profile 保持原定义；两种 macOS 模式均保留目标架构 Mach-O、版本、归档校验及安装器冒烟。

## 缓存与测量

- `zh-dev` 的 push/手动预览、`sync/upstream-*` 的手动预览，以及同仓库非 Dependabot PR 成功后，可保存依赖、目标产物和宿主构建缓存。fork PR、其他事件/分支和正式 Release 不写入这些缓存。PR 缓存受 GitHub 的 merge ref 隔离，只供同一 PR 后续运行恢复。
- 编译缓存继续按工具链、目标、配置、构建输入和 Cargo.lock 区分。沿用现有 `preview-release-lto0-debug0-cgu16-shellopt1-shellcgu16-v1` 键，使正式构建能读取已验证的同配置缓存；键中的 preview 是历史命名。是否实际恢复缓存仍以当次日志为准。
- ARM64 宿主使用 3 路 Cargo 并行；Intel 原生预览测试使用 4 路。保留 `CARGO_INCREMENTAL=0`，没有增加整个 debug/test 目录缓存。
- `macos-build.py` 添加 `--timings`，以 10 秒为目标间隔记录 Cargo 及其子进程的 RSS 总和、rustc 数量和 runner swap 用量，CSV 保留实际采样时刻。采样 RSS 会重复计算共享页，也可能错过瞬时峰值；不能作为独占内存或精确峰值。
- 探测失败或进程已消失时保留为 `null` 并记录原因，不记作零。Cargo 的失败退出码直接传回工作流；取消或异常时清理独立进程组，覆盖 Cargo 和编译器后代。
- 独立的 `grok-zh-macos-<arch>-build-monitor-<run>-<attempt>` artifact 包含 `runner.json`、`samples.csv`、`summary.json` 和 Cargo timings。诊断文件不加入安装归档。

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

手动 CI 的 `macos_build_variant=thin-lto` 仅用于构建对照：使用 `release-dist` profile、debug=0、独立且只读的编译缓存；正式 Release 拒绝该实验选项。默认值 `current` 保持上表配置。两种配置均保留原有测试、features、安装包和门禁。验证方法与边界见 [Unix 包体积说明](UNIX-BINARY-SIZE.md)。

## 六平台预览提速（2026-09-26）

基线 [CI 35998975791](https://github.com/JoyElliot/grok-build-Chinese/actions/runs/35998975791) 从创建到完成为 **91 分 30 秒**。Intel 原生 job 为 91 分 13 秒，其中格式与测试 26 分 38 秒、Cargo 编译 62 分 17 秒；三项缓存均未命中，Cargo 1411 个单元全部重新编译。4 核 Intel runner 上 J3 最大采样进程树 RSS 为 6900.91 MiB，swap 为 0；采样没有 CPU 利用率，不能据此保证 J4 的收益。

预览工作流将 Windows ARM64、Linux 两架构及 macOS 两架构的测试放入 `native-rust-validation` 矩阵，与六个产物构建并行。复合 action 的 `phase` 默认为 `all`；预览显式选择 `test` 或 `build`，正式 Release 拒绝分阶段模式。每个阶段都独立获取锁定依赖，测试命令和安装器、架构、签名、归档校验保持完整。预览测试设置 `CARGO_PROFILE_TEST_DEBUG=0`，保留断言与溢出检查。

`multiplatform-result` 必须同时等待原生测试矩阵、Windows GNU 测试门禁和六个平台制品成功。构建 job 成功或先上传制品，不代表整轮验收成功。macOS 真账号冒烟也等待原生测试矩阵。只有构建阶段保存编译与依赖缓存，并在保存前复查精确键，避免并发重复压缩；测试阶段不保存 debug 目录。

验收目标是整轮 CI 从 `created_at` 到所有必需 job 完成（含缓存保存、上传与汇总）不超过 50 分钟。区分冷缓存与命中缓存的结果，不能把拆分后的理论耗时或单独 Cargo 时间当作整轮达标证据。实际结果按轮次记录如下。

### 第二轮：Intel 交叉编译与原生制品验证

首轮提交 `c5963863` 的 [CI 36164317702](https://github.com/JoyElliot/grok-build-Chinese/actions/runs/36164317702) 中，五平台原生测试和 Windows GNU 测试全部通过；Linux x64/ARM64 构建分别为 31 分 36 秒、37 分 06 秒，Windows x64/ARM64 分别为 38 分 58 秒、49 分 07 秒，macOS ARM64 为 14 分 08 秒。Intel 原生 J4 构建的关键路径已超过 58 分钟，仍未满足目标；不能把 J3→J4 的理论并行比例当作测量结果。

第二轮预览把 Intel 产物的编译移到 `macos-15` ARM64 宿主，Rust host 工具链为 `1.94.0-aarch64-apple-darwin`，目标仍是 `x86_64-apple-darwin`。DotSlash/protoc 使用宿主架构；CoreAudio bindgen 的额外 Clang 参数仅设置到 x86_64 target。生产 profile、Rust CPU 基线、features、符号处理和包协议保持原配置，交叉编译缓存使用独立 schema。该试验的耗时与实际原生依赖兼容性由后续 CI 验证。

`cross_compile` 默认为 false，只允许 ARM64 宿主上的 Intel 预览 `phase=build`；正式 Release 保留原生构建。交叉构建阶段的 CLI 和安装器通过 Rosetta 验证，安装器由 `arch -x86_64 /bin/sh` 启动，使架构检测保持真实的 x86_64 执行上下文。包内 `BUILD-INFO.txt` 明确记录宿主架构和 Rosetta 验证方式。

Intel 原生 locale/updater 测试继续在 `macos-15-intel` 运行。新增的 `macos-intel-native-smoke` 下载同轮交叉构建制品，在 Intel runner 原生验证签名、精确架构、包内外 SHA-256、目录/权限、四类 CLI 入口及二次安装；共享 `verify-macos-package.sh` 保留原有安装与入口链接检查。最终汇总同时要求交叉构建、Intel 原生产物验证及全部原生测试成功，Rosetta 的通过不能替代 Intel 原生验收。
