# 06 · Auth / Login

> 子系统：十余个 AI provider 的凭证生命周期（发现、导入、OAuth PKCE 登录、API Key 存储、多账号切换、token 自动刷新、运行时探活验证、诊断），StartupGuide 首配引导。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

Auth 子系统统一管理十余个 AI provider 的凭证生命周期（发现、导入、OAuth PKCE 登录、API Key 存储、多账号切换、token 自动刷新、运行时探活验证、诊断），并在 TUI 启动时通过 StartupGuide 引导用户完成首配。

## 两层 Login 架构

**第一层：SAITEC 平台认证（jcode）**
- 经 `saitec::auth` 模块登录，获取 session token 用于 MCP 工具权限。
- 流程是「打开浏览器 → 用户手动粘贴 callback URL/query」。
- 凭证存 `~/.saitec_tui/auth.json`；由 `subscription_catalog::has_credentials()` 判断是否已登录。

**第二层：Base Model OAuth / API Key 认证**
- 覆盖 Anthropic(Claude)、OpenAI、Google(Gemini/Antigravity/Google)、GitHub Copilot、Cursor、Azure、Bedrock、OpenRouter 及任意 OpenAI-compatible provider。
- OAuth provider（Claude/OpenAI/Gemini/Antigravity/Google）用标准 PKCE：本地起 TcpListener 等 callback（`oauth.rs` 中 `wait_for_callback_async`），120s/300s 超时后降级为手动粘贴 code/URL。
- 非 OAuth（Cursor/Bedrock/OpenRouter/OpenAI-compatible）经终端交互收集 API Key 存 `.env`。
- Copilot 用 GitHub Device Flow（设备码 + 轮询）。

两层的 `AuthStatus` 最终汇聚在 `AuthStatus` 结构体，TUI 的 `/account`、`/login`、StartupGuide 等均消费此状态。

## PendingLogin 各 Variant

定义在 `src/tui/app/auth_types.rs`：

| Variant | 含义 |
|---|---|
| `StartupGuide { focused, is_reminder }` | 首次启动或缺凭证时的欢迎引导页。`is_reminder=false` 为 Setup mode（必须配 base model）；`is_reminder=true` 为 Reminder mode（base model 已就绪但 SAITEC 未登录）。`focused` 指当前焦点按钮。 |
| `SaitecForm { form }` | 等待用户填 SAITEC 业务登录表单（邮箱/电话 + 密码）。 |
| `ClaudeAccount { verifier, label, redirect_uri }` | 等待粘贴 Claude OAuth 回调 code，绑定编号账号。 |
| `OpenAiAccount { verifier, label, expected_state, redirect_uri }` | 等待粘贴 OpenAI OAuth 回调 URL/query。 |
| `Antigravity { verifier, expected_state, redirect_uri }` | 等待粘贴 Google Antigravity OAuth 回调。 |
| `Gemini { verifier, expected_state, redirect_uri }` | 等待粘贴 Gemini OAuth 回调 code/URL。 |
| `ApiKeyProfile { provider_id, provider, auth_method, docs_url, env_file, key_name, default_model, endpoint, api_key_optional, openai_compatible_profile }` | 等待粘贴某 OpenAI-compatible provider 的 API Key（含自定义 endpoint 和可选 key）。 |
| `OpenAiCompatibleApiBase { profile }` | 等待输入自定义 OpenAI-compatible API base URL（如 LM Studio/Ollama 地址）。 |
| `OpenAiCompatibleModelName { provider, provider_id, env_file, profile }` | API Key 保存后，等待输入可选 default model name。 |
| `CursorApiKey` | 等待粘贴 Cursor API Key。 |
| `Copilot` | GitHub Copilot Device Flow 进行中（后台轮询）。 |
| `AutoImportSelection { candidates }` | 等待选择要导入的外部 auth 源（Claude Code / OpenCode / pi 等）。 |

**Note**：`PendingLogin` 无 Bedrock/OpenRouter/Azure 独立 variant——这些 provider 登录在 CLI（`src/cli/login.rs`）经 `login_bedrock_flow`/`login_openrouter_flow`/`login_azure_flow` 完成，TUI 中统一走 `ApiKeyProfile`。

## AuthStatus 缓存机制

定义在 `src/auth/mod.rs`：
- **主缓存 `AUTH_STATUS_CACHE`**：TTL = **30s**，`AuthStatus::check()` 访问。命中返回 clone，过期调 `check_uncached()` 全量探测（扫描磁盘文件、检查 env var、读 token 过期等）。结果同时写两缓存。
- **快速缓存 `AUTH_STATUS_FAST_CACHE`**：TTL = **5s**，`AuthStatus::check_fast()` 访问。优先复用主缓存（30s 内），否则走 `check_uncached_fast()`——逻辑与全量基本相同但跳过开销大的子进程调用（`cursor-agent status`、`sqlite3`），仅本地文件/env 探测。
- **缓存失效**：`AuthStatus::invalidate_cache()` 同时清两缓存 + Copilot GitHub token 缓存；在每次 login 完成、post-login validation 完成、external auth trust 切换时调用。
- 两者均用 `LazyLock<RwLock<Option<(AuthStatus, Instant)>>>`。

## 关键文件清单

| 路径 | 职责 |
|---|---|
| `src/auth/mod.rs` | 入口，`AuthStatus::check/check_fast/check_uncached` 全量探测、缓存管理、凭证来源汇总 |
| `src/auth/status_types.rs` | `AuthStatus`/`ProviderAuth`/`ProviderAuthAssessment` 定义，re-export jcode-auth-types 枚举 |
| `src/auth/login_flows.rs` | CLI 端所有 provider 交互式登录流程 |
| `src/auth/oauth.rs` | OAuth PKCE 核心：token exchange、refresh、本地 TcpListener callback server、Claude & OpenAI 专用 authorize_url/token_url |
| `src/auth/claude.rs` | Anthropic 多账号管理：`JcodeAuthFile` 格式、legacy 迁移、`load_credentials` 三级 fallback（Claude Code → jcode → OpenCode） |
| `src/auth/codex.rs` | OpenAI 多账号管理：`JcodeOpenAiAuthFile` 格式、legacy 迁移、active account 覆盖 |
| `src/auth/account_store.rs` | 多账号通用工具：canonical label 生成、upsert/relabel/switch/remove |
| `src/auth/copilot.rs` | GitHub Copilot Device Flow、GitHub token 加载链（env→config.json→hosts.json→apps.json→external→gh CLI）、Copilot API token exchange |
| `src/auth/cursor.rs` | Cursor API key / native auth / VSCodeDB 检测 |
| `src/auth/antigravity.rs` / `gemini.rs` / `google.rs` / `azure.rs` | 各 OAuth / 配置实现 |
| `src/auth/external.rs` | 外部 auth 源导入框架（OpenCode auth.json / pi auth.json），consent 管理 |
| `src/auth/validation.rs` | `auth-validation.json` 持久化读写（provider 级运行时探活记录） |
| `src/auth/refresh_state.rs` | `auth-refresh-state.json` 持久化（token refresh 成功/失败记录） |
| `src/auth/doctor.rs` | Auth 诊断：needs_attention 判定、7 天 stale 检测、诊断信息生成 |
| `src/auth/commands.rs` | `command_exists` 与 PATH 扫描（WSL2 优化、PATHEXT、进程级缓存） |
| `src/auth/login_diagnostics.rs` | 登录失败原因分类（browser/callback/port/timeout/rate-limit） |
| `src/cli/login.rs` | CLI 入口 `jcode login`：`run_login`/`run_login_provider`，scriptable flow，post-login validation 与通知 |
| `src/tui/login_picker.rs` | TUI Login Picker overlay |
| `src/tui/app/auth_types.rs` | TUI auth 状态机：`PendingLogin` 枚举（13 variant）、`SaitecLoginField`、`StartupGuideAction`、`AccountCommand` |
| `src/tui/app/auth.rs` | TUI auth 事件处理：`restore_startup_guide_if_needed`、pending login 状态流转 |
| `src/login_qr.rs` | QR 码 Unicode 渲染（无浏览器环境扫码），受 `JCODE_SHOW_LOGIN_QR`/`JCODE_SHOW_TUI_LOGIN_QR` 控制 |
| `src/setup_hints.rs` + `setup_hints/` | Startup hint 系统（`maybe_show_setup_hints`、平台 specific nudge） |
| `crates/jcode-auth-types/src/lib.rs` | 跨 crate 共享类型：`AuthState`/`AuthCredentialSource`/`AuthExpiryConfidence`/`AuthRefreshSupport`/`ProviderValidationRecord`/`ProviderRefreshRecord` |
| `crates/jcode-azure-auth/src/lib.rs` | Azure `DefaultAzureCredential` bearer token 获取 |

## Startup Guide 系统

**Setup mode vs Reminder mode**（`PendingLogin::StartupGuide { is_reminder: bool }`）：
- **Setup mode**（`is_reminder=false`）：无任何 base model 配置。用户必须先完成 base model 登录。UI 显示「Setup Base Model」和「Login SAITEC」两按钮，前者为主 action。
- **Reminder mode**（`is_reminder=true`）：base model 已就绪，仅 SAITEC 未登录。UI「Login SAITEC」为主按钮，「Skip」为次按钮（允许跳过 SAITEC）。

**`restore_startup_guide_if_needed`**（`src/tui/app/auth.rs:55`）调用场景：
1. Login Picker 被关闭（Esc/cancel）时（`src/tui/app/navigation.rs:876`）。
2. 多账号 picker 关闭时（`src/tui/app/auth_account_picker_saved_accounts.rs:24`）。

逻辑：已有用户消息或正在 streaming 则不触发；调 `AuthStatus::check_fast()` 取最新状态；SAITEC 已登录则不做任何事；否则按 `has_any_base_model()` 决定 Setup 还是 Reminder mode。

**`maybe_show_setup_hints`**（`src/setup_hints.rs`）：每次启动调用，递增 `launch_count`；每 3 次启动在 stderr 显示平台 specific nudge（Windows: Alt+; 热键 + Alacritty 安装；macOS: Ghostty 引导）；所有 nudge 支持「Don't ask again」永久跳过；状态持久化 `~/.ssc_tui/setup_hints.json`。

## 账号存储 / OAuth PKCE 流程

**多账号存储格式**：
- Claude: `~/.ssc_tui/auth.json` → `JcodeAuthFile { anthropic_accounts: Vec<AnthropicAccount>, active_anthropic_account: Option<String> }`，每 account 含 `label`/`access`/`refresh`/`expires`/`email`/`subscription_type`/`scopes`；支持从旧单账号 `{"anthropic": {...}}` 自动迁移。
- OpenAI: `~/.ssc_tui/openai-auth.json` → `JcodeOpenAiAuthFile { openai_accounts, active_openai_account }`。
- 活跃账号有 `ACTIVE_ACCOUNT_OVERRIDE: RwLock<Option<String>>` 运行时覆盖，允许 `/account switch` 不落盘立即生效。
- 账号 label 自动编号 `{prefix}-1`/`{prefix}-2`，`relabel_accounts` 保证连续。

**OAuth PKCE 流程**（`src/auth/oauth.rs`）：
1. 生成 64 字符随机 verifier + SHA256 challenge + random state。
2. 拼装 authorize URL（Claude: `claude.com/cai/oauth/authorize`；OpenAI: `auth.openai.com/oauth/authorize`）。
3. `bind_callback_listener(0)` 在随机端口起本地 HTTP server。
4. 打开浏览器（除非 `--no-browser` 或 `NO_BROWSER`/`JCODE_NO_BROWSER`）。QR 码可选。
5. `wait_for_callback_async_on_listener` 等回调（Claude 120s，OpenAI 300s），验 state 防 CSRF。
6. 超时降级为手动粘贴 code/URL。
7. `exchange_claude_code`/`exchange_openai_code` 发 token exchange。
8. Claude 额外校验 `user:inference` scope（`ensure_claude_inference_scope`）。
9. exchange 成功后 `update_claude_account_profile` 取 email 并持久化。
10. 刷新 Claude token 带 scope；被拒（`invalid_scope`）则降级为不带 scope 的 legacy 模式。

## 依赖关系

- 被 [03 Provider](03-provider.md)（`MultiProvider::new_with_auth_status` 凭据探测）、[01 CLI](01-cli.md)（`login.rs` flow、`detect_bootstrap_credentials`）、[05 TUI](05-tui.md)（login picker / `/account`）依赖。
- 依赖 [12 Workspace](12-workspace-build-ci.md)（`jcode-auth-types`/`jcode-azure-auth`）、[09 SAITEC](09-mcp.md)（SAITEC 凭据存储三件套、`subscription_catalog`）。

## 陷阱与历史修复

### AuthTest deadlock from stale `auth-validation.json`（fixed `b18a4c17`）

**Symptom**：在 login picker 按 `R` 显示 `validation failed (just now)` 但无详情。读 `~/.saitec_tui/auth-validation.json` 看实际错误。

**Root cause**：stale `success: false` 行 → `state_for_provider` 返回 `Expired` → probe 在 smoke 前短路 → 写入新失败 → 锁死状态。

**Fix**（`src/cli/auth_test/probes.rs`）：对 `OpenAiCompatible` 目标，绕过 stale `Expired`，直接调 `openai_compatible_profile_is_configured()` 判定 Available。

### auth-validation.json stale 状态（通用）

- `doctor.rs` 中 `VALIDATION_STALE_AFTER_MS = 7 天`。超 7 天的 validation record 视为 stale，触发 `needs_attention`。
- 但 validation record 只在显式运行 `jcode auth-test --provider X` 或 TUI 按 R revalidate 时写入。配置 API key 后从未手动验证过，`last_validation` 为 `None` 也触发 `needs_attention`——完全正常的 API key provider 在 `/account` 也可能显示「needs attention」直到手动做一次 runtime validation。

### auth-validation.json 与 Copilot 交互

`copilot::validation_failure_blocks_auto_use()` 在 validation record 失败且未过期（24h TTL）时阻止 Copilot 作为 auto provider 选用（safety valve）。但跳过 env var token（`copilot_env_token_present()` 时返回 false），因 env token 可能是新提供的。

### Cursor auth 状态在 check vs check_fast 中的差异

- `check_uncached()`（完整）调 `cursor::has_cursor_native_auth()`（可能涉及子进程 `cursor-agent status`），未通过返回 `Expired`。
- `check_uncached_fast()`（快速）仅检查文件/env 存在性，存在即 `Available`——快速检查可能过于乐观。

### SAITEC login 后 server 通知是 best-effort

`notify_running_server_auth_changed_best_effort()` 在 login 完成后尝试通知已运行 jcode server，连接失败则静默忽略——热加载新 provider 可能不立即生效。

### external auth source consent

导入 OpenCode / Claude Code / pi 等外部 auth 需用户显式 consent（`allow_external_auth_source_for_path`）。consent 记录按 source_id + path 存储——外部工具 auth 文件路径改变（如升级后）需重新 consent。

### Cloudflare challenge 拦截 Claude token exchange

`oauth.rs:698` 明确处理 Claude token endpoint 被 Cloudflare challenge 拦截的情况，建议切 VPN exit IP——已知实际部署痛点。

### Setup hints 在非交互终端跳过

`maybe_show_setup_hints()` 在 `!io::stdin().is_terminal() || !io::stderr().is_terminal()` 时直接返回 None——SSH/pipe 场景无任何引导。

## 回指

- SAITEC 凭据三件套（auth.json/saitec.env/mcp.json）与 MCP lifecycle sync：[09-mcp.md](09-mcp.md)
- Provider 凭据探测与 failover：[03-provider.md](03-provider.md)
- CLI login 入口与 auth_test 框架：[01-cli.md](01-cli.md)
