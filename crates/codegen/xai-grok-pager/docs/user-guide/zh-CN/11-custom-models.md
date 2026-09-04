# 自定义模型

Grok 可以连接自定义模型端点，以使用其他提供商、自托管模型，或覆盖内置设置。本指南说明如何选择模型、配置端点以及集成第三方提供商。

---

<a id="default-models"></a>
## 默认模型

默认情况下，Grok 使用 SpaceXAI 托管的模型，新会话从 `grok-4.5` 开始。默认模型无需配置。使用 `grok-zh login` 或 API 密钥完成身份验证，然后启动会话。

列出所有可用模型：

```bash
grok-zh models
```

---

<a id="selecting-a-model"></a>
## 选择模型

<a id="cli-flag"></a>
### CLI 标志

```bash
grok-zh -p "你好" -m grok-build
```

<a id="slash-command"></a>
### 斜杠命令

在 TUI 中，可以在会话期间切换模型：

```
/model grok-build
```

也可以使用别名：

```
/m grok-build
```

<a id="model-picker-ctrlm"></a>
### 模型选择器（Ctrl+M）

在回滚区按 `Ctrl+M` 打开模型选择器。它会列出所有可用模型，包括内置模型和自定义模型，并允许你用一次按键完成切换。当提示输入框获得焦点时，`Ctrl+M` 会改为切换多行输入；此时使用 `/model` 即可在不离开提示输入框的情况下切换。

### 机群允许列表（`requirements.toml`）

企业主机可以在签名的 `requirements.toml` 中固定**可选择的**模型集合，而不只是默认模型。该列表会**替换**用户的 `allowed_models`（不是取并集），因此 `/model`、`Ctrl+M` 和 `-m` 都不会提供列表外的模型。

```toml
[models]
default = "grok-4.5"
allowed_models = ["grok-4.5", "grok-4*"]
```

机群固定规则匹配的是**模型 ID**，而不是用户自定的目录键，因此本地 `[model.<name>]` 条目无法放宽集合。用户配置中的 `allowed_models` 仍可匹配目录键或模型 ID。省略该键会保留用户配置；空数组表示不限制。若规则存在但无法读取，则会按失败关闭处理（没有模型可选）。默认值或 `-m` 指定的模型不在固定集合中时，会在获取模型目录后被拒绝——请联系管理员；用户无法编辑该列表。

<a id="config-default"></a>
### 配置默认值

在 `~/.grok/config.toml` 中设置持久的默认模型：

```toml
[models]
default = "grok-4.5"
```

---

<a id="supported-api-backends"></a>
## 支持的 API 后端

Grok 支持三种 API 后端。在 `[model.*]` 配置中设置 `api_backend`，选择模型使用的协议：

| 值 | API | 默认 |
|-------|-----|---------|
| `"chat_completions"` | OpenAI Chat Completions（`/v1/chat/completions`） | 是 |
| `"responses"` | OpenAI Responses（`/v1/responses`） | |
| `"messages"` | Anthropic Messages（`/v1/messages`） | |

省略 `api_backend` 时，Grok 使用 `chat_completions`。

若要发送提供商专用的身份验证或版本标头——例如 Anthropic 的 `x-api-key`——请使用下面介绍的 `extra_headers` 字段。Grok 会将这些标头原样随每个请求发送到端点。

---

<a id="configuring-custom-models"></a>
## 配置自定义模型

在 `~/.grok/config.toml` 的 `[model.<name>]` 区段中添加自定义模型端点：

```toml
[model.my-model]
model = "model-id"                        # 发送给 API 的模型标识符
base_url = "https://api.example.com/v1"   # 兼容 OpenAI 的端点
name = "显示名称"                          # 显示在模型选择器中
description = "模型描述"                   # 可选描述
api_key = "sk-..."                        # 此提供商的 API 密钥（可选）
env_key = "XAI_API_KEY"                   # 保存 API 密钥的环境变量（可选；字符串或数组）
api_backend = "chat_completions"          # "chat_completions"、"responses" 或 "messages"
temperature = 0.7                          # 采样温度
top_p = 0.95                               # 核采样参数
max_completion_tokens = 8192               # 每次响应的最大 token 数
context_window = 128000                   # token 总上下文窗口
extra_headers = { "x-api-key" = "sk-..." } # 额外请求标头，原样发送（可选）
query_params = { api-version = "2026-07-22" } # 附加到每个请求 URL 的查询参数（可选）
env_http_headers = { "X-Tenant" = "TENANT_TOKEN" }    # 从环境变量读取的标头，在客户端构建时解析（可选）
```

<a id="credential-resolution"></a>
### 凭据解析

Grok 按以下顺序解析 API 密钥：

1. 模型配置中的 `api_key` 字段
2. `env_key` 指定的环境变量（可以是单个字符串或名称数组）。第一个已设置且非空的值胜出（例如，为 SSH `LC_*` 转发设置 `env_key = ["ANTHROPIC_AUTH_TOKEN", "LC_ANTHROPIC_AUTH_TOKEN"]`）
3. 已登录会话的 token（来自 `grok-zh login`），用于自身没有 `api_key`/`env_key` 的模型
4. `XAI_API_KEY` 环境变量（全局回退；为兼容旧版本，Grok 也接受 `GROK_CODE_XAI_API_KEY`）

<a id="context-window"></a>
### 上下文窗口

`context_window` 值告诉 Grok 何时触发自动压缩。覆盖已知模型时，Grok 会继承该模型的上下文窗口。定义新模型并省略 `context_window` 时，Grok 默认使用 200,000 个 token，因此应显式设置为与你的提供商匹配的值。

<a id="global-default-headers"></a>
### 全局默认标头

若要对模型目录中的*每个*模型（内置模型、从 `/v1/models` 预取的模型或自定义模型）应用相同标头，请在全局 `[models]` 区段中设置一次，而不要为每个模型重复：

```toml
[models]
extra_headers = { "X-Request-Tags" = "team=example,env=prod" }
```

这些标头会作为每个模型推理请求的基础。单模型的 `[model.<id>].extra_headers` 条目会按**键**覆盖全局默认值（匹配时不区分大小写）：模型上设置的键优先，模型只继承全局设置的键。与单模型字段一样，这些标头会随该模型的推理调用发送——不会发送到图像生成或视频生成等独立服务——因此适合用作归因标签（例如成本跟踪），无需在新增模型时重新声明。

<a id="global-default-values"></a>
### 全局默认值

一些常见的单模型设置也可以在 `[models]` 下设置一次，作为*每个*模型的默认值。单模型 `[model.<id>]` 的值始终优先；只有模型（或服务器模型列表）未设置字段时，才会使用全局值：

```toml
[models]
temperature                 = 0.7
top_p                       = 0.95
max_completion_tokens       = 8192
max_retries                 = 8
inference_idle_timeout_secs = 600
subagent_rate_limit_max_attempts = 8
stream_tool_calls           = true
```

这是固定的一小组环境级旋钮。用于标识特定模型的设置（`model`、`base_url`、`api_key`、`context_window` ……）不能用这种方式提供默认值；具有专门配置位置的设置也保留原位置——自动压缩（`[session]`）、系统提示标签（`[agent]`）以及推理力度（`[models].default_reasoning_effort`）。

> **关于 `stream_tool_calls` 的说明：** 此设置会影响请求*形状*，不只是采样。一些端点（包括部分 BYOK 提供商）要求不设置它；如果全局 `stream_tool_calls = true` 导致此类模型出现问题，可以在该模型的 `[model.<id>]` 区段中使用 `stream_tool_calls = false` 将其停用。

<a id="request-query-parameters"></a>
### 请求查询参数

一些网关会根据查询字符串进行路由或版本选择。`query_params` 会将百分比编码后的查询参数附加到 Grok 为某个模型发出的每个请求。例如，一个以这种方式选择 API 版本的网关：

```toml
[model.my-gateway]
model = "my-model"
base_url = "https://gateway.example/v1"
api_backend = "responses"
env_key = "GATEWAY_API_KEY"
query_params = { api-version = "2026-07-22" }
```

如果某个键也出现在 `base_url` 的查询字符串中，会覆盖该键（后出现的值胜出），而不是重复添加。查询参数会保存到会话中，因此不要在其中放置机密：机密应使用 `env_http_headers`。

<a id="environment-variable-headers"></a>
### 环境变量标头

`env_http_headers` 将请求标头映射到提供其值的环境变量名称，因此无需把每个请求的机密写入 `config.toml`：

```toml
[model.gateway]
model = "my-model"
base_url = "https://gateway.example/v1"
env_http_headers = { "X-Tenant-Token" = "GATEWAY_TENANT_TOKEN" }
```

Grok 会在为会话构建客户端时读取每个变量，并只将值放入请求标头，绝不会写入磁盘。变量未设置或为空时会跳过该标头；解析出的值会覆盖同名的 `extra_headers` 条目。静态值使用 `extra_headers`，来自环境的值使用 `env_http_headers`。

两个字段也可用于共享的 `[model_providers.<id>]` 区段。当模型通过 `model_provider = "<id>"` 指向某个提供商，并且自身没有设置这些字段时，会继承该提供商的 `query_params` 和 `env_http_headers`，这与 `extra_headers` 的继承方式相同。

---

<a id="overriding-built-in-models"></a>
## 覆盖内置模型

你可以覆盖内置模型的特定字段，而无需重新定义全部设置。只指定要更改的字段：

```toml
# 只覆盖默认模型的 API 密钥
[model.grok-build]
api_key = "my-api-key"

# 覆盖温度并添加自定义 API 密钥
[model.grok-build]
temperature = 0.5
api_key = "sk-custom"
```

覆盖内置模型时，Grok 会先使用默认配置（包括正确的 `base_url`），然后只应用你指定的字段。未指定的字段从默认值继承。

<a id="priority-order"></a>
### 优先级顺序

1. 你的配置（`[model.*]`）——最高优先级
2. 从远程 `/v1/models` 预取的模型
3. 硬编码默认值——最低优先级

---

<a id="provider-examples"></a>
## 提供商示例

<a id="anthropic-claude"></a>
### Anthropic（Claude）

通过 Anthropic Messages API 直接使用 Claude 模型：

```toml
[model.claude-opus]
model = "claude-opus-4-6"
base_url = "https://api.anthropic.com/v1"
name = "Claude Opus 4.6"
api_backend = "messages"
context_window = 200000
extra_headers = { "x-api-key" = "sk-ant-...", "anthropic-version" = "2023-06-01" }
```

`messages` 后端使用 Anthropic Messages 协议。Anthropic 通过 `x-api-key` 标头进行身份验证，而不是使用 `Authorization: Bearer`，因此请通过 `extra_headers` 传入密钥，Grok 会将其原样发送。

<a id="openai-chat-completions"></a>
### OpenAI（Chat Completions）

```toml
[model.gpt-4o]
model = "gpt-4o"
base_url = "https://api.openai.com/v1"
name = "GPT-4o"
env_key = "OPENAI_API_KEY"
```

`api_backend` 默认是 `"chat_completions"`，因此 OpenAI 不需要显式设置它。

<a id="openai-responses-api"></a>
### OpenAI（Responses API）

如果你的提供商支持较新的 Responses API：

```toml
[model.gpt-4o-responses]
model = "gpt-4o"
base_url = "https://api.openai.com/v1"
name = "GPT-4o (Responses)"
api_backend = "responses"
env_key = "OPENAI_API_KEY"
```

<a id="ollama-local-models"></a>
### Ollama（本地模型）

使用 [Ollama](https://ollama.ai) 在本地运行模型：

```toml
[model.ollama-codellama]
model = "codellama"
base_url = "http://localhost:11434/v1"
name = "CodeLlama (Ollama)"
```

确保 Ollama 正在运行（`ollama serve`），并且模型已经拉取（`ollama pull codellama`）。

<a id="together-ai"></a>
### Together AI

```toml
[model.together-mixtral]
model = "mistralai/Mixtral-8x7B-Instruct-v0.1"
base_url = "https://api.together.xyz/v1"
name = "Mixtral 8x7B"
env_key = "TOGETHER_API_KEY"
```

<a id="local-openai-compatible-server"></a>
### 本地 OpenAI 兼容服务器

任何实现 OpenAI Chat Completions 或 Responses API 的服务器都可以：

```toml
[model.local-llama]
model = "llama-3.1-70b"
base_url = "http://localhost:8080/v1"
name = "本地 Llama"
temperature = 0.8
```

---

<a id="custom-models-endpoint"></a>
## 自定义模型端点

将 Grok 指向自定义的、兼容 OpenAI 的 `/v1/models` 端点，而不是默认端点。当模型位于企业网关后面或使用自托管推理服务时，可以采用这种方式。

<a id="environment-variables"></a>
### 环境变量

| 变量 | 必需 | 说明 |
|----------|----------|-------------|
| `GROK_MODELS_BASE_URL` | 是 | 推理的基础 URL。Grok 从 `{base_url}/models` 获取模型列表。 |
| `XAI_API_KEY` | 是 | 作为 `Authorization: Bearer` 发送的 API 密钥。Grok 也接受 `GROK_CODE_XAI_API_KEY`。 |
| `GROK_MODELS_LIST_URL` | 否 | 当模型列表 URL 不同于 `{base_url}/models` 时，用于覆盖模型列表 URL。 |

<a id="setup"></a>
### 设置

```bash
export GROK_MODELS_BASE_URL="https://api.acme.com/v1"
export XAI_API_KEY="xai-..."
grok-zh
```

<a id="config-file-alternative"></a>
### 配置文件替代方案

```toml
[endpoints]
models_base_url = "https://api.acme.com/v1"

# 只覆盖特定模型的 API 密钥
[model.grok-build]
api_key = "my-api-key"
```

使用带有部分模型覆盖项的 `[endpoints]` 时，Grok 会从端点配置继承 `base_url`，因此无需在每个 `[model.*]` 区段中指定它。

<a id="auth-behavior"></a>
### 身份验证行为

设置 `models_base_url` 后，Grok 使用 API 密钥身份验证（`Authorization: Bearer`），而不是会话身份验证。不需要运行 `grok-zh login`——API 密钥就足够了。

---

<a id="web-search-model"></a>
## 网页搜索模型

`web_search` 工具使用独立的模型。按如下方式配置：

```toml
[models]
web_search = "grok-4.5"
```

也可以通过环境变量配置：

```bash
export GROK_WEB_SEARCH_MODEL="grok-4.5"
```

如果将网页搜索指向自定义模型，还需要一个 `[model.*]` 条目，以便 Grok 连接该模型。仅当模型设置 `supports_backend_search = true`（且构建启用了后端搜索）时，服务器端（“backend”）网页搜索才会运行；它不依赖 `api_backend`：

```toml
[models]
web_search = "my-custom-model"

[model.my-custom-model]
model = "my-custom-model"
supports_backend_search = true
```

---

<a id="using-custom-models"></a>
## 使用自定义模型

```bash
# 列出可用模型（包括自定义模型）
grok-zh models

# 通过斜杠命令在 TUI 中使用
/model my-model

# 在无头模式中使用
grok-zh -p "你好" -m my-model

# 在 config.toml 中设为默认值：
[models]
default = "my-model"
```

---

<a id="enterprise-deployment"></a>
## 企业部署

使用自定义模型进行企业部署的完整配置：

```toml
[cli]
auto_update = false

[auth]
auth_provider_command = "/usr/local/bin/my-company-auth-provider"
auth_provider_label = "Acme Corp"
auth_token_ttl = 3600

[models]
default = "company-grok"

[model.company-grok]
model = "grok-build"
base_url = "https://grok-proxy.acme.com/"
name = "Grok Build 最新版（代理）"
context_window = 128000

[features]
telemetry = false
```

---

<a id="troubleshooting"></a>
## 故障排除

<a id="model-not-found"></a>
### 找不到模型

```bash
# 列出可用模型
grok-zh models

# 检查 config.toml 中的 [model.*] 区段是否有拼写错误
```

<a id="connection-errors"></a>
### 连接错误

验证端点是否可访问：

```bash
curl -s https://api.example.com/v1/models \
  -H "Authorization: Bearer $XAI_API_KEY"
```

<a id="debug-logging"></a>
### 调试日志

```bash
RUST_LOG=debug GROK_LOG_FILE=/tmp/grok.log grok-zh
tail -f /tmp/grok.log
```

查找包含 `model` 或 `sampling` 的日志条目，以跟踪模型选择和 API 调用。
