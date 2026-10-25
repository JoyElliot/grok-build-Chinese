# 直接 MSVC 更新兼容验证

此独立测试项目逐字编译工作树中的 community_release.rs、全部协议模块及取消模块，避免依赖完整应用的终端、音视频和模型服务依赖。当前源码自带测试同样执行。生成器仅在完整源码之后添加独立测试模块，不修改生产函数。

历史固定输入：
- GNU 1.0.45：3f5d07f8cb2e9be395131296a3efc4635713db7f。
- GNU 1.0.8 桥接：dfcf92b37854f5b2c0dacc66a9e4195f3881dcae。
- 旧 MSVC 实验客户端：74c5269942d9a47bf09a9268db0bb5c145967e27，含完整原始 release/protocol/embedded 模块。
- 源文件从已有验证提交 3a1359d201957ccd0e33e24684dd4b291a71971b 取回；每次准备均核对原始 Git blob SHA-1，并记录当前文件 SHA-256。历史文件按原样保留，不能格式化或修改。

最近云端 Linux 验证：192 个测试通过，0 失败。当前生产源变更后必须重新运行，不能沿用该计数作为最终证明。

运行：

```sh
python .github/scripts/direct-update-validation/prepare.py
cargo test --locked --manifest-path .github/scripts/direct-update-validation/Cargo.toml
```

build.rs 会在当前源码变化时自动重新准备并校验，防止旧副本造成假通过；也可先手动 prepare。输出 generated/ 为原文测试副本；results/source-provenance.json 记录实际验证文件摘要。Cargo.lock 固定独立依赖。

额外用例将相同的 direct_runtime 描述和真实 PE 格式占位数据放入严格 GNU 目录/文件清单，分别调用四个版本的原始资产选择、完整清单验证及提取函数。连续三个版本用于检查第一次 GNU 到 MSVC 及其后两次包读取。旧 MSVC 专用用例证明双包优先选择 native ZIP 并提取其 EXE，而 GNU direct-only 会拒绝且不留下候选文件，因此双包是维持旧 MSVC 实验客户端可更新的兼容要求。

当前源码的 direct_runtime_tests 还检查 MSVC 目标提取、描述冲突、错误版本/架构、缺失字段、错误镜像、哈希篡改和失败清理。

双包发布矩阵还验证：1.0.8 的严格六资产策略会跳过新的双包发布，仍先选择保留的 1.0.45 桥接；1.0.45 与当前客户端按平台选择最新 GNU direct 包。不能把 1.0.8 直接跨过桥接描述为支持。

边界：这里没有启动 Windows 产品 EXE，没有远程 Release 下载，没有验证 Windows 文件锁/原子替换/版本探测/进程终止/回滚，也不声称产品在真实安装中完成了三轮更新。候选 PE 是非执行合成数据，Release 元数据与 ZIP 均为离线合成输入；当前社区仓库标识直接引用当前 product 模块。入口可用性函数由固定测试接缝代替；不验证完整应用入口开关。这些测试不能替代完整产品构建、原生 Windows 自动化和用户的真实 Windows/安全软件验收。不会运行嵌入 wrapper。
