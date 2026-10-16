# Windows x64：真实 GNU 包直接覆盖 MSVC 主程序的隔离验证

这是独立测试分支 `test/windows-msvc-direct-replacement` 的验证方案。
最终验收重点是旧版本更新流程不中断，以及替换后的 MSVC 能继续更新。只替换 `grok-zh.exe` 的启动检查是其中一个子步骤，不能替代更新链验证；不启用正式迁移，不改变生产发布设计。

## 固定输入

全部身份、来源和 SHA-256 写入 `.github/scripts/windows-direct-replacement-inputs.json`。

- 生产基线：`74c5269942d9a47bf09a9268db0bb5c145967e27`。
- 真实旧包：不可变正式发布 `release-v1.0.45` 的 GNU ZIP，来源提交 `3f5d07f8cb2e9be395131296a3efc4635713db7f`，资产 ID `624680447`。
- 真实新主程序：成功 CI `38842814822`、第 1 次运行、来源提交 `ddede0f98fbbef6349b5040b8390afb9f4d3a5cd` 的原生 MSVC 独立制品，资产 ID `12022879975`。
- 新制品名中的 `migration-experiment` 是既有独立构建制品的名称；本测试只使用其中的原生 EXE，不执行 GNU 迁移启动器。
- 下载时核对 GitHub 元数据中的仓库、提交、run、attempt、成功状态、artifact 绑定与 digest；随后检查外层制品、内层 ZIP、包内 manifest 以及真实 EXE 的固定哈希。
- 历史发布用 GitHub immutable 状态和固定资产哈希作来源校验。本测试不新增 SLSA/签名验真结论，也不替代正式更新器的线上信任链。

## 测试内容

1. 在新临时目录解压两个真实包。拒绝 ZIP 路径穿越、符号链接、重复路径及异常扩展大小。
2. 核对 BUILD-INFO 中版本、源提交和真实编译目标；校验包内 SHA256SUMS。
3. 解析真实 PE 的普通导入、延迟导入、资源目录、签名表、x64/PE32+ 与控制台子系统；输出完整依赖清单，不凭 `crt-static` 标志推断依赖。
4. 启动历史 GNU EXE，验证 `--version`、`--help`、`agent --help`、`update --help`。
5. 复制旧包，只覆盖顶层 `grok-zh.exe`。逐文件哈希要求差异只能是该 EXE；旧 `rg.exe`、启动脚本、安装脚本及其他文件必须保持原样。
6. 对覆盖后的真实 MSVC EXE 运行同样四个命令，并运行保留的旧 `rg.exe --version`。所有命令使用空白测试用户目录、独立 GROK_HOME/TEMP，以及仅含 Windows 系统目录的 PATH；不继承 token、用户配置或开发环境 PATH。
7. 每个命令的执行截止为 35 秒，即 30 秒版本验证预算加 5 秒余量；超时会失败，另外最多约 15 秒用于有界进程树终止和清理。记录退出码、耗时、stdout、stderr。
8. 再次核对旧源包和覆盖后包未被命令修改；检查隔离用户目录未产生 `.grok-zh-msvc`、迁移/启动器文件或 EXE/DLL/PowerShell 启动文件。
9. 使用精确历史 Rust 源码验证旧 parser 对兼容传输元数据及真实 MSVC 字节的接受/拒绝行为。此项独立报告，不能替代真实旧二进制的联网更新。
10. 后续 run 另行对两份真实二进制执行 `update --check --json`，各用独立空白 profile，保留原始输出和时间点。每次最多 90 秒，额外有界清理预算与启动测试相同。该观察只访问原有公开更新入口、不安装；可能在测试 profile 写版本缓存。网络或 API 限流记录为 unavailable，不使确定性兼容测试失败。包文件若改变则属于隔离/完整性失败，不能忽略。
11. 后续 run 另行在真实旧包的新副本运行 `update --version 1.0.45 --force-reinstall`，最多 300 秒。原代码的显式版本分支直接进入安装流程；不会因为版本相同就略过。必须同时看到原始事务完成日志、EXE 文件标识改变、保留的真实旧运行映像备份、更新前后固定哈希和最终版本，才能记为成功，不能把同哈希 no-op 当作通过。这是 GNU → 同版 GNU 的真实联网更新基线，不是 GNU → MSVC 验证。原发布端点、HTTPS、信任配置保持原样；不传入登录信息。只写临时副本及隔离 profile。

EXE-only 和源码事务测试不调用安装器；联网基线只调用旧产品自己的更新入口。全部测试不执行 PowerShell bootstrap、不生成迁移包装器、不触碰现有安装。
旧 BUILD-INFO 和 SHA256SUMS 在“仅覆盖 EXE”的文件系统实验中故意保持旧值；这是旧版 executable-only 更新遗留文件的复现，不能把此目录当成可发布安装包。
parser 的另一个未发布测试 ZIP 会使用真实 MSVC Target 和显式 actual_runtime_target，GNU 字段仅作兼容传输选择器；它不是正式发布包。

## 已执行的 Windows 子步骤与验收边界

本地 Linux 上已下载并哈希核验两个真实包；结构及 EXE-only 文件差异检查通过。
原生 MSVC EXE 没有导入 VCRUNTIME/MSVCP/CONCRT DLL，普通导入为 Windows 系统库；未见延迟导入。两份 EXE 的资源目录和签名表均为空。
这不等于已证明任何 Windows 版本都没有运行时依赖问题。

2026-10-16 的 [CI 38879857879](https://github.com/JoyElliot/grok-build-Chinese/actions/runs/38879857879) 在 `windows-2022` 通过。该 run 的提交为 `d48cbd9096a4840d725e7367eb460ba5ddc5e11a`，tree 为 `b9f64c05fac0be292412f263491d2ab776d3c30f`。唯一验证 job 成功，其他 20 个 job 跳过。

- 真实旧版输出 `grok-zh 1.0.45 (3f5d07f8cb2e)`，耗时 0.156 秒；仅覆盖主程序后输出 `grok-zh 1.0.103 (ddede0f98fbb)`，耗时 0.110 秒。
- 旧、新主程序各四个命令及旧 `rg.exe --version` 均以 0 退出；18 份 stdout/stderr 日志重新计算哈希，与报告一致。
- 唯一文件变化是 `grok-zh.exe`；其余 15 个文件逐字节保持，隔离用户目录没有产生文件。
- 精确历史 Rust parser 的 27 个测试通过；真实 MSVC 完整包提取后 EXE 哈希仍为 `8626e60c908f44848190adc9f3547241c651ed8179601d93e33b91469dc76f20`。
- [报告制品 12036409516](https://github.com/JoyElliot/grok-build-Chinese/actions/runs/38879857879/artifacts/12036409516) 下载后 SHA-256 为 `af6fd44cf9506df7d4bc722ef1991eca76da2a2a3f05505f57ddf9d45061a14c`，与 GitHub digest 一致；包括 parser 的来源、依赖锁、原始日志及结果报告。

这次成功只证明实际 EXE-only 启动和历史 parser 的本地输入行为，尚未证明“旧版更新不受影响”。下一阶段使用历史原函数和明确的本地传输/目标目录测试接口，覆盖发现与选择、流式外层校验、提取、30 秒版本探测和 Windows 替换行为。真实旧二进制的固定 GitHub HTTPS 网络入口不重定向、不拦截 TLS、不修改 hosts 或系统信任。模拟 Release API 元数据将明确标作 fixture，不能冒称真实不可变 Release。

历史 Windows 替换函数先尝试 copy，遇到运行文件占用时才 rename-aside 后 copy，并带恢复逻辑；不能笼统称为原子文件交换。原 `install_community_release` 使用 `current_exe()`，故安全隔离测试若仅运行它的原事务片段，会明确列出未覆盖的外层调用边界。

在现有 `zh-dev-windows-preview.yml` 中，推送到精确分支 `test/windows-msvc-direct-replacement` 即运行隔离测试；也保留手动选择该分支并设置 `direct_replacement_only=true` 的入口：

- 只运行 `windows-direct-replacement`，固定 `windows-2022`；
- 每个原有构建及汇总 job 都显式受反向门控，不重建六平台或 MSVC 产品；
- 只编译小型历史 parser 测试 crate；
- job 只拥有 contents/actions 读取权限；
- 仅上传报告、来源摘要及命令文本日志，不上传 EXE 或 ZIP；
- 既有正式发布开关仍为 false；不创建 Release、不合并 PR、不改 PR #13。

测试分支首次发布会触发上述单 Windows 任务，须由协调者检查冻结清单后进行；无需凭据或手动 dispatch。其他分支的既有 push、PR、schedule 和普通手动构建路径保持原逻辑。

## 明确未覆盖（首个成功 run 38879857879）

- 旧真实 EXE 从真实发布服务下载、验真和完成第一次更新的网络链。
- 更新后 MSVC 继续完成第二次更新。当前实现仍选择 MSVC 资产及已有 GNU 内嵌迁移回退语义，不能据本测试宣称支持新的直接传输协议。
- v1.0.8 客户端：它的精确资产集合约束是独立 bridge 问题；1.0.45 parser 通过不代表 v1.0.8 可直接跳转。
- 实际 TUI、账号、provider、联网请求、插件及长会话。
- 正在运行文件的覆盖、安装器交互、低空间、锁、中断恢复、回滚。
- 用户电脑、干净 Windows 消费者系统、所有系统版本，以及 ESET/其他杀毒软件的真实接受情况。PE 静态导入不能穷举动态 LoadLibrary 行为。

没有授权向杀毒厂商或多引擎站点提交样本；本测试不会提交任何样本，不关闭安全防护、不添加排除项，也不为规避启发式检测调整二进制。
