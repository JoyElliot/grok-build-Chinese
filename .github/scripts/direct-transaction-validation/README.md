# 真实产物的更新器事务验收

本项目生成两个可搬迁的 Windows Rust 测试 EXE；用户只运行已编译 EXE，不需要 Rust、Python 或联网。它们是测试程序，不是旧产品二进制，不得改名伪装为旧产品。

- legacy-transaction：固定 GNU 1.0.45 源码选择/下载流校验/提取/真实候选探测/Windows 事务，目标是完整真实 GNU 包副本，替换为真实 MSVC 1.0.104。
- current-transaction：当前原始更新器源码，目标是真实 MSVC 包副本，分别经 native 包及 GNU 名称 direct 包显式同版本重装；不是两个不同未来版本的升级验证。

历史底座复用 `3a1359d201957ccd0e33e24684dd4b291a71971b` 中 historical-update-parser 的逐字事务切片设计。完整历史输入逐项固定 Git blob，当前输入直接读取工作树原文件。构建时 prepare.py 重新生成原始完整社区发布模块、取消模块、产品常量和逐字提取的 auto_update 函数/事务段，并生成 source-provenance.json。历史 copy→rename→copy→rollback 没有替换成当前加固实现。当前源码使用当前自己的 stage→rename→rollback；两个测试 EXE 均使用当前固定 Rust 工具链及 x86_64-pc-windows-msvc 编译；历史 GNU 平台选择是显式 fixture，不是复现历史 GNU ABI。Cargo.lock 固定此测试构建依赖，但不声称复现旧发布程序的完整编译依赖或二进制。

显式测试接缝：
1. 内存中构造明确标记的未发布 Release 元数据，指定真实 archive 的固定 SHA-256 和字节数；未执行 GitHub 发现、分页、TLS、认证、HTTP 状态/响应头/重定向。
2. 64 KiB 本地文件流替代 download_verified 的 HTTP 适配器；原始 write_verified_download 全部执行。
3. 目标参数替代 current_exe，指向真实包的临时完整副本；GNU/MSVC 平台选择为显式输入。
4. 当前事务显式采用 ExplicitVersion intent 来测试同版本重装；不测试原始 CLI 对 intent 的选择。
5. 不执行外层入口策略、安装器调度、成功回执/配置写入或用户交互。共享锁句柄不是正在运行的旧进程。

测试覆盖真实 EXE 版本探测、包校验、完整原始事务切片、仅 EXE 更新和伴随文件保持、错误外层哈希、截断流、仅篡改声明版本而保留真实 EXE 的应失败版本探测、Windows 更新锁竞争、真实旧 EXE 共享锁下替换、源读取受阻时旧事务回滚/当前事务不触碰目标，以及当前激活失败后真实 EXE 恢复。

历史旧更新器只能证明所测故障下的行为，不能据此承诺所有断电/磁盘/竞态故障均安全。没有执行未修改 GNU EXE 自己的 online update，也没有真实正式 Release 或后续不同版本的发布升级。

构建命令：`cargo test --locked --no-run --manifest-path .github/scripts/direct-transaction-validation/Cargo.toml --message-format=json`。Windows 必须运行两个测试 EXE，非 Windows 的绿色结果不代表 Windows 事务测试执行。

运行时所有输入来自环境，不依赖 CI 源码路径：GROK_DIRECT_HISTORICAL_EXE、GROK_DIRECT_NATIVE_EXE、GROK_DIRECT_NATIVE_ARCHIVE、GROK_DIRECT_DIRECT_ARCHIVE、GROK_TRANSACTION_RESULTS。测试 runner 必须先验证原始包/EXE 哈希并设置隔离 HOME/GROK_HOME/TEMP、清除凭据环境；不要对真实用户安装目录运行。测试只读取输入文件并在 TEMP 下创建自己的目录。
