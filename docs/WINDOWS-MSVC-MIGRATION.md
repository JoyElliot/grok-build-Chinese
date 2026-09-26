# Windows x64 GNU → MSVC 按需迁移试验

## 状态与范围

这是 PR #8 内的可执行实现和 CI 试验。正式 Release 仍使用六个平台的既有构建矩阵；没有创建迁移 Release，没有把正式 Windows x64 产物切换为启动器或 MSVC。CI 中的 `1.0.99` 是隔离的协议测试版本，不能作为正式升级包分发。

旧更新器只验证归档并替换 `grok-zh.exe`，不会执行包内安装器。保留一个真实 GNU 小启动器，才能使已经发布的旧代码进入新的迁移流程。不能将 MSVC 程序改名并谎报 GNU 协议来绕过验证。

## 安装与后续更新

1. 旧 GNU 更新器照常选择 `grok-zh-V-windows-x86_64-gnu.zip`。归档保留旧文件集合、SHA256SUMS、schema 1 和 GNU target；其中主 EXE 是真实 GNU 启动器。
2. 旧更新器提取主 EXE，运行无副作用的 `--version`，完成原有文件替换。迁移发生在下次正常启动；版本检查不会联网或安装。
3. 启动器第一次运行时，下载编译时固定的 helper tag、附件名、大小和 SHA-256。Release 必须 immutable，附件 URL 必须来自本仓库，API digest 与下载字节都必须匹配。
4. helper 下载**与启动器版本相同**的 `windows-x86_64-msvc.zip`，验证完整归档、内部清单、平台协议与候选程序版本，再调用经过验证的完整安装器。
5. MSVC 程序安装到入口旁的 `.grok-zh-msvc/`。顶层 GNU 入口保持原路径，将参数、工作目录、标准输入输出和退出码交给子程序；不修改用户 PATH 和共享 GROK_HOME。
6. 提交 ready 标记后，每次启动直接运行本地 MSVC，不再依赖 PowerShell、临时目录或下载。MSVC 更新器只选择 MSVC 附件，并更新自己的子目录 EXE，后续不用再次下载迁移包。

两次并发首次启动使用与 Rust 旧更新器相同的 `grok-zh.exe.update.lock` 独占锁；第二个进程有界等待，迁移成功后复用已安装程序。不会尝试重命名包含正在运行启动器的目录。

## 失败与兼容边界

- 第一次迁移需要网络、Windows PowerShell 5.1 和当前安装目录的写权限。下载、摘要或安装失败时，保留 GNU 入口以便重试；旧完整应用此前已被旧更新器替换，不能保证迁移失败时旧应用仍可离线使用。
- 现有无效、未知或低于新启动器版本的 `.grok-zh-msvc` 目录保守报错，不覆盖用户文件；此类情况需使用在线安装器修复。不会自动删除未知目录。
- 第一次安装验证所有字节。后续与普通本地安装相同，信任安装目录的访问权限及 ready/安装标记；不会每次重验未来由 MSVC 更新器替换的 EXE 初始哈希。
- 很老的严格附件集合客户端仍需要已有 `v1.0.8` / `release-v1.0.16` 历史桥。此方案解决 GNU 到 MSVC 的迁移，不能逆向改变那些客户端的标签、附件集合或选包代码。历史 Release 必须保留。
- 正常迁移与运行不要求管理员权限。UNC/扩展路径的 C 根解析有测试；没有在本机创建共享目录做 SMB 端到端验收。

## 构建与验证入口

```powershell
python -B .github/scripts/build-windows-migration-bootstrap.py `
  --output migration-dist --version 1.0.99 `
  --migration-tag migration-windows-x64-v1 --cc C:/mingw64/bin/gcc.exe
```

输出小 helper ZIP、固定 pin 和 GNU EXE；提供 `--payload-package` 可基于经过内部摘要验证的 MSVC 包生成旧文件集合兼容 ZIP。它只生成文件，不创建 Release。

- `Test-WindowsMigration.ps1`：真实 GNU C 入口配测试运行时，覆盖纯版本查询、首启下载、后续离线启动、并发、错误摘要/重试、参数/标准流/退出码、保留用户数据和 PATH。测试网络传输只在测试启动器编译时注入，生产程序没有环境变量 URL 或信任绕过开关。
- `Test-MigrationProduct.ps1`：使用本轮真正编译的 MSVC 产品包和 helper，通过离线 HTTP fixture 验证旧 GNU ZIP 的 EXE-only 边界到完整安装，再执行四类 PATH 隔离的 CLI 检查。它不证明 GitHub 上尚未发布的 helper 可下载。
- PR CI 新增独立 x64 MSVC 原生更新器测试和实际产品迁移作业，均进入最终汇总；原有六平台产物、五平台原生测试和 Windows core/ui 保留。构建使用原 `release-dist`、features、Thin LTO、opt3、CGU1 和静态 CRT 配置。

## 首轮真实产品 CI 验收（2026-09-26）

提交 `5c05f367` 的 [CI 36225602973](https://github.com/JoyElliot/grok-build-Chinese/actions/runs/36225602973) 全部必需作业成功：原六平台产物、五平台原生测试、Windows core/ui、x64 MSVC 更新器原生测试及真实产品迁移试验均通过，同步资料完整性成功，真实账号 macOS 冒烟按条件跳过。

该轮从 `07:03:15Z` 创建至 `08:01:28Z` 完成，共 **58 分 13 秒**，未满足整轮 50 分钟目标。原六平台及其测试于 `07:41:52Z` 完成；新增迁移试验作业为 `07:03:58Z–08:01:20Z`，是本轮最长路径。完整计时包含排队、缓存保存、上传和最终汇总，不扣除这些阶段。

新增 x64 MSVC Cargo 构建为 **52 分 57.8 秒**，1356 Dirty / 0 Fresh，依赖源与编译缓存均未命中。`xai-grok-shell` 用时 906.2 秒，其中 codegen 780.4 秒；最终 `grok-zh` binary 单元用时 974.7 秒。Cargo timings 没有将该 binary 单元细分为 codegen 与外部链接，因此不能把这 974.7 秒直接归因于链接器。本轮缺少 CPU 型号、内存及独立外部链接采样，不能确定具体硬件瓶颈；后续试验补充记录宿主与工具链信息。

打包验证约 12 秒，两个缓存保存合计约 80 秒；实际产品迁移步骤共 58 秒，两种 PowerShell 均输出通过。因此本轮优化优先针对编译路径，保留完整验证。仅对同版本重跑命中缓存不能证明冷编译达标。

首次尝试 `698fffbb` 的 PowerShell 5.1 fixture 曾因宿主输入编码的 BOM 行为失败。`5c05f367` 将测试改为精确原始字节写入及 Base64 比较，覆盖 BOM、中文、换行、NUL 和 `0xff`，保留两种宿主编码组合；生产启动器与 helper 不变。本轮真实产品迁移成功也不等于已发布历史 Rust 客户端的端到端验收：旧 ZIP 校验及仅复制 EXE 边界仍由测试模拟，下载使用测试内嵌传输。

### 下一轮：ARM64 宿主编译，x64 原生验收

预览增加独立 `windows-msvc-cross-build`，采用已有 `windows-11-vs2026-arm` runner 与 Rust 1.94.0 ARM64 宿主，只将应用目标设为 `x86_64-pc-windows-msvc`。MSVC 官方支持 ARM64 宿主到 x64 的工具链；实际组件、依赖和提速仍须本轮 CI 证明，不能从原 ARM64 应用的耗时推算 x64 交叉编译耗时。

Cargo 全局保留 ARM64 的 VS 库环境，x64 的 C/C++ 编译器、归档器和 Rust 链接器通过目标专用包装加载 x64 库环境；环境在准备阶段生成一次，避免每次 C 编译重新初始化 VS。完整编译前分别执行 ARM64/x64 Rust 与 C 链接预检，以及与 cmake-rs 默认选择一致的 x64 CMake 预检。部分 VS CMake 工具可能在 ARM runner 上模拟执行；这不替代 x64 产品的原生验收。

编译输入带本轮提交、run/attempt、版本、host/target、profile/features、大小和 SHA-256。独立 Windows x64 作业严格核对后继续执行原 PE、PATH 隔离 CLI、ZIP/清单/安装器与两种 PowerShell 完整迁移测试；x64 MSVC 更新器 Rust 测试仍独立在原生 x64 上运行。新编译作业也加入最终必需门禁，失败、取消或跳过都不能放行。独立缓存首次冷构建，不读取原生 x64 MSVC 缓存；所有运行时优化、feature 与根 Cargo.toml 保持不变。

### 交叉工具包装器修复（2026-09-26）

`11b3b6b0` 的 [CI 36230269506](https://github.com/JoyElliot/grok-build-Chinese/actions/runs/36230269506) 在 ARM64/x64 Rust、C、CMake 预检后进入实际依赖编译，但 MSVC 交叉作业失败，不能计入成功或性能达标。BLAKE3 自身的交叉编译检测只接受空值、`cl` 或 `cl.exe`，完整路径 `cl.cmd` 导致它选择 GNU 汇编；`cc-rs` 此轮是否误判编译器族未被日志证明。AWS-LC 的约 13904 字符归档命令则在 `lib.cmd` 入口处明确报 `The command line is too long.`。

修复将三个目标包装器改成宿主原生 PE：`cl.exe`、`link.exe`、`lib.exe`。目标 CC 使用裸名称 `cl.exe`，PATH 最前放仅含 `cl.exe` 的目录，目标 link/lib 仍用绝对路径；宿主 CC/CXX/AR/link 全部显式指定真实 ARM64 工具的绝对路径。包装器只加载预先捕获的 x64 编译环境，通过 `CreateProcessW` 原样转发参数尾部、标准流和退出码；不经过命令解释器，不修改优化配置或禁用汇编实现。Cargo 主进程仍使用 ARM64 库环境，包装器源码和生成脚本纳入编译缓存键。

新增预检用真实 MSVC 工具覆盖空格/中文路径、错误宿主 INCLUDE/LIB 的隔离、超过 8191 字符的归档命令、链接 response file、x64 PE 架构及工具失败退出码。在完整锁定依赖下载后，另用产品相同版本 `cc 1.2.43`、`find-msvc-tools 0.1.4` 与 `blake3 1.8.2` 编译小型 Cargo 图，强制断言 MSVC family，验证实际优化汇编的选择。本地 x64 主机的上述检查通过；另将 BLAKE3 已编译 build script 的 HOST/TARGET 设为 ARM64/x64，实际执行其交叉条件分支，生成了 SSE2/SSE4.1/AVX2/AVX512 的 MSVC 汇编对象及库。这项本地检查仍运行 x64 工具，不是 ARM64 宿主验证；仍须 ARM64 runner 完整构建和独立 x64 迁移作业证明交叉路径可用。

## 正式启用前的发布顺序

1. 先完成当前 PR 全部 CI 与性能验收。
2. 经正式发布授权，发布专用 helper tag 并使其 immutable；核实实际附件字节与编译 pin 一致。helper 不能使用 `latest` 或可变链接。
3. 正式工作流增加真实 x64 MSVC 产品，并把 GNU 附件改为兼容入口包；更新发布精确附件集合、证明和校验。每个需要让旧版直接升级到的版本均保留 GNU 入口附件，MSVC 与其同版本发布。
4. 保留旧三平台 sidecar 和历史桥；新的 MSVC 附件可继续使用 GitHub digest，无需新增公开 sidecar。发布器需先完成 helper 可用性检查，才能分发引用它的启动器。

当前提交没有执行以上正式发布步骤。
