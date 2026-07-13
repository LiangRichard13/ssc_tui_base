# 03 · Provider（LLM 多后端抽象）

> 子系统：8 内置 provider + 30 OpenAI-compatible profile 的认证、模型路由、failover、多账号轮转、用量追踪。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

Provider 子系统是 SAITEC-TUI 的多后端 LLM 调用抽象层：通过 `MultiProvider` facade 统一管理 8 个内置 provider + 30 个 OpenAI-compatible profile 的认证、模型路由、failover、账号轮转与用量追踪，对上层暴露单一 `Provider` trait 接口。

## 支持的 Provider 清单

**8 个内置 provider（`ActiveProvider` enum）**：

| Provider | 认证方式 | 备注 |
|---|---|---|
| **Claude (Anthropic)** | OAuth（Pro/Max 订阅）或直接 API key（`ANTHROPIC_API_KEY`） | 双实现：`ClaudeProvider`（deprecated CLI）+ `AnthropicProvider`（直接 API），优先后者 |
| **OpenAI** | OAuth（ChatGPT Plus/Pro）或 API key | 支持 service tier、reasoning effort、transport 切换 |
| **Copilot (GitHub)** | Device Code flow | 本地 usage tracking（`~/.jcode/copilot_usage.json`），premium mode 三级控制 |
| **Antigravity** | OAuth | Google Antigravity OAuth |
| **Gemini (Google)** | OAuth（Gemini Code Assist） | Code Assist API（`cloudcode-pa.googleapis.com`），free/legacy tier |
| **Cursor** | Hybrid（API key / CLI） | 浏览器登录或 API key |
| **AWS Bedrock** | IAM / SigV4 / `AWS_BEARER_TOKEN_BEDROCK` | 原生 Converse/ConverseStream，支持 inference profile |
| **OpenRouter** | API key（`OPENROUTER_API_KEY`） | **同时承载全部 30 个 OpenAI-compatible profile 的运行时** |

**30 个 OpenAI-compatible profile**（定义在 `crates/jcode-provider-metadata/src/lib.rs` 的 `OPENAI_COMPAT_PROFILES`）：OpenCode Zen/Go、Z.AI、Kimi Code、Chutes、Cerebras、Alibaba Coding Plan、302.AI、Baseten、Cortecs、DeepSeek、Comtegra、FPT、Firmware、Hugging Face、Moonshot、Nebius、Scaleway、STACKIT、Groq、Mistral、Perplexity、Together、Deep Infra、Fireworks、MiniMax、xAI、LM Studio、Ollama、通用 OpenAI-compatible 等。本地端点（Ollama/LM Studio）`requires_api_key: false`。

另有 **Azure OpenAI** / **Google(Gmail)** 作为 `LoginProviderTarget` 存在但不在 `ActiveProvider` enum 中。

## 关键文件清单

**`src/provider/` 目录（应用层）**：

| 文件 | 职责 |
|---|---|
| `src/provider/mod.rs` | 入口；`MultiProvider` struct、`Provider` trait facade、`set_model_with_auth_refresh`、`openai_compatible_profile_route` |
| `src/provider/multi_provider.rs` | `MultiProvider` 认证探测与构造（`new_with_auth_status`）、account auto-selection、catalog refresh spawner |
| `src/provider/startup.rs` | `auto_default_provider`、`set_active_provider`、`forced_provider_from_env`、`parse_provider_hint` |
| `src/provider/selection.rs` | `ActiveProvider` / `ProviderAvailability` re-export + `auto_default_provider` wrapper |
| `src/provider/dispatch.rs` | `CompletionMode`（Unified/Split）、`complete_on_provider`/`complete_split_on_provider` |
| `src/provider/failover.rs` | failover 序列构建、error 分类（`FailoverDecision`）、no-provider 错误组装 |
| `src/provider/account_failover.rs` | 同 provider 多账号轮转：usage probe、account candidate 枚举、override 设置 |
| `src/provider/accessors.rs` | 子 provider `RwLock` accessor + `reconcile_auth_if_provider_missing` 惰性热初始化 |
| `src/provider/routing.rs` | `anthropic_oauth_route_availability`、`is_transient_transport_error` 等路由辅助 |
| `src/provider/route_builders.rs` | 各 provider 的 `ModelRoute` 构造函数 |
| `src/provider/models.rs` / `models_catalog.rs` | model catalog 持久化、context limit 解析 |
| `src/provider/pricing.rs` | `cheapness_for_route` 路由成本估算 |
| `src/provider/anthropic.rs` / `claude.rs` | `AnthropicProvider`（直接 API）/ `ClaudeProvider`（deprecated CLI） |
| `src/provider/openai*.rs` | `OpenAIProvider`（Responses API + WebSocket health） |
| `src/provider/copilot.rs` / `antigravity.rs` / `gemini.rs` / `cursor.rs` / `bedrock.rs` | 各 provider 实现 |
| `src/provider/openrouter*.rs` | `OpenRouterProvider`（provider routing、pinning、endpoint cache） |

**胶水与 crate 层**：

| 文件 | 职责 |
|---|---|
| `src/provider_catalog.rs` + tests | OpenAI-compatible profile 解析、env 映射、API key 加载/保存、`apply_openai_compatible_profile_env` / `apply_named_provider_profile_env` |
| `src/copilot_usage.rs` | 本地 Copilot 用量追踪（requests/tokens/premium 按天/月/全量） |
| `crates/jcode-provider-core/src/lib.rs` | `Provider` trait、`EventStream`、`ModelRoute`、`shared_http_client` |
| `crates/jcode-provider-core/src/selection.rs` | `ActiveProvider` enum、`auto_default_provider`、`fallback_sequence`、`parse_provider_hint` |
| `crates/jcode-provider-core/src/models.rs` | `ALL_CLAUDE_MODELS`/`ALL_OPENAI_MODELS`、`context_limit_for_model`、`OPENAI_COMPAT_MODEL_CONTEXT_LIMITS` 表 |
| `crates/jcode-provider-core/src/failover.rs` | `FailoverDecision`、`classify_failover_error_message` |
| `crates/jcode-provider-openai/` | OpenAI Responses API 请求构建、encrypted content |
| `crates/jcode-provider-openrouter/` | OpenRouter model/endpoint disk cache、`ProviderRouting`、`rank_providers_from_endpoints` |
| `crates/jcode-provider-gemini/` | Gemini Code Assist API types、model list 解析 |
| `crates/jcode-provider-metadata/` | `LoginProviderDescriptor`、30 个 `OpenAiCompatibleProfile` 常量、43 个 login provider 常量 |

## 核心类型与关键函数

- **`Provider` trait** (`jcode-provider-core/src/lib.rs:52`) — async trait，`complete`/`complete_split`/`set_model`/`model_routes`/`on_auth_changed`/`fork`/`native_compact`/`context_window` 等 30+ 方法。
- **`MultiProvider`** (`src/provider/mod.rs:191`) — facade，持有 8 个 `RwLock<Option<Arc<dyn Provider>>>` + `active: RwLock<ActiveProvider>` + `forced_provider: Option<ActiveProvider>`。
- **`ActiveProvider` enum** — `Claude | OpenAI | Copilot | Antigravity | Gemini | Cursor | Bedrock | OpenRouter`。
- **`ProviderAvailability`** — 8 bool 字段 + `copilot_premium_zero`。
- **`OpenAiCompatibleProfile`** / **`ResolvedOpenAiCompatibleProfile`** — profile 静态/运行时解析版本。
- **`ModelRoute`** — `model + provider + api_method + available + detail + cheapness`。
- **`CompletionMode`** — `Unified { system }` | `Split { system_static, system_dynamic }`。

关键函数：
- `auto_default_provider` — 选默认 provider，优先级：Copilot premium_zero > OpenAI-compatible prefer > OpenAI > Claude > Copilot > Antigravity > Gemini > Cursor > Bedrock > OpenRouter。
- `set_active_provider` — 写 `self.active` RwLock。
- `apply_openai_compatible_profile_env` — profile → `JCODE_OPENROUTER_*` 运行时 env。
- `apply_named_provider_profile_env` — `config.toml [providers.xxx]` → `JCODE_OPENROUTER_*` env。
- `force_apply_openai_compatible_profile_env` — 不检查 profile lock，强制覆写。
- `fallback_sequence` — 每个 active provider 的 failover 顺序（自己排第一）。
- `complete_with_failover` (`mod.rs:226`) — 核心请求路径：遍历 fallback sequence，逐 candidate 检查 configured → unavailable → precheck → dispatch → failover/error。
- `on_auth_changed` (`mod.rs:1361`) — 认证变化后热初始化所有缺失 provider + spawn post-auth model refresh。
- `openai_compatible_profile_is_configured` — 检测 profile 是否有可用 API key（env file / env var / localhost 免 key）。

## Provider 切换 / revalidate / restart 流程

**启动时（`new_with_auth_status`）**：
1. 探测全部凭证（claude/anthropic/codex/copilot/antigravity/gemini/cursor/bedrock + 磁盘 env file 扫描）。
2. 扫描 `OPENAI_COMPAT_PROFILES` 找 `configured_compat_profile`，调 `apply_openai_compatible_profile_env` 设 runtime env。
3. `auto_default_provider` 定初始 active provider。
4. 检查 `JCODE_FORCE_PROVIDER` + `JCODE_ACTIVE_PROVIDER` 环境变量强制覆盖。
5. 检查 `config.toml [provider].default_provider` 偏好覆盖。
6. 应用 `default_model` 配置。
7. spawn 后台 Anthropic/OpenAI model catalog refresh。
8. `auto_select_active_multi_account`：当前账号 exhausted 且有替代账号时自动切换。

**运行时切换（`set_model`）**：
1. 检测 `openai_compatible_model_prefix`（如 `deepseek:model-name`）。
2. 检测 `explicit_model_provider_prefix`（如 `copilot:model-name`）。
3. `--provider` lock 存在则强制路由到 locked provider。
4. 检测 `provider_for_model` 模型名启发式路由。
5. `set_model_on_provider` → `set_active_provider` 切换 active。

**认证变化（`on_auth_changed`）**：遍历所有 provider slot，缺失则 hot-initialize（从磁盘重读凭证构造）；对所有已有 provider spawn `post_auth_model_refresh`（invalidate_credentials + prefetch_models）。

**Failover**：`complete_with_failover` 遍历 `fallback_sequence`，逐 candidate：configured → unavailability_detail → precheck → dispatch → 成功则切 active，失败按 `FailoverDecision` 决定是否标记不可用并继续。active 失败先试同 provider 多账号轮转（`try_same_provider_account_failover`）。

**`--provider` lock（`forced_provider`）**：CLI `--provider` 设入 `forced_provider: Option<ActiveProvider>`，`fallback_sequence_for` 缩减为仅该 provider，`set_model` 经 `ensure_provider_lock_allows_model_target` 阻止跨 provider 路由。

## OpenAI-compatible profile 与 env 体系

**核心设计**：全部 30 个 OpenAI-compatible profile + named provider profile 共享 `OpenRouterProvider` 作为运行时 transport，通过 `JCODE_OPENROUTER_*` env var 族切换 endpoint。

**env var 体系（`RUNTIME_OPENAI_COMPAT_ENV_VARS`，`provider_catalog.rs:347`）**：
- `JCODE_OPENROUTER_API_BASE` / `JCODE_OPENROUTER_API_KEY_NAME` / `JCODE_OPENROUTER_ENV_FILE` / `JCODE_OPENROUTER_CACHE_NAMESPACE`
- `JCODE_OPENROUTER_MODEL` / `JCODE_OPENROUTER_STATIC_MODELS` / `JCODE_OPENROUTER_ALLOW_NO_AUTH`
- `JCODE_OPENROUTER_PROVIDER_FEATURES` / `JCODE_OPENROUTER_MODEL_CATALOG`
- `JCODE_OPENROUTER_AUTH_HEADER` / `JCODE_OPENROUTER_AUTH_HEADER_NAME`
- `JCODE_OPENROUTER_PROVIDER` / `JCODE_OPENROUTER_NO_FALLBACK`
- `JCODE_NAMED_PROVIDER_PROFILE` / `JCODE_PROVIDER_PROFILE_ACTIVE` / `JCODE_PROVIDER_PROFILE_NAME`（named profile 锁定）
- `JCODE_OPENAI_COMPAT_API_BASE` / `_API_KEY_NAME` / `_ENV_FILE` / `_DEFAULT_MODEL`（通用 OpenAI-compatible 覆盖）

**切换流程**：`apply_openai_compatible_profile_env(Some(profile))` 先清空所有 runtime env vars，再从 profile static const 推导并 `set_var`。`clear_openai_compatible_runtime_env_keep_config` 清除 runtime env 但保留 env file 中 durable 配置（logout 场景）。

**API key 加载优先级（`load_api_key_from_env_or_config`）**：进程 env var → env file（`~/.jcode/<file>`）→ 兼容旧名（ZAI_API_KEY → ZHIPU_API_KEY）→ external auth。

## 与 crates/jcode-provider-* 的分层

```
crates/jcode-provider-core        核心抽象：Provider trait、ActiveProvider、ModelRoute、
                                  fallback_sequence、context_limit、FailoverDecision、shared_http_client
    ├─ jcode-provider-metadata    静态元数据：30 profile 常量、43 LoginProviderDescriptor
    ├─ jcode-provider-openai      OpenAI 协议层：Responses API 请求构建、encrypted content
    ├─ jcode-provider-openrouter  OpenRouter 缓存层：model/endpoint disk cache、ProviderRouting
    └─ jcode-provider-gemini      Gemini 协议层：Code Assist API types

src/provider/                     运行时实现层：各 provider 的 Provider trait 实现
src/provider_catalog.rs           胶水层：profile → env var 映射、api key 加载
src/provider/multi_provider.rs    facade 层：MultiProvider 组装、认证探测
```

`crates/` 层只定义类型和纯函数（无 IO），`src/provider/` 实现 IO 密集的 `Provider` trait。`jcode-provider-core` 被所有其他 crate 和 `src/` 共同依赖；`jcode-provider-metadata` 被 `src/provider_catalog.rs` re-export。

## 陷阱与历史修复

### OpenRouter 命名歧义

`ActiveProvider::OpenRouter` 不仅服务真正的 OpenRouter，还承载全部 30 个 OpenAI-compatible profile 的运行时。`openrouter` 字段名和 `JCODE_OPENROUTER_*` env var 命名易让人误以为只针对 OpenRouter，实则是一套通用 OpenAI-compatible transport 层。

### OpenAI-compatible: config vs credentials env hygiene

**Config**（survive logout）：`JCODE_OPENAI_COMPAT_API_BASE`、`JCODE_OPENAI_COMPAT_DEFAULT_MODEL`、`JCODE_OPENAI_COMPAT_API_KEY_NAME`、`JCODE_OPENAI_COMPAT_ENV_FILE`。
**Credentials**（cleared on logout）：API key、ZAI/ZHIPU linked keys、`JCODE_OPENAI_COMPAT_LOCAL_ENABLED`。
**Runtime**（process-env only）：所有 `JCODE_OPENROUTER_*` vars、named-profile guards、4 个 `JCODE_OPENAI_COMPAT_*` overrides。

Env file：`AppData/Roaming/jcode/openai-compatible.env`（NOT `~/.jcode/` 或 `~/.saitec_tui/`）。其 `.bak` 是 pre-write snapshot——调试 config 丢失时比对 mtimes。

**Anti-pattern**：never call `force_apply_openai_compatible_profile_env(None)` from logout/activation-rollback paths——它会 wipe env file 的 config。用 `clear_openai_compatible_runtime_env_keep_config()` 代替。

### OpenAI-compatible: 200K / `anthropic/claude-sonnet-4` regression（fixed `e05304a1`）

**Symptom**：context bar 显示 200K，restart/revalidate 后请求失败 `400: ... passed anthropic/claude-sonnet-4`。

**4 root causes**（均在 `e05304a1`）：
1. **Hardcoded fallback**（`src/provider/mod.rs:732-735`）：`openrouter` 为 `None` 时 `unwrap_or("anthropic/claude-sonnet-4")`。Fix：`jcode-provider-core` 的 `OPENAI_COMPAT_MODEL_CONTEXT_LIMITS` 表覆盖 DeepSeek/Kimi/GLM/Qwen。
2. **Server bootstrap missing env vars**（`src/provider/startup.rs:87-99`）：bootstrap 时 `has_openrouter_creds` 为 `false`。Fix：扫描 env files 并 `apply_openai_compatible_profile_env` 后再 lookup。
3. **`auto_default_provider` priority**（`jcode-provider-core/src/selection.rs`）：`OpenRouter` 排在 Claude/OpenAI 之后。Fix：加 `prefer_openai_compatible` flag。
4. **`JCODE_OPENROUTER_MODEL` cleared but not re-set**（`provider_catalog.rs:444-450`）：`apply_openai_compatible_profile_env_impl` 清了但没重设。Fix：clear 后从 `resolved.default_model` 重新 apply。

**Anti-patterns**：Do NOT 把 `JCODE_OPENROUTER_MODEL` 加入 excluded vars（clear-then-set 才正确）。Do NOT 在 revalidate 路径调 `MultiProvider::set_active_provider(Claude)`。Do NOT 从 revalidate/restart/logout 调 `force_apply_openai_compatible_profile_env(None)`。

### Config.toml named provider: `JCODE_OPENROUTER_MODEL` symmetric cleanup

**Issue**（fixed after `e05304a1`）：config-toml provider 在 `[providers.xxx]` 缺 `default_model` 时 fallback 到 `anthropic/claude-sonnet-4`。

**Fix**（`provider_catalog.rs:531-540`）：无 `default_model` 的 `else` 分支显式 `remove_var("JCODE_OPENROUTER_MODEL")`。Tests：`named_provider_profile_env_*_model_*` in `provider_catalog_tests.rs`。

**Note**：其他 native provider（Claude/OpenAI/Bedrock/Copilot/Cursor/Antigravity/Gemini）不用 `apply_*_env` 模式，不受影响。

### Restart endpoint reversion to localhost:11434 from stale Ollama marker（fixed `dba79fc3`）

**Symptom**：generic openai-compatible profile 配 DeepSeek endpoint 会话内正常，restart 后回退到 `http://localhost:11434/v1` / `anthropic/claude-sonnet-4`。

**Root cause**：stale `ollama.env` 含 `JCODE_OPENAI_COMPAT_LOCAL_ENABLED=1`（prior Ollama probe 留下）使 `openai_compatible_profile_is_configured(OLLAMA_PROFILE)` 返回 true。`MultiProvider::new_with_auth_status`（`startup.rs:96-100`）中 `find()` 按 `OPENAI_COMPAT_PROFILES` 数组顺序遍历——`OLLAMA_PROFILE`（index 27）排在 `OPENAI_COMPAT_PROFILE`（index 29）之前——stale marker 胜过用户真实 DeepSeek 配置。

**3-layer fix**（均在 `dba79fc3`）：
1. `src/provider/startup.rs`：bootstrap scan 先查 `OPENAI_COMPAT_PROFILE` 再 fallback 到 named profiles，用户显式 custom endpoint 胜过 stale local-enabled marker。
2. `src/provider/openrouter.rs`：`autodetected_openai_compatible_profile()` 加 env_override-based bypass（key check miss 因自定义 key env var name 时）。
3. `src/cli/dispatch.rs`：`detect_bootstrap_credentials()` 也扫磁盘 env files，避免 server bootstrap 时误判「无凭据」。

**Diagnostic**：查 `$APPDATA/jcode/` 的 `ollama.env` 是否含 `JCODE_OPENAI_COMPAT_LOCAL_ENABLED=1`——不用 Ollama 就删它。比对 `openai-compatible.env` 与 `ollama.env` 的 mtime 看哪个更新近。Regression tests（JCODE_HOME-isolated）在 `src/provider/openrouter_tests.rs` 和 `src/provider/tests/model_resolution.rs`。`git show dba79fc3` 看完整改动。

### 其他坑

- **RwLock poisoning**：全局所有 `RwLock` 用 `unwrap_or_else(|poisoned| poisoned.into_inner())` 强制恢复——持锁 panic 不会死锁但可能产生不一致状态。
- **env var 全局状态耦合**：`apply_openai_compatible_profile_env` 直接写进程 env var（`crate::env::set_var`），并发环境（多 tokio task / 测试）有 data race 风险；测试用 `EnvVarGuard` + `ENV_LOCK` 缓解，生产代码无保护。
- **Claude dual-provider**：`ClaudeProvider`（deprecated CLI）与 `AnthropicProvider`（直接 API）并存，由 `use_claude_cli` env var 选择；大量 match arm 需 `if let Some(anthropic) ... else if let Some(claude) ...` 双重检查。
- **磁盘缓存无锁**：`jcode-provider-openrouter` 的 `DISK_CACHE_MEMO` / `ENDPOINTS_DISK_CACHE_MEMO` 用 `LazyLock<Mutex<HashMap>>` 但无文件锁，多进程（CLI + TUI 同时运行）可能读过期或写冲突。

## 回指

- 凭据探测与登录：[06-auth-login.md](06-auth-login.md)
- Agent 如何调用 provider：[02-agent-runtime.md](02-agent-runtime.md)
- Server bootstrap 凭据检测：[01-cli.md](01-cli.md)（`detect_bootstrap_credentials`）
