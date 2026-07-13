# 02 · Agent Runtime

> 子系统：Agent 对话循环、system prompt 构建、流式响应处理、tool/skill 执行、上下文压缩、soft interrupt、消息修复。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

Agent runtime 协调 LLM 对话生命周期：管理会话状态、构建 system prompt、调用 provider 获取流式响应、解析并执行 tool calls、处理上下文压缩，并通过 broadcast/mpsc 通道将事件推送给 TUI 或 server 客户端。

## 关键文件清单

| 路径 | 职责 |
|---|---|
| `src/agent.rs` | `Agent` 核心结构体定义与构造函数，所有子模块 mod 声明与 re-export |
| `src/agent/turn_loops.rs` | 主对话循环 `run_turn()`：流式响应解析、tool call 执行、soft interrupt 注入、context-limit 自动重试 |
| `src/agent/turn_execution.rs` | 对外入口 `run_once()` / `run_once_streaming()` / `run_once_streaming_mpsc()` |
| `src/agent/turn_streaming_broadcast.rs` | `run_turn_streaming()` — `broadcast::Sender<ServerEvent>` 推送（server 广播模式） |
| `src/agent/turn_streaming_mpsc.rs` | `run_turn_streaming_mpsc()` — `mpsc::UnboundedSender<ServerEvent>`（per-client 单播） |
| `src/agent/compaction.rs` | context-limit 错误检测与自动硬压缩恢复 |
| `src/agent/prompting.rs` | system prompt 构建（split: static/dynamic 两段优化 cache）、memory prompt 注入 |
| `src/agent/messages.rs` | 消息添加到 session 的封装，每条新消息同步通知 `CompactionManager` |
| `src/agent/provider.rs` | Provider 管理：model 切换、reasoning effort、premium mode、compaction mode |
| `src/agent/environment.rs` | 环境快照 `EnvSnapshot` 构建（git state、provider info、OS/架构） |
| `src/agent/interrupts.rs` | Soft interrupt 队列管理：持久化/恢复、注入时机、`NoToolCallOutcome`/`PostToolInterruptOutcome` |
| `src/agent/response_recovery.rs` | 从纯文本中恢复被包裹的 tool call（`to=functions.xxx` fallback 解析） |
| `src/agent/status.rs` | 查询：message count、last assistant text、transcript 构建 |
| `src/agent/streaming.rs` | 流式 keepalive ticker（30s 间隔）与 pong 事件 |
| `src/agent/tools.rs` | tool output 转 ContentBlock、tool summary 打印 |
| `src/agent/utils.rs` | `trace_enabled()`、`git_state_for_dir()`、generated image side panel 更新 |
| `src/compaction.rs` | `CompactionManager`：后台对话压缩管理器（Reactive/Proactive/Semantic 三模式） |
| `src/tool/mod.rs` | `Registry`：tool 注册表（Arc-wrapped），clone 时独立创建 CompactionManager |
| `src/skill.rs` | `SkillRegistry` / `Skill`：从 SKILL.md 加载技能，支持首次运行从 Claude Code / Codex CLI 导入 |

## 核心类型与关键函数

- **`Agent`** (`agent.rs`) — 核心运行时对象，持有 provider、registry、session、interrupt queue、cache tracker 等全部状态。
- **`CompactionManager`** (`compaction.rs`) — 上下文压缩管理器，跟踪 `compacted_count`（已压缩前缀消息数），支持 Reactive/Proactive/Semantic 三策略；Proactive 用 `token_history` + EWMA 预测，Semantic 用 `embedding_history` 做 topic-shift 检测。
- **`Registry`** (`tool/mod.rs`) — 工具注册表，`Clone` 时共享 tools/skills Arc 但为每个 subagent 独立创建 CompactionManager。
- **`SkillRegistry` / `Skill`** (`skill.rs`) — 全局共享技能注册表（`OnceLock<Arc<RwLock<Self>>>`），支持热重载。
- **`TokenUsage`** — 记录 input/output/cache_read/cache_creation tokens。

关键函数：
- `Agent::run_turn()` — 主对话循环（核心控制流）
- `Agent::run_turn_streaming()` / `run_turn_streaming_mpsc()` — 带事件推送变体
- `Agent::run_once()` / `run_once_streaming_mpsc()` — 对外入口
- `Agent::build_system_prompt_split()` — 构建 static/dynamic 分段 system prompt
- `Agent::try_auto_compact_after_context_limit()` — context-limit 自动恢复
- `Agent::recover_text_wrapped_tool_call()` — 从纯文本恢复结构化 tool call
- `Agent::inject_soft_interrupts()` — tool call 间隙注入 soft interrupt
- `Agent::repair_missing_tool_outputs()` — 修复 session 中缺失的 tool result（`tool_output_scan_index` 增量扫描）
- `tool_output_to_content_blocks()` — ToolOutput（含 images）→ ContentBlock
- `stream_keepalive_ticker()` — 30s keepalive（测试模式 50ms）

## 主控制流

`run_turn()` 是核心循环，每轮：

```
repair_missing_tool_outputs()
  → messages_for_provider()（触发可能的 compaction）
  → 非阻塞构建 memory prompt（用上轮结果，异步预计算下轮）
  → build_system_prompt_split()（static 段可被 provider 缓存）
  → provider.complete_split() 获取流式响应
  → 逐 StreamEvent 累积 text/tool_calls/token_usage
  → context-limit 错误 → 自动 compact + retry（最多 5 次）
  → 流结束 → 解析 tool calls（含 text-wrapped fallback 恢复）
  → 若有 tool calls：逐个执行（优先 SDK 已执行结果，否则本地执行）
       → 结果作为 ToolResult 消息加入 session
       → inject soft interrupts → continue 下一轮
  → 若无 tool calls：turn 结束，返回最终文本
```

## 依赖关系

**内部 crate/模块**：
- `crate::provider` — Provider trait（`complete_split`、`context_window`、`supports_compaction`）
- `crate::session` — Session / StoredMessage / GitState / EnvSnapshot
- `crate::message` — Message / ContentBlock / Role / ToolCall / StreamEvent
- `crate::protocol` — ServerEvent（Compaction/MemoryInjected/ToolUpdated 等）用于 TUI/server 通信
- `crate::tool` — Registry / Tool trait / ToolContext / ToolOutput
- `crate::compaction` — CompactionManager（底层在 `jcode-compaction-core`）
- `crate::skill` — SkillRegistry
- `crate::bus` — Bus/BusEvent（SubagentStatus/ToolEvent）
- `crate::cache_tracker` — CacheTracker（检测 append-only 缓存违规）
- `crate::memory` / `crate::memory_agent` — memory prompt 注入与 memory-agent 管线
- `crate::telemetry` / `crate::config` / `crate::id` / `crate::logging`

**外部 crate**：
- `jcode_agent_runtime` — SoftInterruptQueue / InterruptSignal / GracefulShutdownSignal / BackgroundToolSignal / StreamError
- `jcode_compaction_core` — compaction 常量、prompt 构建、Summary 类型
- `jcode_message_types` — ToolDefinition
- `jcode_tool_core` — Tool trait / ToolContext / ToolExecutionMode / StdinInputRequest
- `jcode_tool_types` — ToolOutput / ToolImage
- `anyhow`、`serde`/`serde_json`、`tokio`、`futures`、`chrono`

## 陷阱与设计约束

- **CompactionManager clone 隔离**：`Registry::clone()` 新建独立 CompactionManager，防止并行 subagent 互相破坏压缩状态。subagent 的 compaction 状态完全独立，不继承父 agent 进度。
- **locked_tools 缓存冻结**：首次 API 请求后冻结 tool 列表（`locked_tools`），避免异步到达的 MCP tools 导致缓存失效；compaction/reset 时清除。对话中途到达的新 MCP tool 不会被包含直到 compaction 或 reset。
- **context-limit 自动重试有上限**：最多 5 次（`MAX_CONTEXT_LIMIT_RETRIES`），incomplete continuation 最多 3 次（`MAX_INCOMPLETE_CONTINUATION_ATTEMPTS`），超过后硬错误返回。
- **text-wrapped tool call 恢复**：某些 provider（如 OpenRouter）把 tool call 包在纯文本中（`to=functions.xxx`），`recover_text_wrapped_tool_call()` 做 fallback 解析，有全局计数器 `RECOVERED_TEXT_WRAPPED_TOOL_CALLS` 追踪。
- **SDK tool result 优先**：provider 已通过 SDK 执行 tool（`StreamEvent::ToolResult`）时优先用该结果；除非是 native tool（selfdev/communicate）且 SDK 返回错误——此时 fallback 到本地执行。
- **Soft interrupt 非阻塞注入**：用 `std::sync::Mutex`（非 tokio Mutex），可在 async 上下文安全访问；在 tool call 间隙注入，不打断执行中的 tool；支持持久化（`soft_interrupt_store`）应对进程重启。
- **Memory 注入是 user message**：memory prompt 以 `<system-reminder>` 包裹后作为最后一条 User 消息追加（非改 system prompt），保持 system prompt prefix 的 cache 命中率。
- **Split prompt 缓存优化**：system prompt 分 static（可被 provider 缓存）和 dynamic（每轮变化）两段，经 `complete_split()` 分别传递。
- **keepalive**：streaming 模式每 30s 发 pong keepalive 防客户端超时（测试模式 50ms）。

## 关联模块

下列辅助模块就近归入 Agent runtime 文档（小表格优先可发现性，后续轮次深化）：

| 模块 | 路径 | 职责 | 规模 |
|---|---|---|---|
| `src/prompt.rs` + `src/prompt/` | 系统提示词组装、`DEFAULT_SYSTEM_PROMPT`(编译时嵌入 `system_prompt.md`)、selfdev 模式提示、SAITEC MCP 安全提示、`SplitSystemPrompt`(static+dynamic 分离)、`ContextInfo`(token 估算) | ~925 行 |
| `src/background.rs` + `src/background/` | 后台任务执行管理器——tool 在后台运行，完成后通知 agent；文件存储崩溃恢复 + event channel 实时通知；`BusEvent::BackgroundTaskCompleted`/`Progress` | ~1676 行 |
| `src/usage.rs` + `src/usage/` | Anthropic OAuth / OpenAI ChatGPT 订阅用量拉取、缓存、多账户管理（TUI info widget / `/usage` 数据源） | ~3136 行 |
| `src/todo.rs` | 会话级 Todo 列表持久化到 `~/.jcode/todos/{session_id}.json`；`BusEvent::TodoUpdated` | 23 行 |
| `src/soft_interrupt_store.rs` | 软中断消息持久化到 `~/.jcode/pending-soft-interrupts/`（User/System/BackgroundTask 三源）；支撑 `src/agent/interrupts.rs` | 121 行 |
| `src/cache_tracker.rs` | 客户端侧 prompt 缓存违规追踪——provider 不报 cache token 时自追踪 message prefix hash 检测缓存被破坏 | 397 行 |

**Note**：`src/prompt.rs` 与 `src/agent/prompting.rs` 不同——前者管理 prompt 组装和大小追踪，后者管理 agent 运行时 prompt 注入逻辑。

**死代码**：`src/usage_display.rs`(175 行) 和 `src/usage_openai.rs`(358 行) 是重构进 `src/usage/` 目录后的遗留孤立文件，`use super::` 但未在 `lib.rs`/`usage.rs` 声明为模块，不参与编译——可安全删除。

## 回指
- Agent 事件如何回流 TUI：[11-bus-message-protocol.md](11-bus-message-protocol.md) + [05-tui.md](05-tui.md)
- Server 如何驱动 Agent（`handle_client` → `process_message_streaming_mpsc`）：[04-server.md](04-server.md)
- CompactionManager 底层（200K budget / 80% 触发 / 95% critical）：[12-workspace-build-ci.md](12-workspace-build-ci.md)（`jcode-compaction-core`）
