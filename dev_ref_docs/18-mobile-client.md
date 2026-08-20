# 18 · Mobile Client

> 子系统：iOS 原生客户端（SwiftUI 外壳 + Rust 应用核心共享层）、Mobile Agent Simulator（Linux 原生模拟器）、jcode-mobile-core 共享 Rust 状态机、jcode-mobile-sim 模拟器守护进程。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

Mobile Client 是 jcode 在移动端的远程控制面，通过 Tailscale 加密隧道 + 本地 WebSocket Gateway 连接笔记本/台式机上的 jcode server。移动端不运行 agent、不执行工具、不访问文件系统——所有重负载留于 server。jcode-mobile-core 为 Rust 共享状态机，在 Linux App Simulator 和 iOS 原生 App 中运行相同代码；iOS App 仅为平台外壳，Swift 只负责渲染和平台 API 桥接。

## Mobile 生态全景

```
┌─────────────────────────────────────────────────────────────────┐
│                      Linux App Simulator                         │
│  jcode-mobile-sim (守护进程)                                      │
│    - 自动化 API (Unix socket)                                     │
│    - fake jcode backend                                           │
│    - 视觉渲染外壳 (可选)                                           │
│    - 回放/黄金测试框架                                             │
│                                                                   │
│  jcode-mobile-core (Rust 共享核心)                                 │
│    - AppState / Action / Effect / Reducer                         │
│    - 语义 UI 树                                                  │
│    - protocol 适配器                                              │
│    - scenario 夹具                                                │
│    - replay                                                       │
└────────────────────────┬────────────────────────────────────────┘
                         │ (通过 FFI C ABI + JSON)
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                      iOS 原生 App (iPhone/iPad)                   │
│  SwiftUI 渲染外壳                                                │
│    - 平台外壳: 窗口、触摸输入、Keychain、APNs                     │
│    - 摄像头/照片选择器、语音识别、触感反馈                         │
│                                                                   │
│  Rust FFI 桥接 (jcode-mobile-ffi)                                 │
│    - opaque app handle + JSON dispatch/state/tree                  │
└────────────────────────┬────────────────────────────────────────┘
                         │ (Tailscale WireGuard P2P)
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                      jcode Server (Laptop/Desktop)                │
│  WebSocket Gateway (TCP:7643)                                     │
│    - POST /pair → 6 位配对码换 auth token                        │
│    - GET /health → server status                                 │
│    - ws://host:7643/ws → WebSocket ↔ Unix socket bridge          │
│    - 设备注册表: ~/.ssc_tui/devices.json                           │
│    - APNs push 发送器                                             │
└─────────────────────────────────────────────────────────────────┘
```

## 网络架构：Tailscale 优先

iOS 客户端通过 **Tailscale** 作为主要传输层连接 jcode server。

```
iPhone (Tailscale App)          Tailscale 网络 (WireGuard Mesh)      Laptop (tailscaled)
       │                                    │                           │
       │  jcode iOS app -> laptop.ts.net:7643                           │
       │────────────────── WireGuard 加密隧道 ─────────────────────────►│
       │                                                                │
       │◄───────── WebSocket (明文，隧道已加密) ───────────────────────►│
```

**设计理由**：
- 任意位置可用（家庭、咖啡厅、蜂窝网络、跨国）
- 自带加密（WireGuard），无需 TLS 证书管理
- 稳定 hostname（`laptop.tail1234.ts.net`），网络切换不中断
- NAT 穿透自动处理
- Tailscale 有原生 iOS 应用，手机已在网络上

**回退方案**（非主要路径）：
- 手动 IP/hostname（需要 TLS，自签名或 Let's Encrypt）
- LAN Bonjour/mDNS（未来可能，但企业/访客 WiFi 不稳定，仅同网段可用）

**无云中转需要**：Tailscale 是 P2P 的，流量直接对等。

## 认证流程（配对）

```
1. 用户在 server 终端运行: jcode pair
   → Server 生成 6 位配对码（5 分钟有效，存 devices.json pending_codes）
   → 显示在终端

2. 用户在 iOS 应用中输入配对码
   → 应用向 POST /pair 发 {code, device_id, device_name, apns_token}
   → Server 验证配对码，生成 64 字节 hex auth token
   → 返回 {auth_token, server_name, server_version}
   → Token 存 iOS Keychain

3. 所有后续 WebSocket 连接使用 Bearer token
   → Authorization: Bearer <64-char-hex-token>
   → Server 验证: SHA256(token) 比对 ~/.ssc_tui/devices.json 存储的哈希
   → Token 本身不持久化，只存哈希值（安全设计）
```

## 推送通知（APNs）

```
jcode Server (笔记本)              Apple APNs (Apple 云)           iPhone (jcode App)

Event 触发 ───► HTTP/2 POST ────► 路由推送 ─────► 🔔 原生推送通知
               APNs 请求             到设备               jcode 应用中
               (device token + JWT 签名)
```

推送值得的事件：
- Task/message 完成（agent 完成一个 turn）
- Tool 审批请求（安全系统 Tier 2 操作）— 可操作的锁屏通知
- Ambient cycle 完成（带摘要）
- Server 下线/上线
- Swarm 任务分配给您

Rich 推送特性：
- 可操作通知：锁屏直接批准/拒绝工具调用
- Live Activities：锁屏和 Dynamic Island 显示任务进度
- 通知分组：按 session 分组
- 静默推送：后台更新 app 状态而不提醒用户
- 关键提醒：安全 tier 需立即关注的操作

## Rust 共享应用核心架构

### Rust 核心负责（仿真器与 iOS 一致的行为）

- onborading 和配对流程状态
- server 列表和选中 server 状态
- 连接生命周期状态机
- 聊天会话状态
- 消息流式传输和文本替换行为
- 工具调用显示和审批状态
- model/session 切换状态
- 离线队列状态
- 错误横幅和恢复流程
- 语义 UI 树构建
- 确定性布局和命中测试元数据
- protocol 序列化/反序列化
- 可回放效果

### 平台外壳专属（iOS 或 Linux）

- 创建窗口或 iOS view
- 通过指定渲染器/后端绘制
- 安全 token 存储实现
- 剪贴板集成
- 摄像头/照片选择器
- 麦克风/语音集成
- 推送通知注册
- 触感反馈
- OS 生命周期事件

### FFI 边界（JSON 优先）

```c
void *jcode_mobile_app_new(const char *initial_scenario_json);
void jcode_mobile_app_free(void *app);
char *jcode_mobile_dispatch(void *app, const char *action_json);
char *jcode_mobile_state(void *app);
char *jcode_mobile_tree(void *app);
char *jcode_mobile_logs(void *app, uint32_t limit);
void jcode_mobile_string_free(char *ptr);
char *jcode_mobile_platform_event(void *app, const char *event_json);
```

Why JSON first: 易于 Xcode log 检查、与模拟器 trace 兼容、易于 fuzz 和 replay、模型演进时弹性好。

### Rust Effect 模型

Rust 发出 effects，由平台外壳执行：

```json
{ "type": "secure_store_write", "key": "server_token", "value": "..." }
{ "type": "websocket_connect", "url": "ws://host:7643/ws", "auth_token": "..." }
{ "type": "register_push_notifications" }
{ "type": "request_camera_qr_scan" }
{ "type": "haptic", "style": "success" }
```

平台返回事件：

```json
{ "type": "pair_finished", "ok": true, "token": "...", "server_name": "jcode" }
{ "type": "websocket_event", "event": { "type": "text_delta", "text": "hello" } }
{ "type": "qr_payload_scanned", "payload": "jcode://pair?... " }
```

Linux 模拟器的 fake backend 应能生成相同事件形状。

## Mobile Agent Simulator

### 核心概念

- **App Simulator**：Linux 原生、agent 可控的 jcode 移动端应用模拟器。
- **Apple iOS Simulator**：Apple 的 Xcode-hosted 模拟器，仅用于后期平台验证。
- **Mobile Core**：共享 Rust 状态、actions、effects、protocol 适配器、业务逻辑、语义 UI。
- **Platform Shell**：薄 iOS/Linux host 提供 OS 能力（窗口、安全存储、通知、麦克风、摄像头、触感）。
- **Semantic UI Tree**：确定性 agent 面向的可见应用表面表示。
- **Scenario**：以已知状态和 fake backend 行为启动应用的确定性夹具。

### 目标

1. 在 Linux 上从普通 checkout 运行移动端应用体验。
2. 运行 iOS app 中内置的相同 Rust 应用核心。
3. 让 AI agent 以人类可用的所有方式自主测试：检查、点击、输入、滚动、手势、等待、断言、截图、布局比较、回放故障。
4. 日复一日的迭代不依赖 Mac 硬件、Xcode、Apple iOS Simulator 或物理 iPhone。
5. 将原生 iOS 专属逻辑隔离在小型平台外壳接口后。

### 架构

```
jcode-mobile-core (Rust)
  - AppState / Action / Effect / Reducer
  - Protocol 适配器
  - 语义 UI 树 / 布局 / 命中测试模型

jcode-mobile-sim (Linux)
  - Simulator 守护进程 (Unix socket 自动化协议)
  - Agent 自动化 API (state/tree/tap/type_text/scroll/assert 等)
  - Fake jcode backend (health/pairing/WebSocket/sessions/deltas/tools/errors/reconnect)
  - Visual shell (可选 Linux 渲染)
  - Screenshot/layout export
  - Replay/golden harness

AI Agents & Tests
  - sim CLI
  - jcode debug/tester integration
  - Linux CI
```

### Agent 自动化 API

语义操作：`state`, `tree`, `find_node`, `tap_node`, `type_text`, `set_field`, `scroll_node`, `assert_screen`, `assert_text`, `assert_node`, `assert_no_error`, `wait_for`, `load_scenario`, `replay`

人类操作：`tap_xy`, `drag_xy`, `key_press`, `paste`, `scroll_delta`, `screenshot`, `hit_test`

调试操作：`transition_log`, `effect_log`, `network_log`, `storage_snapshot`, `fault_inject`, `export_replay`, `shutdown`

### 当前可用场景

- `onboarding`、`pairing_ready`、`connected_chat`、`pairing_invalid_code`、`server_unreachable`
- `connected_empty_chat`、`chat_streaming`、`tool_approval_required`、`tool_failed`
- `network_reconnect`、`offline_queued_message`、`long_running_task`

### Agent 工作流核心循环

```bash
# 1. 启动或重置模拟器
cargo run -p jcode-mobile-sim -- start --scenario pairing_ready

# 2. 检查状态
cargo run -p jcode-mobile-sim -- state
cargo run -p jcode-mobile-sim -- tree

# 3. 驱动交互
cargo run -p jcode-mobile-sim -- set-field host devbox.tailnet.ts.net
cargo run -p jcode-mobile-sim -- set-field pair_code 123456
cargo run -p jcode-mobile-sim -- tap pair.submit

# 4. 断言
cargo run -p jcode-mobile-sim -- assert-screen chat
cargo run -p jcode-mobile-sim -- assert-text "Connected to simulated jcode server."
cargo run -p jcode-mobile-sim -- assert-node chat.send --enabled true --role button
cargo run -p jcode-mobile-sim -- assert-no-error

# 5. 失败时检查日志
cargo run -p jcode-mobile-sim -- log --limit 20
```

## 既存 Swift 代码审计关键结论

当前 Swift 原型拥有过多应用行为，需迁移至 Rust：

| 当前所在 (Swift) | 迁移目标 (Rust) |
|---|---|
| AppModel.swift: app state/connection/pairing/chat | jcode-mobile-core: MobileAppState/Action/Effect/Reducer |
| Connection.swift: lifecycle/reconnect/protocol | jcode-mobile-core: state machine + protocol adapter |
| Protocol.swift: request/event 枚举定义 | jcode-protocol: 共享 wire 类型 |
| Pairing.swift: 验证/错误分类 | jcode-mobile-core: pairing state/reducer |
| CredentialStore.swift: 数据模型/选择/移除 | jcode-mobile-core: storage_model |
| ToolCallInfo: tool-call 状态转换 | jcode-mobile-core: tools reducer |

Swift 保留：
- iOS 视图/窗口宿主 + SwiftUI 渲染外壳
- Keychain 凭据存储实现
- 摄像头/照片选择器
- 二维码相机捕获
- 语音识别桥接
- 推送通知注册
- 触感和 OS 生命周期
- FFI 胶水代码

## 关键文件清单

| 文件路径/模块 | 职责 |
|---|---|
| `crates/jcode-mobile-core/src/` | Rust 共享应用核心：state/action/effect/reducer/protocol/chat/tools/pairing/connection/semantic_ui/layout/scenario/replay |
| `crates/jcode-mobile-sim/src/` | Linux 模拟器：守护进程、自动化 API、fake backend、CLI、回放执行 |
| `crates/jcode-mobile-ffi/` (计划) | C ABI + JSON 桥接：opaque app handle、dispatch/state/tree/logs |
| `src/gateway.rs` | WebSocket Gateway：TCP accept、WebSocket upgrade、auth、Unix socket pair |
| `src/gateway/auth.rs` | WebSocket 握手认证：Bearer header/query param、token 校验 |
| `src/gateway/registry.rs` | 设备注册表：配对码生成/验证、设备注册、token SHA256 验证 |
| `ios/Sources/JCodeKit/` | Swift 协议层 SDK（networking + protocol layer） |
| `ios/Sources/JCodeMobile/` | SwiftUI app shell（配对 + 聊天 UI） |
| `ios/Sources/JCodeMobile/AppModel.swift` | Swift 应用状态模型（待迁移至 Rust） |
| `ios/Sources/JCodeKit/Protocol.swift` | Protocol 请求/事件定义（待迁移至 Rust） |

## 依赖关系

- 依赖 [10 Gateway/Transport](10-gateway-transport.md)：iOS 通过 `src/gateway.rs` 接入，共享 NDJSON 协议。
- 依赖 [04 Server](04-server.md)：server 端的 accept loop / `handle_client` 将 gateway client 作为普通 Stream 处理。
- 依赖 [11 Protocol](11-bus-message-protocol.md)：共享 `Request`/`ServerEvent` NDJSON 协议定义。
- 依赖 [08 Storage](08-storage-session.md)：`devices.json` 持久化、`jcode_dir` 路径。
- 被 [07 Memory](07-memory.md)（ambient/memory 操作可经移动端查看）。
- 被 [16 Overnight](16-overnight.md)（ambient 状态推送、安全系统 Tier 2 通知经 APNs）。

## 陷阱与设计约束

- **手机不执行 agent 工具**：iOS sandbox 限制使工具执行（shell、filesystem、MCP）不可能，server 必须始终存在。
- **Tailscale 强依赖**：大多数用户已使用 Tailscale，但没有 Tailscale 时需 fallback TLS。是否应要求 Tailscale 还是早期投资非 Tailscale 路径是未定决策。
- **Rust-first 而非 Swift-first**：当前 Swift 原型拥有过多应用状态，正在迁移至 Rust。迁移完成前不应在 Swift 中增加新应用行为。
- **模拟器优先于真机**：每项新 iOS 行为应先能在 Linux 模拟器中测试，无需 Mac/Xcode/iPhone。
- **JSON FFI 桥接是起点不是终点**：早期用 JSON 便于调试，但高频路径最终应迁移到 typed binary ABI（uniffi 或其他绑定生成器）。
- **配对码暴力破解风险**：10^6 种可能的 6 位数字码，5 分钟窗口内若无 rate limiting 理论上可暴力枚举（当前代码未见 rate limit）。
- **Gateway 默认 disabled**：GatewayConfig::default() 中 enabled: false，用户可能不知需手动开启。
- **APNs 依赖 Apple 开发者帐号**：生产性 APNs 集成需要付费 Apple Developer Program（$99/年），开发期可 sideload（但 7 天过期）。

## 回指

- WebSocket Gateway 架构（TCP accept、auth、Unix socket pair）：[10-gateway-transport.md](10-gateway-transport.md)
- NDJSON 协议定义（Request/ServerEvent）：[11-bus-message-protocol.md](11-bus-message-protocol.md)
- Server 端 accept loop 和 handle_client：[04-server.md](04-server.md)
- Desktop App 架构（另一客户端参考）：[17-desktop-app.md](17-desktop-app.md)
- 无人值守运行与通知（ambient 状态 / Tier 2 审批）：[16-overnight.md](16-overnight.md)
