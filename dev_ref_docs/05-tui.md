# 05 · TUI（终端 UI）

> 子系统：ratatui + crossterm 终端 UI，本地/远程双模式，NDJSON 行协议通信，Markdown/Mermaid 渲染，会话/账户/登录 picker，键盘绑定。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

TUI 子系统是 jcode 的终端用户界面层：通过 ratatui + crossterm 实现本地/远程双模式聊天界面，以 NDJSON 行协议与后端 server 通信，负责消息流式渲染、Markdown/Mermaid 图表展示、会话管理、侧边面板、键盘绑定与多面板布局等全部终端交互体验。

## 关键文件清单

**入口与模块注册**
- `src/tui/mod.rs` — 顶层 TUI 模块声明，re-export `App`/`RunResult`/`DisplayMessage`/`CopySelection*`
- `src/tui/app.rs` — `App` 结构体（约 1000 字段 God-object），声明 ~40 个子模块（auth/commands/remote/state_ui/turn/replay/inline_interactive 等）

**Backend / 通信层**
- `src/tui/backend.rs` — `RemoteConnection`：NDJSON 行读取（`next_event`）、`Request` 发送、`BackendEvent` 枚举、`DebugEvent` 序列化
- `src/tui/app/remote/server_events.rs` — `ServerEvent` 分发器（`TextDelta`/`ToolStart`/`ToolDone`/`TokenUsage`/`History`/`Done`）
- `src/tui/app/remote/server_event_handlers.rs` — 各 `ServerEvent` variant 具体处理
- `src/tui/app/remote/input_dispatch.rs` / `reconnect.rs` / `queue_recovery.rs` / `session_persistence.rs` / `workspace.rs` / `swarm_plan_core.rs`

**渲染管线（UI 绘制）**
- `src/tui/ui.rs` — 主渲染入口（`draw` / frame 组装）
- `src/tui/ui_layout.rs` — 主布局（chat / side pane / split-view 比例）
- `src/tui/ui_header.rs` / `ui_input.rs` / `ui_messages.rs` / `ui_messages_cache.rs` / `ui_status.rs` / `ui_tools.rs`
- `src/tui/ui_diff.rs` / `ui_file_diff.rs` / `ui_diagram_pane.rs` / `ui_pinned*.rs` / `ui_inline*.rs` / `ui_overlays.rs` / `ui_changelog.rs` / `ui_animations.rs` / `ui_transitions.rs` / `ui_prepare.rs` / `ui_memory*.rs` / `ui_viewport.rs` / `ui_frame_metrics.rs` / `ui_box.rs` / `ui_theme.rs`

**Widgets / Info**
- `src/tui/info_widget*.rs` — 主 info widget 及其 git/graph/memory/model/overview/swarm_background/text/tips 变体

**Session / Pinned Items**
- `src/tui/session_picker.rs` / `account_picker.rs` / `login_picker.rs` / `usage_overlay.rs` / `generated_image.rs` / `image.rs`

**核心辅助**
- `src/tui/core.rs` — 字符串 boundary 辅助、`DisplayMessageRoleExt` trait
- `src/tui/keybind.rs` — keybinding 加载入口（从 config 读取，调用 `jcode-tui-core` 解析）
- `src/tui/stream_buffer.rs` — 语义流缓冲（newline/code-fence 边界 flush，150ms 超时）
- `src/tui/color_support.rs` / `remote_diff.rs` / `layout_utils.rs` / `screenshot.rs` / `visual_debug.rs` / `test_harness.rs` / `permissions.rs` / `workspace_client.rs`

**App 子模块（关键子集）**
- `src/tui/app/remote/` — 远程 server 通信子系统
- `src/tui/app/local.rs` — 本地 harness provider 流程
- `src/tui/app/replay.rs` — session 录制回放
- `src/tui/app/turn.rs` / `turn_memory.rs` — 单轮对话生命周期
- `src/tui/app/state_ui*.rs` / `tui_state.rs` / `tui_lifecycle*.rs` / `navigation.rs` / `copy_selection.rs` / `split_view.rs` / `todos_view.rs` / `observe.rs` / `dictation.rs` / `auth*.rs` / `commands*.rs` / `handterm_native_scroll.rs`
- `src/tui/app/inline_interactive/` — 内联 picker 系统（model/session preview）

## 核心类型与渲染管线

**核心类型**：
- **`App`** (`app.rs:520`) — TUI 的 God-object，约 1000 字段：`provider: Arc<dyn Provider>`、`messages`/`display_messages`、`session`、`stream_buffer`、`streaming_md_renderer`、`runtime_mode`（RemoteClient/Replay/TestHarness）、远程元数据、`side_panel`、大量 scroll/copy/diagram/overlay/queue 状态。
- **`RemoteConnection`** (`backend.rs:226`) — `reader: BufReader<ReadHalf>`、`writer: Arc<Mutex<WriteHalf>>`、`session_id`/`client_instance_id`、`next_request_id`、`tool_diff`、`line_buffer`（复用避免反复分配）、`protocol_error_count`。
- **`BackendEvent`** (`backend.rs:127`) — backend 统一事件枚举（`TextDelta`/`ToolStart`/`ToolDone`/`TokenUsage`/`ThinkingStart`/`ThinkingEnd`/`Done`/`Error`）。
- **`DisplayMessage`** (`jcode-tui-messages`) — TUI 展示用消息（role/content/tool_calls/tool_data/duration_secs/title），builder 方法 `user()`/`assistant()`/`tool()`/`error()`/`background_task()`/`memory()`/`swarm()`/`overnight()`/`usage()`。
- **`PreparedMessages`/`PreparedChatFrame`** — 预计算帧数据（wrapped_lines/copy_targets/image_regions/edit_tool_ranges）。
- **`IncrementalMarkdownRenderer`** (`jcode-tui-markdown`) — 增量 Markdown→ratatui `Line` 渲染器。

**渲染管线**：
1. **事件驱动循环**：`crossterm::EventStream` 读键鼠事件，`tokio::select!` 与 `RemoteConnection::next_event()` 并行。
2. **状态更新**：`App` 字段在事件处理中即时更新。
3. **帧准备**：`PreparedChatFrame` 预计算 wrapped_lines + copy targets + image regions（带 LRU cache，key 为 content hash + width + diff_mode + diagram_mode）。
4. **主 draw**：`ui.rs` 调 `Frame` API，按 layout 拆 header / messages viewport / input / status / side panel / overlays。
5. **Markdown 渲染**：`jcode-tui-markdown` 经 pulldown-cmark 解析 → syntect 高亮 → ratatui `Line`。
6. **Mermaid 渲染**：`jcode-tui-mermaid` 检测 mermaid code block → mermaid-rs parse/layout/SVG → PNG → ratatui-image StatefulProtocol（Kitty/Sixel/iTerm2/halfblock）。
7. **增量优化**：streaming 期间 `StreamBuffer` 按 newline/150ms flush，`IncrementalMarkdownRenderer` 全量重渲染但复用 checkpoint 元数据。

## 与 ratatui / server 的通信

**ratatui 关系**：项目用 ratatui 0.30（`Cargo.toml:178`），8 个 `jcode-tui-*` crate 均依赖；渲染用 `ratatui::DefaultTerminal`（crossterm backend），`Line<'static>`/`Span<'static>` 为基本单位；`jcode-tui-render` 提供 chrome 与 layout；`jcode-tui-style` 封装 Color 构造（truecolor/indexed fallback）与主题色；`jcode-tui-mermaid` 用 `ratatui-image` 做终端图片渲染（Kitty/Sixel/iTerm2/halfblock 自动检测）。

**与 server 通信**：NDJSON over Unix domain socket。
- **连接**：`RemoteConnection::connect_with_session()` 经 `Stream::connect(server::socket_path())` 建 Unix socket，split 为 reader/writer。
- **请求（client→server）**：序列化 `Request` 为 JSON + `\n`，`writer.write_all()` 发送。`Request` variant：`Subscribe`/`GetHistory`/`Message`/`Clear`/`Reload`/`ResumeSession`/`Rewind`/`GetCompactedHistory`/`ModelSwitch`/`AccountSwitch` 等。
- **响应（server→client）**：`next_event()` 循环 `reader.read_line()`，每行 `serde_json::from_str::<ServerEvent>()`。`ServerEvent` variant：`TextDelta`/`TextReplace`/`ToolStart`/`ToolInput`/`ToolExec`/`ToolDone`/`TokenUsage`/`History`/`Done`/`Error`/`SwarmPlanUpdate`/`GeneratedImage`/`BatchProgress` 等。
- **Bus 事件订阅**：本地 harness 模式经 `crate::bus::Bus` + `BusEvent` 做进程内事件分发。
- **Debug Socket**：`backend.rs` 定义 `DebugEvent` 用于可选 debug socket broadcast，暴露完整 TUI 状态快照。

## crates/jcode-tui-* 分层

| Crate | 职责 |
|---|---|
| `jcode-tui-core` | `KeyBinding`/`ScrollKeys`/`StreamBuffer`/`CopySelection*`/graph topology；keybinding 字符串解析（`parse_keybinding`），macOS Option-arrow 兼容 |
| `jcode-tui-messages` | `DisplayMessage`、`PreparedMessages`/`PreparedChatFrame`、LRU 消息渲染缓存（2048 上限）、wrapped line map |
| `jcode-tui-markdown` | Markdown→ratatui 渲染引擎；pulldown-cmark + syntect 高亮；`IncrementalMarkdownRenderer`；lazy/full 渲染；table/copy target 提取 |
| `jcode-tui-mermaid` | Mermaid→PNG→ratatui-image；大量缓存（render/image/source/kitty viewport/LRU，磁盘 PNG 缓存 50MB/3 天）；supersample 1.5x |
| `jcode-tui-render` | chrome（border/box）、layout 几何工具 |
| `jcode-tui-style` | `ColorCapability`/`rgb()`（truecolor/indexed fallback）、主题色函数（`user_color`/`ai_color`/`tool_color`/`accent_color`/`dim_color`） |
| `jcode-tui-account-picker` | `AccountProviderKind`/`AccountPickerCommand`/`AccountPickerItem`/`AccountPickerSummary` |
| `jcode-tui-session-picker` | `SessionSource`（Jcode/ClaudeCode/Codex/Pi/OpenCode）/`ResumeTarget`/`SessionInfo`/`ServerGroup`/`PickerItem` |
| `jcode-tui-tool-display` | `resolve_display_tool_name`/`canonical_tool_name`/`is_edit_tool_name`/`concise_tool_error_summary` |
| `jcode-tui-usage-overlay` | `UsageOverlayStatus`/`UsageOverlayItem`/`UsageOverlaySummary` |
| `jcode-tui-workspace` | Niri-style workspace map widget + color support |

## keybindings 配置入口

入口在 **`src/tui/keybind.rs`**，从 `crate::config::config().keybindings` 加载：
- `load_model_switch_keys()` — 默认 `Ctrl+Tab`/`Ctrl+Shift+Tab`
- `load_scroll_keys()` — 默认 `Ctrl+K/J`（vim）、`Alt+U/D`（page）、`Ctrl+[/]`（prompt jump）、`Ctrl+G`（bookmark）
- `load_workspace_navigation_keys()` — 默认 `Alt+H/J/K/L`
- `load_effort_switch_keys()` / `load_centered_toggle_keys()` / `load_dictation_key()`

解析引擎在 `jcode-tui-core/src/keybind.rs`：`parse_keybinding("Ctrl+Shift+Tab")` → `KeyBinding`；modifier: `ctrl`/`alt`(option/meta)/`cmd`(command/super/win)/`hyper`/`shift`；key: `tab`/`enter`/`esc`/`space`/arrows/`pageup`/`pagedown`/`home`/`end`/`f1`-`f24`/单字符；`"none"`/`"off"`/`"disabled"` 禁用；多绑定逗号分隔；macOS Option-arrow ESC 编码兼容。

## 陷阱与历史修复

### backend.rs `next_event()` / NDJSON 解析行为

- **协议错误容忍但有上限**：`next_event()` 遇 malformed NDJSON 行时跳过继续读下一行，直到连续错误达 `MAX_CONSECUTIVE_PROTOCOL_ERRORS`（10）才断连。单条坏行不崩溃，但 server 持续发非 JSON 数据时 client 会静默吃掉 10 行才断，期间用户无感知。
- **空白行静默跳过**：`line_buffer.trim().is_empty()` 的行 warn+skip。
- **行缓冲复用**：`line_buffer` 是实例字段，`read_line()` 追加到已有 buffer；异常巨大行（server 误发整个 JSON blob 无换行）会让 buffer 无限增长。
- **`has_loaded_history` 可重入**：`rewind()`/`rewind_undo()` 设为 `false` 以允许 server 下个 `History` 事件替换显示状态；server 响应慢或丢失时 client 可能卡在「等待 History」。
- **detach 请求无背压**：`send_request_detached()` 经 `tokio::spawn` 异步发送，仅 2s 超时；高频 detach 请求创建大量未完成 spawn。
- **Markdown 增量渲染退化**：`IncrementalMarkdownRenderer::update_internal()` 注释说明曾试 checkpoint 增量拼接但因 block separator/list continuity 问题导致 streaming artifact，已回退为每次全量重渲染。
- **Mermaid 渲染 panic 防护**：`RENDER_WORK_LOCK` 全局互斥锁序列化所有 mermaid 渲染，并临时替换 panic hook；慢图表会阻塞所有其他图表渲染。
- **消息缓存 LRU 全量清除**：`HighlightCache`/`MessageCacheState` 满容量时 `entries.clear()`（而非逐条驱逐）→ 缓存击穿。
- **`App` God-object**：~1000 字段，几乎所有 TUI 状态集中一处；新 feature 都需改此结构体，合并冲突风险高。

### NDJSON 损坏 → reconnect storm → Unknown tool 链（fixed in `fix/mcp-notification-id`）

涉及 backend.rs 的部分（**Fix 2**）：`RemoteConnection::next_event`（`src/tui/backend.rs:808-819`）原对 ANY JSON parse 失败立即断连 → reconnect storm。Fix：跳过坏 NDJSON 行直到连续 10 次错误才断开。完整根因链见 [09-mcp.md](09-mcp.md)。

## 关联模块

| 模块 | 路径 | 职责 | 规模 |
|---|---|---|---|
| `src/side_panel.rs` | TUI 侧边面板多页 Markdown 管理（创建/追加/聚焦/加载文件/删除）；`BusEvent::SidePanelUpdated`；agent 可经 tool 写入侧边面板展示 plan/notes | ~607 行 |
| `src/perf.rs` | 系统画像（CPU 数、可用内存、负载、WSL/SSH、终端类型）→ `PerformanceTier`(Full/Reduced/Minimal) → `TuiPerfPolicy`(FPS/动画/鼠标/键盘增强) 直接决定渲染策略 | 780 行 |
| `src/dictation.rs` | 外部语音听写命令集成——调用户配置的 dictation command，解析 stdout 为文本 + transcript mode | ~578 行 |
| `src/video_export.rs` | 会话录制导出为视频——找 ffmpeg/ffprobe、解析 TimelineEvent、渲染 TUI frame 为图、合成 MP4；支持 swarm 多 pane | ~1195 行 |

## 回指
- `ServerEvent`/`Request` wire 定义：[11-bus-message-protocol.md](11-bus-message-protocol.md)
- IPC 传输（Unix socket / Windows Named Pipe / WebSocket）：[10-gateway-transport.md](10-gateway-transport.md)
- 登录 picker / `PendingLogin` 状态机：[06-auth-login.md](06-auth-login.md)
