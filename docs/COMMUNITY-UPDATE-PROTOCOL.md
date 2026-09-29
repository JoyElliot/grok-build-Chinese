# 社区单包更新协议与过渡发布

每个平台始终只有一个安装归档。新旧客户端下载同一份包，不增加供用户选择的第二套安装包，也不增加 Release 标签命名空间。

## 两层校验

1. 更新器只从固定社区仓库的不可变 GitHub Release 选择当前平台归档，验证精确名称、地址、上传状态、大小和 API `digest`，下载后核对整个归档的 SHA-256。同名归档重复、digest 缺失或错误时拒绝；其他平台或说明附件不参与当前平台的校验。
2. 归档内的 `BUILD-INFO.txt` 声明更新协议，`SHA256SUMS.txt` 列出文件及 SHA-256。新版按声明读取清单，校验完整文件集合与文件内容，再提取声明的程序入口。

Release 中独立的 `<archive>.sha256` 与包内 `SHA256SUMS.txt` 是两种文件。新版不下载、也不要求独立 sidecar；包内清单仍是新协议的一部分。GitHub digest 覆盖整个归档（包括协议和清单），包内清单再覆盖 `BUILD-INFO.txt`，不存在自我哈希循环。

## 协议 1

在现有 `BUILD-INFO.txt` 的普通构建信息后追加一个完整块，保留原来的首条 `Version:`。以下版本号仅用于展示格式，不表示该版本已经包含本协议或已正式发布：

```text
Version: 1.0.16
...
GROK-UPDATE-PROTOCOL-BEGIN
{
  "schema": 1,
  "version": "1.0.16",
  "platform": "x86_64-pc-windows-gnu",
  "mode": "executable-only",
  "manifest": "SHA256SUMS.txt",
  "executable": "grok-zh.exe",
  "installer": "Install-GrokZh.ps1"
}
GROK-UPDATE-PROTOCOL-END
```

`platform` 使用构建目标的 Rust 三元组：`x86_64-pc-windows-gnu`、`aarch64-pc-windows-msvc`、`aarch64-apple-darwin`、`x86_64-apple-darwin`、`x86_64-unknown-linux-gnu` 或 `aarch64-unknown-linux-gnu`。macOS/Linux 的入口为 `grok-zh`、`Install-GrokZh.sh`。版本与平台必须和已验证的 Release 相符。协议块只有一份；存在但损坏、不支持或缺少字段时明确失败。仅没有协议标记的历史包进入原来的固定清单验证分支。

`manifest` 的文件名和 SHA-256 算法由协议 1 定义，语法沿用 `<64 位十六进制摘要><两个空格><相对路径>`。清单不能包含自己，必须覆盖构建信息、程序入口、安装器和其余全部普通文件；归档不得包含清单以外的文件。目录由清单路径推导，不再写死全部普通文件名。路径必须相对包根，不能含跳转、绝对路径、链接、重解析点、重复路径或文件/目录冲突；继续保留数量、大小和解压上限。

归档外壳继续采用既有同名包根目录；Windows 为 ZIP，Unix 为 USTAR + 单个 gzip 流。协议 1 允许清单内新增文件、子目录或移动程序/安装入口。Unix 程序和安装器入口必须为 `0755`，其他普通文件可为 `0644` 或 `0755`，目录为 `0755`。两个入口必须独立，不能指向清单或构建信息。

Windows 在线入口会读取声明的归档内程序与安装器路径；包内安装器随当次包一起发布，负责部署。协议 1 只放开归档布局，完整安装后的程序目录仍使用 `grok-zh.exe`、`agent-zh.cmd`、`rg.exe` 等既有布局和安装归属记录，以供在线入口检查与便携启动器使用。后续重排归档时必须同步更新该包内安装器。

`mode: executable-only` 保持现有自动更新行为：内置更新只替换主程序，旁载工具、许可证和安装文件由完整安装器管理。需要新版运行时旁载文件、新的安装动作或新的必需语义时，应增加协议版本/模式并先发布支持它的引擎；不能在旧模式下悄悄改变要求。附加说明字段可扩展，但不得用未知字段引入必需动作。更新器不执行包内自定义“校验程序”。

## 约两个月的兼容期

从本次功能合入并正式发布后开始计时，维持：

- 每个平台一份完整归档；历史三平台 Release 有三份归档和三个独立 `.sha256`。六平台正式版公开六份归档，仅为原 Windows x64 GNU、macOS ARM64、Linux x64 GNU 保留三个独立 `.sha256`；新增平台从首版起使用新更新器，不公开独立校验附件。
- 包内现有物理布局、Windows 15 项/Unix 9 项清单保持不变，仅扩展已有 `BUILD-INFO.txt` 内容并重新生成其哈希。
- 新版内置更新器与在线入口已不依赖独立 sidecar，同时接受历史包和新协议包。

生成顺序为普通文件 → 构建信息与协议块 → 包内 SHA256SUMS → 归档 → 独立 sidecar。CI 内部仍为全部归档生成并核验 sidecar、生成来源证明；公开 Release 只上传各平台归档及原三平台的兼容 sidecar。发布前后均以筛选后的精确附件集合和 GitHub digest 验证，包内 `SHA256SUMS.txt` 在所有平台继续保留。旧校验器不解析 BUILD-INFO 内容，因此同一归档可通过旧固定清单和新声明清单两种校验。

兼容期内不能重排归档或增加包内文件；旧客户端的固定文件表仍需要通过。新的动态清单能力先随程序发布，文件布局调整留到停止旧资产兼容之后。

## 停止公开独立 sidecar

发布策略位于 [release-policy.json](../.github/release-policy.json)。默认 `publish_legacy_sha256: true`，表示继续公开原三平台的兼容 sidecar；新增三平台的 sidecar 始终只用于 CI 内部验证。没有按日期自动切换，也不会删除任何历史 Release。

过渡完成后，将 `publish_legacy_sha256` 改为 `false`，并把 `legacy_bridge_tag` 设为**已发布、包含新更新引擎、仍保留原有三平台六个附件的正式 Release**。现有策略验证器以历史三平台过渡包为目标；新增平台的首个客户端已经使用新引擎，无须经过该历史过渡包。版本号或已有 Git 标签不能证明其中包含新引擎；应选用正式构建并验证的 Release。

停发前 CI 会核对过渡版比当前版更早、不可变正式状态、原有三平台完整六附件及其 digest，并确认 `v1.0.8` 仍是保留原始两个 Windows 资产的不可变正式 Release。随后下载所选过渡版的 Windows ZIP，核对 GitHub 摘要、归档安全边界、完整文件清单与哈希，并确认其中存在有效的新协议块；缺失或损坏时拒绝停发。此预检不运行下载包内的程序；正式版本的实际更新行为仍须通过发布 CI 与升级验收。

停止公开后的新 Release 只上传各受支持平台的归档。CI 仍可在内部生成并核验 sidecar、为内部产物生成来源证明；用户的 Release 附件列表不再出现独立 `.sha256`。GitHub 自带的源码下载入口和 Release 证明仍由 GitHub 展示，不属于额外安装包。

必须永久保留原 `v1.0.8` 和选定的三平台六附件过渡 Release。升级路径如下（`B` 是选定过渡版，`N` 是后续发布版）：

| 用户现有版本 | 升级路径 |
| --- | --- |
| 只认识旧 `v*` 的 Windows 1.0.3/1.0.5 | `v1.0.8 → B → N` |
| `v1.0.8` 或 `release-v1.0.13` 等要求 sidecar 的更新器 | `B → N` |
| 已支持本协议的版本 | 直接到 `N` |

旧现代客户端遇到六平台发布版时，可能因附件总集合从六个变为九个而跳过，继续选择 `B`，下一次由 `B` 中的新引擎更新到 `N`。若以后停发剩余三个 sidecar，旧客户端也会跳过缺 sidecar 的新 Release。这里的“过渡结束”表示停止公开新的旧式发布附件，不是删除桥接发布。更早只认识裸 EXE 的预览版仍需按原说明手动迁移。

Windows 在线命令读取 raw `zh-dev` 脚本，因此发布新包时也必须让新版在线入口合入并推送到 `zh-dev`。

## 验证

- Rust `xai-grok-update --features community-build --lib` 覆盖新旧归档、六平台选择、桥接选择、路径扩展和错误协议。
- Windows 在线安装器测试在 PowerShell 5.1/7 验证无 sidecar、同包双校验、安装/修复/回滚。
- `test_package_protocol.py` 检查六平台协议生成和保持物理成员；`Test-ReleasePolicy.ps1` 检查保留/退休策略。
- 实际平台构建、归档权限与安装 smoke 由六平台 Release CI 负责。
