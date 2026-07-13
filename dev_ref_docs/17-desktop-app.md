# 17 · Desktop App

> 子系统：jcode Desktop 原生桌面客户端，基于 wgpu + winit 自定义 GPU 渲染管线，采用分离进程架构（Desktop Frontend ↔ jcode Server/Daemon）。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

jcode Desktop 是一个 Rust 原生桌面客户端，通过自定义 GPU 渲染 UI 连接到本地 jcode server/daemon，提供多 session 多 surface 的 Niri 风格空间化 agent 工作台。Desktop 不对 jcode 运行时做 fork，而将 server 视为会话、工具、agent 执行、持久化和权限的唯一真理源。

**核心哲学**：Desktop 是渲染/输入表面，而非嵌入式 agent 运行时。

## 架构蓝图

```
jcode 桌面客户端
  - 窗口/输入（winit / 平台层）
  - 自定义 GPU 渲染（wgpu）
  - 本地视图模型（ClientCore / SurfaceState）
  - 瞬态 UI 状态（滚动偏移、光标、面板尺寸）
  - Protocol 客户端（连接到 server 的事件流）

         ↕ 版本化类型化事件流（Unix socket / Named Pipe）

jcode 服务端/Daemon
  - 会话管理、agent 运行时、工具执行
  - 后台任务、持久化、权限、model 配置
```

**关键原则**：Server 拥有会话历史、流式事件、工具执行、文件编辑、后台任务、权限、持久化配置。Desktop 前端仅拥有 surface 本地状态（选中 session、草稿输入、光标/文本选择、滚动偏移、面板大小、焦点区域、本地命令面板状态、渲染缓存）。

## 设计决策

### 1. 分离进程架构（Desktop Frontend + Server/Daemon）

- Desktop 始终连接本地 jcode server/daemon，不内嵌 agent 运行时。
- 避免 UI 因运行时工作而冻结。
- CLI/TUI/Desktop 作为 peer 共存。
- 复用现有 reconnect/session lifecycle。
- 更简单的崩溃隔离和 macOS bundle 助手模式。

### 2. 自定义 GPU 渲染（wgpu + winit），无 WebView/Electron/Tauri

- 渲染层：wgpu 自定义 2D 渲染器 + winit 窗口/输入层。
- 无 CSS/DOM 架构、无 React/Vue/Svelte 风格的 UI 栈。
- 保留 UI 树 + 脏跟踪模式（retained UI tree with dirty tracking）。
- 按需渲染：空闲时不连续渲染循环，仅在输入、数据事件、动画或显式失效时渲染。

### 3. Niri 风格的空间化 agent 工作台（SuperApp Workspace）

- 不是单聊窗口，不是 IDE 克隆，而是**键盘驱动、Niri 风格的 agent workspace superapp**。
- 工作区模型：Workspace → Lanes(rows) → Columns → Surfaces。
- Surface 类型包括 AgentSession、Activity、WorkspaceFiles、CodeView、Diff、TerminalOutput、Settings、Debug 等。
- 导航模型：Leader 键（Space 或 Cmd+K）+ h/j/k/l 的 Vim 式模式导航。
- 输入模式：导航模式（hjkl 控制焦点/布局）↔ 文本编辑模式（Escape 返回导航模式）。

### 4. 文本策略

- 使用 Rust 原生文本栈（cosmic-text/swash）。
- 维护 GPU glyph atlas。
- 按 stable block ID 和可用宽度缓存已成型文本行。
- 专门优化流式追加路径，使新输出不重新排版整个 transcript。
- 目标主字体：`JetBrainsMono Nerd Font` Light 字重。

### 5. 客户端核心（ClientCore）提取策略

Desktop 不应直接从 TUI 迁移代码，而应提取共享的 `jcode-client-core` crate：

```
jcode server/runtime/protocol
  -> client-core reducer and view model
    -> desktop product views
      -> custom UI tree/layout
        -> display list
          -> wgpu renderer
```

`jcode-client-core` 必须不依赖 wgpu、winit、AppKit、Wayland/X11、ratatui、crossterm、terminal markdown rendering。

## 核心类型

### ClientCore（与 TUI 共享的 surface 无关状态）

```rust
struct ClientCore {
    sessions: SessionListState,
    active_surface: Option<SurfaceId>,
    surfaces: SurfaceMap,
    connection: ConnectionState,
    commands: CommandRegistry,
    activity: ActivityState,
    permissions: PermissionState,
    workspace: WorkspaceState,
    diagnostics: DiagnosticsState,
}
```

### SurfaceState（Desktop 本地 UI 状态，每个 surface 一份）

```rust
struct SurfaceState {
    session_id: SessionId,
    transcript: TranscriptState,
    composer: ComposerState,
    selection: SelectionState,
    scroll: ScrollState,
    focused_region: FocusRegion,
    overlays: OverlayStack,
    pane_layout: PaneLayoutState,
}
```

### TranscriptBlock（通用语义 transcript 表示，取代 TUI 的 `DisplayMessage` + terminal `Line`）

```rust
struct TranscriptState {
    blocks: Vec<TranscriptBlock>,
    block_index: HashMap<BlockId, usize>,
    streaming_block: Option<BlockId>,
    version: u64,
}

enum TranscriptBlock {
    User(UserBlock),
    Assistant(AssistantBlock),
    Tool(ToolBlock),
    System(SystemBlock),
    BackgroundTask(TaskBlock),
    Swarm(SwarmBlock),
    Usage(UsageBlock),
    Memory(MemoryBlock),
    Compaction(CompactionBlock),
}
```

### Surface 注册表

```rust
enum SurfaceKind {
    AgentSession,
    Activity,
    WorkspaceFiles,
    CodeView,
    Diff,
    TerminalOutput,
    Settings,
    Debug,
    Extension,
}
```

### Workspace 布局状态

```rust
struct WorkspaceLayoutState {
    workspaces: Vec<WorkspaceNode>,
    active_workspace: WorkspaceId,
    active_surface: Option<SurfaceId>,
}

struct WorkspaceNode { id: WorkspaceId, name: String, lanes: Vec<LaneNode> }
struct LaneNode { id: LaneId, columns: Vec<ColumnNode> }
struct ColumnNode { id: ColumnId, surfaces: Vec<SurfaceId>, active_surface_index: usize }
```

## Crate 结构提案

```
crates/
  jcode-protocol/             # 最终从 src/protocol.rs 提取，TUI + Desktop + Mobile 共享
  jcode-client-core/          # surface 无关的客户端状态/reducer/view model（共享层）
  jcode-desktop-ui/           # 自定义 UI 树、布局、输入路由、样式 token
  jcode-desktop-renderer/     # wgpu 渲染器、display list、glyph/image atlas
  jcode-desktop-platform/     # winit/AppKit/Linux shell 抽象、菜单、剪贴板
  jcode-desktop/              # 产品应用：窗口、面板、protocol 客户端、composition
```

依赖方向：

```
jcode-desktop
  -> jcode-desktop-platform
  -> jcode-desktop-renderer
  -> jcode-desktop-ui
  -> jcode-client-core
  -> jcode-protocol
```

`jcode-desktop-renderer` 不应知道什么是 jcode session，它只渲染 display list、text runs、images、clips 和 primitives。

## 关键文件清单（按 crate 分组）

| 文件/模块 | 职责 |
|---|---|
| `jcode-desktop/src/main.rs` | 入口 |
| `jcode-desktop/src/app.rs` | DesktopApp 顶层编排 |
| `jcode-desktop/src/protocol_client.rs` | socket 连接、读写 task |
| `jcode-desktop/src/daemon.rs` | 启动/连接/查找 bundled daemon |
| `jcode-desktop/src/views/` | 产品视图：root、top_bar、session_sidebar、timeline、composer、activity_panel、workspace_panel、inspector_panel、command_palette、permission_modal、settings、debug_hud |
| `jcode-desktop/src/reducers/` | platform_events、commands、view_actions |
| `jcode-desktop-ui/src/tree.rs` | 保留 UI 节点树 |
| `jcode-desktop-ui/src/layout/` | flex、stack、split、scroll、virtual_list |
| `jcode-desktop-ui/src/text/` | buffer、selection、shaping、cache |
| `jcode-desktop-ui/src/display_list.rs` | DisplayList + DrawCommand |
| `jcode-desktop-renderer/src/` | gpu、surface、pipeline、primitives、text_renderer、glyph_atlas、image_atlas、clips、stats、screenshot |
| `jcode-desktop-platform/src/` | event、window、clipboard、menus、dialogs、appearance、shortcuts、macos、linux |

## 依赖关系

- 依赖 [04 Server](04-server.md)：Desktop 始终连接本地 jcode server/daemon，不内嵌 runtime。
- 依赖 [11 Protocol](11-bus-message-protocol.md)：消费 NDJSON `Request`/`ServerEvent` 协议，与 TUI 相同 wire 格式。
- 依赖 [08 Storage](08-storage-session.md)：server 端的会话持久化。
- 依赖 [06 Auth](06-auth-login.md)：server 侧的凭据验证，Desktop 本身不需处理认证。
- 被 [05 TUI](05-tui.md) 引用为 feature reference 和共享 client-core 来源。
- 使用 `wgpu`（渲染抽象）、`winit`（窗口/输入）、`cosmic-text`/`swash`（文本 shaping/光栅化）。

## 禁用项（明确不入 MVP）

- 无 blur 效果、复杂阴影、完整动画框架
- 无 SVG 重渲染
- 无完整 markdown 渲染器
- 无完整终端模拟器
- 无内嵌代码编辑器（Level 1：外部编辑器 + 只读文件预览；Level 2：轻量语法高亮查看器；Level 3：才是真正编辑器）
- 无 Level 3 编辑器（rope buffer、LSP、multi-cursor）直到 session/diff 工作流成熟

## 开发阶段

### Phase 0：空白白画布

全屏空白 wgpu 白色画布，验证 winit + wgpu 初始化、resize、scale factor、clean exit、按需 event loop。

### Phase 1：fake-data 空间化工作台

多 fake session surface、Niri 风格布局（h/j/k/l 导航）、leader 键模式、surface open/close/move/zoom、command palette、debug HUD。

### Phase 2：协议连接

连接到本地 jcode server、列出 session、订阅/恢复 session、发送 prompt、流式 event 渲染、cancel/stop、daemon 重启恢复。

### Phase 3：有用 agent 控制台

activity center、permission request overlay、workspace/git status panel、changed-file list、open external editor、session search/filter、macOS .app bundle 原型。

## 性能预算

| 指标 | MVP 目标 | 长期目标 |
|---|---|---|
| 冷启动到可见窗口 | < 500 ms | < 150 ms |
| 前端空闲 CPU | ~0% | ~0% |
| 前端空闲 RSS | < 100 MiB | < 50 MiB |
| 输入到绘制延迟 | < 32 ms | < 16 ms |
| 滚动 | 60 fps | 120 fps |
| 假 transcript 压力 | 100k blocks 可用 | 100k blocks 流畅 |
| 追加时全 transcript 重排版 | 禁止 | 禁止 |
| 空闲时渲染帧 | 禁止 | 禁止 |

## 平台策略

- **开发平台**：Linux 优先（最快内循环），渲染器压力测试、协议集成测试、benchmark 在 Linux 上完成。
- **产品目标**：macOS 优先（早期用户平台），Retina 渲染、trackpad 滚动、系统外观适配、Command 快捷键、.app bundle、notarization。
- **避免 Linux-shaped 假设**：不绑定 Niri-style WM、不依赖 X11/Wayland-only API、不假设终端优先工作流。

## Desktop 与 SAITEC-TUI 项目的关系

SAITEC-TUI（本仓库根 crate `jcode`）的主要关注点是 **TUI 终端 UI**（ratatui）。Desktop 文档在此作为**参考架构**，原因如下：

| 维度 | TUI (SAITEC-TUI) | Desktop（参考） |
|---|---|---|
| 渲染后端 | ratatui 终端 cell | wgpu 自定义 GPU |
| 布局 | frame 大小 terminal rect | 保留布局树 + dirty flags |
| 滚动 | 行/cell 基（整数行） | 像素基 + 分数偏移 |
| 文本 | terminal spans + display widths | shaped runs + glyph positions |
| 选择 | 行/cell 范围 | 语义选择：block ID + text range |
| 缓存 | 全局缓存 | instance-owned + attributable 缓存 |
| 会话 | 单 session 或 remote connection | 多 surface 多 session 空间化布局 |
| 共享层 | 无明确 client-core 提取 | 目标提取 jcode-client-core |

Desktop 的目标是成为 TUI 的补充而非替代，两者通过共享的 server/runtime 和未来的 `jcode-client-core` 层关联。

## 陷阱与设计约束

- **不要复制 TUI 结构到 Desktop**：TUI 有约 144 个 Rust 文件 ~115k 行，包含大量 terminal 特有的渲染/输入/布局/滚动/缓存逻辑。Desktop 应使用独立的自定义 UI 和渲染架构。
- **不要将 ratatui::Line 作为 surface 间表示**：terminal 包装行不适合作 Desktop 的 truth source。
- **避免巨大 DesktopApp 状态对象**：TUI 的 monolithic App 状态混合了 runtime、transport、UI、render 关注点，Desktop 应通过 ClientCore 边界避免。
- **桌面始终 server-first**：不内嵌 agent 运行时，避免本地模式路径。
- **输入模式冲突**：Vim 式全局快捷键会在文本输入时冲突，必须区分导航模式和文本编辑模式。
- **macOS 快捷键约束**：不覆盖 Cmd+H（隐藏）、Cmd+M（最小化）、Cmd+Q（退出）、Cmd+W（关闭）等系统级快捷键。
- **Leader 键优于全局快捷键**：Space 或 Cmd+K 作为 leader + h/j/k/l 更安全，避免 macOS 系统快捷键冲突。
- **非活跃 surface 应 lazy 渲染**：保留状态但避免布局/文本/渲染开销，除非可见或预预热。

## 回指

- server 端架构与 runtime：[04-server.md](04-server.md)
- TUI 终端 UI（feature reference）：[05-tui.md](05-tui.md)
- NDJSON wire 格式（Request/ServerEvent）：[11-bus-message-protocol.md](11-bus-message-protocol.md)
- WebSocket Gateway（iOS/Web 远程客户端接入）：[10-gateway-transport.md](10-gateway-transport.md)
- 会话持久化与崩溃恢复：[08-storage-session.md](08-storage-session.md)
