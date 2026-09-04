# 身份验证

Grok 支持多种身份验证方式，包括交互式浏览器登录、企业单点登录（SSO）以及无头 CI/CD 运行器。

---

## 浏览器登录（默认）

首次启动时，Grok 会打开浏览器，引导你通过 grok.com 完成身份验证：

```bash
grok-zh
```

Grok 会将凭据保存在 `~/.grok/auth.json` 中，并在各个会话间复用。Grok 会在后台自动刷新访问令牌。令牌无法刷新时，Grok 会提示你重新登录。没有服务器提供的过期时间的凭据，其有效期默认为 30 天。

### 凭据存储

`~/.grok/auth.json` 中的令牌（以及 `~/.grok/mcp_credentials.json` 中的 MCP OAuth 令牌）会以仅所有者可读写的权限写入（Unix 上为 `0600`）。任何能访问这些路径的用户都可以使用其中的凭据，因此：

- 优先使用全盘加密（FileVault、BitLocker、LUKS 或等效方案）。
- 不要将 `auth.json` 或 `mcp_credentials.json` 复制到共享目录、工单或聊天中。
- 在多用户主机上，请将 `$HOME` / `$GROK_HOME` 保持为你的账户私有。

### 重新进行身份验证

要切换账户或解决身份验证问题，请运行：

```bash
grok-zh login
```

运行 `grok-zh login` 会重新启动登录流程，并替换缓存的会话。默认情况下，它会打开浏览器，通过位于 `auth.x.ai` 的 SpaceXAI OAuth 登录。传入标志可选择其他流程：

| 标志 | 说明 |
|------|-------------|
| `--oauth` | 通过位于 `auth.x.ai` 的 SpaceXAI OAuth 登录。这是默认方式，因此该标志可省略。 |
| `--device-auth`（别名 `--device-code`） | 在无头或远程环境中使用设备代码流程登录。 |

要退出登录，请运行 `grok-zh logout`。该命令不接受标志，并会清除缓存的凭据。

---

## API 密钥

对于 CI/CD、自动化或无法访问浏览器的环境，请使用来自 [console.x.ai](https://console.x.ai) 的 API 密钥：

```bash
export XAI_API_KEY="xai-..."
grok-zh
```

没有活动会话令牌时，Grok 会将 API 密钥作为回退。如果你已经以交互方式登录，保存的会话令牌优先。要回退到 API 密钥，请运行 `grok-zh logout` 或删除 `~/.grok/auth.json`。

---

## OIDC（客户 SSO）

通过你自己的身份提供方（IdP）（例如 Okta、Azure AD 或 Auth0）对开发者进行身份验证，而不是使用 grok.com。

### 1. 在 IdP 中注册公共客户端

- 授权类型：Authorization Code with PKCE（Proof Key for Code Exchange）
- 重定向 URI：`http://127.0.0.1/callback` —— 环回地址。Grok 会在登录时绑定随机端口，大多数 IdP 按照 [RFC 8252](https://tools.ietf.org/html/rfc8252) 将环回重定向视为与端口无关。
- 不需要客户端密钥。PKCE 会取代它。

### 2. 配置 CLI

通过配置文件：

```toml
# ~/.grok/config.toml
[grok_com_config.oidc]
issuer = "https://acme.okta.com"
client_id = "0oa1b2c3d4e5f6g7h8i9"
```

或通过环境变量：

```bash
export GROK_OIDC_ISSUER="https://acme.okta.com"
export GROK_OIDC_CLIENT_ID="0oa1b2c3d4e5f6g7h8i9"
```

你还可以覆盖 API 端点，使其指向自己的代理：

```bash
export GROK_CLI_CHAT_PROXY_BASE_URL="https://grok-proxy.acme.com/v1"
```

### 3. 运行 `grok-zh`

CLI 会通过 `{issuer}/.well-known/openid-configuration` 发现端点，打开 IdP 登录页面，并将令牌保存在 `~/.grok/auth.json` 中。它会通过保存的 `refresh_token` 静默自动刷新令牌。

### 可选字段

| 字段 | 默认值 | 说明 |
|-------|---------|-------|
| `scopes` | `["openid", "profile", "email", "offline_access", "api:access"]` | `offline_access` 启用静默令牌刷新 |
| `audience` | None | 某些 IdP（例如 Auth0）需要此字段 |

---

<a id="external-auth-provider"></a>
## 外部身份提供方

无法进行基于浏览器的登录时（例如在沙箱虚拟机、CI 运行器或隔离网络中），可以将身份验证委托给外部二进制文件或脚本。

### 工作原理

```
+--------------+     sh -c     +------------------------+
|     Grok     |-------------->|  your auth binary      |
|              |               |                        |
|  reads       |<-- stdout ----|  prints token          |
|  auth.json   |               |                        |
|              |   (stderr)    |  prints status/URLs    |--> surfaced to user
+--------------+               +------------------------+
```

1. Grok 通过 `sh -c "<command>"` 运行你的命令。
2. 你的二进制文件运行所需的身份验证流程（SSO、设备代码、证书交换）。
3. **stderr** 携带面向人的输出，例如登录 URL 和状态消息。Grok 会读取 stderr 并将其展示给用户；在 TUI 中，它会将第一个 `https://` URL 转换为可点击的登录链接。
4. **stdout** 会被 Grok 捕获并保存为访问令牌。
5. 退出码 0 = 成功；非零退出码 = Grok 回退到交互式登录。

### stdout / stderr 契约

| 流 | 要打印的内容 | 谁能看到 |
|--------|---------------|-------------|
| **stdout** | 令牌——不要打印其他内容 | Grok（解析后保存到 auth.json） |
| **stderr** | 登录 URL、状态消息、错误 | 用户（Grok 读取 stderr，并在 TUI 中将登录 URL 显示为可点击链接） |

**除令牌外，不要向 stdout 打印任何内容。** 不要打印进度消息或调试输出。Grok 会读取 stdout，去掉两端空白，然后将结果解析为令牌。

### stdout 令牌格式

**裸字符串** —— 仅原始令牌：

```
eyJhbGciOiJSUzI1NiIs...
```

**JSON** —— 可选包含刷新令牌、过期时间和签发方：

```json
{"access_token": "eyJhbGciOi...", "refresh_token": "ref-tok", "expires_in": 3600, "issuer": "https://idp.example.com"}
```

如果令牌会过期，并且你希望 Grok 在过期前自动重新运行二进制文件，请使用 JSON。

JSON 字段：

| 字段 | 必需 | 含义 |
|-------|----------|---------|
| `access_token` | yes | Grok 发送给 xAI API 的 Bearer 令牌 |
| `refresh_token` | no | 仅作参考保存。Grok 会重新运行你的二进制文件进行刷新，而不是使用 OAuth 刷新授权 |
| `expires_in` | no | 令牌有效期（秒）；启用在过期前主动刷新 |
| `issuer` | no | 标识令牌签发方 |

### 配置

通过配置文件：

```toml
# ~/.grok/config.toml
[auth]
auth_provider_command = "/usr/local/bin/my-auth-provider"
auth_provider_label = "Acme Corp"   # 可选 —— 自定义 TUI 登录按钮
auth_token_ttl = 3600               # 可选 —— 令牌有效期（秒）
```

或通过环境变量：

```bash
export GROK_AUTH_PROVIDER_COMMAND="/usr/local/bin/my-auth-provider"
export GROK_AUTH_PROVIDER_LABEL="Acme Corp"
export GROK_AUTH_TOKEN_TTL=3600
```

### 令牌刷新

Grok 会按两种不同的契约运行你的二进制文件，`GROK_AUTH_EXPIRED` 用于区分它们。每次运行都会完全替换保存的凭据，因此每次调用（包括刷新）都要输出相同的 JSON 字段（例如 `issuer`）。

- **`GROK_AUTH_EXPIRED=1` —— 无头刷新。** Grok 正在基于已有凭据重新签发令牌：可能是令牌即将过期轮换，也可能是服务器拒绝了令牌。此时没有人观察。stdin 已关闭，stderr 会被吞掉，二进制文件在几秒后就会被终止。请静默签发令牌或以非零状态退出——绝不要阻塞。
- **未设置 —— 登录。** `grok-zh login`、登录界面，或 Grok 在无头运行无法签发令牌时执行的升级流程。用户正在等待，stderr 会传给用户，并且你有 300 秒——足以完成一次浏览器往返或设备代码流程。

```bash
#!/bin/sh
if [ "$GROK_AUTH_EXPIRED" = "1" ]; then
    # 无头模式：仅静默刷新。当 SSO 会话已过期且只有用户能续期时，
    # 拒绝刷新是快速且正确的选择。
    echo "Refreshing token..." >&2
    TOKEN=$(my-company-auth --refresh --silent) || exit 1
else
    echo "Authenticating via Acme Corp SSO..." >&2
    TOKEN=$(my-company-auth --login --interactive)
fi

if [ -z "$TOKEN" ]; then
    echo "Authentication failed" >&2
    exit 1
fi

echo "{\"access_token\": \"$TOKEN\", \"expires_in\": 3600}"
```

无头运行无法生成令牌时，Grok 会不再将保存的凭据视为可用，并改为启动登录流程——与从未登录过的机器上看到的流程相同，同时显示你的二进制文件的 stderr，使设备代码 URL 或浏览器提示能够到达你。`GROK_AUTH_EXPIRED=1` 时及时退出可以让交接快速完成；如果二进制文件阻塞，每次启动都要等待刷新超时。会话进行中，本轮会失败并显示重新身份验证提示，而 `/login` 会以交互方式重新运行该二进制文件。

有一种情况仍然含糊，而且仅发生在**主导模式**（`--leader` 或 `[cli] use_leader = true`；默认关闭）中：完全没有凭据时，主导模式会在启动后不久在后台额外尝试一次，此次运行的变量未设置，与登录相同。不需要帮助即可签发令牌的二进制文件（服务账户、keytab、挂载的令牌）会在此处成功，且会话自行恢复。必须提示用户的二进制文件则可能一直等待，最多达到 300 秒的登录上限——没有任何东西在等待它，登录界面已经显示，而此次运行的 stderr 会写入 `~/.grok/leader.log`，不会显示给你。

### 环境变量

| 变量 | 说明 |
|----------|-------------|
| `GROK_AUTH_PROVIDER_COMMAND` | 身份验证二进制文件的路径 |
| `GROK_AUTH_PROVIDER_LABEL` | TUI 登录界面上的显示名称（例如 "Acme Corp"） |
| `GROK_AUTH_TOKEN_TTL` | 令牌有效期（秒；用于没有 `expires_in` 的裸字符串令牌） |
| `GROK_AUTH_EXPIRED` | 无头刷新时设为 `1`：不要提示用户，也不要返回缓存令牌。登录时未设置，此时有用户在场 |
| `GROK_AUTH_EARLY_INVALIDATION_SECS` | 在过期前主动刷新的秒数（默认：300） |

---

<a id="device-code-flow"></a>
## 设备代码流程

对于本地没有浏览器可用的无头环境（SSH 会话、Docker 容器、远程虚拟机）：

```bash
grok-zh login --device-auth    # 或：grok-zh login --device-code
```

该命令会在终端打印 URL 和代码。在任意设备上打开 URL，输入代码并完成身份验证。Grok 会持续轮询，直到登录得到确认。

你也可以通过[外部身份提供方](#external-auth-provider)实现设备代码流程，以获得完全控制。

---

## 自动刷新凭据

Grok 会自动刷新过期的凭据：

- **过期前：** 如果身份提供方返回了 `expires_in`（JSON 输出），或你设置了 `auth_token_ttl`，Grok 会在过期前约 5 分钟重新运行身份验证二进制文件。
- **身份验证错误时：** 如果服务器返回 401 Unauthorized，Grok 会刷新凭据并重试请求。
- **OIDC：** 如果有 `refresh_token`，Grok 会通过 IdP 静默刷新，而不会重新打开浏览器。

调整刷新缓冲区：

```bash
# 在过期前 5 分钟刷新（默认）
export GROK_AUTH_EARLY_INVALIDATION_SECS=300

# 禁用主动刷新缓冲区：在过期时或收到 401 时刷新（设为 0）
export GROK_AUTH_EARLY_INVALIDATION_SECS=0
```

---

## 热重载

Grok 会自动获取 `~/.grok/auth.json` 的变更。如果你在外部更新凭据（例如使用脚本写入新令牌），Grok 会在下一次 API 调用时使用新凭据，无需重启。

---

## 身份验证优先级

Grok 按以下顺序（从高到低）为每个请求解析凭据：

1. **每个模型的 `api_key` 或 `env_key`** —— 在 `config.toml` 的 `[model.<name>]` 下设置。只要存在就优先使用。
2. **活动会话令牌** —— 通过浏览器、OIDC/OAuth2 或外部身份提供方登录获得，并保存于 `~/.grok/auth.json`。
3. **`XAI_API_KEY`** —— 没有活动会话令牌时的回退。

配置了多个登录流程时，Grok 会按以下顺序（从高到低）从第一个可用来源填充会话令牌：

1. **外部身份提供方**（`auth_provider_command`）
2. **企业 OIDC** —— 配置 OIDC 后，通过 `config.toml` 中的 `[grok_com_config.oidc]`，或通过 `GROK_OIDC_ISSUER` 和 `GROK_OIDC_CLIENT_ID` 环境变量
3. **SpaceXAI OAuth2 浏览器登录** —— 默认方式

会话期间，活动方法负责处理所有会话内刷新。

---

<a id="related-settings"></a>
## 相关设置

编码数据共享——设置中的**编码数据、保留期限和训练**（由 `/privacy` 打开）——不会更改以下配置项：

| 设置 | 设置方式 |
|---------|---------------|
| `[features] telemetry` | `config.toml` 或 `GROK_TELEMETRY_ENABLED` |
| `[telemetry] trace_upload` | `config.toml` 或 `GROK_TELEMETRY_TRACE_UPLOAD` |
| 外部 OpenTelemetry | `GROK_EXTERNAL_OTEL` / `[telemetry] otel_*`。参见[监控用量](24-monitoring-usage.md)。 |

在团队账户中，只有团队管理员可以更改编码数据共享。团队管理员还可以为团队启用或禁用零数据保留（ZDR）。参见[如何启用 ZDR](https://docs.x.ai/developers/faq/security#how-to-enable-zdr)。
启用 ZDR 后，编码数据共享完全无法更改——设置行会显示 `ZDR`，而不是具体值。ZDR 不会关闭外部 OTEL 或 `user.email`；详见[此数据流与 ZDR](24-monitoring-usage.md#zdr-and-this-stream)。

另请参见[监控用量](24-monitoring-usage.md#related-settings)和[配置](05-configuration.md#telemetry)。

---

## 故障排除

### 调试日志

设置 `RUST_LOG` 可控制文件日志和无头 stderr 输出的详细程度。（TUI 屏幕上的跟踪面板使用固定过滤器，会忽略 `RUST_LOG`。）在 TUI 中，文件日志默认为 `DEBUG`；在无头模式（`-p`）中，`RUST_LOG` 默认为 `off`，因此只打印答案——设置 `RUST_LOG=error`（或更宽泛的值）即可在 stderr 中查看日志。

在 TUI 中，将 `GROK_LOG_FILE` 设置为绝对路径即可把日志写入该文件：

```bash
GROK_LOG_FILE=/tmp/grok.log RUST_LOG=debug grok-zh
tail -f /tmp/grok.log
```

`GROK_LOG_FILE` 会被视为字面文件路径。相对值（例如 `1`）会在当前目录中写入名为 `1` 的文件。

在无头模式中，日志会写入 stderr。将其重定向到文件：

```bash
RUST_LOG=debug grok-zh -p "hello" 2> /tmp/grok.log
```

### 常见日志消息

| 日志消息 | 含义 |
|-------------|---------------|
| `auth: running external auth provider (headless refresh)` / `(interactive login)` | Grok 正在运行你的二进制文件，以及当前使用哪种契约 |
| `auth: external auth provider returned fresh token` | Grok 已解析并保存令牌 |
| `auth: external auth provider failed` | 二进制文件以非零状态退出，或 stdout 为空 |
| `auth: external auth provider timed out (likely needs interactive auth), killing` | 二进制文件未能在超时前退出，已被终止 |
| `auth: failed to start external auth provider` | 无法生成命令（找不到二进制文件） |

### 常见修复

- **“Authentication failed”** —— 运行 `grok-zh logout` 清除缓存的凭据，然后运行 `grok-zh login` 重新登录。
- **令牌过快过期** —— 设置 `auth_token_ttl`，或在身份提供方的 JSON 输出中返回 `expires_in`。
- **OIDC 重定向失败** —— 确保你的 IdP 允许环回重定向 URI（`http://127.0.0.1/callback`）。
- **找不到外部身份提供方** —— 检查 `auth_provider_command` 路径正确且二进制文件可执行。
