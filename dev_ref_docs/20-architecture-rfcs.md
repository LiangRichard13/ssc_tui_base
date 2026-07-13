# 20 · 架构规划 RFC 与拆分方案

> 子系统：模块化架构规划、Client-Core/Presentation 拆分、多会话客户端、Server/Service 拆分、crate 拆分计划。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

本文件收录 jcode 项目中已提出或部分实施的架构规划文件——包括模块化架构 RFC、Client-Core/Presentation 拆分方案、多会话客户端架构、Server/Service 拆分方案、统一 Selfdev Server 方案以及 crate 拆分计划。标注每个方案的实施状态（Implemented / In Progress / Proposed）。

## 依赖关系

**内部文档**：
- [00-overview-and-entry.md](00-overview-and-entry.md) — workspace 全景
- [02-agent-runtime.md](02-agent-runtime.md) — Agent runtime 细节
- [03-provider.md](03-provider.md) — Provider 子系统
- [04-server.md](04-server.md) — Server 运行时
- [05-tui.md](05-tui.md) — TUI 客户端
- [12-workspace-build-ci.md](12-workspace-build-ci.md) — workspace 与 CI

**源文档**：
- `docs/MODULAR_ARCHITECTURE_RFC.md` — 模块化架构 RFC
- `docs/CLIENT_CORE_PRESENTATION_SPLIT_PLAN.md` — Client-Core/Presentation 拆分方案
- `docs/MULTI_SESSION_CLIENT_ARCHITECTURE.md` — 多会话客户端架构
- `docs/SERVER_SERVICE_SPLIT_PLAN.md` — Server/Service 拆分方案
- `docs/UNIFIED_SELFDEV_SERVER_PLAN.md` — 统一 Selfdev Server 方案（已实施）
- `docs/dev/crate-splitting-plan.md` — crate 拆分计划

---

## 1 · 模块化架构 RFC

> **状态：Proposed**（部分已在实施中）
> 源文档：`docs/MODULAR_ARCHITECTURE_RFC.md` (927 行)

### 职责一句话

定义 jcode 从今天的"带 workspace shell 的模块化单体"演进为分层 workspace 的目标架构、依赖规则、crate 职责和分阶段迁移计划。

### 当前架构现状

**Runtime 模型**已经清晰：单 server、多 client。server 拥有 sessions、swarm state、background tasks、provider state；client 主要是 TUI 前端。

**代码组织**是混合态：
- **根 crate `jcode`** 仍包含大部分产品逻辑
- **Workspace crates** 已隔离一些重型或稳定的 seam
- **`src/` 下子目录**越来越多反映领域边界（agent、cli、server、tool、tui）

当前 workspace 成员（来自 `Cargo.toml`）：
- 根 package: `jcode`
- foundation/runtime support: `jcode-agent-runtime`, `jcode-core`, `jcode-storage`, `jcode-terminal-launch`, `jcode-tool-core`
- data-contract crates: 约 14 个 `jcode-*-types` crate
- protocol: `jcode-protocol`, `jcode-plan`
- heavy/optional: `jcode-embedding`, `jcode-pdf`, `jcode-notify-email`
- auth/provider: `jcode-azure-auth`, `jcode-provider-core`, `jcode-provider-metadata`, `jcode-provider-openrouter`, `jcode-provider-gemini`
- TUI extraction: `jcode-tui-core`, `jcode-tui-markdown`, `jcode-tui-mermaid`, `jcode-tui-render`, `jcode-tui-workspace`
- product surfaces: `jcode-desktop`, `jcode-mobile-core`, `jcode-mobile-sim`

**根 crate 仍拥有的主要责任**：CLI 解析与分派、server 编排、session 持久化、agent turn 执行、provider 实现组合、protocol/message/config type、tool registry、TUI app 状态、auth/memory/safety/ambient。

**当前 chokepoints（可观测行数）**：
- `src/provider/mod.rs`: ~2283 行
- `src/session.rs`: ~2730 行
- `src/server.rs`: ~1731 行
- `src/protocol.rs`: ~1198 行

### 目标架构：分层 workspace

```
Layer 2: Interfaces / Product Surfaces
  jcode-tui, jcode-selfdev, jcode-cli (或 CLI 留在根 crate)

Layer 1: Domain / Runtime
  jcode-server, jcode-agent, jcode-provider, jcode-session

Layer 0: Foundation / Support
  jcode-core, jcode-*-types, jcode-agent-runtime, jcode-embedding, jcode-pdf,
  jcode-provider-core, jcode-provider-*, jcode-azure-auth, jcode-notify-email,
  jcode-tui-workspace, jcode-tui-*, jcode-terminal-launch
```

**重要**：依赖方向只能从上到下，不能反向。合同/type crate 只能依赖 serde 和小型工具 crate，不能依赖 tokio、reqwest、ratatui、provider SDK。

### 目标 crate 职责

| Crate | 应包含 | 不应包含 |
|---|---|---|
| `jcode-core` | IDs、protocol DTOs、message/content types、config primitives、小工具 | TUI、server lifecycle、provider network、tokio task orchestration |
| `jcode-session` | session model、persistence、state transitions、memory hooks | socket、TUI、provider HTTP |
| `jcode-provider` | Provider trait、routing、composition、streaming abstractions | provider-specific heavy catalogs、server/TUI logic |
| `jcode-agent` | turn-loop engine、stream handling、tool orchestration、compaction | server socket、TUI、provider leaf impl |
| `jcode-server` | socket listeners、client lifecycle、swarm coordination、reload | TUI、provider impl、session persistence internals |
| `jcode-tui` | app state & reducers、remote client behavior、renderer | server daemon、session persistence、provider network |
| `jcode-selfdev` | self-dev workflows、build/reload orchestration | generic server lifecycle、TUI rendering |

### 10 条依赖规则

1. **依赖向下流动**：高层可依赖低层，低层不可依赖高层
2. **TUI type 不能进入 interface 层以下**：ratatui/crossterm 不能出现在 server/agent/provider/core
3. **Server daemon type 不能在 core/provider-support 中**：socket/session attachment 代码不能进入 jcode-core/jcode-provider-core
4. **Provider impl crate 只能依赖 contract，不能依赖 server/TUI**
5. **Async/network-heavy 依赖不属于 jcode-core**
6. **稳定 contract 变化速度应慢于编排代码**
7. **避免 cross-cutting "utils" crate**
8. **根 package 可以组合多个 crate，但 peer crate 应保持狭窄**
9. **新 crate 边界应同时考虑所有权和编译失效**
10. **迁移期间用 facade 保持行为**

### 分阶段迁移计划

| 阶段 | 目标 | 状态 |
|---|---|---|
| Phase 0: Codify architecture | 编写本 RFC，建立跨文档链接，记录依赖规则 | **Done**（本文件） |
| Phase 1: Finish internal module decomposition | 继续 CLI 分解、Server.rs 瘦身、Agent turn-loop 统一、TUI state/reducer 分离 | **In Progress** |
| Phase 2: Extract `jcode-core` | IDs、小型 protocol DTOs、tool definition、config primitives | **Planned** |
| Phase 3: Extract runtime/domain crates | jcode-provider, jcode-agent, jcode-server, jcode-session | **Planned** |
| Phase 4: Extract `jcode-tui` | app/reducer/rendering 移出根 crate | **Planned** |
| Phase 5: Extract `jcode-selfdev` | self-dev workflow 隔离 | **Planned** |
| Phase 6: Shrink root to composition shell | src/main.rs 保持薄，jcode::run() 主要是 wiring | **Planned** |

### Split Readiness Checklist

一个根模块准备好成为 crate 的条件：
- 其 public API 可以用不到一页描述
- 不需要回调任意根模块
- 依赖要么是低层 contract，要么是自有 leaf adapter
- 测试可在 crate 级运行，无需启动完整产品
- 触碰文件基准测试显示它在有意义的失效路径上
- 在根 crate 中有稳定 facade 用于迁移兼容

### 最高 ROI 的下一个 crate seam

1. Provider contracts — 持续缩小 `src/provider/mod.rs`
2. Server core — 提取与协议无关的客户端生命周期状态机
3. TUI reducer/state core — 提取非渲染的 app state transitions
4. Tool contracts — 分离 tool definitions 与 tool implementations
5. Session domain — 隔离 session state transitions
6. Auth facade — 将 provider-neutral auth data 保持在 jcode-auth-types

### 实施状态清单

| Item | Status |
|---|---|
| 已有 38 个 workspace crate（含 type crates） | **Implemented** |
| CLI 分解进行中 | **In Progress** |
| `jcode-storage` 已提取为 leaf crate | **Implemented** |
| `jcode-provider-core` 已提取 | **Implemented** |
| `src/server/state.rs` 等子模块提取 | **In Progress** |
| TUI state/reducer 分离 | **Planned** |
| `jcode-core` 提取 | **Planned** |

---

## 2 · Client-Core / Presentation 拆分方案

> **状态：Proposed**
> 源文档：`docs/CLIENT_CORE_PRESENTATION_SPLIT_PLAN.md` (877 行)

### 职责一句话

审计当前 TUI/client 栈，提出在可复用 `client-core` 层与 ratatui/crossterm presentation 层之间进行安全的增量拆分。

### 当前痛点

**`App` 对象过重且语义混杂**：`src/tui/app.rs` 中的 `App` 混合了 runtime handles、conversation/session data、composer/input state、turn execution state、streaming state、remote client state、workspace state、surface-local UI state、config/feature toggles。

**无 typed action/reducer 边界**：主要 reducer 是隐式的（`local.rs::handle_tick`、`remote/server_events.rs::handle_server_event` 等）。

**Workspace state 是 process-global**：`src/tui/workspace_client.rs` 用 `static WORKSPACE_STATE: Mutex<Option<...>>`，与多客户端实例不兼容。

**Render layer 依赖全局变量**：`LAST_MAX_SCROLL`、`PINNED_PANE_TOTAL_LINES` 等应该是 renderer-instance state。

**Runtime loops 与 rendering 紧密交织**：`terminal.draw(...)` 出现在多个控制流路径。

### 目标结构

```
client-core (instance-owned client state + reducers + effects)
  ├── ConversationState
  ├── ComposerState
  ├── TurnState
  ├── StreamState
  ├── RemoteState
  ├── WorkspaceState (必须从全局静态变为实例所有)
  ├── SurfaceState
  ├── FeatureState
  └── NoticeState

presentation (ratatui widgets, layout, drawing, render caches)
  ├── ui.rs / ui_*.rs
  ├── markdown*.rs / mermaid*.rs
  ├── session_picker*.rs / info_widget*.rs
  └── 其他 render-time 模块
```

### 提议的 8 个 State 类型

| State | 文件提议 | 主要责任 |
|---|---|---|
| `ClientCoreState` | `client_core/state/mod.rs` | 顶层，聚合所有子 state |
| `ConversationState` | `client_core/state/conversation.rs` | messages, display_messages, tool output tracking |
| `ComposerState` | `client_core/state/composer.rs` | input, cursor, queueing |
| `TurnState` | `client_core/state/turn.rs` | is_processing, failover, lifecycle |
| `StreamState` | `client_core/state/stream.rs` | streaming_text, tokens, TPS |
| `RemoteState` | `client_core/state/remote.rs` | remote session, reconnect, queue recovery |
| `WorkspaceState` | `client_core/state/workspace.rs` | workspace map, enabled, imported sessions |
| `SurfaceState` | `client_core/state/surface.rs` | scroll, selection, pane focus |
| `FeatureState` | `client_core/state/features.rs` | memory/swarm/diff/centered toggles |
| `NoticeState` | `client_core/state/notices.rs` | transient status notices |

### Effects 边界

Reducers 不应直接调用 terminal、remote socket 或 persistence API。提议引入 `ClientEffect` enum：

- `SendRemoteMessage`
- `ResumeRemoteSession`
- `LaunchRemoteSplit`
- `PersistSession`
- `ExtractMemories`
- `StartCompaction`
- `RunInputShell`
- `RequestQuit`
- `RequestRedraw`

### 分阶段提取顺序

| 阶段 | 内容 | 状态 |
|---|---|---|
| Phase 0: 文档 | 确定命名约定，不移动代码 | **Done**（本文档） |
| Phase 1: 在 crate 内引入 state slices | 创建 `src/client_core/` 模块和空 state 类型 | **Planned** |
| Phase 2: 提取最易抽的纯 reducer | `state_ui_messages.rs`、`conversation_state.rs` 等 | **Planned** |
| Phase 3: workspace state 去全局化 | `workspace_client.rs` 静态 → `App` 实例所有 | **Planned**（最高杠杆） |
| Phase 4: 提取 remote event reduction | `server_events.rs` 拆分为 core + adapter | **Planned** |
| Phase 5: 提取标准化 terminal intents | 不把 raw crossterm::Event 放入 core | **Planned** |
| Phase 6: 收紧 renderer 边界 | `PresentationSnapshot` 替代宽 `TuiState` trait | **Planned** |
| Phase 7: runtime adapter 后置 effects | local/remote/reconnect 变为薄壳 | **Planned** |
| Phase 8: 可选 crate 拆分 | 创建 `crates/jcode-client-core` | **Planned** |

### 关键原则

- 拆分应为 `client-core = state + reducers + effects`，`presentation = ratatui 渲染`
- 最安全的第一步是 workspace state 去全局化而非渲染改动
- 不要在第一个 wave 中重写 renderer、删除 `TuiState`、或引入巨型 Redux-style 全局 action enum

---

## 3 · 多会话客户端架构

> **状态：Proposed**
> 源文档：`docs/MULTI_SESSION_CLIENT_ARCHITECTURE.md` (617 行)

### 职责一句话

将 jcode 从当前"单会话每客户端"模型演进为"多会话客户端"，内置 Niri 风格的工作区空间管理 UX。

### 术语

| 术语 | 定义 |
|---|---|
| **Session** | server 所有的运行时：conversation history, provider/model state, tool execution, persistence, memory |
| **Surface (Attachment)** | 客户端的交互/被动会话视图：input draft, scroll, cursor, selection, pane focus |
| **Client** | 一个 TUI 进程，可托管一个或多个 surface |

### 当前 vs 目标模型

**当前**：
```
Server: Session A, Session B, Session C
Client 1 → Session A
Client 2 → Session B
Client 3 → Session C
```

**目标**：
```
Server: Session A, Session B, Session C, Session D
Client 1 (workspace mode):
  Surface A → Session A
  Surface B → Session B
  Surface C → Session C
Client 2 (independent mode):
  Surface D → Session D
```

### 关键设计规则

Shared session state（server 所有）与 Surface-local UI state（client surface 所有）必须分离。这是支持以下场景的前提：
- 同一 session 在不同位置展示
- pop out 到独立窗口
- dock 回 workspace
- 不同 surface 对相同 session 的不同视图状态

### 客户端两种模式

**Single-surface mode**（等价于今天）：一个 client → 一个 surface → 一个 session，保持为默认心智模型。

**Multi-surface mode**（workspace 模式）：一个 client → 多个 surface，内置空间导航。

### Niri 风格 Workspace UX

- 主视口一次显示一个全尺寸 session
- 每个 session 占据 workspace 中一个逻辑全屏格
- 左右移动在工作区水平条内切换 session
- 上下移动切换 workspace 行
- 每个 workspace 行记住上次聚焦的 session
- 新 session 插入当前聚焦 session 右侧

```
workspace +1: [session C]
workspace  0: [session A] [session B]
workspace -1: [session D] [session E] [session F]
```

### Workspace Map / Info Widget

一个形状优先、文本精简的 workspace 可视化 widget：
- 每行 = 一个 workspace，每矩形 = 一个 session
- 颜色和动画编码状态：空闲/聚焦/运行中/完成/等待/错误/分离
- 无需在每个地图单元内放详细标签

### 提议的内部模型

```rust
struct ClientShell {
    surfaces: Vec<SessionSurface>,
    focused_surface: Option<SurfaceId>,
    mode: ClientMode,
    layout: LayoutState,
}
struct SessionSurface {
    surface_id: SurfaceId,
    session_id: SessionId,
    controller: SessionController,
    ui: SessionSurfaceState,
}
struct SessionSurfaceState {
    input: String,
    cursor_pos: usize,
    scroll_offset: usize,
    side_pane_focus: bool,
    zoomed: bool,
}
```

### Transport 策略

**Phase 1**（快速路径）：每个 active surface 一个专属 remote connection。

**Phase 2**（长期目标）：多路复用客户端协议——一个 client connection 可订阅多个 session，request/event 显式用 `session_id` 标记。

### Pop-Out / Dock 流程

- **Pop out**：用户选择 workspace surface → client spawn 独立 jcode client 附着到同一 session → 独立 surface 成为 active interactive owner
- **Dock**：用户请求 dock → workspace client 创建 surface → workspace surface 成为 active owner → 独立 client 退出或分离

### Interop API

潜在对外暴露的操作：`list_sessions`、`list_surfaces`、`focus_session(session_id)`、`open_session_in_window(session_id)`、`dock_session(session_id)`。

### 分阶段迁移

| 阶段 | 内容 | 状态 |
|---|---|---|
| Phase 0: renderer extraction | 提取可复用 session rendering 层 | **Planned**（依赖 Client-Core 拆分） |
| Phase 1: surface/controller 拆分 | 拆分当前单体 client state | **Planned** |
| Phase 2: workspace model + map widget | Niri-style workspace row model | **Planned** |
| Phase 3: full-screen camera navigation | 单进程支持多 session surface | **Planned** |
| Phase 4: pop-out support | 在独立 client 中打开 hosted session | **Planned** |
| Phase 5: dock support | 独立 session 重新附着到 workspace | **Planned** |
| Phase 6: protocol cleanup | session-multiplexed 协议 | **Planned** |

---

## 4 · Server/Service 拆分方案

> **状态：Proposed**（部分已在实施中）
> 源文档：`docs/SERVER_SERVICE_SPLIT_PLAN.md` (598 行)

### 职责一句话

审计当前 server 栈，提出在单进程内将 server 拆分为五个 in-process service：session、client、swarm、debug、maintenance，以改善所有权边界，减少参数扇出。

### 核心问题

**当前瓶颈**不是传输或进程边界，而是在进程内引入 **service-owned state + service APIs**：
- `Server` 用一个 struct 持有几乎所有共享状态
- `ServerRuntime` 将完整状态包克隆到连接处理器
- `handle_client()` 既是连接循环又是应用路由器
- session 流程直接改变 swarm 状态
- maintenance loop 直接修改共享 maps
- debug 路径绕过未来边界

### 模块热度图

| 文件 | 行数 | 主要关注点 | 未来服务 |
|---|---|---|---|
| `src/server/client_lifecycle.rs` | 1767 | 客户端请求循环和路由器 | client |
| `src/server/client_comm.rs` | 1492 | swarm 通信请求 | swarm |
| `src/server/client_actions.rs` | 1249 | session-local actions | session |
| `src/server/swarm.rs` | 1202 | swarm 状态变异和扇出 | swarm |
| `src/server/comm_control.rs` | 1183 | swarm 控制 / await-members / debug bridge | swarm + debug |
| `src/server/client_session.rs` | 1091 | subscribe/resume/clear/reload | session + client boundary |
| `src/server/comm_session.rs` | 987 | spawn/stop session flows | session + swarm boundary |
| `src/server/debug.rs` | 980 | debug socket 命令路由器 | debug |
| `src/server/reload.rs` | 826 | reload 和优雅关闭 | maintenance |
| `src/server/debug_server_state.rs` | 748 | 所有 store 的 debug snapshot | debug |

### 5 个提议 Service

#### 1. Session Service
**拥有**：sessions、session_id、shutdown_signals、soft_interrupt_queues、session event fanout、agent actions。
**边界**：不应直接拥有 swarm membership rules。

#### 2. Client Service
**拥有**：socket/debug/gateway accept loops、client connection registry、request routing。
**边界**：路由请求但不拥有 session/swarm/debug 的业务状态。

#### 3. Swarm Service
**拥有**：swarm_members、swarms_by_id、shared_context、plans、coordinators、channel subscriptions、file touches、await-members runtime。
**边界**：可通过 session service 请求消息投递，但不直接操作 session maps。

#### 4. Debug Service
**拥有**：debug socket router、debug jobs、server/swarm snapshots。
**边界**：通过 service snapshot 读取，通过显式 service methods 写入。

#### 5. Maintenance Service
**拥有**：reload monitor、registry publish、idle timeout、memory logging、ambient loop。
**边界**：编排 services，不拥有它们的 domain maps。

### 依赖方向

```
Client → Session, Swarm, Debug
Swarm → Session
Debug → Session, Swarm
Maintenance → Session, Swarm, Client
```

`Server` 变为 bootstrap + wiring only，`ServerRuntime` 变为 transport runtime only。

### 5 个 Concrete Extraction Seams

| Seam | 描述 | 安全性 |
|---|---|---|
| A: `state.rs` → Session delivery 基础 | 将 event sender registration/fanout 变为 session service 骨干 | 逻辑已集中 |
| B: 分离连接路由与业务 handler | `client_lifecycle.rs` 拆为 ClientConnection + ClientRequestRouter | 无协议变更 |
| C: 从 session lifecycle 移出 swarm membership 副作用 | subscribe/resume 不再直接操作 swarm maps | 最重要语义 seam |
| D: maintenance loops 调用 service API | `monitor_bus()` 等调用 `session_service.queue_soft_interrupt(...)` 而非直接操作 maps | 行为不变 |
| E: debug 消费 snapshot 而非直接访问存储 | `session_service.snapshot_sessions()` 等方式 | debug 保持强大 |

### 首次安全移动顺序

1. 文档和所有权规则（已完成）
2. 引入 service handle structs，零行为改变
3. `ServerRuntime` 持有 service handles 而非 raw maps
4. 改变 `handle_client()` 和 `handle_debug_client()` 签名
5. 从 `client_session.rs` 提取 swarm membership orchestration
6. `monitor_bus()` 移到 swarm/session API 边界后

### 实施状态清单

| Item | Status |
|---|---|
| `state.rs` 已提取 | **Implemented** |
| `socket.rs` 已提取 | **Implemented** |
| `reload_state.rs` 已提取 | **Implemented** |
| service handle structs | **Planned** |
| client/session 路由分离 | **Planned** |
| session→swarm 副作用去除 | **Planned** |

---

## 5 · 统一 Selfdev Server 方案

> **状态：Implemented**
> 源文档：`docs/UNIFIED_SELFDEV_SERVER_PLAN.md` (179 行)

### 职责一句话

移除专用 selfdev daemon/socket pair，将 selfdev 作为 session capability 放在共享 server 上，减少 RAM 占用。

### 旧架构
- Selfdev 有独立 socket `/tmp/jcode-selfdev.sock` 和 `/tmp/jcode-selfdev-debug.sock`
- 通过 `canary-wrapper` 启动独立 server 进程
- 重复 Tokio runtime、allocator heap、MCP pool、embedding 生命周期

### 当前架构（已实施）

**一个共享 server**，selfdev 是 session-local：
- 客户端通过 `{ working_dir, selfdev: true }` 标志订阅
- Server 标记该 session 为 canary
- 仅该 session 注册 selfdev tools、添加 selfdev prompt
- 一个共享 debug socket

**重要策略**：selfdev session 触发的 reload 会重载 **共享 server**，所有客户端重连。reload 使用哪个二进制的选择取决于触发 session 的 canary 状态。

### 实施阶段完成情况

| 阶段 | 内容 | 状态 |
|---|---|---|
| Phase 1: Client-side selfdev on shared server path | 停止 repo auto-detection 走独立 daemon | **Done** |
| Phase 2: Move explicit `jcode self-dev` onto shared server path | 显式 selfdev 命令用共享 server | **Done** |
| Phase 3: Session-targeted reload selection | reload 选择基于 session canary 状态 | **Done** |
| Phase 4: Remove dedicated selfdev socket assumptions | 退休独立 socket、更新文档/测试 | **Done** |

---

## 6 · Crate 拆分计划

> **状态：部分实施**
> 源文档：`docs/dev/crate-splitting-plan.md`

### 原则

1. 先提取稳定叶子：storage、protocol/types、parser、provider request/stream codec、TUI render primitives
2. 避免循环 crate 依赖
3. 按重新编译波动性拆分（而非按目录名）
4. 重型可选依赖保持为 crate/feature guard
5. 迁移期保留兼容 facade

### 推荐的下一步提取

| Crate | 内容 | 状态 |
|---|---|---|
| `jcode-storage` | app paths、permission hardening、atomic JSON writes、JSONL helpers | **Implemented**（~0.9s check） |
| `jcode-provider-anthropic` | 从 `src/provider/anthropic.rs` 移出 | **In Progress** |
| `jcode-provider-openai` | 从 `src/provider/openai.rs` 移出 | **In Progress** |
| `jcode-session-core` | session storage paths、journal metadata | **Planned** |
| `jcode-tui-app-state` | key/input/navigation state transitions | **Planned** |
| `jcode-server-protocol-runtime` | websocket/client event fanout | **Planned** |

### 反模式

- 提取依赖根 `jcode` 的 crate（保留编译瓶颈）
- 每文件一个微型 crate（增加元数据开销）
- 只移动 type alias 而把实现留在根 crate

---

## 回指

- Workspace 全景与 crate 分组：[00-overview-and-entry.md](00-overview-and-entry.md)、[12-workspace-build-ci.md](12-workspace-build-ci.md)
- Agent runtime 细节：[02-agent-runtime.md](02-agent-runtime.md)
- Provider 子系统：[03-provider.md](03-provider.md)
- Server 运行时：[04-server.md](04-server.md)
- TUI 客户端：[05-tui.md](05-tui.md)
- 代码质量与重构计划：[21-quality-audit.md](21-quality-audit.md)
