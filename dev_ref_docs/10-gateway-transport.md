# 10 · Gateway / Transport

> 子系统：WebSocket Gateway（远程 iOS/Web 客户端接入）、Transport 平台抽象（Unix socket / Windows Named Pipe）。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

WebSocket Gateway 将远程客户端（iOS/Web）通过 TCP:7643 WebSocket 连接桥接到服务器内部的 Unix socket 通信协议，使远程客户端与本地 TUI 客户端共用同一套 NDJSON 协议交互 agent session；Transport 子系统提供平台无关的 IPC 抽象（Unix socket / Windows Named Pipe）。

## Gateway 架构

```
远程客户端
  ↓ WebSocket (TCP:7643)
run_gateway() → TcpListener::bind("0.0.0.0:7643")
  ↓ handle_connection() — peek 首包判断是否 Upgrade:websocket
  ├─ 是 WebSocket → handle_ws_connection()
  │    ↓ tokio_tungstenite::accept_hdr_async() — 握手回调中提取 auth
  │    ↓ DeviceRegistry::validate_token() — 验证设备 token
  │    ↓ transport::stream_pair() — 创建虚拟 Unix socket pair
  │    ↓ client_tx.send(GatewayClient{server_stream, device_name, device_id})
  │    ↓ 两个 relay task:
  │       Task 1: WebSocket Text → write_all(bridge_stream) [WebSocket→Unix]
  │       Task 2: read_line(bridge_stream) → WebSocket Text [Unix→WebSocket]
  │       + keepalive task: 每 20s 发 Ping
  └─ 不是 WebSocket → handle_http()
       ├─ GET /health → server status JSON
       ├─ POST /pair → 配对码换 token 流程
       └─ OPTIONS → CORS preflight

ServerRuntime::spawn_gateway_accept_loop()
  ↓ 从 client_rx channel 接收 GatewayClient
  ↓ spawn_gateway_client_task()
  ↓ run_client_stream(gw_client.stream, ...) → handle_client()
     ↑ 与本地 Unix socket 客户端完全相同的处理路径
```

关键点：gateway client 的 `server_stream` 端被当作普通 `Stream` 送入 `handle_client()`，server 侧完全不感知它是 WebSocket 桥接过来的。

## 远程客户端接入与虚拟 Unix socket pair

**接入方式**：
1. **配对流程**：客户端向 `POST /pair` 发 `{code, device_id, device_name, apns_token}` JSON，服务端验证 6 位配对码（`jcode pair` CLI 生成，5 分钟有效），返回 64 字节 hex auth token + server_name + server_version。
2. **WebSocket 连接**：客户端连 `ws://host:7643/ws`，携带 token（Authorization header 或 `?token=` query param）。
3. **通信**：连接建立后双向传输 WebSocket Text 帧，每帧一行 JSON（NDJSON 格式），与本地 Unix socket 客户端协议完全相同。

**虚拟 Unix socket pair 机制**：
- `transport::stream_pair()` 在 Unix 上调 `tokio::net::UnixStream::pair()`（内核级 socketpair），在 Windows 上经 `NamedPipeServer` + `NamedPipeClient` 模拟。
- 一对 stream：`server_stream` → 送给 `handle_client()`；`bridge_stream` → gateway relay task 做 WebSocket↔NDJSON 转译。
- WebSocket Text 帧到达 bridge_stream writer 时自动追加 `\n`（若缺失），保证 NDJSON 帧边界。

## 认证方式

**Bearer Token（优先，推荐）**：`Authorization: Bearer <64-char-hex-token>` HTTP header，在 WebSocket 握手回调 `extract_ws_auth()` 中提取。

**Query Param（已 deprecated，兼容浏览器客户端）**：`?token=<64-char-hex-token>` URI query parameter。若 Header 和 Query 同时存在但不一致，拒绝连接（401）。

**Token 验证**：`DeviceRegistry::validate_token()` 将传入 token 做 SHA256 哈希，与 `~/.jcode/devices.json` 中存储的 `sha256:<hex>` 比对。token 本身不持久化，只存哈希值（安全设计）。

**配对码**：6 位数字，5 分钟过期，存 `devices.json` 的 `pending_codes` 数组，验证后立即 consume。

## Transport 子系统职责

**一句话**：提供平台无关的 IPC 传输层抽象，将 Unix Domain Socket 和 Windows Named Pipe 统一为相同接口（`Listener`/`Stream`/`ReadHalf`/`WriteHalf`/`SyncStream`），供 server socket accept loop、gateway bridge、debug socket 共用。

**Unix 实现**（`src/transport/unix.rs`）：直接 re-export `tokio::net::UnixListener`/`UnixStream` 等；`stream_pair()` 调 `UnixStream::pair()`（内核 socketpair，零拷贝，最高效）；`is_socket_path()` 检查文件存在。

**Windows 实现**（`src/transport/windows.rs`）：
- `Listener`：封装 `NamedPipeServer`，`accept()` 后立即创建下一个 server 实例（类似 Unix accept 语义）。
- `Stream`：枚举 `Server(NamedPipeServer)`|`Client(NamedPipeClient)`，实现 `AsyncRead`/`AsyncWrite`。
- `stream_pair()`：用 `AtomicU64` 计数器生成唯一 pipe 名 `\\.\pipe\jcode-pair-{pid}-{counter}`，手动 poll server connect future（client 已同步连接，server connect 首次 poll 即完成）。
- `into_split()` 用 `Arc<Mutex<Stream>>` 包装（Named Pipe 不支持原生 split），borrowed split 用 unsafe raw pointer。
- `SyncStream`：用 `std::fs::File` 打开 named pipe，供阻塞 IPC 场景。
- `path_to_pipe_name()`：将文件路径哈希（SHA256，取前 16 hex chars）生成稳定 pipe 名。

## 关键文件清单

| 文件路径 | 职责 |
|---|---|
| `src/gateway.rs` | WebSocket gateway 入口：TCP accept、WebSocket upgrade、auth、Unix socket pair bridge、HTTP /health /pair 端点 |
| `src/gateway/auth.rs` | WebSocket 握手认证：Bearer header / query param 提取、hex token 校验、错误响应 |
| `src/gateway/registry.rs` | 设备注册表：配对码生成/验证、设备注册、token SHA256 验证、`~/.jcode/devices.json` 持久化 |
| `src/gateway_tests.rs` | Gateway 单元测试 |
| `src/transport/mod.rs` | Transport 平台分发：cfg(unix)/cfg(windows) 条件编译入口 |
| `src/transport/unix.rs` | Unix transport：re-export tokio UnixStream/UnixListener、stream_pair()、socket path 工具 |
| `src/transport/windows.rs` | Windows transport：Named Pipe 封装，模拟 Unix socket 接口 |
| `src/server/socket.rs` | Server socket 管理：socket path 计算、connect/accept 工具、daemon lock、server ready probe、spawn+notify 子进程 |
| `src/server/runtime.rs` | Server 运行时：三个 accept loop（main/debug/gateway），将 GatewayClient 送入 handle_client() |
| `src/server/client_lifecycle.rs` | 客户端生命周期：handle_client() 主循环，NDJSON 读写，请求分发 |
| `src/channel.rs` | 消息通道抽象（Telegram/Discord），与 gateway 无关但同属 server 通信层 |
| `crates/jcode-gateway-types/src/lib.rs` | Gateway 类型定义：`PairedDevice`/`PairingCode`（共享数据结构） |
| `crates/jcode-protocol/src/lib.rs` | NDJSON 协议定义：`Request`/`ServerEvent` 枚举、`encode_event()`、`decode_request()` |

## NDJSON 协议在 transport 层的角色

Transport 层本身不感知协议内容——纯字节流传输。NDJSON 协议逻辑在 `client_lifecycle.rs` 的 `handle_client()` 中实现：
- **读取**：`BufReader::read_line()` 逐行读 `bridge_stream`（或本地 Unix socket），每行一条 JSON。
- **写入**：`encode_event()` 将 `ServerEvent` 序列化为 JSON + `\n`，`writer.write_all()` 写入 stream。
- Gateway relay task 中，WebSocket Text 帧 ↔ NDJSON 行转换发生在 bridge stream 两侧：WebSocket→Unix 收 Text 帧后 `write_all(text + "\n")`；Unix→WebSocket `read_line()` 读取 trim 后作为 `Message::Text` 发回。

**与 `crates/jcode-protocol` 的关系**：`jcode-protocol` 定义协议 schema（`Request` client→server 60+ 种、`ServerEvent` server→client 40+ 种，均 `#[serde(tag = "type")]` 内部标签枚举）；`encode_event()`/`decode_request()` 是唯一序列化入口，确保 NDJSON 帧完整性（assert 无内部换行）。Gateway 客户端与本地客户端看到完全相同的协议，区别仅在传输层（WebSocket 帧 vs Unix socket 字节流）。

## 依赖关系

- 被 [04 Server](04-server.md)（accept loop / `handle_client` 经 `Stream`）、[09 MCP/SAITEC](09-mcp-saitec.md)（MCP transport 平台抽象）依赖。
- 依赖 [11 Protocol](11-bus-message-protocol.md)（NDJSON wire 格式）、[08 Storage](08-storage-session.md)（`devices.json` 持久化、`jcode_dir`）、[12 Workspace](12-workspace-build-ci.md)（`jcode-gateway-types`）。

## 陷阱与设计约束

- **Windows `Stream::split()` 使用 unsafe raw pointer**（`src/transport/windows.rs:121-128`）：`&mut self` 转为两个独立 `SplitReadRef`/`SplitWriteRef` 绕过借用检查。同时两 task 中使用 borrowed split（而非 `into_split()`）会导致 UB。代码主要用 `into_split()`（`Arc<Mutex>` 版本），borrowed split 主要用于单 task 内。
- **Windows `stream_pair()` 手动 poll 用 dummy waker**（`:148-168`）：构造 null pointer 的 `RawWaker`。首次 poll 即 Ready 场景下安全，但 Named Pipe 行为异常返回 Pending 时 dummy waker 不会唤醒 task——注释说「Should not happen」但仍是潜在风险。
- **Query param auth 已 deprecated 但仍接受**：`auth.rs:180-185` 仅 log 警告不拒绝。安全敏感场景下，query param 中 token 会出现在 URL 日志、Referer header、浏览器历史中。
- **DeviceRegistry 每次连接都从磁盘 reload**（`gateway.rs:189-191`）：`*reg = DeviceRegistry::load()` 在每个 WebSocket 连接时重新读 `~/.jcode/devices.json`。高频连接场景下可能成瓶颈。pair 请求同样 reload（`:438`）。
- **Token 验证非 constant-time 比较**（`registry.rs:120`）：`self.devices.iter().find(|d| d.token_hash == token_hash)` 普通字符串比较，理论上 timing attack 风险（实际场景中网络延迟基本不可利用）。
- **Gateway 默认 disabled**：`GatewayConfig::default()` 中 `enabled: false`，需显式启用。可能导致用户配对后连接被拒（未意识到需手动开启 gateway）。
- **`handle_http()` 手工拼 HTTP 响应**（`gateway.rs:323-328`）：`http_response()` 用 `format!()` 拼 HTTP 报文，无 HTTP 框架。简单但后续需支持 chunked encoding 或更复杂 HTTP 特性时难维护。
- **配对码 6 位数字暴力破解风险**：10^6 = 100 万种可能，5 分钟窗口内若无 rate limiting 理论上可被暴力枚举。代码中未见 `/pair` 端点 rate limit 实现。

## 回指

- server 侧 accept loop 与 `handle_client`：[04-server.md](04-server.md)
- NDJSON wire 格式（`encode_event`/`decode_request`/`ServerEvent`/`Request`）：[11-bus-message-protocol.md](11-bus-message-protocol.md)
- MCP transport 平台抽象复用：[09-mcp-saitec.md](09-mcp-saitec.md)
