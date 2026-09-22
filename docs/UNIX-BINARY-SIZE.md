# Unix 发布包体积与验证

Linux 和 macOS 只处理发布副本；Cargo 构建输出、程序功能、依赖 features、安装包成员、USTAR/gzip 格式及更新协议保持原约定。符号处理不会改变编译优化等级。

| 平台 | 发布副本处理 | 必须保持的运行信息 |
| --- | --- | --- |
| Linux x86_64 GNU | `strip --strip-unneeded`，移除普通静态符号及调试信息 | ELF 入口、程序头、全部 SHF_ALLOC 节的地址、属性和内容，包括动态符号、重定位和展开表 |
| macOS ARM64 | 原生 `strip -x`，移除局部符号；重新生成 ad-hoc 签名 | Mach-O 入口、UUID、依赖、全部运行节、动态绑定/重定位/导出 trie、external/undefined 符号及间接引用语义 |

`.github/scripts/strip-unix-binary.py` 在剥离前后提取上述信息并比较。信息变化、输出变大、工具失败或 CLI 不一致均令打包失败，不自动忽略错误或回退为未验证的安装包。脚本在隔离的 GROK_HOME 中比较原程序与发布副本的 `--version`、`--help`、`agent --help` 和 `update --help`，随后平台 action 继续执行原有包内外哈希、解包、版本、权限、安装与更新协议检查。

macOS 只接受当前无特殊权限的 ad-hoc 输入；Developer ID/带内容的 CMS、entitlements、非空 requirements 或额外运行限制都会拒绝处理。Apple 的 ad-hoc 签名器会生成只有 8 字节头、没有负载的 CMS 占位记录；仅精确接受该空记录，仍拒绝任何有内容的 CMS。显式保留签名标识和 metadata，重新签名后执行 `codesign --verify --strict`。它仍不是 Developer ID 签名或 Apple 公证。动态符号不会像局部符号一样裁剪，也不会删除整个 `__LINKEDIT` 或展开表。

## 诊断记录

原程序的 `LC_ALL=C nm -an` 地址/类型/名称映射压缩为 `symbols.nm.gz`，manifest 绑定输入和输出 SHA-256、版本、Git SHA、处理前后大小及 CLI 输出哈希。映射保留离线地址到函数名查找能力；剥离后程序自身的原生崩溃信息可能少一些函数名，映射也不包含 DWARF 源码行表。不要据此声称完整崩溃诊断体验不变。

- Linux：诊断放在既有 Actions package artifact 的 `diagnostics/linux/` 子目录，不进入 tar.gz、包内 SHA256SUMS 或正式 Release 资产。
- macOS：诊断并入既有 build-monitor artifact；上传移到打包之后，仍保留 `always()`，失败时也尽可能保留监控记录。
- 不新增 artifact 类型或第二套用户安装包。正式 Release 的三归档及兼容校验 sidecar 选择、来源证明和发布门禁不变。Linux 的诊断目录不参与只枚举顶层文件的 Release 资产集合，也不在上传资产列表中。

## 基线及本地验证

正式 1.0.35 的 Linux 主程序为 186,891,400 bytes。对实际下载副本执行新脚本后为 163,402,688 bytes（减少 22.40 MiB）；所有运行信息一致，WSL Ubuntu 24.04 上四个 CLI 输出也一致。该结果不是新提交的完整 Linux CI 验收。

正式 1.0.35 的 macOS 主程序为 190,891,136 bytes。该文件的原始 Mach-O 解析验证通过；实际局部符号裁剪、签名与启动由 Apple Silicon CI 验证，不预先承诺理论估计的节省量。

Python 测试覆盖程序代码、入口、加载属性、dyld/导出数据、符号名及 UUID 被意外改变时拒绝发布，并覆盖签名权限和截断输入。CLI 检查不等于完整账号、TUI、模型或工具吞吐验收；这些能力的原有测试保持，不通过删功能或降低运行优化换体积。

Rust/LLVM 的原始 ELF 可能将初始化数组 `sh_entsize` 留为 0，GNU strip 会补成 8。对 ELF64 x86_64 的 INIT/FINI/PREINIT 三类函数指针数组，校验按 8 字节表项规范化这两个值，拒绝其它表项大小或非整项长度；数组内容、地址、偏移、flags、大小和完整加载信息仍须一致。该行为符合 [ELF 动态链接规范的函数指针数组定义](https://refspecs.linuxfoundation.org/elf/gabi4%2B/ch5.dynamic.html)。本地 Rust 1.94.0 带调试信息的 PIE 已复现此元数据变化；下载的旧包已经 strip-debug，未覆盖这个输入条件。其它运行信息变化会继续阻断，并在日志输出具体字段及前后值。

## macOS 编译配置对照

默认预览和正式 Release 仍使用此前已验证的配置。手动 CI 可显式选择 `macos_build_variant=thin-lto`，使用上游 `release-dist` profile、Thin LTO、CGU1、opt-level=3、debug=0 与原有 features。实验配置有独立 target/host 缓存键，缓存只读；正式 Release 拒绝选择未验收实验配置。

这仍是原有串行 macOS 作业，保留完整测试、打包和聚合门禁。实验产物使用原来的包类型，不新增发布资产。先验证默认配置的符号裁剪，再用同一提交和嵌入版本比较 current/thin-lto 的大小、构建耗时与运行指标；通过前不切换正式默认配置。保留 GROK_VERSION 的真实失效关系，不修改源文件时间来伪造缓存收益。
