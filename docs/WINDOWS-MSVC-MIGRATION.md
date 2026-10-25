# Windows x64 GNU → MSVC 单包迁移

> 当前状态：本分支的候选发布格式已改为 `distribution: direct` / `direct_runtime`，GNU 文件名归档内直接放置与原生包字节相同的 MSVC EXE，正式 `enabled` 开关仍关闭。当前恢复约束、驻留旧 GNU 写入者风险和验收边界见 [Windows 更新恢复与旧进程边界](WINDOWS-UPDATE-RECOVERY.md)。以下正文保留的是 `embedded_runtime` 历史设计及其回归说明，不代表当前 direct 路径仍有 wrapper、首次展开或独立 MSVC 子目录。

## 状态与公开包数量

本分支实现单包方案，取代早期“轻量 GNU 入口下载固定 helper，再下载 MSVC 完整包”的试验设计。正式发布开关仍关闭：`.github/release-policy.json` 中 `windows_x64_msvc.enabled` 为 `false`。合入代码本身不切换用户渠道，也不创建标签或 Release。必须先通过本分支的 Windows PR CI 和真实历史客户端验收，再单独审核发布策略变更。

启用 `distribution: embedded` 时，每个正式版仍有 **六个安装归档**：五个其他平台归档，加一个 Windows x64 归档 `grok-zh-V-windows-x86_64-gnu.zip`。后者使用历史文件名，但包含真正的 GNU 启动器及其内嵌的完整 MSVC ZIP。不会再公开第二个 x64 MSVC 归档。MSVC ZIP、校验 sidecar 和来源证明仍作为内部构建/校验输入。GitHub 自动显示的源码下载和证明不是额外安装包。

## 三条使用路径

### 旧 GNU 用户

1. 旧更新器下载该版本的兼容 ZIP，校验旧文件集合、GNU schema 1 协议及摘要。
2. 旧更新器仍只提取主 EXE，执行纯 `--version` 后替换原路径。启动器 EXE 已携带完整压缩的 MSVC 安装包；更新过程中不执行归档内脚本，也不利用版本查询偷偷安装。
3. 下次正常启动在本地展开并验证内嵌包，暂存后安装到入口旁的 `.grok-zh-msvc/`。会显示准备/等待状态，可取消；不再下载 helper 或第二份程序。
4. 原 `grok-zh`、`agent-zh` 入口与 PATH 不变。GNU 入口转发参数、工作目录、标准流及退出码。用户的 GROK_HOME、配置与认证位置不变。

“下次启动无需联网下载”不等于“更新当时已经展开安装”。历史更新器没有安装回调，仍需要至少一次执行新入口来展开本地文件。首次展开用时与磁盘/杀毒软件有关，未作真实 Windows 基准前不承诺秒数。取消或空间不足时保留内嵌源，可修复环境后重试；旧更新器已经覆盖的完整 GNU 应用字节无法凭空恢复，不承诺首次失败能自动运行旧完整 GNU。

### 新安装

在线安装器默认 `Auto`，先选最高的合格不可变正式版本，再在同版本优先原生 MSVC 附件；只有 GNU 附件时识别经过认证的容器并直接提取内层 MSVC 安装。用户安装后的主入口就是 MSVC，不必先运行 GNU 迁移入口。手动解压容器后运行一键安装，使用同一离线提取器。正式开关关闭、当前最新包仍是完整 GNU 时，Auto 保持原 GNU 安装兼容；`-WindowsX64Runtime Msvc` 只接受原生或有效内嵌 MSVC。

### 已迁移用户继续更新

正在运行的是 MSVC 产品，其更新器先选择最新合格的不可变正式兼容版本，再在同版本优先原生 MSVC 包。没有原生附件时，GNU 文件名只代表“待验证容器”，不是直接接受 GNU 程序：下载后必须验证其内嵌描述、footer、内层摘要、版本、架构和完整清单。最终仅将内层 MSVC EXE 作为更新候选，绝不把 GNU wrapper 写进 MSVC 子程序路径。

固定入口 N 可以启动已更新到 N+1、N+2 的子程序。ready 快路径不重解压、重复整包哈希或检查远程 helper；`--version` / `-V` 返回已安装子程序的版本。尚未安装时版本查询仅返回入口版本且无副作用。同版本同时有有效原生包与容器时选原生；原生同名附件重复、摘要/元数据无效时失败，不用容器掩盖坏发布。较旧原生包不会抢占较新版本容器。

## 容器与信任边界

外层保留旧 GNU 包的精确 15 项普通文件加 SHA256SUMS（共 16 个文件）的布局、schema 1、`x86_64-pc-windows-gnu` 目标和真实 GNU EXE。内嵌内容不是巨大 C 数组，构建时把完整压缩 ZIP 一次追加到 PE 后面：

```text
GNU PE | native MSVC ZIP | 64-byte footer
footer = <16sQQ32s (little endian)
magic = ASCII "GROKMSVCBUNDLE01" (16 bytes, no NUL)
offset:u64 | length:u64 | sha256:32 raw bytes
```

读取固定 EOF footer，不扫描任意 ZIP/MZ 字节。必须满足边界无溢出、合理 PE 前缀、正长度、`offset + length + 64 == file length`，ZIP 不超过 512 MiB。外层协议增加描述性 `embedded_runtime`：

```json
{
  "schema": 1,
  "format": "grok-msvc-bundle-v1",
  "version": "V",
  "platform": "x86_64-pc-windows-msvc",
  "archive_size": 123,
  "archive_sha256": "64 lowercase hex characters"
}
```

GitHub immutable Release 的 digest 认证整个外层归档，外层清单认证 EXE 与描述；已编译启动器还固定内层大小/摘要。footer 只是定位器，不能单独成为信任来源。内层为同版本真实 MSVC schema 1 完整包，继续校验所有文件、清单及其哈希，拒绝跳转、重复/冲突路径、链接、错误 PE 架构及超出数量/解压上限的内容。安装器的 LibraryOnly/ExtractOnly 模式只供验证后的离线提取路径使用，不能绕过归档认证。

## 恢复、并发和主动降级

- 首次安装使用隔离暂存目录、空间检查和互斥锁；中断不得把未完成运行目录标成 ready。锁等待有可见提示和有界超时，不进行静默 20 分钟等待。
- 已有运行目录更新保留经验证的备份，激活后重新验证；失败时在锁内恢复。Windows 1177、文件占用或回滚本身失败时保留可恢复路径并报错，不宣称成功。
- Windows Rust 更新器在取得安装锁后重新探测磁盘版本。自动更新的旧候选不能覆盖先获得锁的新版本；明确指定版本/渠道的用户操作与自动更新意图区分。Unix 原有安装器并发时序不在本轮修复范围，不能把 Windows 防护表述为所有平台的全局保证。
- 已迁移目录损坏/子 EXE 丢失，不应把入口版本当成最新版本而静默覆盖。若整个运行目录及状态都被用户删除，固定入口无法知道此前达到的版本，重新展开的是内嵌版本；需要恢复最新版本时使用官方在线安装器。不得承诺任意删除/恢复情况下绝不降级。
- 在线安装器的 `-Version V` 是精确版本选择；已装更高版本时还必须明确 `-AllowDowngrade`。未指定版本不得使用该开关。完整安装器也要求显式允许较低版本。主动降级前备份 GROK_HOME；应用数据格式未必向后兼容。
- MSVC → 历史完整 GNU → MSVC 使用完整安装器完成。替换安装时退休旧迁移 sidecar，防止旧 ready 子程序覆盖用户选定版本；配置、认证和 PATH 不随 ABI 转换删除。内置 MSVC 更新器不负责把自己换成旧完整 GNU。

## 将来停止生成 wrapper

`distribution: native` 将公开的 Windows x64 包改为原生 MSVC；其他五平台不变，仍为六个安装归档。必须指定 `final_legacy_wrapper` 的精确版本、外层大小及 SHA-256。发布前验证该较早版本的 Release 为已发布、不可变、非预览版；同名 GNU 附件唯一、URL/digest/大小匹配；实际下载字节和完整内嵌协议也通过核验。不能把一个普通完整 GNU 包当作最后容器。

最后容器和既有 `v1.0.8`、`release-v1.0.45` 历史桥接 Release 必须永久保留，不得删改。已确认 `release-v1.0.45` 的更新器会分页选择最高兼容 GNU 版本，因此可在退休后找到最后容器，再由 MSVC 追上最新原生版。更早客户端仍按原桥接链逐段升级；这不是所有未知历史预览版都可直接升级的保证。原始裸 EXE 预览版按历史说明手动迁移。

## CI、测试与启用门槛

PR CI 必须同时执行：

- Python 容器构造、篡改、边界和发布策略合约。
- PowerShell 7 与 Windows PowerShell 5.1 的 `Test-EmbeddedMigration.ps1`，验证真实 GNU 启动器的纯版本查询、首次离线解包、ready 快路径、连续子版本、参数/标准流、并发/失败恢复。
- 同两种 PowerShell 的 `Test-EmbeddedMsvcInstaller.ps1`，验证干净安装、完整 GNU/MSVC 往返、精确旧版选择和损坏容器拒绝。
- `Test-EmbeddedMigrationProduct.ps1` 使用本轮真实 MSVC release-dist 包进行离线迁移与 CLI 验收。fixture 不冒充真实 MSVC 产品。
- MSVC 原生 core/ui 与完整 updater `--lib`；与 GNU 共用测试清单及构建配置，原六平台和 GNU 回归门禁继续保留。更新选择、容器解析、退休路径及 Windows 锁内竞态有 Rust 单元覆盖。

早期远程 helper 试验保留为回归参考：`build-windows-migration-bootstrap.py`、`Test-WindowsMigration.ps1`、`Test-MigrationProduct.ps1` 不再是新正式产品路径；它们通过不代表新单包实现已通过。

本地 Linux 只能执行可用的静态/Python 检查。没有 Windows、PowerShell、Rust 工具链时，新增 Rust/Windows 测试仅为待 CI 验证，不能复用历史通过数。先通过代码预审后建立草稿 PR，利用 PR CI 获得实际结果；合并前处理全部强制门禁。正式启用还须真实 `v1.0.8` / `release-v1.0.45` 更新链、连续两次实际 MSVC 更新、运行中程序/杀毒/低磁盘/中断验收，以及真实账号 TUI/core/provider 功能与资源用时回归。测试版本不是正式发布版本，不作未经测量的速度或体积承诺。
