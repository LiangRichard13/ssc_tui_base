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

## Soft Interrupt 设计详解

> **设计文档来源**：`docs/SOFT_INTERRUPT.md`（原生 jcode 设计，SAITEC-TUI 的 `src/agent/interrupts.rs` 实现了此机制，具体细节可能有差异）

### 设计目标

允许用户在 AI 正在生成时注入新消息，**不取消**当前 generation。避免当前 hard interrupt 的三问题：
1. 丢失 AI 已完成的 partial work
2. 等待 cancellation 完成的延迟
3. 全量 context 重新发送的 API 浪费

### 安全注入点（Injection Points）

Anthropic API 的核心约束：**每个 `tool_use` block 必须紧随对应的 `tool_result` block**，中间不可插入 user text。因此注入点受限。

#### Agent Loop 中的四个点

```
loop {
    // 1. Build messages and call provider.stream()
    // === PROVIDER OWNS THE CONNECTION HERE ===
    // Stream events: TextDelta, ToolStart, ToolInput, ToolUseEnd

    // 2. Stream ends

    // 3. Add assistant message to history
    //    (MUST happen before injection to preserve cache and conversation order)

    // 4. Check if tool calls exist
    if tool_calls.is_empty() {
        // ═══════════════════════════════════════════════
        // ✅ POINT B: No tools, turn complete
        // ═══════════════════════════════════════════════
        break;
    }

    // 5. Execute tools and add tool_results
    for tc in tool_calls {
        // Execute single tool...
        // Add result to history...

        // ═══════════════════════════════════════════════
        // ✅ POINT C: Between tool executions
        // (only for urgent aborts — must add skipped tool_results first)
        // ═══════════════════════════════════════════════
    }

    // ═══════════════════════════════════════════════
    // ✅ POINT D: All tools done, before next API call
    // ═══════════════════════════════════════════════

    // Loop continues → next provider.stream() call
}
```

| 注入点 | 时机 | 用途 |
|--------|------|------|
| **B** | Turn complete, no tools | 安全：无 tool_use blocks 配对问题 |
| **C** | Inside tool loop（urgent only） | 紧急中止——必须先添加 stub tool_results 给跳过 tool |
| **D** | After all tools, before next API call | **默认注入点**：最安全、最可预测 |

### 协议变更

```rust
// 原生 jcode 的 SoftInterrupt request type
#[serde(rename = "soft_interrupt")]
SoftInterrupt {
    id: u64,
    content: String,
    /// If true, can abort remaining tools at point C
    urgent: bool,
}
```

### Agent 端实现要点

1. **`soft_interrupt_queue: Vec<SoftInterruptMessage>`** 在 Agent 结构中。
2. **`inject_soft_interrupts()`** 检查队列，合并多条消息（`\n\n` 分隔），作为 User 消息加入对话。
3. **`has_urgent_interrupt()`** 检查是否有 urgent flag。
4. 非紧急中断**只在 Point D 注入**，Point C 仅用于紧急中止（urgent abort）。

### Point C 紧急中止流程

```
1. 检测 to has_urgent_interrupt()
2. 为剩余 tool_calls 添加 stub ToolResult（is_error: true，内容 "[Skipped: user interrupted]"）
3. 注入 user message（combined urgent interrupts）
4. break 工具执行循环
5. Loop 继续 → next API call（AI 看到全部 tool_results + user 消息）
```

### Point B 特殊行为

当无 tool calls 时本该退出循环，但若有 soft interrupt 则注入后 `continue` 而非 `break`——让 AI 在同一轮中处理用户新输入。

### 用户交互模式

- **默认模式**（非 urgent）：消息排队，UI 显示 `"⏳ Will inject at next safe point"` → tool 执行完 → 注入 → `"✓ Message injected"`
- **紧急模式**（Shift+Enter / urgent flag）：UI 显示 `"⚡ Will inject ASAP (may skip tools)"` → 当前 tool 完成 → 剩余 tools 跳过 + 注入

### Server Event 反馈

```rust
ServerEvent::SoftInterruptInjected {
    content: String,
    point: String,  // "B", "C", or "D"
}
```
允许 TUI 显示反馈如 "Message injected after tool X"。

### 边缘场景

1. **多个同时 soft interrupt**：combine 为单条 message（`\n\n` 分隔）。
2. **文本响应期间注入**：Point B，continue 循环。
3. **Provider 内部处理 tools**（如 Claude CLI 模式）：仍在 agent loop 中注入。
4. **Urgent interrupt 但无 tools**：降级为正常 Point B 注入。
5. **Stream error**：清空 soft interrupt queue，正常报告错误。

### SAITEC-TUI 实现现状

- `src/agent/interrupts.rs` 实现了 `SoftInterruptQueue`，包含持久化/恢复（`soft_interrupt_store`）。
- `NoToolCallOutcome`/`PostToolInterruptOutcome` 两个枚举控制注入时机。
- TUI 侧在 `RemoteConnection` 中实现 `soft_interrupt()` 方法。
- `src/soft_interrupt_store.rs` 处理中断消息持久化（User/System/BackgroundTask 三源）。
- 紧急性可配置：`request_cancel` 仍走 hard interrupt，`queue_soft_interrupt` 走 soft 路径。

---

## Browser Tool / Provider 交互

> **设计文档来源**：`docs/BROWSER_PROVIDER_PROTOCOL.md`
> 完整协议规范见 [22-browser-provider.md](22-browser-provider.md)

### 设计目标

jcode 应暴露一个一等公民 `browser` tool，同时兼容多个浏览器自动化后端（Firefox Agent Bridge、Chrome Agent Bridge、CDP、WebDriver/BiDi、Safari 等）。Agent 通过 browser tool 调用 provider 执行浏览器操作。

### Tool → Provider 调用流程

```
Agent run_turn() 循环
  → 模型返回 tool_call: browser { action: "page.open", url: "..." }
  → Registry 解析 browser tool
  → browser tool 调用当前 browser provider（由用户选择或自动协商）
  → provider 执行浏览器操作并返回结果
  → ToolOutput 回到 agent 循环
```

### 核心操作集（browser tool 向模型暴露的 action）

| Action | 描述 | Provider 方法映射 |
|---|---|---|
| `open` | 打开 URL | `page.open` |
| `snapshot` | 返回当前页面的可访问快照 | `page.snapshot` |
| `click` | 点击元素（element_ref/selector/text/position） | `page.click` |
| `type` | 在输入框输入文本 | `page.type` |
| `wait` | 等待条件（text/selector/navigation） | `page.wait` |
| `screenshot` | 截图 | `page.screenshot` |
| `go_back` | 回退 | `page.go_back`（可选） |
| `go_forward` | 前进 | `page.go_forward`（可选） |
| `eval` | 执行 JS | `page.eval`（可选） |
| `press` | 按键 | `page.press`（可选） |
| `scroll` | 滚动 | `page.scroll`（可选） |

### 能力协商

Provider 通过 `provider.describe` 返回其能力（certification_tier、core/optional/custom methods、features）。Agent runtime 据此：
- 选择合适的 provider
- 避免调用未支持的操作
- 在无可用 provider 时显示设置指南

### 状态引用模型

- **session** → server-owned browser session handle
- **page** → 一个 tab 或浏览表面
- **element_ref** → provider 发放的不透明 handle，模型可后续引用

### 传输方法

provider 可通过多种方式集成：
- 直接 Rust trait 调用（in-process）
- stdio JSON-RPC
- 本地 socket RPC
- Wrapped remote API

详见 [22-browser-provider.md](22-browser-provider.md) 的完整信封格式和错误模型。

---

## Agent-Native VCS 核心行为

> **设计文档来源**：`docs/AGENT_NATIVE_VCS_CORE_BEHAVIOR.md`（324 行，草案）

### 设计目标

这是一个 Git/jj 层 VCS，核心能力并非"更好的合并算法"，而是：
1. 让 agent 的并发编辑可表述
2. 消除匿名脏状态
3. 保留足够上下文使未来 agent 能维护本地变更
4. 保持机器历史丰富、人类历史干净
5. 与 Git 生态系统兼容

### 核心实体

| 实体 | 定义 | 属性 |
|---|---|---|
| **Lane** | 正在进行的工作的主单元 | goal/agent 键、本地顺序、自有草稿状态、provenance、上游锚点、合约/不变量、维护策略 |
| **Draft patch (micro-commit)** | agent 每次有意义编辑的捕捉 | 关联一个 lane、可归属于 agent/model/session、基于特定 revision、可回退和重放、安全可压缩 |
| **Burst** | 一个子任务内多个时间相邻的 draft patch 的集合 | 将多个 rapid coherent edits 分组为一个工作片段 |
| **Published commit** | 面向人类的 commit，可由一个或多个 draft patches / bursts 压缩而成 | 用于人类审查和分享 |
| **Maintenance packet** | 每个本地 delta 的上下文包 | 包含 intent、behavioral contract、semantic anchors、assumptions、validation hooks、provenance、lifecycle policy |
| **Anchor** | lane/patch 附加到的上游概念记录 | symbol/function/type/endpoint/config key/UI element 等，比 line-level diff 更强 |

### 核心不变量

1. **无匿名脏状态**：所有未提交变更必须归属到一个 lane、draft patch 或显式 scratch area
2. **捕捉与发布分离**：agent 编辑自动捕捉为 draft unit，发布/压缩稍后执行
3. **本地意义优先于旧补丁形状**：上游维护时保留 delta 的"意义"而非旧的文本 diff
4. **交织正常**：不同 lane 的 commit 可在全局历史中交织

### Agent-VCS 交互模式

```
Agent 执行编辑
  → 系统自动捕捉为 draft patch（所属 lane、归属信息、基于 revision）
  → 多个 temporal coherent edits 形成 burst
  → 后续可压缩为 published commit
  → 每个 delta 附带 maintenance packet（intent、contract、anchors、validation）

上游变更时
  → 系统帮助 agent 在以下策略中选择：
      1. Replay delta
      2. 结构性适配 delta
      3. 从 goal + contract 重新生成
      4. 丢弃（上游已覆盖）
      5. 重新设计（上游变化过大）
```

### 与 Registry 的关系

- VCS 操作通过 `src/tool/` 中的工具暴露给 agent
- 工具注册在 `Registry` 中
- 操作包括：lane 管理、draft patch 捕捉、burst 分组、maintenance packet 查看

### 关键设计区别

- Git 是 **branch-first**，jj 是 **change-first**，本系统是 **lane-first** 和 **maintenance-packet-first**
- 对每个本地 lane/customization 鼓励或要求存储：intent、behavioral contract、semantic anchors、assumptions、provenance、rationale、upstream policy、lifecycle、validation hooks

## 关联模块

下列辅助模块就近归入 Agent runtime 文档（小表格优先可发现性，后续轮次深化）：

| 模块 | 路径 | 职责 | 规模 |
|---|---|---|---|
| `src/prompt.rs` + `src/prompt/` | 系统提示词组装、`DEFAULT_SYSTEM_PROMPT`(编译时嵌入 `system_prompt.md`)、selfdev 模式提示、SAITEC MCP 安全提示、`SplitSystemPrompt`(static+dynamic 分离)、`ContextInfo`(token 估算) | ~925 行 |
| `src/background.rs` + `src/background/` | 后台任务执行管理器——tool 在后台运行，完成后通知 agent；文件存储崩溃恢复 + event channel 实时通知；`BusEvent::BackgroundTaskCompleted`/`Progress` | ~1676 行 |
| `src/usage.rs` + `src/usage/` | Anthropic OAuth / OpenAI ChatGPT 订阅用量拉取、缓存、多账户管理（TUI info widget / `/usage` 数据源） | ~3136 行 |
| `src/todo.rs` | 会话级 Todo 列表持久化到 `~/.ssc_tui/todos/{session_id}.json`；`BusEvent::TodoUpdated` | 23 行 |
| `src/soft_interrupt_store.rs` | 软中断消息持久化到 `~/.ssc_tui/pending-soft-interrupts/`（User/System/BackgroundTask 三源）；支撑 `src/agent/interrupts.rs` | 121 行 |
| `src/cache_tracker.rs` | 客户端侧 prompt 缓存违规追踪——provider 不报 cache token 时自追踪 message prefix hash 检测缓存被破坏 | 397 行 |

**Note**：`src/prompt.rs` 与 `src/agent/prompting.rs` 不同——前者管理 prompt 组装和大小追踪，后者管理 agent 运行时 prompt 注入逻辑。

**死代码**：`src/usage_display.rs`(175 行) 和 `src/usage_openai.rs`(358 行) 是重构进 `src/usage/` 目录后的遗留孤立文件，`use super::` 但未在 `lib.rs`/`usage.rs` 声明为模块，不参与编译——可安全删除。

## 回指
- Agent 事件如何回流 TUI：[11-bus-message-protocol.md](11-bus-message-protocol.md) + [05-tui.md](05-tui.md)
- Server 如何驱动 Agent（`handle_client` → `process_message_streaming_mpsc`）：[04-server.md](04-server.md)
- CompactionManager 底层（200K budget / 80% 触发 / 95% critical）：[12-workspace-build-ci.md](12-workspace-build-ci.md)（`jcode-compaction-core`）
