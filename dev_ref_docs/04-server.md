# 04 · Server 多进程运行时

> 子系统：长驻 server daemon，多 session/多 client 运行时，swarm 多 agent 协调，headless session，hot-reload exec，ambient 后台调度循环。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

Server 子系统是一个长驻 Unix daemon（或 Windows named-pipe 等效物），通过一对 socket（main + debug）和可选 WebSocket gateway 维护多个并发 AI session 及其 Agent 实例，协调 swarm 多 agent 编排、文件活动冲突检测、热重载 exec 切换、ambient 后台调度循环，并向所有连接客户端广播 NDJSON 编码的事件流。

## 原生 jcode Server 的命名设计

> **设计文档来源**：`docs/SERVER_ARCHITECTURE.md`（原生 jcode 设计，SAITEC-TUI 可能未同步）

原生 jcode 的 server-session 命名使用双单词组合：

```
SERVER = Adjective/Verb modifier          SESSIONS = Animal nouns
────────────────────────────              ────────────────────────
🔥 blazing   ❄️ frozen   ⚡ swift          🦊 fox    🐻 bear   🦉 owl
🌀 rising    🍂 falling  🌊 rushing        🌙 moon   ⭐ star   🔥 fire
```

- Server 启动时随机取一个形容词/动词（如"blazing"），每个 session 取一个动物名词（如"fox"），组合为 `"🔥 blazing 🦊 fox"` 显示在 UI。
- Server name 跨 reload 持久化（通过 `~/.ssc_tui/servers.json` registry）。
- 当 server exec 到新 binary（`/reload`），新进程用新名字注册，旧 entry 自动清理。
- **SAITEC-TUI 差异**：SAITEC-TUI 可能未实现此命名系统，而是使用简单的 session ID 或用户自定义名。

### 原生 jcode 的 Client Reconnection

原生 jcode 客户端有内建 reconnect loop（SAITEC-TUI 通过 `RemoteConnection` 实现类似机制）：
1. 断连时显示 "Connection lost - reconnecting..."
2. 指数退避重试 1s → 2s → 4s → ... → 30s 上限
3. 重连后 resume 同一 session（状态在磁盘持久化）
4. 若 server reload 后 client binary 版本也更新了，client 可重新 exec 自身

### 原生 jcode Self-Dev 模式

在 jcode 仓库内运行时自动触发：
1. Auto-detect 仓库并启用 self-dev 模式
2. 连接到正常共享 jcode server
3. 标记该 session 为 canary/self-dev（通过 subscribe metadata）
4. 仅为该 session 启用 selfdev prompt/tooling
5. `/reload` 热重载共享 server，所有 client 重新连接

## 关键文件清单

**A. 核心运行时 / 入口**

| 文件 | 职责 |
|---|---|
| `src/server.rs` | 模块根；`Server` 结构体、`run()` 入口、accept loop 启动、Bus monitor、idle timeout、background task 全局编排 |
| `src/server/runtime.rs` | `ServerRuntime`（从 `Server` 借出的轻量 clone，持全部 `Arc<RwLock<...>>` 句柄），实现三个 accept loop（main/debug/gateway）的 spawn |
| `src/server/state.rs` | 核心共享类型：`SwarmState`、`SwarmMember`、`SwarmEvent`、`FileAccess`、`SharedContext`、`SessionControlHandle`、event fanout / interrupt queue 注册辅助 |

**B. Client 接入与生命周期**

| 文件 | 职责 |
|---|---|
| `src/server/client_lifecycle.rs` | `handle_client()` 主循环：解析 NDJSON `Request`、调度 agent、per-client event forwarder、cancel/soft-interrupt、Bus 事件转发 |
| `src/server/client_session.rs` | session resume/clear/subscribe/reload 请求处理 |
| `src/server/client_state.rs` | `get_state`/`get_history`/`get_compacted_history` 请求处理 |
| `src/server/client_actions.rs` | agent task、subagent、split、transfer、compact、rename、stdin response 等动作 |
| `src/server/client_api.rs` | `Client` 类型（客户端侧 API 封装，用于内部 ping/connect） |
| `src/server/client_disconnect_cleanup.rs` | 客户端断连后清理 swarm membership、file touches、channel subscriptions、debug state |

**C. Swarm 通信（comm_*）**

| 文件 | 职责 |
|---|---|
| `src/server/client_comm.rs` | `comm_message`/`comm_share`/`comm_read`/`comm_list` 等 agent-to-agent 通信 |
| `src/server/client_comm_channels.rs` | channel subscribe/unsubscribe 辅助 |
| `src/server/client_comm_context.rs` | shared context 读写辅助 |
| `src/server/client_comm_message.rs` | message 投递（direct/broadcast/channel）辅助 |
| `src/server/comm_await.rs` | `comm_await_members` — 等待 swarm 成员达到指定 status 的持久化 await |
| `src/server/comm_control.rs` | `comm_assign_task`/`assign_next`/`assign_role`/`task_control` — coordinator 向 worker 分派任务 |
| `src/server/comm_plan.rs` | `comm_propose_plan`/`approve_plan`/`reject_plan` — 多 agent 协作计划提议/审批 |
| `src/server/comm_session.rs` | `comm_spawn`/`comm_stop` — 动态创建/停止 headless session |
| `src/server/comm_sync.rs` | `comm_status`/`summary`/`read_context`/`resync_plan`/`plan_status` — 状态同步查询 |

**D. Swarm 底层协调**

| 文件 | 职责 |
|---|---|
| `src/server/swarm.rs` | 核心 swarm 函数：`broadcast_swarm_status`（含 debounce）、`broadcast_swarm_plan`、`update_member_status`、`record_swarm_event`、task staleness heartbeat/sweep、`run_swarm_message`（自动 plan-break-then-run 并行 subagent） |
| `src/server/swarm_channels.rs` | session 对 channel 的订阅/退订，维护双向索引 |
| `src/server/swarm_persistence.rs` | swarm plan/coordinator/members 持久化到 `jcode-swarm-state/`，server 启动时加载恢复 |
| `src/server/swarm_mutation_state.rs` | `SwarmMutationRuntime` — mutation 操作幂等去重注册表，持久化到磁盘，支持 reload 后重放 |

**E. Debug 子系统**

| 文件 | 职责 |
|---|---|
| `src/server/debug.rs` | `handle_debug_client()` — debug socket 主循环，路由 `debug_command` 到 server/tester/client 命名空间 |
| `src/server/debug_ambient.rs` | ambient 相关 debug 命令 |
| `src/server/debug_command_exec.rs` | 通用 debug 命令执行框架 |
| `src/server/debug_events.rs` | swarm event 查询与实时订阅 |
| `src/server/debug_help.rs` | help 文本输出 |
| `src/server/debug_jobs.rs` | 异步 debug job 管理（长命令后台运行） |
| `src/server/debug_server_state.rs` | server 状态查询（sessions、connections、swarm overview） |
| `src/server/debug_session_admin.rs` | session 管理命令（create_headless、stop session） |
| `src/server/debug_swarm_read.rs` / `debug_swarm_write.rs` | swarm 只读查询 / 写入命令 |
| `src/server/debug_testers.rs` | 内置 tester 命令 |

**F. Headless / Socket / Lifecycle**

| 文件 | 职责 |
|---|---|
| `src/server/headless.rs` | `create_headless_session()` — 无 TUI 后台 session 创建（fork provider、注册 swarm member、`is_headless: true`） |
| `src/server/socket.rs` | socket 路径计算、连接、daemon lock（Unix `flock`）、`spawn_server_notify`（pipe ready-fd 通知）、`wait_for_server_ready` |
| `src/server/lifecycle.rs` | `TemporaryServerPolicy` — 临时 server 模式（owner PID 驱动，idle 超时自动退出）；metadata 写入/清理 |

**G. Reload（Hot-reload）**

| 文件 | 职责 |
|---|---|
| `src/server/reload.rs` | `await_reload_signal()` — 监听 reload channel，graceful shutdown（信号中断 running sessions、等 checkpoint），然后 exec 替换二进制 |
| `src/server/reload_state.rs` | `ReloadState`/`ReloadPhase`/`ReloadSignal`；reload marker 文件（`jcode.reload`）；同步原语 |
| `src/server/reload_recovery.rs` | reload 恢复意图持久化：reload 前把 running sessions 恢复指令写盘，新 server 启动后读取续跑 |

**H. 其他辅助**

| 文件 | 职责 |
|---|---|
| `src/server/durable_state.rs` | 通用 JSON 持久化辅助（`load_json_state`/`save_json_state`/`hashed_request_key`/TTL 检查） |
| `src/server/background_tasks.rs` | `BusEvent::BackgroundTaskCompleted`/`Progress` 事件分发到对应 session（idle 时直接投喂 agent，busy 时 soft-interrupt 排队） |
| `src/server/provider_control.rs` | 模型切换、premium mode、service tier、transport 切换、auth 变更通知等 provider 层控制 |
| `src/server/file_activity.rs` | 文件活动 scope label 格式化辅助 |

**I. Channel 与 Ambient 系统**

| 文件 | 职责 |
|---|---|
| `src/channel.rs` | `MessageChannel` trait + `ChannelRegistry`：Telegram/Discord 通知 channel，支持 send（单向）和 reply_loop（双向轮询，注入消息到 ambient runner） |
| `src/ambient.rs` | ambient 模块根：`AmbientState`、`AmbientStatus`、`ScheduledItem`、`ScheduleTarget`、`AmbientCycleResult` 等类型 |
| `src/ambient_runner.rs` / `src/ambient_scheduler.rs` | re-export `crate::ambient::runner` / `scheduler` |
| `src/ambient/scheduler.rs` | `UsageLog`/`AdaptiveScheduler`：滚动 token 用量记录，自适应计算 ambient 周期间隔 |
| `src/ambient/runner.rs` | `AmbientRunnerHandle` — ambient 后台循环运行时：调度 cycle、spawn agent session、处理结果、notification 推送、与 server 的 wake/nudge 交互 |

## 核心类型与关键函数

- **`Server`** (`server.rs`) — 顶层状态持有者；构造时生成唯一 server identity（memorable name + icon）、初始化 `SwarmState`（从磁盘恢复）、`AmbientRunnerHandle`、broadcast channels、MCP pool。`run()` bind 两 socket、spawn background tasks、进 accept loop。
- **`ServerRuntime`** (`runtime.rs`) — `Server` 的轻量 clone（全 `Arc` 引用），`spawn_main_accept_loop`/`spawn_debug_accept_loop`/`spawn_gateway_accept_loop`，每连接 spawn 一个 tokio task。
- **`SwarmState`** (`state.rs`) — 四个 `Arc<RwLock<HashMap>>` 聚合：`members`、`swarms_by_id`、`plans`、`coordinators`；`load_runtime()` 组装 `SwarmRuntime` 快照。
- **`SwarmMember`** — 单成员运行时记录：session_id、event_tx/event_txs（多 client attachment）、status、role（agent/coordinator/worktree_manager）、is_headless、report_back_to。
- **`SessionControlHandle`** (`state.rs`) — lock-free 控制面：`request_cancel()`/`queue_soft_interrupt()`/`request_background_current_tool()`，可在 agent 持锁时安全调用。
- **`AmbientRunnerHandle`** (`ambient/runner.rs`) — ambient 后台循环公共句柄：`nudge()` 唤醒、`inject_message()` 注入外部消息、`run_loop()` 主循环。

关键函数：`Server::run()`、`ServerRuntime::spawn_main_accept_loop()`、`handle_client()`、`create_headless_session()`、`await_reload_signal()`、`broadcast_swarm_status()`、`broadcast_swarm_plan()`、`update_member_status()`、`process_message_streaming_mpsc()`。

## 客户端接入与 NDJSON 协议

**传输层**：
- **Unix domain socket**（主路径）：路径由 `socket_path()` 计算（默认 `~/.local/share/jcode/jcode.sock`，`JCODE_SOCKET` 覆盖）。main socket 处理常规 client，debug socket（`jcode-debug.sock`）处理 introspection/command。
- **WebSocket gateway**（可选）：`GatewayConfig` 启用，`spawn_gateway()` 在额外 TCP 端口监听，`GatewayClient` 转为内部 `Stream` 后走与 Unix socket 相同的 `handle_client()` 路径。gateway 客户端不参与 ambient nudge。
- **Daemon lock**：Unix 上 `flock(LOCK_EX | LOCK_NB)` 保证单实例。

**NDJSON 协议**：一行一个 JSON。`decode_request()` → `Request` 枚举（`Ping`/`Subscribe`/`Message`/`Cancel`/`CommMessage`/`CommSpawn` 等数十 variant）；`encode_event()` → `ServerEvent` 枚举。轻量控制请求（`is_lightweight_control_request()`）不需创建持久 session。Debug socket 只接受 `Ping`/`GetState`/`DebugCommand` 三种 Request。

## Headless / Swarm / Hot-reload

**Headless 模式**：`create_headless_session()` 创建，标 `is_headless: true`；不连 TUI，event_tx 写 /dev/null（drain loop）；永不自动成为 coordinator。Server 启动时 `recover_headless_sessions_on_startup()` 从磁盘恢复 swarm state，对非 completed/failed/stopped 的 headless session 加载 `ReloadContext`，有恢复意图则 spawn tokio task 续跑。

**Swarm 协调**：
- **成员管理**：`SwarmMember` 按 swarm_id 分组在 `swarms_by_id`。Coordinator 角色由第一个 TUI 连接 session 自动获取；退出时自动转移到下一个非 headless member。
- **Plan 协作**：`comm_propose_plan` → `approve_plan`/`reject_plan`；plan 带版本号（`VersionedPlan.version`），`broadcast_swarm_plan` 推送；支持 `blocked_by` 依赖和 `newly_ready_ids` 通知。
- **任务分配**：coordinator 经 `comm_assign_task`/`assign_next` 向 worker 分派；worker heartbeat 更新 progress，stale 检测（默认 45s 无 heartbeat 标 `running_stale`）。
- **文件活动冲突**：`monitor_bus()` 监听全局 `Bus::FileTouch`，维护正向（path→accesses）和反向（session→paths）索引；同 swarm 不同成员改同一文件时双向发 `FileConflict` 通知 + soft interrupt。
- **消息传递**：`comm_message` 支持 direct（to_session）/ channel（broadcast to subscribers）；`wake` 参数可在 idle session 触发 agent 处理。
- **Completion report**：`comm_report` 记录结构化完成报告（含 validation/follow_up），自动通知 coordinator。

### 原生 jcode Swarm 设计参考

> **设计文档来源**：`docs/SWARM_ARCHITECTURE.md`（原生 jcode 设计文档，SAITEC-TUI 实现可能不完全同步）

**Agent 生命周期状态**（原生设计九状态）：

```
spawned → ready → running → blocked → completed → [wait for new assignment]
                                              → failed → [coordinator 决策]
                                              → stopped → [coordinator shut down]
                                              → crashed → [unexpected exit]
```

- **blocked**：因依赖/冲突/信息不足无法继续。
- **completed**：assigned scope 完成，等待新任务。
- **failed**：不可恢复错误，等待 coordinator 决策。
- **stopped**：coordinator 主动 shut down。
- 每个状态变更都发出 lifecycle 事件驱动 UI 更新。

**Completion Report Policy**（原生设计）：
- 由 coordinator 创建的 agent（`report_back_to_session_id`）必须每次 prompted work turn 结束时返回有意义的 final assistant response。
- Server 自动把该 final response 转发给 coordinator 作为 completion report。
- Report 应包含：outcome/status、changes/findings、validation performed、blockers/follow-ups。
- 不应是简单 `done`、lifecycle 状态变更或 tool transcript。
- 若 worker 在产出 final response 前失败，coordinator 仍通过 lifecycle notification 收到失败信息。
- 不需要 report 的场景：未带 prompt 的 idle spawn、无 report-back 的 user-created peer、work 进行中的普通 status broadcast、idle worker 的清理/stop。

**Plan 分发与更新**（原生设计）：
- Swarm plan 是 server 级对象（按 `swarm_id` 范围），而非 session todo list。
- Session todos 保持私有，不作为 swarm plan 存储。
- Plan v1 由 coordinator 创建/拥有。
- Plan 更新由 agent 提议，coordinator 审批后广播给 plan participants（非全部 swarm 成员）。
- 参与 plan 需显式声明（coordinator 分配/spawn 策略或 resync attach）。
- Plan **不**存储在 repo 文件中。
- Plan 更新流：Agent → propose update → Coordinator → approve → Plan → Participants → (Coordinator 也可以直接更新 Plan)。

**通信拓扑**（原生设计）：
- **DM**（Direct Message）：agent-to-agent 一对一。
- **Channel**：topic-based group chat，subscribe/unsubscribe 模式。
- **Swarm broadcast**：全 swarm 广播。
- **Shared context keys**：set/read/append 共享内存。
- 所有通信作为 notification 投递（soft interrupt 排队），在 running agent 的安全点注入，不打断当前 tool 执行。
- 三种读取操作分离：
  - **Status snapshot**：lock-free 成员元数据 + 当前 processing/tool snapshot（busy 时也可用）。
  - **Summary read**：短活动 feed（tool calls + intent + 结果）。
  - **Full context read**：explicit 重读——整个 agent context，需谨慎使用避免 context bloat。

**Worktree 分组与集成**：
- Worktree 由 Coordinator 判断是否需要，将相关 agents 分组到同一 worktree。
- 每 worktree 有一个 Worktree Manager，负责 scope 内集成。
- 集成完成后 Worktree Manager 合并到 Integration Branch → Main Branch。
- 每 worktree 分配逻辑 `swarm_id`，通信/plan 更新/UI 跨所有 worktree 可见。

**冲突处理（无锁乐观）**：
- 默认乐观无锁。
- 冲突触发 agents 之间 DM 或 channel 通信协商，不通过 coordinator。

**UI Widgets**（原生设计）：
- **Swarm info widget**：graph 视图显示 agents、worktree managers、coordinator、channels；边表示通信路径（DM/channel/broadcast）；节点显示 status + current task/intent。
- **Plan info widget**：task DAG 图，节点显示 owner/scope/status（queued/running/running_stale/done/blocked/failed）；checkpoint 作为 badge 或 subnode；coordinator 可查看每 task 持久化进度（assignment metadata、heartbeat age、last checkpoint summary）。

**File Touch 与 Intent**：
- File touch notification 用于冲突检测。
- 可选的 `intent` 字段在 tool calls 上，提供 tool 意图的简短摘要，用于构建 summary activity feed。

**Hot-reload**：
1. self-dev session 改代码后调 `send_reload_signal()`。
2. 写 reload marker `jcode.reload`；`await_reload_signal()` 监听 channel。
3. `persist_reload_recovery_intents()` 把 running session 恢复意图写盘。
4. `graceful_shutdown_sessions()` 向所有 running session 发 `InterruptSignal`，等 checkpoint（2s timeout）。
5. `exec()` 替换为新二进制（`platform::replace_process`），传 `--socket` 重用 socket 路径。
6. 新 server 启动后恢复 marker、加载 recovery intent、续跑 headless sessions。

## 与 Ambient 系统的关系

- Server 构造时无条件创建 `AmbientRunnerHandle`（即使 ambient mode 禁用），用于 session-targeted scheduled tasks 投递循环。
- `spawn_background_tasks()` 中若 `ambient_runner` 存在则 spawn `ambient_handle.run_loop(ambient_provider)` 后台循环。
- **Channel 桥接**：`ChannelRegistry`（Telegram/Discord）reply_loop 调 `runner.inject_message()` 将外部消息注入当前 ambient cycle 或排队。
- **Nudge**：客户端断连时 server 调 `runner.nudge()` 唤醒 ambient loop 重新评估。
- **Schedule target**：`ScheduledItem.target` 支持 `Ambient`（默认，交 ambient runner）/ `Session`（投特定 session）/ `Spawn`（建新 session）。
- `UsageLog`/`AdaptiveScheduler` 跟踪 user vs ambient token 用量，自适应调节 ambient cycle 频率避免 rate limit。

## 依赖关系

- 依赖 [02 Agent](02-agent-runtime.md)（`process_message_streaming_mpsc` 驱动 Agent）、[11 Bus/Protocol](11-bus-message-protocol.md)（事件分发 + NDJSON）、[10 Gateway/Transport](10-gateway-transport.md)（socket 抽象）、[09 MCP](09-mcp.md)（MCP pool 初始化）、[03 Provider](03-provider.md)（`provider_control`）、[08 Storage](08-storage-session.md)（durable_state / swarm_persistence / reload_recovery）。
- 被 [01 CLI](01-cli.md) 的 `spawn_server` 孵化、`debug` 子命令连接。

## 陷阱与设计约束

- **`clippy::too_many_arguments` 泛滥**：`handle_client()`/`handle_debug_client()`/`create_headless_session()` 等核心函数参数普遍超 20 个，大量 `#[expect(...)]` suppress——`Arc<RwLock<...>>` 状态逐个传递（而非封装到 context 对象）的后果。
- **RwLock 竞争**：`SwarmState` 四个 `Arc<RwLock<HashMap>>` 在 file touch、status broadcast、plan update 等高频操作中被频繁获取写锁；`broadcast_swarm_status` 已加 debounce，但大量 `write().await` 仍在热路径。
- **`Arc<Mutex<Agent>>` 的 `try_lock`**：background_tasks/queue_soft_interrupt_for_session 多处用 `try_lock()` 避免阻塞，失败时可能丢事件或走降级路径（persisted soft interrupt store）。
- **Debug socket 安全**：经 `debug_control_allowed()` env var 控制开关，但 socket 文件权限与 main socket 相同（owner-only）；一旦启用可执行任意 server 命令（创建 headless、改 swarm plan、注入 transcript）。
- **Reload 的 exec 语义**：hot-reload 用 `exec()` 替换进程（Unix 特有），经 `JCODE_READY_FD` pipe 通知就绪；Windows/exec 失败时有 poll 回退；exec 前须断开 stdio（防 SIGPIPE）、关闭 socket `CLOEXEC` 标志使新进程继承 listener fd。
- **Swarm state 持久化一致性**：reload 短暂窗口中旧 server 最后写入与新 server 首次读取间可能 lost update（`clear_reload_marker_if_stale_for_pid` 只查 PID 不查 state version）。
- **`client_lifecycle.rs` select loop 优先级**：`biased` select 优先处理客户端 I/O，Bus 事件需 `client_subscribed` guard 才参与——防止 bus traffic starvation，但 subscribe 前发出的 bus 事件会丢失。
- **Temporary server owner PID**：依赖 `kill(pid, 0)` 探测 owner 存活，不处理 PID 回收复用（30min idle timeout 通常足够）。

## 回指

- 客户端侧（`RemoteConnection`）：[05-tui.md](05-tui.md)
- NDJSON wire 格式定义：[11-bus-message-protocol.md](11-bus-message-protocol.md)
- IPC 传输（socket / named pipe / WebSocket）：[10-gateway-transport.md](10-gateway-transport.md)
- 持久化与崩溃恢复（durable_state / reload_recovery）：[08-storage-session.md](08-storage-session.md)
