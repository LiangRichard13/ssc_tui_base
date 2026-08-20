# 11 · Bus / Message / Protocol

> 子系统：全局事件总线（`Bus`）、消息类型（含 secret redaction）、wire protocol（NDJSON over socket）。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 1. Bus 系统（`src/bus.rs`）

**核心机制**：全局单例 `Bus`，内部 `tokio::sync::broadcast::channel(256)` 广播 `BusEvent` 枚举。`Bus::global()` 获取静态实例，`.subscribe()` 获取 `broadcast::Receiver<BusEvent>`，`.publish(event)` 发送事件。

**BusEvent 枚举变体**（~25 个）：

| 变体 | 用途 |
|---|---|
| `ToolUpdated(ToolEvent)` | 工具调用状态变更（Running/Completed/Error），携带 session/message/tool_call ID |
| `TodoUpdated(TodoEvent)` | Todo 列表变更通知，携带 session_id + TodoItem 列表 |
| `SubagentStatus(SubagentStatus)` | 子代理（Task tool）状态上报，如 "calling API"、"running grep" |
| `ManualToolCompleted(ManualToolCompleted)` | 手动工具执行完成，含输出、耗时、错误标志 |
| `BatchProgress(BatchProgress)` | 批量工具进度更新 |
| `FileTouch(FileTouch)` | 文件被 agent 读/写/编辑，用于 swarm 冲突检测 |
| `BackgroundTaskCompleted(BackgroundTaskCompleted)` | 后台任务完成，含 task_id/status/exit_code/output_preview/duration |
| `BackgroundTaskProgress(BackgroundTaskProgressEvent)` | 后台任务进度上报 |
| `UsageReport(Vec<ProviderUsage>)` / `UsageReportProgress` | Provider 用量报告 / 渐进更新 |
| `LoginCompleted(LoginCompleted)` | OAuth 登录完成 |
| `SaitecAuthCleared` | SAITEC 会话凭证清除（logout） |
| `ProviderValidationCompleted` | Provider 非登录流的运行时校验完成 |
| `InputShellCompleted` | `!cmd` shell 命令执行完成 |
| `ClipboardPasteCompleted` | 剪贴板粘贴/图片 URL 处理完成 |
| `ModelRefreshCompleted` | 模型目录刷新完成 |
| `GitStatusCompleted` | git status 命令完成 |
| `UpdateStatus(UpdateStatus)` | 版本更新检查状态（Checking/Available/Downloading/UpToDate/Error） |
| `SessionUpdateStatus` | 按 session 的交互式更新状态 |
| `DictationCompleted` / `DictationFailed` | 语音转写完成/失败 |
| `CompactionFinished` | 后台上下文压缩完成 |
| `ModelsUpdated` | Provider 可用模型列表变更（带 750ms 防抖） |
| `ProviderModelActivated` | 后台 provider 设置任务为 session 选定模型 |
| `SidePanelUpdated` | 侧边面板页面更新 |
| `MermaidRenderCompleted` | Mermaid 渲染完成 |

**防抖逻辑**：`publish_models_updated()` 用 `OnceLock<Mutex<ModelsUpdatedPublishState>>` 实现 750ms 窗口内事件合并（coalescing），避免模型列表刷新风暴。

**补充**：`SwarmEvent`/`SwarmEventType` 不在 `BusEvent` 中，是 server 内部独立 `broadcast::channel`，定义在 `src/server/state.rs`，用于 swarm 成员生命周期、文件冲突、plan 更新等实时事件的 ring buffer（5000 条）历史与订阅。

## 2. Message 子系统

### 2.1 核心类型（`crates/jcode-message-types/src/lib.rs`）

- **`Message`** — 对话消息，含 `role: Role`、`content: Vec<ContentBlock>`、`timestamp`、`tool_duration_ms`。
- **`Role`** — `User | Assistant`（serde lowercase）。
- **`ContentBlock`** — `#[serde(tag = "type")]` tagged enum：`Text{text, cache_control}`、`Reasoning{text}`（隐藏推理）、`ToolUse{id, name, input}`、`ToolResult{tool_use_id, content, is_error}`、`Image{media_type, data}`、`OpenAICompaction{encrypted_content}`。
- **`ToolCall`** — `{id, name, input, intent?}`。
- **`ToolDefinition`** — `{name, description, input_schema}`，含 `prompt_token_estimate()`/`description_token_estimate()`（chars/4 启发式）。
- **`CacheControl`** — `{kind, ttl?}` prompt caching 元数据。
- **`StreamEvent`** — provider 流式事件枚举（TextDelta/ToolUseStart/ToolInputDelta/ToolUseEnd/ToolResult/GeneratedImage/ThinkingStart/Delta/End/Done/MessageEnd/TokenUsage/ConnectionType/ConnectionPhase/StatusDetail/Error/SessionId/Compaction/UpstreamProvider/NativeToolCall）。
- **`ConnectionPhase`** — `Authenticating|Connecting|WaitingForResponse|Streaming|Retrying`。
- 辅助函数：`messages_with_dynamic_system_context()`（最新用户 prompt 后插 system-reminder）、`stable_message_hash()`、`sanitize_tool_id()`、`ends_with_fresh_user_turn()`、`Message::with_timestamps()`。

### 2.2 Re-export 与扩展（`src/message.rs`）

从 `jcode_message_types` re-export 核心类型，自身扩展：
- **`redact_secrets(text) -> String`** — secret 脱敏，三层策略：
  1. 快速路径：检查文本含 `sk-`、`ghp_`、`AIza`、`ya29.`、`xox`、`api_key`、`token` 等关键词，不含则直接返回。
  2. 直接模式正则：匹配 `sk-ant-*`、`sk-or-v1-*`、`ghp_*`、`github_pat_*`、`ya29.*`、`AIza*`、`xox*` 等 token 格式，替换为 `[REDACTED_SECRET]`。
  3. 赋值模式正则：匹配 `KEY=VALUE` 形式环境变量（覆盖 ~30 个 API key 名称），保留 key 名替换 value。
  4. 运行时自定义：读 `JCODE_OPENROUTER_API_KEY_NAME`/`JCODE_OPENAI_COMPAT_API_KEY_NAME` 环境变量追加正则。
- **图片生成辅助**：`generated_image_tool_input()`、`generated_image_summary()`、`generated_image_visual_context_blocks()`。

### 2.3 Background Task 通知解析（`src/message/notifications.rs` + `src/protocol/notifications.rs`）

两个文件提供 format/parse 函数，用于后台任务完成通知的 Markdown 双向转换：
- `format_background_task_notification_markdown()` / `parse_background_task_notification_markdown()` → `ParsedBackgroundTaskNotification { task_id, tool_name, display_name?, status, duration, exit_label, failure_summary?, preview?, full_output_command }`。
- `format_background_task_progress_markdown()` / `parse_background_task_progress_notification_markdown()`。
- `src/protocol/notifications.rs` 中的版本更完整，额外支持 `display_name`（带反引号 sanitization）和 `failure_summary`（从预览中提取 `error:` 行）。

## 3. Protocol 子系统（`crates/jcode-protocol/src/lib.rs`）

### 3.1 crate 角色

`jcode-protocol` 定义 TUI 客户端与 server 之间的**全部 wire protocol**。传输层为 Unix socket 上的 NDJSON。分两个 socket 类型：main socket（TUI 通信）和 agent socket（agent-to-agent 通信）。

### 3.2 ServerEvent 枚举

server 发给 client 的事件，~40+ 变体，`#[serde(tag = "type")]` tagged enum。主要分类：
- **流式响应**：`Ack`/`TextDelta`/`TextReplace`/`ToolStart`/`ToolInput`/`ToolExec`/`ToolDone`/`GeneratedImage`/`BatchProgress`/`TokenUsage`/`MessageEnd`
- **连接状态**：`ConnectionType`/`ConnectionPhase`/`StatusDetail`/`UpstreamProvider`
- **Swarm**：`SwarmStatus`/`SwarmPlan`/`SwarmPlanProposal`/`Notification`/`CommContext`/`CommMembers`/`CommChannels`/`CommSummaryResponse`/`CommStatusResponse`/`CommReportResponse`/`CommPlanStatusResponse`/`CommAssignTaskResponse`/`CommTaskControlResponse`/`CommContextHistory`/`CommSpawnResponse`/`CommAwaitMembersResponse`
- **会话生命周期**：`SessionId`/`SessionCloseRequested`/`SessionRenamed`/`History`/`CompactedHistory`/`SidePanelState`/`SplitResponse`/`CompactResult`
- **Memory**：`MemoryInjected`/`MemoryActivity`
- **Compaction**：`Compaction`（含 trigger/pre_tokens/post_tokens/tokens_saved/duration_ms/messages_dropped 等丰富字段）
- **控制流**：`SoftInterruptInjected`/`Interrupted`/`StdinRequest`/`InputShellResult`/`Transcript`
- **配置变更响应**：`ModelChanged`/`ReasoningEffortChanged`/`ServiceTierChanged`/`TransportChanged`/`CompactionModeChanged`/`AvailableModelsUpdated`
- **Debug**：`State`/`DebugResponse`/`ClientDebugRequest`/`McpStatus`
- **Reload**：`Reloading`/`ReloadProgress`
- **Error/Done**：`Error`/`Done`/`Pong`
- **Unknown**：`#[serde(other)] Unknown` — **forward-compat catch-all**

### 3.3 HistoryMessage

```rust
pub struct HistoryMessage {
    pub role: String,         // "user"/"assistant" (raw string, not enum)
    pub content: String,
    pub tool_calls: Option<Vec<String>>,
    pub tool_data: Option<ToolCall>,
}
```
用于 `GetHistory` 响应传递完整对话历史。注意 `role` 是 `String` 而非 enum，比 `jcode_message_types::Role` 更宽松。

### 3.4 encode_event / NDJSON

```rust
pub fn encode_event(event: &ServerEvent) -> String {
    let mut json = serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string());
    debug_assert!(
        !json.contains('\n'),
        "encode_event produced JSON with internal newline (breaks NDJSON framing): {}",
        json
    );
    json.push('\n');
    json
}
```
将 `ServerEvent` 序列化为单行 JSON + 换行符。`debug_assert!` 确保序列化结果不含内部换行，否则破坏 NDJSON 帧。对应解码 `decode_request(line: &str) -> Result<Request, serde_json::Error>`。

### 3.5 Request 枚举

~50+ 变体：Message/Cancel/BackgroundTool/SoftInterrupt/Clear/Rewind/Ping/Subscribe/GetHistory/Reload/ResumeSession/InputShell/CycleModel/SetModel/SetReasoningEffort/SetServiceTier/SetTransport/SetCompactionMode/RenameSession/Split/Transfer/Compact/及完整 swarm Comm* 系列。每变体带 `id: u64` 用于请求-响应关联。

### 3.6 子模块

- `notifications.rs`：`NotificationType`（FileConflict/SharedContext/Message）和 `FeatureToggle`（Memory/Swarm/Autoreview/Autojudge）。
- `protocol_memory.rs`：`MemoryActivitySnapshot`/`MemoryPipelineSnapshot`/`MemoryStateSnapshot` 等 memory 子系统快照类型。

## 4. protocol.rs 在 src/ 中的 re-export

`src/protocol.rs` 仅一行：`pub use jcode_protocol::*;`，将 `crates/jcode-protocol` 全部公开类型 re-export 到 `crate::protocol`。`src/protocol/notifications.rs` 存在但只作为子模块路径，实际被 `jcode-protocol` crate 内部引用。

## 5. registry.rs 的角色

`src/registry.rs` 是 **server 注册表**，用于多 server 架构下的 server 发现与管理（**不是** tools/skills registry）：
- **`ServerInfo`**：运行中 server 信息（id/name/icon/socket path/debug socket/git_hash/version/pid/started_at/sessions）。
- **`ServerRegistry`**：`HashMap<String, ServerInfo>` 包装，持久化到 `~/.ssc_tui/servers.json`。
- 功能：`load()`/`save()`/`register()`/`unregister()`/`find_by_name()`/`servers_by_time()`/`cleanup_stale()`（检测死进程 + 同 socket 去重）/`add_session()`/`remove_session()`。
- 辅助函数：`server_socket_path(name)`/`server_debug_socket_path(name)`/`list_servers()`/`find_server_by_socket_sync()`（同步查找，用于 client 端 window title 等非 async 场景）。

**Note**：tools 和 skills 的注册分散在其他模块中（通过 `ServerEvent::History` 响应中的 `skills` 字段传递）。

## 关键文件清单

| 路径 | 职责 |
|---|---|
| `src/bus.rs` | 全局 tokio broadcast Bus，定义 BusEvent 枚举和所有事件类型，ModelsUpdated 防抖 |
| `src/message.rs` | re-export jcode_message_types，扩展 secret redaction 和图片生成辅助 |
| `src/message/notifications.rs` | background task 通知 Markdown format/parse（较老版本，缺 display_name） |
| `src/message_notifications.rs` | 消息通知模块入口（re-export notifications 子模块） |
| `src/protocol.rs` | 单行 re-export：`pub use jcode_protocol::*` |
| `src/protocol/notifications.rs` | protocol 层 NotificationType + FeatureToggle（实际在 jcode-protocol crate 内） |
| `src/registry.rs` | 多 server 架构的 server 注册表，`~/.ssc_tui/servers.json` 持久化 |
| `crates/jcode-message-types/src/lib.rs` | Message/Role/ContentBlock/ToolCall/ToolDefinition/StreamEvent 等核心消息类型 |
| `crates/jcode-protocol/src/lib.rs` | wire protocol 全部定义：ServerEvent/Request/HistoryMessage/encode_event/decode_request + swarm 辅助格式化 |
| `crates/jcode-protocol/src/notifications.rs` | NotificationType（FileConflict/SharedContext/Message）+ FeatureToggle |
| `crates/jcode-protocol/src/protocol_memory.rs` | MemoryActivitySnapshot/MemoryPipelineSnapshot 等 memory 快照类型 |
| `src/server/state.rs`（L238-283） | SwarmEvent/SwarmEventType 定义，server 内部 swarm 事件系统 |

## 依赖关系

- 被几乎所有子系统依赖：[02 Agent](02-agent-runtime.md)（ServerEvent/StreamEvent）、[04 Server](04-server.md)（Request/ServerEvent/Bus）、[05 TUI](05-tui.md)（RemoteConnection 解析 ServerEvent）、[07 Memory](07-memory.md)（MemoryInjected/MemoryActivity）、[09 MCP](09-mcp.md)（McpStatus）。
- 依赖 [12 Workspace](12-workspace-build-ci.md)（`jcode-protocol`/`jcode-message-types`）。

## 陷阱与历史修复

### encode_event 的 debug_assert 只在 debug 模式生效

```rust
debug_assert!(!json.contains('\n'), "...");
```
**release 模式下此 assert 不执行**。某 `ServerEvent` 变体字段值含原始换行符（如 `tool_done.output` 或 `error.message`）时，serde 默认转义为 `\n`（JSON 字符串内合法）所以正常不出问题；但若将来有字段经自定义序列化注入未转义换行，release 构建将静默产生损坏 NDJSON 帧导致客户端解析失败。这是正确的防御但覆盖不完全。

### ServerEvent::Unknown 的 forward-compat 陷阱（Fix 4 of `fix/mcp-notification-id`）

`#[serde(other)] Unknown` catch-all 保证旧客户端连新 server 时不会因未知 `type` tag 断连，但代价：
- **静默丢弃**：所有未知事件被无差别吞掉，客户端无法知道丢弃了什么。
- **无诊断信息**：`Unknown` 不携带任何数据（无原始 JSON、无 type 字符串），调试时无法追踪哪些事件被忽略。
- **类型安全漏洞**：模式匹配 `ServerEvent` 时编译器无法提醒新 server 添加了需客户端处理的新事件类型。

完整根因链（NDJSON 损坏 → reconnect storm → Unknown tool）见 [09-mcp.md](09-mcp.md)。

### jcode_message_types::Role vs jcode_protocol::HistoryMessage.role 类型不一致

`jcode_message_types::Role` 是 `enum { User, Assistant }`；`jcode_protocol::HistoryMessage.role` 是 `String`。history 数据经一次 wire 传输后类型安全性降低，解析端需自己处理大小写和未知 role。

### message/notifications.rs 与 protocol/notifications.rs 的重复

两套近乎相同的 format/parse 函数。`src/message/notifications.rs` 版本较老（`ParsedBackgroundTaskNotification` 缺 `display_name` 和 `failure_summary` 字段，header 直接用 `task.tool_name` 而非 `background_task_header_label()`）。`src/message.rs` re-export 的是 `notifications.rs` 中的函数，但外部消费方可能混用两套造成解析不兼容。

### Bus channel capacity 256 的背压风险

`broadcast::channel(256)` buffer 256 条消息。某 subscriber 处理慢于生产速度时 `recv()` 收 `Lagged` 错误并跳过消息。`publish()` 忽略 send 返回值（`let _ = self.sender.send(event)`），极端情况（所有 subscriber 都 lag）下事件被静默丢弃且无任何日志。

## 回指

- 谁发事件 / 谁订阅：[02-agent-runtime.md](02-agent-runtime.md)（SubagentStatus/ToolEvent）、[04-server.md](04-server.md)（Bus monitor/client forwarder/SwarmEvent）、[05-tui.md](05-tui.md)（next_event 解析 ServerEvent）
- NDJSON 损坏根因链（Fix 3 的 `encode_event` debug_assert）：[09-mcp.md](09-mcp.md)
