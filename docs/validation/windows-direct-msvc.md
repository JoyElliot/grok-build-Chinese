# Windows x64 MSVC 直接覆盖验收

## 方案与发布边界

本方案不包含自解压启动器。兼容 ZIP 保留 GNU 资产名称、目录名、旧版完整文件集合及 SHA256SUMS.txt；其中 grok-zh.exe 与同轮原生 MSVC ZIP 中的产品 EXE 逐字节相同。

BUILD-INFO.txt 的 schema 1 协议继续以 platform=x86_64-pc-windows-gnu 标识传输容器，并新增 direct_runtime={schema:1,version:同版本,platform:x86_64-pc-windows-msvc} 标识直接执行文件。新客户端拒绝声明冲突、错误 ABI、错误版本、损坏清单、非法路径和错误 PE 类型，仍只提取并替换 EXE。旧 GNU 客户端不需要理解新字段。

发布开关仍为 enabled=false。本测试不合并 zh-dev、不创建 Release、不改变任何现有安装或安全软件配置。预览包不是公开稳定更新，禁止把模拟 GitHub 元数据/运输层的测试写成真实线上升级成功。

## 兼容矩阵

- 旧 GNU：历史解析器读取 GNU 名称直接包；实际旧二进制的线上发现仍需要发布后独立验收，不能由源码 harness 代替。
- 已有旧 MSVC 试验版：使用原生 MSVC 名称包。其旧 updater 不能读取只带 direct_runtime 的 GNU 包，不能宣称直接兼容。
- 本次新 MSVC：优先原生 MSVC 名称包；无原生资产时读取具有 direct_runtime 的 GNU 兼容包；继续接受历史 embedded_runtime 包但绝不执行其启动器。
- 全新安装：下载同轮原生 MSVC ZIP，按包内安装说明操作；直接兼容包保留旧传输结构。
- 降级：普通 GNU ZIP 不可误当成 MSVC 包；需要 GNU 完整安装流程，不采用隐式 ABI 降级。版本保留、显式重装与锁策略继续由 updater 处理。

## 用户 Windows 实机步骤

1. 只下载最终指定 CI 提交对应的候选制品，核对 SHA-256 清单与来源，不使用历史 1.0.103 构建替代本次代码。
2. 保持 Defender/ESET 正常启用；若有拦截，保留检测名称、日志和文件哈希，停止运行，不添加排除项、不关闭保护。
3. 在独立临时目录与独立 GROK_HOME 测试，备份原安装，不把测试目录加入全局 PATH。先执行 Test-DirectMsvcProduct.ps1，检查 EXE-only 替换、备份、命令行与伴随文件完整性。
4. 在正常终端与实际机器上运行 --version、--help、agent --help、update --help；检查版本、退出码、启动速度及缺失 DLL 等问题。
5. 按同轮历史 updater/当前 updater 测试报告核查发现、选择、哈希校验、替换、锁竞争与失败恢复结果；离线 fixture 测试不会访问/改写真实稳定 Release。
6. 若需要真实旧 GNU→新 MSVC→后续新 MSVC 的线上全链路验收，必须另行批准正式或隔离发布环境；本次不会为测试修改生产发布渠道。

## 自动化证据要求

交付时记录最终提交、CI URL、实际 EXE 与两种 ZIP 的 SHA-256、源码兼容 harness 报告、Windows 原生 updater 测试、产品 smoke 报告以及静态审查/修复轮次。缺失任何构建或失败测试都不得标为“仅剩 Windows 验证”。
