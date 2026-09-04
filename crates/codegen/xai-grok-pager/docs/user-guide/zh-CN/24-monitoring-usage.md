<a id="related-settings"></a>
# 监控使用情况（外部 OpenTelemetry）

> **状态：alpha。**下面的架构已经版本化（grok_code.schema.version = v1）；可以在不另行通知的情况下进行增加字段的变更，重命名/删除会提升版本号，并在变更日志中说明。

Grok CLI 可以将使用情况的**指标**和**事件**导出到你组织自己的 OpenTelemetry collector，使平台团队能够监控整个机群的采用情况、token 消耗、工具权限决策和错误——不会有任何数据经过 SpaceXAI。

## 相关设置

以下开关彼此独立（也独立于本指南的外部 OTEL 流）：

| 设置 | 设置方式 |
|---------|---------------|
| Telemetry 总开关 | [features] telemetry / GROK_TELEMETRY_ENABLED |
| 编码数据、保留和训练 | 设置 — /privacy 打开该行 |
| Trace 上传 | [telemetry] trace_upload / GROK_TELEMETRY_TRACE_UPLOAD |
| 外部 OpenTelemetry | GROK_EXTERNAL_OTEL / [telemetry] otel_*（本指南） |

另见[身份验证](02-authentication.md#related-settings)和[配置](05-configuration.md#telemetry)。

## 外部 OTEL 流

外部流具有以下特性：

- **默认关闭**，并且需要*双重选择加入*（主开关**和**明确的 exporter 选择）。
- **默认不含内容**：不包含提示、助手正文、代码、文件路径（仅扩展名）、工具参数、bash 命令；MCP/skill/plugin 名称会折叠为类别。可选的内容开关可以重新启用其中一部分。
- 与 SpaceXAI 内部 telemetry **在结构上分离**：其 exporter 只携带你配置的标头，从不携带 SpaceXAI 凭据。
- **独立于 SpaceXAI 数据保留退出设置**：即使 telemetry 已禁用，以及对 ZDR（zero-data-retention）团队也能工作。这些设置只控制 SpaceXAI 一侧的保留；外部流完全由你自己的 OTEL 配置控制。

<a id="zdr-and-this-stream"></a>
### ZDR 与此数据流

`/privacy` 和零数据保留（ZDR）**不会**禁用此数据流。ZDR 关闭的是 SpaceXAI 一侧的
保留（产品分析、会话跟踪上传和编码数据共享），不会静音 `GROK_EXTERNAL_OTEL`。

启用此数据流后：

- `user.id`、`session.id` 以及组织/团队/部署 ID 始终导出。
- OAuth 或网关身份验证提供非空地址时，日志和指标都会附加 `user.email`。它属于身份，
  不受内容开关控制；除非关闭整个数据流，否则不能单独固定关闭。
- 提示文本、助手 `response` 和工具正文只会在对应开关开启时导出。第一方产品分析永远
  不会收到这些正文。

要让 ZDR 机器完全不连接 collector，请固定 `otel_enabled = false`（或不要启用此流）。
只采集指标的 SIEM 应把四个 `otel_log_*` 键全部固定为 `false`。

## 快速开始

```bash
export GROK_EXTERNAL_OTEL=1                  # master switch
export OTEL_METRICS_EXPORTER=otlp
export OTEL_LOGS_EXPORTER=otlp
export OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf  # or grpc
export OTEL_EXPORTER_OTLP_ENDPOINT=https://collector.corp.example:4318
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer <collector-token>"
grok-zh
```

单独设置 GROK_EXTERNAL_OTEL=1 **不会启用任何内容**——还必须至少选择一个 exporter。反过来，仅有 OTEL_* 变量而没有主开关也不会启用任何内容。

## 环境变量

| 变量 | 默认值 | 含义 |
|---|---|---|
| GROK_EXTERNAL_OTEL | 0 | 主开关。与控制 SpaceXAI 内部产品分析的 GROK_TELEMETRY_ENABLED 不同；两者控制方向相反的数据流。 |
| OTEL_METRICS_EXPORTER | none | otlp \| console \| none。 |
| OTEL_LOGS_EXPORTER | none | otlp \| console \| none。控制事件流。 |
| OTEL_EXPORTER_OTLP_PROTOCOL | http/protobuf | http/protobuf \| grpc。两个信号共用的基础协议。 |
| OTEL_EXPORTER_OTLP_LOGS_PROTOCOL / ..._METRICS_PROTOCOL | — | 各信号的协议覆盖（取值同基础协议）；无法识别的值会禁用数据流。 |
| OTEL_EXPORTER_OTLP_ENDPOINT | HTTP 为 http://localhost:4318，gRPC 为 http://localhost:4317 | 基础端点。对于 http/protobuf，按 OTLP 规范追加 /v1/logs 和 /v1/metrics；对于 grpc，按原样使用 collector 端点。路径是否追加取决于**该信号自己的**协议。 |
| OTEL_EXPORTER_OTLP_LOGS_ENDPOINT / ..._METRICS_ENDPOINT | — | 信号专用覆盖，按原样使用。对于 gRPC，通常应为不含 /v1/... 路径的 collector 端点。 |
| OTEL_EXPORTER_OTLP_HEADERS（及信号专用变体） | — | Collector 身份验证（k=v,k2=v2）。外部 exporter 发送的**唯一**标头，也是唯一支持的 collector 身份验证机制（没有配置文件 headers 键——token 从不存储在磁盘上）。 |
| OTEL_EXPORTER_OTLP_CERTIFICATE（及信号专用变体） | — | PEM bundle 路径，用于验证 collector 的额外受信任 CA 证书（适用于位于私有/企业 CA 后的 collector）。它会叠加在默认信任根（系统存储和内嵌 Mozilla 根）之上。 |
| OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE / OTEL_EXPORTER_OTLP_CLIENT_KEY（及信号专用 ..._LOGS_... / ..._METRICS_... 变体） | — | mTLS 客户端身份的 PEM **路径**。证书和密钥必须成对设置（基础级或同一信号级）；只配置一半会忽略并警告。仅支持未加密 PEM 密钥。也可通过 `[telemetry] otel_client_certificate` / `otel_client_key` 设置。 |
| OTEL_EXPORTER_OTLP_TIMEOUT | 10000（ms） | 导出超时。 |
| OTEL_METRIC_EXPORT_INTERVAL | 60000（ms） | 指标导出间隔。 |
| OTEL_BLRP_SCHEDULE_DELAY（或别名 OTEL_LOGS_EXPORT_INTERVAL） | 5000（ms） | 日志批次间隔。 |
| OTEL_EXPORTER_OTLP_METRICS_TEMPORALITY_PREFERENCE | delta | delta \| cumulative。 |
| OTEL_METRICS_INCLUDE_SESSION_ID | 1 | 为指标附加 session.id（基数退出）。 |
| OTEL_METRICS_INCLUDE_VERSION | 0 | 为指标附加 app.version。 |
| OTEL_LOG_USER_PROMPTS | 0 | 内容开关：grok_code.user_prompt 中的提示文本（上限 60 KB，并清除 secret）。 |
| OTEL_LOG_ASSISTANT_RESPONSES | 未设置时跟随 prompts | 内容开关：grok_code.assistant_response 的 `response`（上限 60 KB，并清除 secret）。未设置时跟随 OTEL_LOG_USER_PROMPTS；显式设为 0 可在保留提示的同时关闭回复。`response_length` 始终导出。仅使用环境变量且设置 OTEL_LOG_USER_PROMPTS=1 的机群若要只采集提示，还必须设置 OTEL_LOG_ASSISTANT_RESPONSES=0（或在 requirements 中固定关闭）。 |
| OTEL_LOG_TOOL_DETAILS | 0 | 元数据开关：4 KB `tool_parameters` 预览、完整文件路径、原样 MCP/skill/plugin 名称；**不**包含完整正文。企业推荐默认开启 DETAILS、关闭 CONTENT。 |
| OTEL_LOG_TOOL_CONTENT | 0 | 正文开关：`tool_input`、`tool_output`、`full_command` 和失败时的 `error_message`（清除 secret；输入/输出/命令上限 60 KB，错误消息上限 4 KB）。它与 DETAILS 相互独立，CONTENT **不会**隐含 DETAILS；默认关闭。 |

### 推荐的机群开关组合

企业推荐默认**开启 DETAILS、关闭 CONTENT**：保留路径、4 KB `tool_parameters` 预览和
原样 MCP/skill/plugin 名称等元数据，但不采集 Read、bash 或 MCP 结果正文。只有
collector 确实需要存储这些正文时才开启 CONTENT。CONTENT 不会隐含 DETAILS；只开
CONTENT 时，即使存在 60 KB `tool_output`，`tool_name` / `mcp_tool.name` 仍会折叠成
`mcp_tool`。

```toml
[telemetry]
otel_log_user_prompts = false
otel_log_assistant_responses = false   # 未设置时会跟随 prompts；此处固定关闭
otel_log_tool_details = true           # 用于 SIEM 关联的元数据
otel_log_tool_content = false          # 正文，与 details 相互独立
```

OTEL_RESOURCE_ATTRIBUTES 会被有意忽略：resource 从固定且经过审计的属性集合构建。

> **迁移说明：**较旧版本可能会与产品自己的分析管线共享 OTEL_EXPORTER_OTLP_*。此行为已弃用：设置 GROK_EXTERNAL_OTEL 时，产品分析会忽略这些变量；如果产品分析已经使用它们，CLI 会拒绝在该配置下启用外部流——你的 collector 只会收到你选择加入的外部流。

## 配置文件

组织默认值位于 config.toml 中现有的 [telemetry] 表下（环境变量优先）。键是其他 [telemetry] 设置的 otel_ 前缀同级键：

```toml
[telemetry]
otel_enabled = true
otel_metrics_exporter = "otlp"
otel_logs_exporter = "otlp"
otel_endpoint = "https://collector.corp.example:4318"
otel_protocol = "http/protobuf"  # or "grpc"
# 私有 CA 信任和 mTLS 的可选 PEM 路径（绝不是 PEM 内容）：
otel_certificate = "/etc/ssl/corp-ca.pem"
otel_client_certificate = "/etc/ssl/client.crt"
otel_client_key = "/etc/ssl/client.key"
otel_log_user_prompts = false   # admins can pin these via requirements
otel_log_assistant_responses = false
otel_log_tool_details = false   # 代码默认值；SIEM 机群通常开启，见上文推荐组合
otel_log_tool_content = false
```

配置键是 [telemetry] 下的 otel_*；为了生态互操作，**环境变量保留标准 OTEL 名称**（GROK_EXTERNAL_OTEL、OTEL_*），因此两层有意使用不同的命名空间。otel_protocol 配置键映射到 OTEL_EXPORTER_OTLP_PROTOCOL。

有意不提供 headers 键：请通过 OTEL_EXPORTER_OTLP_HEADERS 提供 collector 身份验证，
以便 token 永不存储在磁盘上。证书和密钥配置键都只能填写**路径**，切勿把私钥内容
嵌入 TOML。CA 和客户端身份路径均以环境变量优先。

签名 `requirements.toml` 中每个**实际出现**的 `[telemetry] otel_*` 键都是固定值，
环境变量无法覆盖；`managed_config.toml` 则不是锁，环境变量仍然优先。固定
`otel_endpoint` 会移除开发者的通用/信号专用端点环境变量和未列出的用户/托管文件
同级项（显式列出的端点除外）；固定客户端证书/密钥也会移除开发者端点和未列出的
文件同级项。固定 CA（`otel_certificate`）**不会**移除端点。若 requirements 列出
`otel_log_user_prompts`、`otel_log_tool_details`、`otel_log_assistant_responses` 或
`otel_log_tool_content` 中任意一项而遗漏同级开关，遗漏项默认关闭；不要跨越这一层
依赖 prompts→responses 的回退。

外部流只导出**日志和指标**，没有面向客户的 trace exporter。

机群启用应通过一份签名 `requirements.toml` 同时固定目的地、exporter 和内容开关。
`user.email` 不是可固定键，而是随 OAuth/网关身份提供。headers 在过滤后仍由启动器
通过进程环境提供，绝不会写入该 TOML。

## 启动抑制（最初几秒没有数据的原因）

由于 xAI 可以在全机群范围强制禁用该流，CLI 在启动时会保持发射关闭，直到确认开关状态——它从 /v1/settings 获取机群策略，之后才开始导出。在健康设置下，这个过程远少于一秒且不可见。

**等待有上限**，因此无法访问 xAI 的部署仍会导出：

- 如果完全没有机群策略可应用——[features] remote_fetch = false，或 [endpoints] cli_chat_proxy_base_url 指向 xAI 以外的位置——流会立即启动，由本地配置控制。
- 如果策略获取失败或一直未完成（主机被防火墙阻断、笔记本离线），尝试耗尽后仍会开始发射，并且无论如何最迟在启动后 30 秒开始。

之后到达的机群策略仍会生效；它只能*收紧*设置（禁用流或强制关闭内容开关），绝不会启用本地配置未启用的内容。

如果 collector 完全收不到数据，请检查调试日志（grok-zh --debug）中的 external otel: 行——它们记录流是否解析了配置，以及当前是在导出还是被抑制。

## Resource 属性

| 属性 | 值 |
|---|---|
| service.name | grok-cli |
| service.version、client.version | 构建/客户端版本 |
| app.entrypoint | cli \| headless \| agent |
| terminal.type | 终端模拟器品牌 |
| grok_code.schema.version | v1 |

身份属性（user.id，以及已知时的 organization.id / team.id / deployment.id）会在身份验证完成后附加到每个指标数据点和每个事件。使用 OAuth 或网关账户登录且存在非空地址时，日志和指标还会附加 `user.email`；它属于身份而非内容开关，也不会从 git、API Key 或部署密钥中获取。prompt.id（每个提示一个 UUID）只出现在事件中，从不出现在指标中。

## 指标（meter scope ai.xai.grok_code）

| 指标 | 单位 | 属性 |
|---|---|---|
| grok_code.session.count | {session} | 仅基本属性 |
| grok_code.token.usage | {token} | type = input \| output \| reasoning \| cache_read；model |
| grok_code.turn.count | {turn} | outcome = completed \| cancelled \| error；model |
| grok_code.tool.decision | {decision} | tool_name，decision = allow \| deny \| cancelled \| followup，access_kind，permission_mode |
| grok_code.tool.usage | {call} | tool_name，outcome |
| grok_code.error.count | {error} | error_category，model |
| grok_code.startup.total | ms | outcome = ok \| timeout \| error；auth_mode |
| grok_code.startup.phase_duration | ms | phase，outcome，auth_mode |
| grok_code.startup.timeout | {timeout} | stuck_in，auth_mode |

startup.total 测量从进程启动到会话可用的时间，每个进程记录一次；outcome = timeout 或 error 表示启动结束时没有可用会话。

phase_duration 按步骤拆解连接尝试（load_config、managed_policy、bootstrap、model_catalog、spawn_worker、leader_connect、acp_initialize、eager_auth）；按其 outcome（ok | timeout | cancelled | error）筛选，避免截断样本扭曲 ok 百分位。后续的 app_init 和 session_create 阶段会出现在日志时间线和摘要字符串中，而不在此指标中。超时记录的 stuck_in 是未完成的步骤。auth_mode 为 personal、team、deployment 或 unknown：启动成本因类型而异，比较前应按它拆分。

没有 cost.usage 指标：将 grok_code.token.usage 与你自己的价格表连接。lines_of_code.count 和 active_time.total 计划在后续阶段提供。

tool_name 值：内置工具名称原样传递；除非设置 OTEL_LOG_TOOL_DETAILS=1，否则 MCP 工具折叠为 mcp_tool，其他非内置工具折叠为 custom_tool。

## 事件（OTLP 日志记录）

每个事件都带有 event.sequence、session.id、turn_number（回合内）、prompt.id 以及身份属性。开关图例：**details** = 需要 OTEL_LOG_TOOL_DETAILS，**prompts** = 需要 OTEL_LOG_USER_PROMPTS，**responses** = 需要 OTEL_LOG_ASSISTANT_RESPONSES（未设置时跟随 prompts），**content** = 需要 OTEL_LOG_TOOL_CONTENT（与 details 相互独立，默认关闭）；流活动时其他内容始终导出。

| event.name | 属性 |
|---|---|
| grok_code.session_start | model、permission_mode、mcp_server_count、plugin_count、skill_count、hook_count、memory_enabled、is_git_repo、client_identifier |
| grok_code.session_end | duration_secs、turn_count、tool_call_count、compaction_count、model |
| grok_code.user_prompt | prompt_length、model、screen_mode?（fullscreen \| inline \| minimal \| headless \| other）、command_name?（斜杠命令/技能名，始终导出的元数据）；prompt（**prompts**） |
| grok_code.assistant_response | response_length；response（**responses**；纯工具回合省略） |
| grok_code.turn_completed | outcome、duration_ms、tool_call_count、model、error_category?、cancellation_category? |
| grok_code.api_request | model、duration_ms、stop_reason?、input_tokens、output_tokens、reasoning_tokens、cache_read_tokens |
| grok_code.api_error | error_category、model、status_code?、duration_ms? |
| grok_code.tool_result | tool_name、outcome、success、duration_ms、file_extension、tool_use_id；始终提供缩减后的 mcp_tool.name / mcp_server.name（**details** 开启时为原名）；tool_parameters 预览和 file_path（**details**）；tool_input、tool_output、full_command、error_message（**content**） |
| grok_code.tool_decision | tool_name、decision、access_kind、permission_mode、source、tool_use_id；始终提供缩减后的 MCP 名称（**details** 开启时为原名）；tool_parameters 预览（**details**）；tool_input、full_command（**content**） |
| grok_code.mcp_server_connection | status、transport_type、duration_ms、tool_count?、error_type?；始终提供缩减后的 mcp_server.name（**details** 开启时为原名）；error_message（**content**） |
| grok_code.permission_mode_changed | from_mode、to_mode、trigger |
| grok_code.skill_activated | skill_source、trigger = slash_command \| skill_md_read \| skill_tool；skill.name（**details**） |
| grok_code.plugin_loaded | install_kind?、success、error_category?；plugin_name（**details**） |
| grok_code.compaction | duration_ms、tokens_before、tokens_after、model? |
| grok_code.subagent | phase = launched \| completed、subagent_type?、outcome?、duration_ms? |
| grok_code.auth | auth_method |
| grok_code.internal_error | error_type（仅类别——无消息、无位置） |
| grok_code.model_switched | from_model、to_model、success、error_code? |

## 隐私模型

三种独立的故障关闭机制保护线路格式：

1. **类型化 schema：**属性键是封闭枚举；无法附加枚举之外的内容。
2. **发射时清理：**每个字符串都经过 secret 形状清理和主目录清理，并进行截断（嵌套工具参数中的每个字符串 512→128 字符；DETAILS 的 `tool_parameters` 预览和 CONTENT 的 `error_message` 上限 4 KB；`prompt`、`response`、`tool_input`、`tool_output`、`full_command` 上限 60 KB）。
3. **导出时验证器：**任何包含非 schema 键、已关闭开关键或未清理 secret 形状的记录，都会在离开进程前被丢弃；带有超出 schema 属性键的指标导出会整体丢弃。

永不导出：思考/推理文本、原始 API 请求正文、api_key.id、机器指纹和订阅层级。提示文本、助手 `response`、文件路径、工具参数预览以及 CONTENT 正文（`tool_input`、`tool_output`、`full_command`、`error_message`）只会在对应开关开启时导出；第一方产品分析永远不会收到这些正文。OAuth/网关身份存在地址时，`user.email` 会随身份导出，不受内容开关控制。

## Collector 配置示例

```yaml
receivers:
  otlp:
    protocols:
      http:
        endpoint: 0.0.0.0:4318
      grpc:
        endpoint: 0.0.0.0:4317

processors:
  batch:

exporters:
  prometheus:
    endpoint: 0.0.0.0:9464

service:
  pipelines:
    metrics:
      receivers: [otlp]
      processors: [batch]
      exporters: [prometheus]
    logs:
      receivers: [otlp]
      processors: [batch]
      exporters: []   # point at your log backend (loki, elasticsearch, …)
```

示例查询（PromQL，使用上面的 Prometheus exporter）：

```promql
# Tokens by model and type across the org, 1h rate
sum by (model, type) (rate(grok_code_token_usage_total[1h]))

# Sessions per team per day
sum by (team_id) (increase(grok_code_session_count_total[1d]))

# Tool-permission denial ratio
sum(rate(grok_code_tool_decision_total{decision="deny"}[1h]))
  / sum(rate(grok_code_tool_decision_total[1h]))
```

## 调试

设置 OTEL_LOGS_EXPORTER=console / OTEL_METRICS_EXPORTER=console 可将已清理的记录打印到 **stderr**（在 agent/headless 入口中抑制，以保持捕获的日志干净）。导出错误不会显示在 TUI 中；请检查调试日志。
