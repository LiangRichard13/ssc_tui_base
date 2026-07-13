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

## AWS Bedrock Provider 详解

> **来源**：原生 jcode 设计文档 `docs/AWS_BEDROCK_PROVIDER.md`。SAITEC-TUI 继承此 provider，以下内容适用于本项目。

### 认证方式

支持标准 AWS 凭据机制和专属 API key 两种方式：

1. **API key**：`jcode login --provider bedrock` 保存 `AWS_BEARER_TOKEN_BEDROCK` + `JCODE_BEDROCK_REGION` 到 `~/.config/jcode/bedrock.env`。
2. **IAM/SSO profile**：通过 `AWS_PROFILE` + `AWS_REGION` 配置，jcode 特定覆盖为 `JCODE_BEDROCK_PROFILE` + `JCODE_BEDROCK_REGION`。使用 SSO 时先执行 `aws sso login --profile <profile>`。
3. **实例/容器元数据**：无本地 profile 时设 `JCODE_BEDROCK_ENABLE=1` + `AWS_REGION` 启用。

### IAM 权限

运行时最少需要 `bedrock:InvokeModel` 和 `bedrock:InvokeModelWithResponseStream`。模型发现额外需 `bedrock:ListFoundationModels` 和 `bedrock:ListInferenceProfiles`。STS 验证（`JCODE_BEDROCK_VALIDATE_STS=1`）需 `sts:GetCallerIdentity`。

### 模型发现

使用静态模型列表即刻可用；后台 catalog refresh 时调 `ListFoundationModels` + `ListInferenceProfiles` 并缓存结果。

### 可选请求参数

```
JCODE_BEDROCK_MAX_TOKENS=4096  JCODE_BEDROCK_TEMPERATURE=0.2
JCODE_BEDROCK_TOP_P=0.9       JCODE_BEDROCK_STOP_SEQUENCES='</done>,STOP'
```

### 使用与故障排查

```bash
# 直接指定 provider 和 model
jcode --provider bedrock --model anthropic.claude-3-5-sonnet-20241022-v2:0
# 使用 inference profile ID/ARN
jcode --model bedrock:us.anthropic.claude-3-5-sonnet-20241022-v2:0
```

- `AccessDenied`：授予 Bedrock invoke/list 权限并在 AWS Console 启用模型访问。
- `model not found`：确认 model ID/inference profile 和 region 支持。
- SSO token 错误：执行 `aws sso login --profile <profile>`。
- 缺少 region：设 `AWS_REGION` 或 `JCODE_BEDROCK_REGION`。

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

## Provider/Session/Shared-Contract 边界审计

> **来源**：原生 jcode 设计审计 `docs/PROVIDER_SESSION_SHARED_CONTRACT_AUDIT.md`（2026-04-16）。以下分析和推荐来源于原生 jcode，SAITEC-TUI 作为二次开发，部分决策路径可能不同。

### 执行摘要

当前 workspace 中 Provider 和 Session 边界的最佳下一步改进方向：**不是**完整提取 `Provider` trait 或 `session.rs`。最高杠杆的动作是（按优先级排序）：

1. 新增 `jcode-shared-contracts` crate（纯 serde 的 protocol/session 重叠类型）
2. 然后新增 `jcode-session-contracts`（session 元数据/回放/视图 struct）
3. 如需进一步 provider 侧改进，提取纯 provider identity/selection 层

### 禁止提取的告诫

以下提取看似诱人，但现实中会把已有的高 churn 耦合转为 workspace crate 间的 churn，**不建议现在做**：

| 不应做的事情 | 根本障碍 |
|---|---|
| 提取 `Provider` / `EventStream` 到共享 crate | trait 仍深度耦合 `message`、auth、runtime failover、logging、bus |
| 搬移整个 `provider_catalog.rs` | 混合 catalog/profile 值 + env 修改 + auth 探测 + 配置文件查找 + logging |
| 搬移整个 `protocol.rs` | `Request` / `ServerEvent` 依赖 `message` / `provider` / `session` / `side_panel` / `bus` |
| 搬移整个 `session.rs` | 混用 contract struct + runtime state + rendering + journaling + persistence |

### 推荐提取顺序

**Phase 1 — `jcode-shared-contracts`**（纯 serde，零额外依赖）：
- `PlanItem`（来自 `src/plan.rs`）
- 小 shared struct/enum：`TranscriptMode`、`CommDeliveryMode`、`FeatureToggle`、`SessionActivitySnapshot`
- Swarm 相关：`SwarmMemberStatus`、`AgentInfo`、`ContextEntry`、`SwarmChannelInfo`、`AwaitedMemberStatus`、`NotificationType`

**Phase 2 — `jcode-session-contracts`**（仅在 Phase 1 之后）：
- `SessionStatus`、`SessionImproveMode`、`StoredDisplayRole`、`StoredTokenUsage`
- `StoredCompactionState`、`StoredMemoryInjection`、`RenderedImageSource`、`RenderedImage`
- `StoredReplayEvent` / `StoredReplayEventKind`（等其 swarm/plan payload 不再指向 `protocol.rs`）

**Phase 3 — provider identity/selection**（可选）：
- provider identity enum（`ActiveProvider`）
- pure fallback ordering helpers
- **不包含**：`Provider` trait、`EventStream`、account failover、auth state、runtime availability、logging/bus 副作用

### 对 SAITEC-TUI 的参考意义

如果遇到类似耦合问题可参考上述提取路径。若做了代码裁剪或合并，需重新评估耦合度。「不要提取」的告诫同样重要——未遇编译性能瓶颈时，过早提取 crate 可能引入不必要的依赖管理复杂度。

---

## Browser Provider 设计

> **设计文档来源**：`docs/BROWSER_PROVIDER_PROTOCOL.md`
> 完整协议规范见 [22-browser-provider.md](22-browser-provider.md)

### Provider 特性概览

Browser Provider 与 8 个内置 LLM provider（Claude/OpenAI/Copilot 等）不同，它不是 LLM provider，而是**浏览器自动化后端**。jcode 的 `browser` tool 通过这个 provider 进行页面导航、快照、点击和截屏。

### 支持的 Browser Provider 清单

| Provider 类型 | 后端 | 认证方式 | 传输协议 |
|---|---|---|---|
| **Firefox Agent Bridge** | Firefox 浏览器 | Native host manifest | stdio JSON-RPC |
| **Chrome Agent Bridge** | Chrome/Chromium | DevTools Protocol | WebSocket/CDP |
| **WebDriver / BiDi** | 通用 WebDriver | 标准 WebDriver 协议 | HTTP |
| **Safari** | Safari 浏览器 | Safari WebDriver | HTTP |

### 实现方式

提供者可通过以下方式集成到 provider 子系统：
- **直接 Rust trait**：在 `src/provider/` 中实现 `Provider` trait（对应 `src/tool/browser.rs` 调用）
- **进程外 adapter**：通过 stdio JSON-RPC 或本地 socket 通信
- **包装的远程 API**：通过 HTTP 转发

### 与 `src/tool/browser.rs` 的关系

实际 `src/tool/browser.rs` (~1144 LOC) 实现了 `browser` tool 的核心逻辑（页面操作、对话管理）。Browser Provider Protocol 定义了 tool 调用下层 browser adapter 的标准接口。Agent 在 `run_turn()` 中调用 browser tool，后者通过 protocol 与具体 adapter 通信。

### 搜索能力（Search）

jcode 中的 **web search** 功能（`src/tool/search.rs` 或类似实现）也使用 browser provider 的页面导航和快照能力来：
- 打开搜索页面
- 提取搜索结果
- 导航到结果链接
- 截取页面快照供模型推理

### 与 MultiProvider 的关系

Browser Provider 当前**不在** `ActiveProvider` enum（8 个 LLM provider）中，而是通过 `src/tool/browser.rs` 直接管理 browser adapter 连接。未来可考虑将其纳入 provider 子系统，使 failover、认证刷新等基础设施可复用。

### 协议关键设计

协议定义了一个传输中立的语义层，核心操作集（`page.open`、`page.snapshot`、`page.click`、`page.type`、`page.wait`、`page.screenshot`）是所有认证 provider 必须实现的。provider 通过 `provider.describe` 上报能力（core methods、optional methods、features、custom methods）。

详见 [22-browser-provider.md](22-browser-provider.md) 的传输信封、错误模型、能力协商和认证分级。

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

## 关联模块

| 模块 | 路径 | 职责 | 规模 |
|---|---|---|---|
| `src/network_retry.rs` | 网络中断检测（连接重置、DNS 失败、超时）并生成等待策略；provider 流式传输中断时自动重连 | 169 行 |
| `src/copilot_usage.rs` | 本地 Copilot 用量追踪（requests/tokens/premium 按天/月/全量），持久化 `~/.jcode/copilot_usage.json` | — |

## 回指
- Agent 如何调用 provider：[02-agent-runtime.md](02-agent-runtime.md)
- Server bootstrap 凭据检测：[01-cli.md](01-cli.md)（`detect_bootstrap_credentials`）
