# 09 · MCP 集成

> 子系统：MCP（Model Context Protocol）JSON-RPC 客户端与全局连接池，通过 `McpTransport::Http` 走公共 HTTP 服务，headers 按原样持久化。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

MCP 子系统以 JSON-RPC 2.0 over NDJSON / HTTP 双通道协议连接外部 MCP Server 进程，通过全局共享连接池（`SharedMcpPool`）多 session 复用同一组 server，将远程 tool 注册为 jcode 内部 `Tool` trait 对象供 LLM agent 调用。用户通过 `$JCODE_HOME/mcp.json` 配置自己的 MCP 服务器（type=stdio | type=http+url+headers），headers 原样持久化，无需任何运行时凭据注入。

## MessageTransport / Stdio / Http / transport_for()

**文件**：`src/mcp/transport.rs`

- **`MessageTransport` trait** — 三个 async 方法：`round_trip(request: String) -> Result<Value>`（发 JSON-RPC 请求返回解析后 JSON）、`notify(notification: String) -> Result<()>`（发 JSON-RPC notification，无 id、无响应预期）、`shutdown()`（幂等终止）。
- **`StdioMessageTransport`** — 经 `tokio::process::Command` 启动子进程，stdin/stdout 逐行交换 NDJSON。惰性 spawn（`ensure_spawned()`），stderr 转发至日志。
- **`HttpMessageTransport`** — 向 `url` POST JSON-RPC payload，支持 `Accept: application/json, text/event-stream`；维护 `Mcp-Session-Id` header 做会话粘连；响应为 `text/event-stream` 时走 `parse_sse_payload()` 解析 `data:` 行。
- **`transport_for(config)`** — 根据 `McpServerConfig.transport`（enum `Stdio`|`Http`）分发构造。Http 需 `url` 字段，否则报错。

## McpClient::initialize 的 JSON-RPC 交互

**文件**：`src/mcp/client.rs:149-194`

流程：
1. 发 `initialize` request（id=1，protocolVersion `2024-11-05`，clientInfo name=`jcode`）。
2. 解析 `InitializeResult`，写入 `server_info` 和 `capabilities`。
3. 发 `notifications/initialized` notification。

**关键坑**：notifications/initialized 不含 `id` 字段。用 `JsonRpcRequest::new(0, ...)` 会序列化出 `"id": 0`，严格服务端（用 Pydantic discriminated-union 验证）会拒绝，报 `Input should be 'ping'` / `Input should be 'initialize'`。解决：绕过 `JsonRpcRequest`，直接 `serde_json::json!()` 构建不含 `id` 的 payload，经 `transport.notify()` 发送。

## SharedMcpPool / McpHandle / call_tool / refresh_tools

**文件**：`src/mcp/pool.rs`

- **`SharedMcpPool`** — 全局单例（`static SHARED_POOL: OnceCell<Arc<SharedMcpPool>>`），拥有所有 `McpClient` 实例和连接句柄。内部：
  - `clients: Mutex<HashMap<String, McpClient>>` — 真实连接
  - `handles: RwLock<HashMap<String, McpHandle>>` — 克隆句柄
  - `ref_counts` — 会话级引用计数
  - `connecting: Mutex<HashMap<String, Arc<Notify>>>` — 并发连接去重（leader/waiter 模式）
  - `last_errors` — 30 秒冷却记录（`FAILED_CONNECT_RETRY_COOLDOWN`）

- **`McpHandle`** — 轻量 clone，持 `Arc<Box<dyn MessageTransport>>` + atomic request_id + `RwLock<Vec<McpToolDef>>`。`call_tool(name, arguments)`（构造 `tools/call` request，null arguments 转空 object）、`refresh_tools()`（发 `tools/list`，覆写本地 tools 缓存）。

- **公共接口**：`init_shared_pool()`/`get_shared_pool()`/`init_shared_pool_with()`（全局单例管理）、`acquire_handles(session_id)`/`release_handles(session_id, names)`（会话级引用计数）、`call_tool(server, tool, arguments)`（路由到对应 McpHandle）、`connect_all()`/`disconnect_server(name)`/`reload()`（连接生命周期）。

## 关键文件清单

| 路径 | 职责 |
|---|---|
| `src/mcp/mod.rs` | MCP 子系统模块入口，re-export 所有公共类型 |
| `src/mcp/protocol.rs` | JSON-RPC 2.0 类型定义（Request/Response/Error/Initialize/ToolCall/McpConfig/McpServerConfig 等），含 `McpConfig::load()` 多源合并逻辑；`McpServerConfig.transport` enum（Stdio/Http），`url` + `headers` 字段 |
| `src/mcp/transport.rs` | `MessageTransport` trait + Stdio/Http 双实现 + `transport_for()` 工厂 |
| `src/mcp/client.rs` | `McpClient`（拥有 transport）和 `McpHandle`（轻量克隆句柄），含 `initialize()` 全握手 |
| `src/mcp/pool.rs` | `SharedMcpPool` 全局连接池，leader/waiter 并发去重，30s 冷却重连 |
| `src/mcp/manager.rs` | `McpManager` 会话级管理器，区分 shared（pool 复用）与 owned（独占）服务器 |
| `src/mcp/tool.rs` | `McpTool` 实现 `Tool` trait，桥接 MCP ContentBlock 到 jcode ToolOutput |
| `src/mcp/transport_tests.rs` / `protocol_tests.rs` | Http transport 端到端测试 / 协议类型序列化测试 |
| `src/transport/mod.rs` | 平台抽象层入口（`#[cfg(unix)]`/`#[cfg(windows)]` 条件编译） |
| `src/transport/windows.rs` | Windows Named Pipe 实现（路径到 pipe 名称的 SHA256 哈希映射） |
| `src/transport/unix.rs` | Unix Domain Socket re-export（`tokio::net::UnixListener`/`UnixStream`） |
| `crates/jcode-protocol/src/lib.rs` | wire protocol，含 `ServerEvent::McpStatus` 事件 |

## 依赖关系

- 被 [02 Agent](02-agent-runtime.md)（`Registry` 中 MCP tools、`call_tool`）、[04 Server](04-server.md)（MCP pool 初始化、`register_mcp_tools` in `handle_subscribe`）依赖。
- 依赖 [11 Protocol](11-bus-message-protocol.md)（`ServerEvent::McpStatus`）、[08 Storage](08-storage-session.md)（`~/.ssc_tui/` 路径、`jcode_dir`）、[12 Workspace](12-workspace-build-ci.md)（无独立 MCP crate，全在 src/mcp）。

## 陷阱与历史修复

### NDJSON 损坏 → reconnect storm → Unknown tool 链（fixed in `fix/mcp-notification-id`）

**3 bugs，一个 PR 修**：
1. **notifications/initialized with id:0** → Python MCP 拒绝 → tools list 可能不完整。
2. **`RemoteConnection::next_event`**（`src/tui/backend.rs:808-819`）对 ANY JSON parse 失败立即断连 → reconnect storm。
3. **Wire NDJSON 损坏**（Windows named pipes 上大 MCP tool results）→ 触发 bug 2。

**Fixes applied**：
- Fix 1：notifications/initialized 不含 `id` 字段（`src/mcp/client.rs`）。
- Fix 2：跳过坏 NDJSON 行直到连续 10 次错误才断开（`src/tui/backend.rs`）。
- Fix 3：`encode_event` 加 `debug_assert!` 抓内部换行（`crates/jcode-protocol/src/lib.rs:1968`）。
- Fix 4：`ServerEvent` 加 `#[serde(other)] Unknown` variant 做 forward compat（`crates/jcode-protocol/src/lib.rs:1195`）。

**Key takeaway**：reconnect 后的「Unknown tool」错误**不是来自 MCP**——`register_mcp_tools` in `handle_subscribe` 每次都重新获取 pool handles。它们是 AI model 在 disconnect 打断其执行上下文后调用 tools 时漏了 `mcp__` prefix 导致的。

### 其他坑

- **NDJSON 损坏风险**：`StdioMessageTransport::round_trip()` 逐行读 stdout（`Lines::next_line()`），假设每个 JSON-RPC 响应恰好占一行。子进程输出含嵌入换行的 JSON 或 stderr 混入 stdout 会导致 JSON 解析失败。stderr 已独立 spawn 转发至日志，但仍依赖子进程正确分离 fd。
- **Reconnect storm / 冷却机制**：`SharedMcpPool::ensure_connected()` 有 30s 失败冷却；多 session 同时触发重连时，leader/waiter 去重能合并并发请求，但冷却窗口内后续调用直接返回带冷却信息的错误，调用方需处理这种非致命失败。`reload()` 调 `disconnect_all()` + `connect_all()` 对所有服务器全量重连，可能产生瞬时连接风暴。
- **Unknown tool 链**：`McpTool::execute()` 调 `McpManager::call_tool()` 在 pool_handles 和 owned_clients 中查 server name；MCP server 因认证失败未连接但 tools 列表已缓存（来自之前连接）时，LLM agent 仍可能尝试调用这些 tool，得 `"MCP server '<name>' not connected"` 错误。`refresh_tools()` 只在 `connect()` 成功后调用，断连后不主动清空缓存。
- **Windows Named Pipe 的 unsafe 块**：`src/transport/windows.rs` 中 `Stream::split()` 用 `unsafe` 裸指针转换（`&mut self as *mut Stream`）绕过借用检查器，单线程 accept 场景下安全但无文档化 safety invariant。详见 [10-gateway-transport.md](10-gateway-transport.md)。

## 回指

- Agent 如何调 MCP tool（`Registry`/`call_tool`）：[02-agent-runtime.md](02-agent-runtime.md)
- Server MCP pool 初始化与 `handle_subscribe` 重新获取 handles：[04-server.md](04-server.md)
- transport 平台抽象（Unix socket / Windows Named Pipe）：[10-gateway-transport.md](10-gateway-transport.md)
- 用户如何配置自己的 MCP 服务器：`$JCODE_HOME/mcp.json`（type=stdio | type=http+url+headers）
