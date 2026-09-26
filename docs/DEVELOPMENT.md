# 源码构建与开发维护

[返回项目首页](../README.md)

开始修改前请阅读[贡献说明](../CONTRIBUTING.zh-CN.md)，并通过本仓库 Issue 与维护者沟通。上游不接受外部拉取请求；社区 Fork 尚未公布独立贡献流程。

## 从源码构建

- Rust 版本由 [`rust-toolchain.toml`](../rust-toolchain.toml) 固定。
- [DotSlash](https://dotslash-cli.com) 用于运行 `bin/` 下的工具，尤其是 `bin/protoc`。安装后确保 `dotslash` 已加入 `PATH`：

  ```sh
  cargo install dotslash
  dotslash --help
  ```

- `protoc` 优先通过 DotSlash 解析仓库内的 `bin/protoc`，也会回退到 `PATH` 或 `PROTOC` 指定的程序。
- Windows x64 使用 GNU 工具链，ARM64 使用 MSVC；依赖准备和完整打包步骤见 [Windows x64 GNU 环境配置](../.github/actions/setup-windows-gnu/action.yml)、[Windows ARM64 MSVC 构建](../.github/actions/build-windows-arm/action.yml)及 [CI 工作流](../.github/workflows/zh-dev-windows-preview.yml)。

在仓库根目录运行：

```sh
cargo run -p xai-grok-pager-bin
cargo build --locked -p xai-grok-pager-bin --release
```

普通 release 构建产物为 `target/release/grok-zh`（Windows 为 `grok-zh.exe`）。首次登录见[身份验证指南](../crates/codegen/xai-grok-pager/docs/user-guide/zh-CN/02-authentication.md)。测试时可用 `GROK_HOME` 指向独立目录，避免使用日常会话和配置。

工作区较大，日常检查优先指定具体 crate：

```sh
cargo check --locked -p xai-grok-pager-bin --bin grok-zh --features release-dist
cargo test --locked -p xai-grok-locale
cargo test --locked -p xai-grok-config
cargo clippy -p <crate>
cargo fmt --all --check
```

Windows 完整包还携带 `rg.exe`，搜索入口优先使用旁载工具，缺失时回退到系统 `PATH`。单独编译出的 EXE 不等于通过完整包校验的发布产物。

## 仓库结构

| 路径 | 内容 |
| --- | --- |
| `crates/codegen/xai-grok-locale` | 集中式语言目录、locale 解析与回退 |
| `crates/codegen/xai-grok-product` | 社区版程序名、共享数据目录、更新与隐私策略 |
| `crates/codegen/xai-grok-pager-bin` | 组合入口，生成 `grok-zh` |
| `crates/codegen/xai-grok-pager` | TUI、回滚区、提示输入、模态框和渲染 |
| `crates/codegen/xai-grok-shell` | 智能体运行时及 leader/stdio/headless 入口 |
| `crates/codegen/xai-grok-tools` | 终端、文件编辑、搜索等工具实现 |
| `crates/codegen/xai-grok-workspace` | 文件系统、版本控制、执行和检查点 |
| `crates/codegen/...` | 配置、MCP、Markdown、沙箱等其他依赖 crate |
| `crates/common/`、`crates/build/`、`prod/mc/` | 共享与构建辅助 crate |
| `third_party/` | vendored 源码；归属见其中的 `NOTICE` |

根 `Cargo.toml` 的工作区成员、依赖版本、lint 和 profile 由上游生成，应视为只读。新增社区功能优先放在独立 crate 或局部适配层，避免大范围改写上游结构。

## 汉化与兼容约定

优先修改集中式 locale 目录，不在业务代码中逐处硬编码中文。下列内容保持原样：

- CLI 子命令、参数与取值，例如 `agent`、`--resume`、`--output-format json`。
- 配置键、环境变量和序列化字段，例如 `[ui] screen_mode`、`GROK_HOME`、JSON key。
- MCP、ACP、OAuth、OIDC、OSC 52 等协议名。
- 工具名、模型 ID、会话 ID、路径、URL、日志字段和服务端原始错误。
- 代码块、占位符及 `pending`、`in_progress`、`completed`、`cancelled` 等规范状态。

中文标题与计划仅增加按语言条件生效的提示约束；标题为空或中文请求生成纯英文标题时，回退到用户输入。协议身份或兼容性所需的内部 `grok-pager` 名称仍可保留。

中文文档使用稳定文档 ID 和 `zh-CN` 平行目录，保留英文文档的查找身份。[上游用户指南](../crates/codegen/xai-grok-pager/docs/user-guide/README.md)可用于对照。

`grok` 与 `grok-zh` 直接读写同一 `~/.grok`（或 `GROK_HOME`），没有复制或双向同步层，并沿用上游的文件锁与并发规则。关闭上传的范围及维护要求见[社区版隐私策略](COMMUNITY-PRIVACY.md)。

动态译文跟随官方目录的加载和刷新检查，无独立轮询定时器；失败时使用缓存或内置译文，未收录的内容回退到官方原文。模型 ID、能力、请求参数和用户内容不受翻译影响。维护入口：

- [动态公告翻译](../community/announcements/README.md)
- [动态界面译文](../community/display-translations/README.md)

## 上游与发布策略

- `main` 保持官方上游镜像，用于同步和审查；`zh-dev` 用于汉化开发、上游合并、构建和测试。
- [`SOURCE_REV`](../SOURCE_REV) 记录对应的官方 monorepo 提交；发布构建信息另外记录 Fork 的 Git 提交。
- 稳定版不另建长期 `zh-stable` 分支，只从审核并通过 CI 的 `zh-dev` 精确提交创建受保护的 `release-vA.B.C` Tag；程序版本保持严格三段 SemVer。`v1.0.8` 是旧通道的最后一个桥接 Tag。
- 仓库 Ruleset 必须同时限制 `v*` 与 `release-v*` Tag 的创建、更新和删除权限，仅允许维护者给已审核提交打 Tag；工作流内的 SHA 复核不能替代服务端 Tag 保护。
- 上游 `main` 更新只触发审查和测试，不能直接进入用户更新源。社区更新器仅消费本仓库的 Immutable Releases，禁用官方 npm、GitHub、x.ai 和 GCS 更新源。
- [CI 预览产物](https://github.com/JoyElliot/grok-build-Chinese/actions/workflows/zh-dev-windows-preview.yml)用于构建和设备验收，不独立创建 Release；正式 Tag 工作流汇总并核验各平台资产。
- 正式更新日志、协议兼容检查、平台构建与测试、Immutable Releases 开关和精确资产摘要共同构成发布门槛。SHA-256 与 GitHub Artifact Attestation 用于核验完整性和构建来源，不等同于操作系统代码签名。

发布说明按用户可见的功能与修复归并，分别说明社区版和上游变化。每版在 `.github/release-notes/versions/<Tag>.json` 整理中文重点及来源；缺少说明时停止发布，不回退为提交列表。版本基线、来源校验、跨版本汇总及排版规则见 [Release 说明维护](../.github/release-notes/README.md)。

安装包布局、新旧客户端兼容和校验协议见[单包更新协议](COMMUNITY-UPDATE-PROTOCOL.md)；历史迁移与恢复步骤见各平台安装说明。

## 文档维护

项目首页只保留介绍、安装、基本使用和常用文档入口。安装参数与故障处理写入平台安装说明；构建、翻译及发布细节放在对应维护文档。

更新记录统一由 [Releases](https://github.com/JoyElliot/grok-build-Chinese/releases)承载，首页不逐版本追加日志链接、迁移历史或 CI 验收记录。源码中的[中文版本日志](../crates/codegen/xai-grok-shell/changelogs/)继续保留，供程序和维护者查阅。
