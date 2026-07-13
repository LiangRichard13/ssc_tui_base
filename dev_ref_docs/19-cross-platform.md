# 19 · Cross-Platform Strategy

> 跨平台策略：jcode 在三类客户端形态（TUI / Desktop / Mobile）间的代码共享、平台差异、共享层划分与架构决策。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

jcode 的跨平台策略确保 **server/runtime 是所有客户端的公共基础**，而不同形态的客户端（TUI 终端、Desktop 原生 GPU 窗口、iOS 移动端）通过共享的 protocol 和逐步提取的 `jcode-client-core` ／ `jcode-mobile-core` 层复用业务逻辑，避免三叉分叉。

## 三大客户端形态

```
                    ┌─────────────────────────────────────────┐
                    │           jcode Server/Daemon             │
                    │  (session mgmt, agent runtime, tools,     │
                    │   persistence, permissions, providers)    │
                    └────────────┬─────────────────────────────┘
                                 │ NDJSON Protocol
                                 │ (Unix socket / WebSocket / Named Pipe)
                    ┌────────────┼─────────────────────────────┐
                    ▼            ▼                             ▼
           ┌────────────┐ ┌──────────┐          ┌──────────────────┐
           │   TUI      │ │ Desktop  │          │ Mobile (iOS)     │
           │ (ratatui)  │ │ (wgpu)   │          │ (SwiftUI + Rust) │
           │            │ │          │          │                  │
           │ 终端 cell  │ │ GPU 渲染 │          │ 触摸原生 UI      │
           │ 单会话     │ │ 多会话   │          │ 薄平台外壳       │
           │ local+rmt  │ │ 仅 server│          │ 仅 server        │
           └────────────┘ └──────────┘          └──────────────────┘
```

关键原则：

1. **Server 是公共基础设施**，所有客户端通过相同 NDJSON 协议连接。
2. **每个客户端使用最适合其平台的渲染技术**：ratatui（终端）、wgpu（桌面）、SwiftUI（iOS）。
3. **共享层逐步提取**：TUI 优先积累了产品特性原型，Desktop 和 Mobile 从中提取共享业务逻辑。

## 共享代码层

### 已共享

| 共享层 | 内容 | 使用方 |
|---|---|---|
| `jcode-protocol` (计划提取自 `src/protocol.rs`) | `Request`/`ServerEvent` 枚举、NDJSON 序列化 | TUI, Desktop, Mobile |
| `jcode-gateway-types` | `PairedDevice`/`PairingCode` 等共享数据结构 | Server, Mobile (通过 gateway) |
| Server runtime (`src/server/`) | Accept loop、session manager、`handle_client()` | TUI, Desktop, Mobile (均以 client 身份连接) |

### 部分共享

| 共享层 | 内容 | 使用方 | 说明 |
|---|---|---|---|
| `jcode-client-core` (计划提取) | 事件 reducer、transcript block 模型、command registry、activity/permission/session 模型 | TUI, Desktop | 目前 TUI 内联实现了这些，Desktop 推动提取 |
| `jcode-mobile-core` | `MobileAppState`/Action/Effect/Reducer、语义 UI tree、protocol 适配器 | Mobile (iOS + Linux Simulator) | Mobile 有独立的核心，因为交互模型与 TUI/Desktop 差异较大 |

### 不共享

| 层 | 使用方 | 原因 |
|---|---|---|
| 渲染引擎 | 各自独立 | TUI: ratatui cell；Desktop: wgpu display list；iOS: SwiftUI native |
| 输入处理 | 各自独立 | TUI: crossterm key events；Desktop: winit keyboard/pointer；iOS: UITouch |
| UI 布局 | 各自独立 | TUI: terminal rect；Desktop: 保留布局树；iOS: SwiftUI layout |
| 文本渲染 | 各自独立 | TUI: terminal spans；Desktop: cosmic-text shaped runs；iOS: native Text |

## 各客户端详细对比

### TUI（当前主力，产品特性参考）

| 维度 | 细节 |
|---|---|
| 渲染后端 | ratatui 终端 cell 渲染 |
| 进程模型 | 支持 local mode（内嵌 runtime）和 remote mode（连接 server） |
| 会话模型 | 单活跃会话为主，支持 session 切换 |
| 输入 | 终端键盘事件（crossterm） |
| 文本 | terminal spans + display widths，行包装 |
| 滚动 | 整数行/细胞偏移，行级虚拟化 |
| 布局 | frame 大小的终端矩形 |
| 缓存 | 全局渲染器缓存（`LAST_MAX_SCROLL` 等） |
| 选择 | 行/细胞范围 |
| 规模 | ~144 文件 / ~115k 行（含 server 共享部分） |
| 场景 | 开发和日常使用主力 |

### Desktop（设计阶段，参考架构）

| 维度 | 细节 |
|---|---|
| 渲染后端 | wgpu 自定义 GPU 渲染器 + display list |
| 进程模型 | 始终 server-first，不内嵌 runtime |
| 会话模型 | 多 session 多 surface 空间化布局（Niri 风格） |
| 输入 | winit 键盘/鼠标/触摸板，Leader + hjkl 模式导航 |
| 文本 | cosmic-text/swash shaped runs + glyph positions + GPU glyph atlas |
| 滚动 | 像素基 + 分数偏移，按像素范围虚拟化 |
| 布局 | 保留布局树 + dirty flags |
| 缓存 | instance-owned + attributable 缓存 |
| 选择 | 语义选择：block ID + text range + structured copy target |
| 规模 | 计划中，初始 6-8 个 crate |
| 场景 | 键盘驱动的多会话 agent 工作台 |

### Mobile iOS（早期原型，Rust 迁移中）

| 维度 | 细节 |
|---|---|
| 渲染后端 | SwiftUI 原生（Stage 1），可能后续共享布局模型（Stage 2） |
| 进程模型 | 仅 server 模式，通过 Tailscale + WebSocket 连接 |
| 会话模型 | 单会话 chat view + ambient dashboard |
| 输入 | 触摸 + 语音 + 相机（二维码） |
| 文本 | SwiftUI native Text / 平台原生文本渲染 |
| 滚动 | 原生 SwiftUI 列表滚动 |
| 布局 | SwiftUI declarative layout |
| 缓存 | 平台原生 |
| 选择 | 平台原生文本选择 |
| 规模 | iOS: ~10 Swift files；Rust core: 2 crates（持续扩展中） |
| 场景 | 移动端 agent 监控、审批、ambient 面板 |

## 跨平台协议策略

所有客户端使用相同的 NDJSON `Request`/`ServerEvent` 协议，区别仅在传输层：

| 客户端 | 传输层 | 备注 |
|---|---|---|
| TUI（本地） | Unix socket | `src/server/socket.rs` - 本地 socket path |
| TUI（远程） | WebSocket → Unix socket bridge | 经 gateway |
| Desktop | Unix socket / Named Pipe | 同 TUI 本地模式，Desktop-only event cursors 可选 |
| Mobile (iOS) | Tailscale + WebSocket → Unix socket bridge | 经 gateway，TLS fallback 可选 |

## 客户端核心（ClientCore）提取路线

Desktop 文档推动了 `jcode-client-core` 的提取计划。这是未来跨平台代码共享的关键层：

```
Phase 1: Desktop prototype, fake data, 不影响 TUI
Phase 2: 协议复用（共用 server protocol / Request / ServerEvent）
Phase 3: 提取 jcode-client-core（transcript block、server event reducer、command registry）
          → TUI 逐步转为 jcode-client-core 的另一种 presentation
Phase 4: 特性对等
```

Mobile 则始终有独立的 `jcode-mobile-core`，因为移动端的交互模式（触摸、短会话、推送）与桌面/TUI 差异太大，不适合共用同一个 client-core 抽象。

## iOS Host Integration 中的 Rust 共享策略

Mobile 的跨平台不是 TUI vs Desktop，而是 **Linux Simulator vs iOS App**：

```
同源代码：jcode-mobile-core (Rust)
  ├── Linux 模拟器中：jcode-mobile-sim 直接调用
  └── iOS app 中：jcode-mobile-ffi (C ABI + JSON) → Swift 桥接调用
```

**关键约束**：
- 每个新应用行为应先能在 Linux 模拟器中验证（无 Mac/Xcode）。
- Swift 不应复制 reducers、protocol 解析或 chat/tool 状态转换。
- 场景夹具（scenarios）应在 Linux 模拟器和 iOS host smoke tests 间复用。

## 核心架构决策总结

| 决策 | 影响 | 原因 |
|---|---|---|
| Server 是所有客户端的唯一真理源 | 一致的数据模型、无分叉状态 | 客户端仅缓存 UI 本地状态 |
| 各客户端使用最适合平台的渲染技术 | 不可复用渲染代码，但避免扭曲各平台优势 | TUI 终端 / Desktop wgpu / iOS SwiftUI 各有最佳路径 |
| Client-core 共享 reducer/view model | TUI 与 Desktop 可共享 event 处理逻辑 | 减少重复实现 product 行为 |
| Mobile 有独立 core | 移动端交互范式与桌面差异大 | 触摸/短会话/推送驱动模型不同 |
| Protocol 层绝对共享 | 所有客户端看到完全相同的 event/request 类型 | 最大化 server-client 兼容性，最小化新客户端接入成本 |

## 陷阱与设计约束

- **不要过早提取共享抽象**：Desktop 启动时不应从 TUI 大量提取代码。先验证 Desktop 的 reducer 形态，再从 TUI 和 Desktop 中提取固定的 `jcode-client-core`。
- **Mobile 不与 TUI/Desktop 共享 client-core**：移动端交互模型（触摸、短时交互、推送驱动、摄像头/语音输入）与桌面差异太大。共享代码仅限 protocol 层。
- **避免三层架构膨胀**：Server ↔ Client-Core ↔ UI 的分层不应变成过度工程。Client-Core 应轻量，仅提取 TUI 和 Desktop 的共同业务逻辑。
- **Desktop 不从 Swift 移动端代码迁移**：反之亦然。Desktop 和 Mobile 是独立的客户端，不是同一产品的不同端口。
- **模拟器不是最终的 iOS 验证替换**：Linux App Simulator 可以替换日复一日的迭代，但最终 iOS 设备验证、Xcode 构建、TestFlight 分发仍然需要 Mac。

## 回指

- Desktop 架构与 client-core 提取计划：[17-desktop-app.md](17-desktop-app.md)
- Mobile Client 架构与 Rust mobile-core：[18-mobile-client.md](18-mobile-client.md)
- TUI 终端 UI：[05-tui.md](05-tui.md)
- NDJSON 协议（所有客户端共享）：[11-bus-message-protocol.md](11-bus-message-protocol.md)
- WebSocket Gateway（Mobile/远程客户端接入）：[10-gateway-transport.md](10-gateway-transport.md)
- Server 运行时（客户端连接入口）：[04-server.md](04-server.md)
