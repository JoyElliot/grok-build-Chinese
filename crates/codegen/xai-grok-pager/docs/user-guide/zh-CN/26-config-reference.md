# 配置参考

此文件随 CLI 一同提供，并在启动时提取到 `~/.grok/docs/user-guide/26-config-reference.md`。它完整列出了 `config.toml`、`managed_config.toml` 和 `requirements.toml` 的字段。概念性说明见 [05-configuration.md](05-configuration.md)。

## 如何配置

Grok Build 使用三个配置文件，它们由不同角色维护。

| 文件 | 编写者 | 位置 | 用途 |
| --- | --- | --- | --- |
| `config.toml` | 开发者 | `~/.grok/config.toml`，以及项目中的 `.grok/config.toml` | 设置个人默认值。使用这台机器的人可以更改其中任何内容。 |
| `managed_config.toml` | 管理员（通过控制台或部署工具） | `/etc/grok/managed_config.toml` | 向机群下发初始默认值；开发者自己的文件可覆盖它。 |
| `requirements.toml` | 管理员（带签名） | `/etc/grok/requirements.toml`，或 macOS 设备管理 | 设置开发者无法更改的值。下表标记为 `pin` 的键会压过其他文件、环境变量和命令行。 |

希望用户能够自行调整时，请使用 `managed_config.toml`；不允许调整时，请使用 `requirements.toml`。

Grok Build 还会按以下层级读取配置；后列层级优先，但 requirements 固定值或下表“托管”列另有说明时除外。

1. 编译时默认值。
2. `/etc/grok/managed_config.toml`，然后是 `$GROK_HOME/managed_config.toml`（机群默认值，由控制台同步）。
3. `$GROK_HOME/config.toml`（用户设置；`/settings` 写入这里）。`$GROK_HOME` 默认是 `~/.grok`。
4. 项目 `.grok/config.toml`：仅支持 `[mcp_servers]`、`[plugins]`、`[permission]` 和 `[mcp] max_output_bytes`。
5. `GROK_CONFIG`（内联 JSON）或 `GROK_CONFIG_PATH`（JSON/TOML 文件），仅接受允许列表中的键。
6. `$GROK_HOME/requirements.toml`、`/etc/grok/requirements.toml`，再到 macOS MDM `ai.x.grok`。这是管理员层。表中标记为 `pin` 的键不能覆盖；标记为 `yes` 的键也可出现在此文件。
7. `GROK_*` 环境变量。
8. `--model`、`--sandbox`、`--yolo` 等 CLI 标志。

运行 `grok-zh inspect` 或 `grok-zh inspect --json` 可查看最终生效的文件和值。

## config.toml

用户级配置位于 `$GROK_HOME/config.toml`（默认 `~/.grok/config.toml`；Windows 为 `%USERPROFILE%\.grok\config.toml`）。项目级覆盖位于 `.grok/config.toml`，且仅能提供 `[mcp_servers]`、`[plugins]`、`[permission]` 和 `[mcp] max_output_bytes`。

“Requirements”列说明同一键能否写入 `requirements.toml`：`pin` 表示不可覆盖（解析器遵守固定值时，环境变量和 CLI 也不例外）；`yes` 表示可写入；`—` 表示 `requirements.toml` 不读取它。“托管”列中，`fleet` 表示机群 `managed_config.toml` 的值优先，`user` 表示用户文件优先。

### `agent`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `agent.definition` | `string (path)` | `yes` | `user` | 带 YAML frontmatter 的智能体定义 Markdown 文件路径。 |
| `agent.name` | `string` | `yes` | `user` | 内置或发现的智能体定义名称；也对应 `GROK_AGENT` 和 `--agent-profile`。 |
| `agent.system_prompt_label` | `string` | `yes` | `user` | 全局系统提示身份标签；单模型覆盖优先。 |

### `announcements`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `announcements` | `array of tables` | `—` | `user` | 加载时使用的远程公告载荷，不是用户手写表。 |

### `auth`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `auth` | `table` | `yes` | `user` | `[grok_com_config]` 的别名；所有 `grok_com_config.*` 键也可写成 `auth.*`。 |
| `auth.auth_provider_command` | `string` | `yes` | `user` | 外部认证程序；标准输出作为令牌。也对应 `GROK_AUTH_PROVIDER_COMMAND`。 |
| `auth.auth_provider_label` | `string` | `yes` | `user` | 外部认证提供方的登录按钮标签。也对应 `GROK_AUTH_PROVIDER_LABEL`。 |
| `auth.auth_token_ttl` | `number` | `yes` | `user` | 仅返回裸令牌的提供方所用令牌 TTL（秒）。也对应 `GROK_AUTH_TOKEN_TTL`。 |
| `auth.disable_api_key_auth` | `boolean` | `pin` | `user` | 拒绝 API Key 认证，仅允许部署的 IdP 登录。也对应 `GROK_DISABLE_API_KEY_AUTH`。 |
| `auth.force_login_team_uuid` | `string / string[]` | `pin` | `user` | 强制登录指定团队 UUID（或数组中的任一团队）；空数组按失败关闭处理。也对应 `GROK_FORCE_LOGIN_TEAM_ID`。 |
| `auth.grok_ws_origin` | `string` | `yes` | `user` | grok.com 的 WebSocket origin。也对应 `GROK_WS_ORIGIN`。 |
| `auth.grok_ws_url` | `string` | `yes` | `user` | 中继 WebSocket URL。也对应 `GROK_WS_URL`。 |
| `auth.oauth2` | `table` | `yes` | `user` | 未设置企业 OIDC 时使用的 OAuth2 提供方。 |
| `auth.oauth2.client_id` | `string` | `yes` | `user` | OAuth2 客户端 ID。也对应 `GROK_OAUTH2_CLIENT_ID`。 |
| `auth.oauth2.issuer` | `string` | `yes` | `user` | OAuth2 颁发者 URL。也对应 `GROK_OAUTH2_ISSUER`。 |
| `auth.oauth2.principal_id` | `string` | `yes` | `user` | 设置 `principal_type` 时必填的主体 ID。也对应 `GROK_OAUTH2_PRINCIPAL_ID`。 |
| `auth.oauth2.principal_type` | `string` | `yes` | `user` | 令牌主体类型，例如 Team。也对应 `GROK_OAUTH2_PRINCIPAL_TYPE`。 |
| `auth.oauth2.referrer` | `string` | `yes` | `user` | OAuth 使用归因的 referrer。也对应 `GROK_OAUTH2_REFERRER`。 |
| `auth.oauth2.scopes` | `string[]` | `yes` | `user` | OAuth2 scope。也对应 `GROK_OAUTH2_SCOPES`。 |
| `auth.oidc` | `table` | `yes` | `user` | 客户 OIDC 身份提供方设置。 |
| `auth.oidc.audience` | `string` | `yes` | `user` | 可选 OIDC audience。也对应 `GROK_OIDC_AUDIENCE`。 |
| `auth.oidc.client_id` | `string` | `yes` | `user` | OIDC 客户端 ID。也对应 `GROK_OIDC_CLIENT_ID`。 |
| `auth.oidc.issuer` | `string` | `yes` | `user` | OIDC 颁发者 URL。也对应 `GROK_OIDC_ISSUER`。 |
| `auth.oidc.scopes` | `string[]` | `yes` | `user` | OIDC scope。也对应 `GROK_OIDC_SCOPES`。 |
| `auth.preferred_method` | `api_key / oidc` | `yes` | `user` | 将自动认证固定为一种方式，不回退。 |
| `auth.token_header` | `string` | `yes` | `user` | 携带 CLI 认证令牌的标头名；默认为 `xai-grok-cli`。 |

### `auth_provider`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `auth_provider.<name>` | `table` | `yes` | `user` | 由 `[model.<id>] auth_provider` 引用的命名凭据助手。 |

### `auto_mode`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `auto_mode.enabled` | `boolean` | `yes` | `user` | 启用 Auto 权限模式。 |

### `campaigns`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `campaigns` | `array of tables` | `yes` | `user` | 在 requirements 之下应用的命名 campaign 补丁，由部署系统发布。 |

### `cli`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `cli.auto_update` | `boolean` | `pin` | `user` | 启动时检查 CLI 更新；`GROK_DISABLE_AUTOUPDATER` 可禁用。 |
| `cli.channel` | `stable / alpha` | `pin` | `user` | 首选发布通道。 |
| `cli.installer` | `string` | `—` | `user` | 最近安装此 CLI 的安装器，用于选择更新路径。 |
| `cli.maximum_version` | `string` | `pin` | `user` | 不触发硬阻止时可运行的最高 CLI 版本。也对应 `GROK_MAXIMUM_VERSION`。 |
| `cli.minimum_version` | `string` | `pin` | `user` | 不触发硬阻止时可运行的最低 CLI 版本。也对应 `GROK_MINIMUM_VERSION`。 |
| `cli.npm_registry` | `string` | `yes` | `user` | 自动更新器使用的 npm registry。 |
| `cli.required_maximum_version` | `string` | `pin` | `user` | 强制最高 CLI 版本。也对应 `GROK_REQUIRED_MAXIMUM_VERSION`。 |
| `cli.required_minimum_version` | `string` | `pin` | `user` | 强制最低 CLI 版本。也对应 `GROK_REQUIRED_MINIMUM_VERSION`。 |
| `cli.session_picker_grouped` | `boolean` | `yes` | `user` | 在会话选择器和 CLI 列表中按仓库分组。 |
| `cli.session_registry` | `boolean` | `yes` | `user` | 加入跨进程会话注册表。 |
| `cli.show_tips` | `boolean` | `pin` | `user` | 是否显示启动提示。 |
| `cli.use_leader` | `boolean` | `pin` | `user` | 使用 leader 进程处理配置重载和 MCP 监视。 |
| `cli.worktree_type` | `string` | `yes` | `user` | 首选工作树实现。 |

### `compat`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `compat.claude.agents` | `boolean` | `yes` | `user` | 扫描 CLAUDE.md。也对应 `GROK_CLAUDE_AGENTS_ENABLED`。 |
| `compat.claude.hooks` | `boolean` | `yes` | `user` | 扫描 Claude hooks。也对应 `GROK_CLAUDE_HOOKS_ENABLED`。 |
| `compat.claude.mcps` | `boolean` | `yes` | `user` | 扫描 Claude MCP 配置。也对应 `GROK_CLAUDE_MCPS_ENABLED`。 |
| `compat.claude.rules` | `boolean` | `yes` | `user` | 扫描 Claude rules。也对应 `GROK_CLAUDE_RULES_ENABLED`。 |
| `compat.claude.skills` | `boolean` | `yes` | `user` | 扫描 Claude skills。也对应 `GROK_CLAUDE_SKILLS_ENABLED`。 |
| `compat.codex.hooks` | `boolean` | `yes` | `user` | 存在时扫描 Codex hooks。 |
| `compat.codex.skills` | `boolean` | `yes` | `user` | 存在时扫描 Codex skills 目录。 |
| `compat.cursor.agents` | `boolean` | `yes` | `user` | 从 Cursor 兼容来源扫描智能体定义。也对应 `GROK_CURSOR_AGENTS_ENABLED`。 |
| `compat.cursor.hooks` | `boolean` | `yes` | `user` | 扫描 Cursor hooks。也对应 `GROK_CURSOR_HOOKS_ENABLED`。 |
| `compat.cursor.mcps` | `boolean` | `yes` | `user` | 扫描 Cursor `mcp.json`。也对应 `GROK_CURSOR_MCPS_ENABLED`。 |
| `compat.cursor.rules` | `boolean` | `yes` | `user` | 扫描 `.cursor/rules/`。也对应 `GROK_CURSOR_RULES_ENABLED`。 |
| `compat.cursor.skills` | `boolean` | `yes` | `user` | 扫描 Cursor skills 目录。也对应 `GROK_CURSOR_SKILLS_ENABLED`。 |

### `dashboard`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `dashboard.enabled` | `boolean` | `yes` | `user` | 显示智能体 dashboard。 |
| `dashboard.grouping` | `state / directory` | `yes` | `user` | dashboard 行的分组方式。 |

### `default_auto_mode`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `default_auto_mode` | `boolean` | `yes` | `user` | 未设置单会话覆盖时，以 auto 权限模式启动。 |

### `diagnostics`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `diagnostics.crash_handler` | `boolean` | `yes` | `user` | 在 `$GROK_HOME/crash/` 下写入 panic 报告。也对应 `GROK_CRASH_HANDLER`。 |

### `disable_web_search`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `disable_web_search` | `boolean` | `yes` | `user` | 在本进程中移除 `web_search` 工具。也对应 `--disable-web-search`。 |

### `disabled_mcp_servers`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `disabled_mcp_servers` | `string[]` | `yes` | `user` | 不删除 `[mcp_servers]` 配置块的情况下跳过指定 MCP 服务器。 |

### `disabled_mcp_tools`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `disabled_mcp_tools` | `map<string, string[]>` | `yes` | `user` | 按服务器名设置 MCP 工具拒绝列表。 |

### `doom_loop_recovery`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `doom_loop_recovery.enabled` | `boolean` | `yes` | `user` | 对高置信工具调用循环重新采样；设为 false 可禁用。 |

### `endpoints`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `endpoints.cli_chat_proxy_base_url` | `string` | `pin` | `user` | 会话服务 API 基础 URL。 |
| `endpoints.deployment_key` | `string` | `pin` | `user` | 企业部署的管理密钥。也对应 `GROK_DEPLOYMENT_KEY`。 |
| `endpoints.feedback_base_url` | `string` | `yes` | `user` | 反馈提交目标。也对应 `GROK_FEEDBACK_BASE_URL`。 |
| `endpoints.managed_config_url` | `string` | `yes` | `user` | 覆盖托管配置端点。也对应 `GROK_MANAGED_CONFIG_URL`。 |
| `endpoints.models_base_url` | `string` | `pin` | `user` | 自定义推理基础 URL。也对应 `GROK_MODELS_BASE_URL`。 |
| `endpoints.models_list_url` | `string` | `pin` | `user` | 覆盖模型列表 URL。也对应 `GROK_MODELS_LIST_URL`；别名 `models_endpoint`。 |
| `endpoints.trace_upload_bucket` | `string` | `yes` | `user` | 跟踪数据直传的 `gs://` 或 `s3://` bucket，绕过代理。也对应 `GROK_TRACE_UPLOAD_BUCKET`。 |
| `endpoints.trace_upload_credentials` | `string` | `yes` | `user` | bucket 的内联 GCS 服务账号 JSON 或 AWS 凭据；优先于凭据文件，且没有环境变量。 |
| `endpoints.trace_upload_credentials_file` | `string (path)` | `yes` | `user` | bucket 所用 GCS 服务账号 JSON 或 AWS 凭据文件路径。也对应 `GROK_TRACE_UPLOAD_CREDENTIALS_FILE`。 |
| `endpoints.trace_upload_endpoint_url` | `string` | `yes` | `user` | `s3://` bucket 上传所用自定义 S3 兼容端点。也对应 `GROK_TRACE_UPLOAD_ENDPOINT_URL`。 |
| `endpoints.trace_upload_region` | `string` | `yes` | `user` | `s3://` bucket 上传所用 AWS 区域；默认 `us-east-1`。也对应 `GROK_TRACE_UPLOAD_REGION`。 |
| `endpoints.trace_upload_url` | `string` | `pin` | `user` | 未配置直传 bucket 时的跟踪代理目标。也对应 `GROK_TRACE_UPLOAD_URL`。 |
| `endpoints.xai_api_base_url` | `string` | `pin` | `user` | 公共 xAI API 基础地址。也对应 `GROK_XAI_API_BASE_URL`。 |

### `features`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `features.active_agent_messages` | `boolean` | `pin` | `user` | 启用或禁用 `active_agent_messages`；默认 false。也对应 `GROK_ACTIVE_AGENT_MESSAGES`。 |
| `features.ask_user_question` | `boolean` | `pin` | `user` | 启用或禁用 `ask_user_question`；默认 true。也对应 `GROK_ASK_USER_QUESTION`。 |
| `features.auto_wake` | `boolean` | `pin` | `user` | 启用或禁用 `auto_wake`；默认 true。也对应 `GROK_AUTO_WAKE`。 |
| `features.backend_tools` | `boolean` | `pin` | `user` | 启用或禁用 `backend_tools`；默认 true。也对应 `GROK_BACKEND_SEARCH`。 |
| `features.campaigns` | `boolean` | `yes` | `user` | 启用远程 campaign 补丁；即使 requirements 设为 true，`GROK_CAMPAIGNS=0` 仍会禁用。 |
| `features.cancel_rewind` | `boolean` | `pin` | `user` | 启用或禁用 `cancel_rewind`；默认 true。也对应 `GROK_CANCEL_REWIND`。 |
| `features.codebase_indexing` | `boolean / string[]` | `pin` | `user` | 代码库图索引；true 会索引 Git 仓库，也可传入包含／排除 glob。 |
| `features.compaction_detail` | `none / minimal / balanced / verbose` | `yes` | `user` | `segments` 压缩的逐字细节级别。也对应 `GROK_COMPACTION_DETAIL`。 |
| `features.compaction_mode` | `summary / transcript / segments` | `yes` | `user` | 压缩策略。也对应 `GROK_COMPACTION_MODE`。 |
| `features.compaction_tool_choice` | `string` | `yes` | `user` | 压缩期间使用的 tool-choice 提示。 |
| `features.compaction_verbatim_input` | `boolean` | `pin` | `user` | 启用或禁用 `compaction_verbatim_input`；默认 true。也对应 `GROK_COMPACTION_VERBATIM_INPUT`。 |
| `features.dock` | `boolean` | `pin` | `user` | 启用或禁用 `dock`；默认 false。也对应 `GROK_DOCK`。 |
| `features.feedback` | `boolean` | `pin` | `user` | 启用或禁用反馈；默认 true。也对应 `GROK_FEEDBACK_ENABLED`。 |
| `features.feedback_trace_card` | `boolean` | `pin` | `user` | `/feedback` 后显示跟踪上传同意问题；默认 false。也对应 `GROK_FEEDBACK_TRACE_CARD`。 |
| `features.image_edit_model_override` | `string` | `yes` | `user` | `image_edit` 使用的 Imagine 模型 ID。 |
| `features.image_gen` | `boolean` | `pin` | `user` | 启用 `image_gen` / `/imagine`。 |
| `features.image_gen_model_override` | `string` | `yes` | `user` | `image_gen` 使用的 Imagine 模型 ID；空值回退到远程默认值。 |
| `features.lsp_tools` | `boolean` | `pin` | `user` | 启用或禁用 `lsp_tools`；默认 false。也对应 `GROK_LSP_TOOLS`。 |
| `features.managed_config` | `boolean` | `yes` | `user` | 从部署系统获取 `managed_config.toml` 和 `requirements.toml`。 |
| `features.mcp_auto_restart` | `boolean` | `yes` | `user` | stdio MCP 服务器传输失败后自动重启。也对应 `GROK_MCP_AUTO_RESTART`。 |
| `features.mcp_liveness_watchers` | `boolean` | `yes` | `user` | 轮询 MCP 传输并推送 `server_status`；false 是紧急关闭开关。 |
| `features.mcp_push_server_status` | `boolean` | `yes` | `user` | pager 订阅 MCP `server_status` 推送；进程环境变量 `GROK_MCP_PUSH_SERVER_STATUS` 在启动时优先。 |
| `features.mcp_recursive_config_watch` | `boolean` | `yes` | `user` | 监视 `<cwd>/` 和 `<cwd>/.grok/` 中的项目 MCP 配置编辑；名称虽含 recursive，实际为非递归监视。 |
| `features.non_git_warning` | `boolean` | `yes` | `user` | Grok 在非 Git 仓库中启动时显示阻塞警告。 |
| `features.remember_mode` | `boolean` | `—` | `—` | 跨会话记住上次权限模式；仅从用户 `config.toml` 读取。 |
| `features.remote_fetch` | `boolean` | `pin` | `fleet` | 固定远程模型目录和资源获取；托管值与用户值同时存在时，托管值优先。 |
| `features.repo_status_in_system_prompt` | `boolean` | `pin` | `user` | 启用或禁用 `repo_status_in_system_prompt`；默认 true。也对应 `GROK_REPO_STATUS_IN_SYSTEM_PROMPT`。 |
| `features.session_recap` | `boolean` | `pin` | `user` | 启用或禁用 `session_recap`；默认 true。也对应 `GROK_SESSION_RECAP`。 |
| `features.session_search` | `boolean` | `pin` | `user` | 启用或禁用 `session_search`；默认 true。也对应 `GROK_SESSION_SEARCH`。 |
| `features.subagent_worktree_snapshot` | `boolean` | `pin` | `user` | 启用或禁用 `subagent_worktree_snapshot`；默认 false。也对应 `GROK_SUBAGENT_WORKTREE_SNAPSHOT`。 |
| `features.support_permission` | `boolean` | `yes` | `user` | 允许智能体为工具执行请求权限。 |
| `features.telemetry` | `boolean / session_metrics / off` | `pin` | `user` | 产品遥测模式；企业默认关闭。 |
| `features.title_refresh` | `boolean` | `pin` | `user` | 会话早期自动刷新标题；在 requirements 中固定可压过 `GROK_TITLE_REFRESH`。 |
| `features.turn_summary` | `boolean` | `pin` | `user` | 启用或禁用 `turn_summary`；默认 true。也对应 `GROK_TURN_SUMMARY`。 |
| `features.two_pass_compaction` | `boolean` | `pin` | `user` | 启用或禁用 `two_pass_compaction`；默认 true。也对应 `GROK_TWO_PASS_COMPACTION`。 |
| `features.video_gen` | `boolean` | `pin` | `user` | 启用视频工具／`/imagine-video`。 |
| `features.voice_mode` | `boolean` | `pin` | `user` | 启用或禁用 `voice_mode`；默认 true。也对应 `GROK_VOICE_MODE`。 |
| `features.web_fetch` | `boolean` | `pin` | `user` | 启用或禁用 `web_fetch`；默认 false。也对应 `GROK_WEB_FETCH`。 |
| `features.write_file` | `boolean` | `pin` | `user` | 启用或禁用 `write_file`；默认 true。也对应 `GROK_WRITE_FILE`。 |
| `features.zdr_access_enabled` | `boolean` | `pin` | `user` | 团队启用零数据保留时，是否展示与 ZDR 不兼容的工具。也对应 `GROK_ZDR_ACCESS_ENABLED`。 |

### `feedback`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `feedback.user.command` | `string` | `yes` | `user` | 输出反馈提交者姓名和邮箱 JSON 的 Shell 命令。 |
| `feedback.user.email` | `string[]` | `yes` | `user` | 反馈提交者邮箱来源（`git_email` 或字面值）。 |
| `feedback.user.name` | `string[]` | `yes` | `user` | 反馈提交者姓名来源（`os_user` 或字面值）。 |

### `goal`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `goal.enabled` | `boolean` | `yes` | `user` | 启用 `/goal`。 |

### `grok_com_config`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `grok_com_config` | `table` | `yes` | `user` | Grok.com WebSocket 和 OAuth/OIDC 设置；`[auth]` 是别名。 |
| `grok_com_config.auth_provider_command` | `string` | `yes` | `user` | 外部认证程序；标准输出作为令牌。也对应 `GROK_AUTH_PROVIDER_COMMAND`。 |
| `grok_com_config.auth_provider_label` | `string` | `yes` | `user` | 外部认证提供方的登录按钮标签。也对应 `GROK_AUTH_PROVIDER_LABEL`。 |
| `grok_com_config.auth_token_ttl` | `number` | `yes` | `user` | 裸令牌提供方的令牌 TTL（秒）。也对应 `GROK_AUTH_TOKEN_TTL`。 |
| `grok_com_config.disable_api_key_auth` | `boolean` | `pin` | `user` | 拒绝 API Key 认证，仅允许部署的 IdP 登录。也对应 `GROK_DISABLE_API_KEY_AUTH`。 |
| `grok_com_config.force_login_team_uuid` | `string / string[]` | `pin` | `user` | 强制登录指定团队 UUID（或数组中的任一团队）；空数组按失败关闭处理。也对应 `GROK_FORCE_LOGIN_TEAM_ID`。 |
| `grok_com_config.grok_ws_origin` | `string` | `yes` | `user` | grok.com WebSocket origin。也对应 `GROK_WS_ORIGIN`。 |
| `grok_com_config.grok_ws_url` | `string` | `yes` | `user` | 中继 WebSocket URL。也对应 `GROK_WS_URL`。 |
| `grok_com_config.oauth2` | `table` | `yes` | `user` | 未设置企业 OIDC 时使用的 OAuth2 提供方。 |
| `grok_com_config.oauth2.client_id` | `string` | `yes` | `user` | OAuth2 客户端 ID。也对应 `GROK_OAUTH2_CLIENT_ID`。 |
| `grok_com_config.oauth2.issuer` | `string` | `yes` | `user` | OAuth2 颁发者 URL。也对应 `GROK_OAUTH2_ISSUER`。 |
| `grok_com_config.oauth2.principal_id` | `string` | `yes` | `user` | 设置 `principal_type` 时必填的主体 ID。也对应 `GROK_OAUTH2_PRINCIPAL_ID`。 |
| `grok_com_config.oauth2.principal_type` | `string` | `yes` | `user` | 令牌主体类型，例如 Team。也对应 `GROK_OAUTH2_PRINCIPAL_TYPE`。 |
| `grok_com_config.oauth2.referrer` | `string` | `yes` | `user` | OAuth 使用归因的 referrer。也对应 `GROK_OAUTH2_REFERRER`。 |
| `grok_com_config.oauth2.scopes` | `string[]` | `yes` | `user` | OAuth2 scope。也对应 `GROK_OAUTH2_SCOPES`。 |
| `grok_com_config.oidc` | `table` | `yes` | `user` | 客户 OIDC 身份提供方设置。 |
| `grok_com_config.oidc.audience` | `string` | `yes` | `user` | 可选 OIDC audience。也对应 `GROK_OIDC_AUDIENCE`。 |
| `grok_com_config.oidc.client_id` | `string` | `yes` | `user` | OIDC 客户端 ID。也对应 `GROK_OIDC_CLIENT_ID`。 |
| `grok_com_config.oidc.issuer` | `string` | `yes` | `user` | OIDC 颁发者 URL。也对应 `GROK_OIDC_ISSUER`。 |
| `grok_com_config.oidc.scopes` | `string[]` | `yes` | `user` | OIDC scope。也对应 `GROK_OIDC_SCOPES`。 |
| `grok_com_config.preferred_method` | `api_key / oidc` | `yes` | `user` | 将自动认证固定为一种方式，不回退。 |
| `grok_com_config.token_header` | `string` | `yes` | `user` | 携带 CLI 认证令牌的标头名；默认为 `xai-grok-cli`。 |

### `harness`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `harness.block_for_upload` | `boolean` | `yes` | `user` | 工作区快照上传完成前阻止轮次结束。 |
| `harness.disable_workspace_teleport` | `boolean` | `pin` | `user` | 每轮工作区快照的关闭开关。 |

### `hints`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `hints.fork_worktree_mode` | `ask / always / never` | `yes` | `user` | `/fork` 是否提供工作树选项。 |
| `hints.new_session_worktree_mode` | `ask / always / never` | `yes` | `user` | `/new` 是否提供工作树选项。 |

### `hooks`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `hooks.<event>` | `array of tables` | `yes` | `user` | PreToolUse、Stop 等生命周期事件的匹配器组；详见 Hooks。 |
| `hooks.<event>[].hooks[].command` | `string` | `yes` | `user` | 为该 hook 运行的命令；加载时不展开 `$VAR`。 |
| `hooks.<event>[].hooks[].type` | `command` | `yes` | `user` | hook 处理器类型；支持命令 hook。 |
| `hooks.<event>[].matcher` | `string` | `yes` | `user` | 该 hook 组的工具名匹配器。 |

### `managed_mcps`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `managed_mcps.enabled` | `boolean` | `pin` | `user` | 启动时获取托管 MCP 配置。也对应 `GROK_MANAGED_MCPS_ENABLED`。 |
| `managed_mcps.gateway_tools_enabled` | `boolean` | `yes` | `user` | 暴露托管 MCP 网关工具。也对应 `GROK_MANAGED_MCP_GATEWAY_TOOLS_ENABLED`。 |

### `marketplace`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `marketplace.sources` | `array of tables` | `yes` | `user` | `[[marketplace.sources]]` 插件市场仓库。 |

### `mcp`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `mcp.max_output_bytes` | `number` | `yes` | `user` | MCP 工具输出的字节上限；项目文件可以设置。 |

### `mcp_servers`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `mcp_servers.<name>.args` | `string[]` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `args`。 |
| `mcp_servers.<name>.bearer_token_env_var` | `string` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `bearer_token_env_var`。 |
| `mcp_servers.<name>.command` | `string` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `command`。 |
| `mcp_servers.<name>.cwd` | `string` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `cwd`。 |
| `mcp_servers.<name>.enabled` | `boolean` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `enabled`。 |
| `mcp_servers.<name>.env` | `table` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `env`。 |
| `mcp_servers.<name>.expose_image_base64` | `boolean` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `expose_image_base64`。 |
| `mcp_servers.<name>.headers` | `table` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `headers`。 |
| `mcp_servers.<name>.oauth` | `table` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `oauth`。 |
| `mcp_servers.<name>.oauth_client_id` | `string` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `oauth_client_id`。 |
| `mcp_servers.<name>.oauth_client_secret_env_var` | `string` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `oauth_client_secret_env_var`。 |
| `mcp_servers.<name>.oauth_scopes` | `string[]` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `oauth_scopes`。 |
| `mcp_servers.<name>.setup` | `table` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `setup`。 |
| `mcp_servers.<name>.startup_timeout_sec` | `number` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `startup_timeout_sec`。 |
| `mcp_servers.<name>.tool_timeout_sec` | `number` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `tool_timeout_sec`。 |
| `mcp_servers.<name>.tool_timeouts` | `table` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `tool_timeouts`。 |
| `mcp_servers.<name>.type` | `string` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `type`。 |
| `mcp_servers.<name>.url` | `string` | `yes` | `user` | stdio 或 HTTP MCP 服务器的 `url`。 |

### `memory`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `memory.enabled` | `boolean` | `pin` | `user` | 跨会话记忆总开关。也对应 `GROK_MEMORY`。 |

### `model`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `model.<id>` | `table` | `yes` | `user` | 单模型覆盖或 BYOK 定义；内联 `api_key` 不如 `env_key` 安全。 |
| `model.<id>.agent_type` | `string` | `yes` | `user` | 与该模型关联的智能体定义类型。 |
| `model.<id>.api_backend` | `chat_completions / responses / messages` | `yes` | `user` | 该模型使用的线协议。 |
| `model.<id>.api_base_url` | `string` | `yes` | `user` | 配合 XAI_API_KEY 解析使用的替代 API 基础 URL。 |
| `model.<id>.api_key` | `string` | `yes` | `user` | 内联 API Key；优先使用 `env_key`，不要把密钥放入共享仓库。 |
| `model.<id>.auth_provider` | `string` | `yes` | `user` | 为模型签发 bearer token 的 `[auth_provider.<name>]` 助手名称。 |
| `model.<id>.auto_compact_threshold_percent` | `integer` | `yes` | `user` | 单模型自动压缩阈值（0–100）。 |
| `model.<id>.base_url` | `string` | `yes` | `user` | 提供方端点基础 URL。 |
| `model.<id>.compaction_at_tokens` | `number / table` | `yes` | `user` | 该模型触发压缩的 token 阈值。 |
| `model.<id>.compactions_remaining` | `string / table` | `yes` | `user` | 压缩后剩余上下文的发送方式；别名 `send_compactions_remaining`。 |
| `model.<id>.context_window` | `number` | `yes` | `user` | 上下文窗口 token 数，决定自动压缩时机。 |
| `model.<id>.description` | `string` | `yes` | `user` | 模型选择器中的可选描述。 |
| `model.<id>.env_http_headers` | `map<string,string>` | `yes` | `user` | 从已设置环境变量填充的 HTTP 标头。 |
| `model.<id>.env_key` | `string / string[]` | `yes` | `user` | 保存提供方 API Key 的环境变量名。 |
| `model.<id>.extra_headers` | `map<string,string>` | `yes` | `user` | 该模型每个请求附加的标头。 |
| `model.<id>.hidden` | `boolean` | `yes` | `user` | 从选择器隐藏模型；仍可用 `-m` 选择。 |
| `model.<id>.inference_idle_timeout_secs` | `number` | `yes` | `user` | 该模型流式推理的空闲超时。 |
| `model.<id>.max_completion_tokens` | `number` | `yes` | `user` | 该模型最大补全 token 数。 |
| `model.<id>.max_retries` | `number` | `yes` | `user` | 该模型的推理重试次数。 |
| `model.<id>.model` | `string` | `yes` | `user` | 发送给 API 的模型 ID。 |
| `model.<id>.model_family` | `string` | `yes` | `user` | 用于压缩和能力分组的模型家族 ID。 |
| `model.<id>.model_provider` | `string` | `yes` | `user` | 该模型使用的命名 `[model_providers.<name>]` 提供方 ID。 |
| `model.<id>.name` | `string` | `yes` | `user` | 模型选择器中显示的标签。 |
| `model.<id>.query_params` | `map<string,string>` | `yes` | `user` | 该模型请求的额外查询参数。 |
| `model.<id>.reasoning_effort` | `string` | `yes` | `user` | 已弃用的单模型推理强度；优先使用 `reasoning_efforts`。 |
| `model.<id>.reasoning_efforts` | `array of tables` | `yes` | `user` | 该模型允许的推理强度取值。 |
| `model.<id>.show_model_fingerprint` | `boolean` | `yes` | `user` | 提供方返回模型 fingerprint 时在 UI 中显示。 |
| `model.<id>.stream_tool_calls` | `boolean` | `yes` | `user` | 该模型工具调用流式请求的形状。 |
| `model.<id>.supported_in_api` | `boolean` | `yes` | `user` | 此目录条目是否作为公共 API 模型提供。 |
| `model.<id>.supports_backend_search` | `boolean` | `yes` | `user` | 端点是否支持 Grok 托管的服务端搜索工具。 |
| `model.<id>.supports_reasoning_effort` | `boolean` | `yes` | `user` | 已弃用；优先使用 `reasoning_efforts`。 |
| `model.<id>.system_prompt_label` | `string` | `yes` | `user` | 单模型系统提示身份标签。 |
| `model.<id>.temperature` | `number` | `yes` | `user` | 单模型采样 temperature。 |
| `model.<id>.top_p` | `number` | `yes` | `user` | 单模型 `top_p`。 |
| `model.<id>.use_concise` | `boolean` | `yes` | `user` | 对该模型使用精简工具描述包。 |

### `model_providers`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `model_providers.<name>` | `table` | `yes` | `user` | 命名的自定义模型提供方定义。 |

### `models`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `models.agent_type` | `string` | `yes` | `user` | 未配置单模型覆盖时的 `agent_type` 回退值。 |
| `models.allowed_models` | `string[]` | `pin` | `user` | 模型选择器、默认模型和 `-m` 的 glob 允许列表；空数组表示不限制。 |
| `models.default` | `string` | `pin` | `user` | 新会话使用的模型。也对应 `GROK_DEFAULT_MODEL`、`--model`、`-m`。 |
| `models.default_reasoning_effort` | `string` | `yes` | `user` | 默认模型支持推理强度时使用的默认值。 |
| `models.disabled_models` | `string[]` | `yes` | `user` | 从目录移除这些模型 ID；优先于 `hidden_models`。 |
| `models.extra_headers` | `map<string,string>` | `yes` | `user` | 应用于所有模型的请求标头；单模型同名键优先。 |
| `models.hidden_models` | `string[]` | `yes` | `user` | 从选择器隐藏这些模型 ID；仍可用 `-m` 选择。 |
| `models.image_description` | `string` | `yes` | `user` | 转写用户图像的视觉模型。 |
| `models.inference_idle_timeout_secs` | `number` | `yes` | `user` | 模型未设置时使用的全局流式推理空闲超时。 |
| `models.max_completion_tokens` | `number` | `yes` | `user` | 模型未设置时使用的全局最大补全 token 默认值。 |
| `models.max_retries` | `number` | `yes` | `user` | 模型未设置时使用的全局推理重试默认值。 |
| `models.prompt_suggestion` | `string` | `yes` | `user` | 下一提示幽灵文本所用模型；未设置时依次回退到远程值和当前会话模型。 |
| `models.session_summary` | `string` | `yes` | `user` | 生成会话标题和摘要的模型。 |
| `models.stream_tool_calls` | `boolean` | `yes` | `user` | 全局工具调用流式请求形状；某些 BYOK 端点需要 false。 |
| `models.temperature` | `number` | `yes` | `user` | 模型未设置时使用的全局采样 temperature。 |
| `models.top_p` | `number` | `yes` | `user` | 模型未设置时使用的全局 `top_p`。 |
| `models.web_search` | `string` | `pin` | `user` | 客户端 `web_search` 工具使用的模型。也对应 `GROK_WEB_SEARCH_MODEL`。 |

### `path_not_found_hints`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `path_not_found_hints` | `boolean` | `yes` | `user` | 为路径不存在错误补充 CWD 提醒和相似名称建议。 |

### `paths`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `paths.extra_rule_dirs` | `string[]` | `yes` | `user` | 额外规则目录（每个目录含 `*.md`）。 |
| `paths.extra_skill_dirs` | `string[]` | `yes` | `user` | 额外技能目录（每个目录含 `<skill>/SKILL.md`）。 |

### `permission`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `permission.allow` | `string[]` | `yes` | `user` | `Bash(git *)` 等紧凑 allow 规则；优先级为 deny > ask > allow。项目文件可设置。 |
| `permission.ask` | `string[]` | `yes` | `user` | 紧凑 ask 规则；项目文件可设置。 |
| `permission.deny` | `string[]` | `yes` | `user` | 紧凑 deny 规则；项目文件可设置。 |
| `permission.rules` | `array of tables` | `yes` | `user` | 详细的 action/tool/pattern 对象规则；项目文件可设置。 |

### `plugins`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `plugins.disabled` | `string[]` | `yes` | `user` | 仍发现但不加载的插件 ID；项目文件可设置。 |
| `plugins.enabled` | `string[]` | `yes` | `user` | 要启用的插件 ID；默认关闭的项目插件需要此项。 |
| `plugins.paths` | `string[]` | `yes` | `user` | 额外插件目录；文件夹受信任时项目文件可以设置。 |

### `privacy`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `privacy.privacy_banner_acked` | `string` | `—` | `—` | 本地隐私横幅关闭时的 RFC 3339 UTC 时间戳；pager 仅从用户 `config.toml` 读取。 |

### `relay`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `relay.enabled` | `boolean` | `yes` | `user` | 启用会话中继同步。 |

### `sandbox`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `sandbox.auto_allow_bash` | `boolean` | `pin` | `user` | 激活沙箱配置时跳过 Bash 权限提示。也对应 `GROK_SANDBOX_AUTO_ALLOW_BASH`。 |
| `sandbox.profile` | `off / workspace / read-only / strict / string` | `pin` | `user` | 文件系统沙箱配置。也对应 `--sandbox` 和 `GROK_SANDBOX`。 |

### `session`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `session.auto_compact_threshold_percent` | `integer` | `yes` | `user` | 上下文用量达到此百分比（0–100）时自动压缩。 |
| `session.load_envrc` | `boolean` | `yes` | `user` | 将 `.envrc` 变量注入 Bash。 |

### `shell_environment_policy`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `shell_environment_policy.exclude` | `string[]` | `yes` | `user` | 从 Bash 环境移除的变量名；允许由覆盖层提供。 |
| `shell_environment_policy.ignore_default_excludes` | `boolean` | `yes` | `user` | 跳过内置环境变量拒绝列表；允许由覆盖层提供。 |
| `shell_environment_policy.include_only` | `string[]` | `yes` | `user` | 设置后，Bash 仅继承这些环境变量；允许由覆盖层提供。 |
| `shell_environment_policy.inherit` | `string` | `yes` | `user` | Bash 从父进程继承哪些环境变量名；允许由覆盖层提供，但不能注入值。 |
| `shell_environment_policy.set` | `map<string,string>` | `yes` | `user` | 向 Bash 注入环境变量值；不允许由覆盖层提供。 |

### `skills`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `skills.disabled` | `string[]` | `yes` | `user` | 仍发现但不激活的技能名称。 |
| `skills.paths` | `string[]` | `yes` | `user` | 额外技能目录。 |

### `storage`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `storage` | `table` | `yes` | `user` | 本地会话存储清理策略。 |

### `subagents`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `subagents.enabled` | `boolean` | `pin` | `user` | 子智能体／任务工具总开关。也对应 `GROK_SUBAGENTS`。 |
| `subagents.limit_behavior` | `queue / fail` | `yes` | `user` | 达到并发子智能体上限时的处理方式。 |
| `subagents.max_concurrent` | `integer` | `yes` | `user` | 最大并发子智能体数。 |
| `subagents.max_depth` | `integer` | `yes` | `user` | 最大嵌套深度（至少为 1）。 |
| `subagents.models.<name>` | `string` | `yes` | `user` | 每种子智能体的模型 ID 覆盖。 |
| `subagents.toggle.<name>` | `boolean` | `yes` | `user` | 启用或禁用某种子智能体类型；省略时默认启用。 |

### `telemetry`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `telemetry.otel_enabled` | `boolean` | `pin` | `user` | 外部 OTEL 总开关。也对应 `GROK_EXTERNAL_OTEL`。 |
| `telemetry.otel_metrics_exporter` | `otlp / console / none` | `pin` | `user` | 外部 OTEL 指标 exporter。也对应 `OTEL_METRICS_EXPORTER`。 |
| `telemetry.otel_logs_exporter` | `otlp / console / none` | `pin` | `user` | 外部 OTEL 日志 exporter。也对应 `OTEL_LOGS_EXPORTER`。 |
| `telemetry.otel_endpoint` | `string` | `pin` | `user` | 外部 OTLP 基础端点。也对应 `OTEL_EXPORTER_OTLP_ENDPOINT`。固定后会移除开发者环境变量及未列出的用户/托管文件同级项，显式列出的端点除外。 |
| `telemetry.otel_logs_endpoint` | `string` | `pin` | `user` | 日志信号 OTLP 端点（按原样使用）。也对应 `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT`。 |
| `telemetry.otel_metrics_endpoint` | `string` | `pin` | `user` | 指标信号 OTLP 端点（按原样使用）。也对应 `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT`。 |
| `telemetry.otel_protocol` | `http/protobuf / grpc` | `pin` | `user` | 外部 OTLP 传输协议。也对应 `OTEL_EXPORTER_OTLP_PROTOCOL`。固定后会移除信号专用协议环境变量及未列出的文件同级项。 |
| `telemetry.otel_logs_protocol` | `http/protobuf / grpc` | `pin` | `user` | 日志信号 OTLP 协议。也对应 `OTEL_EXPORTER_OTLP_LOGS_PROTOCOL`。 |
| `telemetry.otel_metrics_protocol` | `http/protobuf / grpc` | `pin` | `user` | 指标信号 OTLP 协议。也对应 `OTEL_EXPORTER_OTLP_METRICS_PROTOCOL`。 |
| `telemetry.otel_timeout` | `number` | `pin` | `user` | 导出超时毫秒数。也对应 `OTEL_EXPORTER_OTLP_TIMEOUT`。 |
| `telemetry.otel_metric_export_interval` | `number` | `pin` | `user` | 指标导出间隔毫秒数。也对应 `OTEL_METRIC_EXPORT_INTERVAL`。 |
| `telemetry.otel_certificate` | `string` | `pin` | `user` | collector 额外 CA 证书的 PEM 路径。也对应 `OTEL_EXPORTER_OTLP_CERTIFICATE`；固定 CA **不会**移除端点。 |
| `telemetry.otel_logs_certificate` | `string` | `pin` | `user` | 日志信号 CA PEM 路径。也对应 `OTEL_EXPORTER_OTLP_LOGS_CERTIFICATE`。 |
| `telemetry.otel_metrics_certificate` | `string` | `pin` | `user` | 指标信号 CA PEM 路径。也对应 `OTEL_EXPORTER_OTLP_METRICS_CERTIFICATE`。 |
| `telemetry.otel_client_certificate` | `string` | `pin` | `user` | mTLS 客户端证书的 PEM 路径。也对应 `OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE`；固定后会移除凭据副本、开发者端点和未列出的文件同级项。 |
| `telemetry.otel_client_key` | `string` | `pin` | `user` | mTLS 客户端密钥的 PEM 路径；token 不会写入此文件。也对应 `OTEL_EXPORTER_OTLP_CLIENT_KEY`。 |
| `telemetry.otel_logs_client_certificate` | `string` | `pin` | `user` | 日志信号 mTLS 客户端证书 PEM 路径。也对应 `OTEL_EXPORTER_OTLP_LOGS_CLIENT_CERTIFICATE`。 |
| `telemetry.otel_logs_client_key` | `string` | `pin` | `user` | 日志信号 mTLS 客户端密钥 PEM 路径。也对应 `OTEL_EXPORTER_OTLP_LOGS_CLIENT_KEY`。 |
| `telemetry.otel_metrics_client_certificate` | `string` | `pin` | `user` | 指标信号 mTLS 客户端证书 PEM 路径。也对应 `OTEL_EXPORTER_OTLP_METRICS_CLIENT_CERTIFICATE`。 |
| `telemetry.otel_metrics_client_key` | `string` | `pin` | `user` | 指标信号 mTLS 客户端密钥 PEM 路径。也对应 `OTEL_EXPORTER_OTLP_METRICS_CLIENT_KEY`。 |
| `telemetry.otel_metrics_include_session_id` | `boolean` | `pin` | `user` | 为指标附加 session.id。也对应 `OTEL_METRICS_INCLUDE_SESSION_ID`。 |
| `telemetry.otel_log_user_prompts` | `boolean` | `pin` | `user` | grok_code.user_prompt 提示文本的内容开关。也对应 `OTEL_LOG_USER_PROMPTS`。固定任一内容开关而未列出同级项时，遗漏项默认关闭。 |
| `telemetry.otel_log_tool_details` | `boolean` | `pin` | `user` | 工具参数预览、路径和原样名称的元数据开关；SIEM 关联建议开启，不包含完整正文。也对应 `OTEL_LOG_TOOL_DETAILS`。 |
| `telemetry.otel_log_assistant_responses` | `boolean` | `pin` | `user` | grok_code.assistant_response 文本的内容开关。未设置时跟随 otel_log_user_prompts，但 requirements 固定任一同级开关时除外。仅用环境变量并设置 OTEL_LOG_USER_PROMPTS=1 的机群若要只采集提示，必须将本项设为 0（或固定为 false）。也对应 `OTEL_LOG_ASSISTANT_RESPONSES`。 |
| `telemetry.otel_log_tool_content` | `boolean` | `pin` | `user` | tool_input、tool_output、full_command 和 error_message 的正文开关；与 details 相互独立，默认关闭。只开 CONTENT 不会保留原样 MCP 名称和路径。也对应 `OTEL_LOG_TOOL_CONTENT`。 |
| `telemetry.trace_upload` | `boolean` | `pin` | `user` | 上传会话跟踪数据；requirements 固定值优先于用户配置。 |

### `tools`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `tools.disable_zdr_incompatible_tools` | `boolean` | `yes` | `user` | 在 ZDR 下限制需要 xAI 托管输出的工具。也对应 `GROK_DISABLE_ZDR_INCOMPATIBLE_TOOLS`。 |
| `tools.media_gen.max_parallel_image_gen_calls` | `integer` | `yes` | `user` | 单个模型步骤中并行 `image_gen`/`image_edit` 调用上限。也对应 `GROK_MAX_PARALLEL_IMAGE_GEN_CALLS`。 |
| `tools.media_gen.max_parallel_video_gen_calls` | `integer` | `yes` | `user` | 单个模型步骤中并行 `video_gen` 调用上限。也对应 `GROK_MAX_PARALLEL_VIDEO_GEN_CALLS`。 |
| `tools.respect_gitignore` | `boolean` | `pin` | `user` | 为 true 时，搜索和读取工具跳过被 Git 忽略的文件。也对应 `GROK_RESPECT_GITIGNORE`。 |
| `tools.zdr_video_output_s3` | `table` | `yes` | `user` | ZDR 视频输出所用团队 S3 bucket；详见 ZDR 视频存储。 |

### `toolset`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `toolset.ask_user_question.timeout_secs` | `number` | `yes` | `user` | `ask_user_question` 工具超时。 |
| `toolset.bash.auto_background_on_timeout` | `boolean` | `yes` | `user` | 前台超时触发时将命令转入后台。 |
| `toolset.bash.login_shell_capture` | `boolean` | `yes` | `user` | 为 Bash 捕获用户登录 Shell 环境；允许由覆盖层提供。 |
| `toolset.bash.max_timeout_secs` | `number` | `yes` | `user` | 模型请求的前台超时上限。 |
| `toolset.bash.output_byte_limit` | `number` | `yes` | `user` | Bash 输出捕获字节上限。 |
| `toolset.bash.timeout_secs` | `number` | `yes` | `user` | Bash 前台命令超时秒数。 |
| `toolset.file_toolset` | `standard / hashline` | `yes` | `user` | 文件编辑工具方案。 |
| `toolset.web_fetch.allowed_domains` | `string[]` | `yes` | `user` | `web_fetch` 域名允许列表覆盖。 |
| `toolset.web_fetch.proxy_endpoint` | `string` | `yes` | `user` | `web_fetch` 出站代理 URL。也对应 `GROK_WEB_FETCH_PROXY`。 |
| `toolset.web_search.allowed_domains` | `string[]` | `yes` | `user` | 客户端 `web_search` 域名允许列表；允许由覆盖层提供。 |
| `toolset.web_search.excluded_domains` | `string[]` | `yes` | `user` | 客户端 `web_search` 域名拒绝列表；允许由覆盖层提供。 |

### `ui`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `ui.approval_mode` | `string` | `yes` | `user` | 已弃用；请使用 `ui.permission_mode`。 |
| `ui.auto_dark_theme` | `string` | `yes` | `user` | `theme = auto` 且操作系统为深色时使用的主题。 |
| `ui.auto_light_theme` | `string` | `yes` | `user` | `theme = auto` 且操作系统为浅色时使用的主题。 |
| `ui.cancel_subagents_on_turn_cancel` | `ask / always_stop / always_continue` | `yes` | `user` | 取消父轮次时如何处理运行中的子智能体。 |
| `ui.collapsed_edit_blocks` | `boolean` | `yes` | `user` | 将编辑显示为单行 +N/-M 摘要。也对应 `GROK_COLLAPSED_EDIT_BLOCKS`。 |
| `ui.combine_queued_prompts` | `boolean` | `yes` | `user` | 将连续的普通后续提示合并为一个轮次。 |
| `ui.compact_mode` | `boolean` | `yes` | `user` | 使用更紧凑的消息留白。也对应 `/compact-mode`。 |
| `ui.confirm_before_rewind` | `boolean` | `yes` | `user` | 回退对话历史前询问确认。 |
| `ui.contextual_hints.image_input` | `boolean` | `yes` | `user` | 模型支持图像时显示剪贴板图像粘贴提示。 |
| `ui.contextual_hints.plan_mode` | `boolean` | `yes` | `user` | 针对规划型提示建议使用计划模式（Shift+Tab）。 |
| `ui.contextual_hints.send_now` | `boolean` | `yes` | `user` | 轮次中途排队后续提示后，空提示按 Enter 可立即发送。 |
| `ui.contextual_hints.small_screen` | `boolean` | `yes` | `user` | 终端高度较小时建议 `/compact-mode`。 |
| `ui.contextual_hints.ssh_wrap` | `boolean` | `yes` | `user` | SSH 缺少剪贴板接收端时建议 `grok-zh wrap`。 |
| `ui.contextual_hints.undo` | `boolean` | `yes` | `user` | 提示 Ctrl+Z 可恢复被清除的草稿。 |
| `ui.contextual_hints.word_select` | `boolean` | `yes` | `user` | 使用折叠／导航选择双击后，提示设置中的“词选择”。 |
| `ui.cursor_blink` | `boolean` | `yes` | `user` | 强制块状光标闪烁（true）或常亮（false）；未设置时继承终端。 |
| `ui.default_selected_permission` | `string` | `yes` | `user` | 会话首次权限提示中预选的行。也对应 `GROK_DEFAULT_SELECTED_PERMISSION`。 |
| `ui.display_refresh.auto_cadence_enabled` | `boolean` | `yes` | `user` | 使流式输出／滚动节奏匹配显示器刷新率。也对应 `GROK_DISPLAY_REFRESH_AUTO_CADENCE`。 |
| `ui.follow_up_behavior` | `queue / steer` | `yes` | `user` | 轮次中途后续消息的路由方式。 |
| `ui.fork_secondary_model` | `string` | `yes` | `user` | 分叉时第二个智能体使用的模型；默认使用主默认模型。 |
| `ui.group_tool_verbs` | `boolean` | `yes` | `user` | 折叠连续的读取／搜索／列出工具行。也对应 `GROK_GROUP_TOOL_VERBS`。 |
| `ui.hunk_tracker_mode` | `agent_only / all_dirty / off` | `yes` | `user` | 文件改动 hunk 跟踪。也对应 `GROK_HUNK_TRACKER` 和 `--hunk-tracker-mode`。 |
| `ui.invert_scroll` | `boolean` | `yes` | `user` | 反转垂直滚动方向。也对应 `GROK_INVERT_SCROLL`。 |
| `ui.keep_text_selection` | `flash / hold / word_select` | `yes` | `user` | 应用内选择行为：短暂闪烁、保持或双击选择词。 |
| `ui.max_thoughts_width` | `number` | `yes` | `user` | 思考面板列宽（40–500）。 |
| `ui.mouse_reporting_toggle` | `boolean` | `yes` | `user` | 回滚区中的 Ctrl+R 切换终端鼠标捕获。也对应 `GROK_MOUSE_REPORTING_TOGGLE`。 |
| `ui.page_flip_on_send` | `boolean` | `yes` | `user` | 将已发送提示吸附到视口顶部。 |
| `ui.permission_mode` | `default / ask / auto / always-approve` | `yes` | `user` | 默认工具权限行为；企业锁定通过 `requirements.toml` 设置。 |
| `ui.prompt_suggestions` | `boolean` | `yes` | `user` | 每轮结束后显示下一提示幽灵文本。也对应 `GROK_PROMPT_SUGGESTIONS`；远程紧急开关可以在整个机群禁用。 |
| `prompt_suggestions.max_output_tokens` | `number` | `yes` | `user` | 建议请求的可见输出 token 数；限制为 16–256，默认 64，另为推理保留空间。可由远程配置覆盖。 |
| `prompt_suggestions.temperature` | `number` | `yes` | `user` | 建议请求的采样 temperature（默认 0.2）。可由远程配置覆盖。 |
| `prompt_suggestions.reasoning_effort` | `none / minimal / low / medium / high` | `yes` | `user` | 建议请求的推理强度；默认值和 `none` 禁用推理，其余值使用模型支持的强度。可由远程配置覆盖。 |
| `ui.remember_tool_approvals` | `boolean` | `yes` | `user` | 显示按工具“始终允许”选项。也对应 `GROK_REMEMBER_TOOL_APPROVALS`。 |
| `ui.render_mermaid` | `auto / on / off` | `yes` | `user` | Mermaid 代码围栏显示方式：可点击打开行或原始源文本。 |
| `ui.screen_mode` | `fullscreen / minimal` | `yes` | `user` | 普通 `grok-zh` 的默认渲染模式；需要重启。 |
| `ui.scroll_lines` | `integer` | `yes` | `user` | 每次滚动的行数（1–10）。也对应 `GROK_SCROLL_LINES`。 |
| `ui.scroll_mode` | `auto / wheel / trackpad` | `yes` | `user` | 滚动输入分类。也对应 `GROK_SCROLL_MODE`。 |
| `ui.scroll_speed` | `integer` | `yes` | `user` | 鼠标／触控板滚动速度倍数（1–100）。也对应 `GROK_SCROLL_SPEED`。 |
| `ui.show_thinking_blocks` | `boolean` | `yes` | `user` | 流式输出时显示思考／推理块。也对应 `GROK_SHOW_THINKING_BLOCKS`。 |
| `ui.show_timeline` | `boolean` | `yes` | `user` | 显示每轮刻度轨道而不是滚动条。 |
| `ui.show_timestamps` | `boolean` | `yes` | `user` | 在消息旁显示时钟时间。也对应 `/timestamps`。 |
| `ui.simple_mode` | `boolean` | `yes` | `user` | true 时使用 readline 提示编辑；false 时使用实验性 Vim 提示按键。 |
| `ui.status_line.command` | `string` | `yes` | `user` | `command` 状态栏脚本；campaign 会移除此路径，但 requirements 层仍可合并。 |
| `ui.status_line.type` | `disabled / command` | `yes` | `user` | 快捷键栏上方的可选状态行，默认关闭；详见状态栏指南。 |
| `ui.theme` | `string` | `yes` | `user` | 主题名称，或用 `auto`/`system` 跟随操作系统。也对应 `/theme` 和 `GROK_THEME`。 |
| `ui.ui_theme` | `string` | `yes` | `user` | `ui.theme` 的旧版别名。 |
| `ui.vim_mode` | `boolean` | `yes` | `user` | 在回滚区（而非提示框）启用 Vim 按键。也对应 `/vim-mode`。 |
| `ui.voice_capture_mode` | `hold / toggle` | `yes` | `user` | 按住说话或按键切换语音采集。 |
| `ui.voice_keybind_enabled` | `boolean` | `yes` | `user` | 启用 Ctrl+Space / F8 语音听写；false 时 `/voice` 仍可用。 |
| `ui.voice_stt_language` | `string` | `yes` | `user` | 语音转文本语言代码或 `auto`；会话中覆盖 `[voice].language`。 |
| `ui.yolo` | `boolean` | `pin` | `user` | 始终批准工具调用；requirements 可固定为 false 并阻止 `--yolo`。 |

### `version_overrides`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `version_overrides` | `array of tables` | `yes` | `user` | 合并前应用的按 CLI 版本配置补丁；详见 `[[version_overrides]]`。 |

### `voice`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `voice.api_base` | `string` | `yes` | `user` | 语音转文本 HTTPS API 根地址；未设置时继承 `[endpoints].xai_api_base_url`。 |
| `voice.language` | `string` | `yes` | `user` | 首选 STT 语言目录代码或 `auto`。 |
| `voice.sample_rate` | `number` | `yes` | `user` | STT 采集采样率（Hz）。 |

### `workflows`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `workflows.enabled` | `boolean` | `yes` | `user` | 启用工作流。 |

### `worktree`

| 键 | 类型／取值 | Requirements | 托管 | 说明 |
| --- | --- | --- | --- | --- |
| `worktree.auto_gc` | `table` | `yes` | `user` | 自动清理工作树的策略。 |

## managed_config.toml

`managed_config.toml` 接受上表中的所有键。它设置机群默认值，因此开发者自己的 `config.toml` 会覆盖它。希望用户能够调整时使用此文件；不允许调整时使用 `requirements.toml`。

这条规则有一个例外：

| 键 | 行为 |
| --- | --- |
| `features.remote_fetch` | 托管值优先于开发者的值。 |

Grok Build 先读取 `/etc/grok/managed_config.toml`，再读取由控制台同步的 `$GROK_HOME/managed_config.toml`；后者的值会替换前者。

上表“托管”列给出每个键的结果：`fleet` 表示机群值优先，`user` 表示用户文件优先，`—` 表示此文件不读取该键。

## requirements.toml

`requirements.toml` 是管理员强制配置。读取位置依次为 `$GROK_HOME/requirements.toml`（带签名缓存）、`/etc/grok/requirements.toml`、macOS MDM `ai.x.grok`。`config.toml` 表中的“Requirements”列列出此文件接受的每个键（`pin` 或 `yes`）；省略的键不受限制。

以下键仅存在于 `requirements.toml`：

| 键 | 类型／取值 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `fail_closed` | `boolean` | `false` | 无法应用带签名 requirements 或 `version_overrides` 时拒绝启动；默认 false。 |
| `features.image_edit` | `boolean` | — | 固定 `image_edit` 可用性。仅 requirements 支持；写入用户文件无法识别，未设置时沿用远程默认值。 |
| `ui.disable_bypass_permissions_mode` | `boolean` | — | 禁止始终批准。只有 requirements 层会执行此锁；用户或托管文件中的 true 会被忽略。 |

## 设置被拒绝时会怎样

| 情况 | Grok Build 的处理 |
| --- | --- |
| 开发者设置了管理员固定的键 | 使用固定值；`grok-zh inspect` 会列出贡献该值的 requirements 文件。 |
| 开发者设置了 `managed_config.toml` 下发的键 | 使用开发者的值，`features.remote_fetch` 除外。必须强制时请改用固定值。 |
| `requirements.toml` 缺失或签名验证失败 | 不应用固定值，Grok Build 仍会启动；设置 `fail_closed = true` 可改为拒绝启动。 |
| 固定键使用了当前版本不认识的值 | 忽略该键，文件其余内容继续生效。 |

## 检查实际生效值

在开发者机器上运行 `grok-zh inspect`。它会列出所有参与合并的配置文件，包括 requirements 和托管层，因此一条未生效的策略可以通过一个命令定位。
