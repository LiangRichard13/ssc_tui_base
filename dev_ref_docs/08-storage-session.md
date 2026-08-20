# 08 · Storage / Session 持久化与崩溃恢复

> 子系统：JSON 持久化与自动损坏恢复，snapshot + journal 二级会话存储，PID 文件崩溃检测，重启快照。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

负责将 TUI 会话状态（Session）通过 snapshot + journal 二级存储格式持久化到磁盘，并提供基于 PID 文件检测的崩溃发现、自动备份恢复（`.bak` fallback）、以及重启快照（restart snapshot）的全生命周期管理。

## JSON 持久化与自动损坏恢复

核心写入函数在 `crates/jcode-storage/src/lib.rs`：

- **`write_json`**（durable）：先写带随机后缀的 `.tmp.{pid}.{nonce}` 临时文件 → `BufWriter` + `sync_all()`（fsync）→ 将已有目标文件 rename 为 `.bak` → atomic rename 临时文件为目标文件 →（Unix）fsync 父目录。失败时清理临时文件。
- **`write_json_fast`**：与 `write_json` 共享流程但跳过 `sync_all()`。适用于频繁保存（tool execution 期间 session save），防御进程崩溃（atomic rename 保证无半写文件）但不防御突然断电。
- **`write_json_secret`**：在 `write_json` 基础上追加 `0o600` 文件权限 + `0o700` 目录权限。
- **`read_json` / `read_json_with_recovery_handler`**：先读主文件反序列化；失败且 `.bak` 存在则从 `.bak` 恢复，恢复成功后回拷 `.bak` 到主路径。两次均失败报错。`read_json_with_recovery_handler` 接受回调供上层注入日志。
- **`append_json_line_fast`**：append-only JSONL 追加，不做 fsync，用于 journal 文件。
- **备份文件**：每次 atomic write 时将旧目标 rename 为 `.bak`，仅保留最近一个版本（非滚动）。

## 关键类型与函数

| 类型/函数 | 定义位置 | 职责 |
|---|---|---|
| `SessionJournalMeta` | `src/session/journal.rs:10` | Journal 条目元数据快照（parent_id/title/status/last_pid/compaction 等全部 Session 标量字段） |
| `SessionJournalEntry` | `src/session/journal.rs:38` | 单条 journal 记录：meta + 4 个增量向量（append_messages/append_env_snapshots/append_memory_injections/append_replay_events） |
| `SessionPersistState` | `src/session/journal.rs:59` | 内存中的 persist 追踪器（serde skip）：记录 snapshot 是否存在 + 4 向量已持久化长度与脏模式，决定下次 save 走 snapshot 还是 journal append |
| `PersistVectorMode` | `src/session/journal.rs:51` | 三态枚举 `Clean`/`Append`/`Full`，控制向量持久化策略 |
| `detect_crashed_sessions` | `src/session/crash.rs:132` | 扫描 sessions 目录检测 `SessionStatus::Crashed`，排除已被 recovery 覆盖的，应用 60s crash window 过滤 |
| `recover_crashed_sessions` | `src/session/crash.rs:12` | 对 crashed session 创建 `session_recovery_{id}` 新 session，只保留 Text block（丢弃 ToolUse/ToolResult/Image），复制元数据，设状态 Closed |
| `find_recent_crashed_sessions` | `src/session/crash.rs:248` | 快速路径：扫 `~/.ssc_tui/active_pids/` 查已死 PID 的 session（O(n) n 通常 0-5）；降级路径：全量扫描 sessions 目录 |
| `register_active_pid` / `unregister_active_pid` | `src/session/active_pids.rs:7`/`:14` | 将 `{session_id}` 为文件名、PID 为内容写入/删除 `~/.ssc_tui/active_pids/` |
| `Session::save` | `src/session/persistence.rs:170` | 核心保存：根据 `persist_state` 判断走 snapshot checkpoint 还是 journal append；journal 超 512KB 时自动触发 checkpoint |
| `Session::load` / `load_from_path` | `src/session/persistence.rs:32,91` | 加载：先读 snapshot JSON，再逐行读 `.journal.jsonl` replay 合成最终状态 |
| `RestartSnapshot` | `src/restart_snapshot.rs:9` | 重启快照：version + created_at + auto_restore_on_next_start + sessions 列表 |
| `StorageRecoveryEvent` | `crates/jcode-storage/src/lib.rs:300` | `read_json` 损坏恢复回调枚举：`CorruptPrimary`/`RecoveredFromBackup` |
| `SessionStatus` | `crates/jcode-session-types/src/lib.rs:38` | 会话状态枚举：Active/Closed/Crashed/Reloaded/Compacted/RateLimited/Error |
| `StoredMessage` | `crates/jcode-session-types/src/lib.rs:152` | 持久化消息：id + role + content（ContentBlock vec）+ display_role + timestamp + token_usage |

## 关键文件清单

| 路径 | 职责 |
|---|---|
| `crates/jcode-storage/src/lib.rs` | 底层 JSON 文件 I/O：atomic write（durable/fast）、read-with-recovery、append-jsonl、路径工具（`jcode_dir`/`runtime_dir`/`app_config_dir`）、文件权限加固 |
| `crates/jcode-session-types/src/lib.rs` | Session 相关纯数据类型（SessionStatus/StoredMessage/EnvSnapshot/StoredCompactionState/SessionSearch） |
| `crates/jcode-core/src/fs.rs` | Unix 文件权限 helper（0o600/0o700，Windows no-op） |
| `src/storage.rs` | App 层 storage 门面：re-export `jcode-storage` 全部公共 API，包装 `read_json` 注入 app 级日志回调 |
| `src/session.rs` | Session 主模块入口：定义 `Session` 结构体（1400+ 行）、pub use 导出、journal_meta/persist_state 管理、消息增删改、provider 缓存、memory profile |
| `src/session/journal.rs` | Journal 内部类型（SessionJournalMeta/SessionJournalEntry/PersistVectorMode/SessionPersistState/metadata_requires_snapshot） |
| `src/session/persistence.rs` | Session 的 load/save/checkpoint_snapshot 逻辑：snapshot checkpoint + journal append + 512KB journal 自动合并 |
| `src/session/crash.rs` | 崩溃检测与恢复（detect/recover/find_recent/find_session_by_name_or_id，PID 快速路径 + legacy 全量扫描降级） |
| `src/session/active_pids.rs` | PID 文件管理 |
| `src/session/storage_paths.rs` | 路径计算（session_path/session_journal_path/session_journal_path_from_snapshot/session_exists） |
| `src/session/model.rs` | `StoredReplayEvent`/`StoredReplayEventKind`（display_message/swarm_status/swarm_plan） |
| `src/session/memory_profile.rs` | `ContentBlockMemoryStats`/`SessionMemoryProfileCache`/`SessionMemoryProfileSnapshot`（内存占用分析） |
| `src/session/render.rs` | 将 `StoredMessage` 渲染为 UI 可用的 `RenderedMessage`/`RenderedImage` |
| `src/restart_snapshot.rs` | 重启快照管理（save/load/clear/arm_auto_restore_from_recent_crashes/restore_snapshot/capture_current_snapshot） |
| `src/platform.rs` | `is_process_running(pid)`：跨平台 PID 存活查询（崩溃检测用） |

## 崩溃检测与恢复流程

**检测阶段**（`find_recent_crashed_sessions`）：
1. 快速路径：遍历 `~/.ssc_tui/active_pids/`（通常 0-5 文件），每文件内容为 PID。
2. `is_pid_running(pid)` 检查进程存活。
3. PID 已死则加载对应 session JSON，调 `session.mark_crashed(...)` 设 status 为 `Crashed`。
4. 过滤 24h 内记录，按时间倒序返回。
5. `active_pids/` 不存在（升级后首次运行）时降级到 `find_crashed_legacy_scan`：全量扫描 sessions 目录，用文件名时间戳和 mtime 双重过滤，先做字符串 `"Crashed"` 预过滤避免不必要 JSON 解析。

**恢复阶段**（`recover_crashed_sessions`）：
1. 加载所有 session，对 Active 状态调 `detect_crash()`（PID 检查，无 PID 时用 120s 超时启发）。
2. 收集 Crashed 状态 session，应用 60s crash window。
3. 排除已被 `session_recovery_*` 子 session 覆盖的父 session（去重）。
4. 为每个 crashed session 创建新 session（`session_recovery_{id}`），复制全部元数据，仅保留 Text content block（丢弃 tool use/result/image），设 status 为 Closed，附加 recovery header message。

**主动 PID 注册**：
- `Session::mark_active` 调 `register_active_pid`，在 `~/.ssc_tui/active_pids/{session_id}` 写当前进程 PID。
- `Session::mark_closed`/`mark_crashed` 调 `unregister_active_pid` 删文件。
- 正常退出时 PID 文件清理，异常退出时残留供下次启动检测。

## StorageRecoveryEvent 回调机制

```rust
pub enum StorageRecoveryEvent<'a> {
    CorruptPrimary { path: &'a Path, error: &'a serde_json::Error },
    RecoveredFromBackup { backup_path: &'a Path },
}
```
`read_json_with_recovery_handler` 在主文件反序列化失败时触发 `CorruptPrimary`，从 `.bak` 恢复成功后触发 `RecoveredFromBackup`。默认 `read_json` 用 `eprintln!`；app 层 `src/storage.rs` 包装版本路由到 `crate::logging::warn/info`，确保走 TUI 日志系统而非 stderr。

## restart_snapshot 的角色

`src/restart_snapshot.rs` 管理 `~/.ssc_tui/restart-snapshot.json`：
- `capture_current_snapshot`：遍历 `active_session_ids()`，过滤 Active 且未 crash 的 session 生成 `RestartSnapshot`。
- `arm_auto_restore_from_recent_crashes`：扫近期 crashed session（24h 内）生成带 `auto_restore_on_next_start = true` 的快照，用于崩溃后自动恢复。
- `restore_snapshot`：读快照，为每个 session spawn 新终端窗口恢复会话。
- `save_current_snapshot`/`clear_snapshot`/`set_auto_restore_on_next_start`：保存/清除/控制自动恢复标志。

本质：restart_snapshot 是「要恢复哪些 session」的清单，与 crash detection 协作——crash detection 负责发现和标记，restart_snapshot 负责下次启动时重新拉起。

## crates/jcode-storage 与 src/storage 的分层

- **`crates/jcode-storage`**（底层 crate）：所有文件 I/O 原语——`write_json`/`write_json_fast`/`write_json_secret`/`read_json`/`read_json_with_recovery_handler`/`append_json_line_fast`/路径工具/权限加固/env 文件操作。不依赖 app 层日志。
- **`src/storage.rs`**（app 层门面）：`pub use jcode_storage::*;` re-export 全部，仅覆盖 `read_json` 注入 app 级 `StorageRecoveryEvent` 日志回调。还提供测试环境锁（`test_env_lock`/`lock_test_env`）。

分层原则：crate 不依赖 app，app 可替换 crate 默认行为（目前仅覆盖 recovery handler）。

## 依赖关系

- 被 [02 Agent](02-agent-runtime.md)（session save/load）、[04 Server](04-server.md)（durable_state/swarm_persistence/reload_recovery/background_tasks）、[05 TUI](05-tui.md)（session picker / resume）、[07 Memory](07-memory.md)（MemoryGraph 持久化）依赖。
- 依赖 [12 Workspace](12-workspace-build-ci.md)（`jcode-storage`/`jcode-session-types`/`jcode-core`）、`src/platform.rs`（`is_process_running`）。

## 陷阱与设计约束

- **`.bak` 仅保留一份**：`write_bytes_inner` 每次写入前 rename 旧文件为 `.bak`，只保留最近一个备份；需回溯更早版本时当前机制不支持。
- **`write_json_fast` 无 fsync**：注释「safe against process crashes but not power loss」；tool execution 期间 session save 的权衡合理，但写入瞬间断电可能丢失最近一次 journal append。
- **journal 单行解析失败即中断**：`persistence.rs:57` 的 journal replay 循环遇 JSON parse error 时 `break` 而非 `continue`——journal 文件中间一行损坏会导致后续所有条目被丢弃。journal 是 append-only，最后一行不完整常见（进程被 kill 时写一半），但中间行损坏也会截断恢复。
- **PID 复用风险**：PID 文件方案依赖「PID 是否被占用」判断 session 是否 crash；OS 可能复用 PID，导致新进程意外关联到旧 session 的 active_pid 文件。无 PID + 启动时间戳交叉验证。
- **crash window 60 秒硬编码**：`recover_crashed_sessions`/`detect_crashed_sessions` 用 `Duration::seconds(60)`；多台机器同时 crash（批量重启）超出 60s 窗口的 session 会被忽略。
- **legacy scan 字符串预过滤**：`find_crashed_legacy_scan` 用 `content.contains("\"Crashed\"")` 粗筛，会误匹配消息内容含 "Crashed" 文本的非 crashed session（后续 JSON 反序列化精确过滤兜底）。
- **recovery session 只保留 Text block**：`recover_crashed_sessions` 丢弃所有 ToolUse/ToolResult/Image content block——恢复后 session 丢失工具调用上下文，用户看不到 crash 前的工具执行历史。
- **`session_journal_path_from_snapshot` 命名规则**：journal 文件名由 snapshot 文件名 stem + `.journal.jsonl` 拼接（如 `session_fox.json` → `session_fox.journal.jsonl`）；session ID 本身含 `.journal` 子串可能产生命名歧义。
- **Windows 权限 no-op**：`jcode-core::fs::set_permissions_owner_only`/`set_directory_permissions_owner_only` 在 Windows 是 no-op——secret 文件（如 `openrouter.env`）在 Windows 上不会被限制权限。
- **`Session::save` 无事务保证**：snapshot checkpoint 先写新 snapshot 再删 journal；删 journal 前 crash 则下次 load 读到新 snapshot + 旧 journal 残余条目，可能产生重复消息（journal append-only 且 `apply_journal_entry` 是 `extend`，重复消息唯一影响是内存膨胀而非逻辑错误）。

## 关联模块

| 模块 | 路径 | 职责 | 规模 |
|---|---|---|---|
| `src/import.rs` + `src/import_tests.rs` | 导入 Claude Code/Codex/OpenCode/Pi 会话到 jcode——发现、解析 JSONL session 文件转 jcode Session；核心解析在 `jcode-import-core` | ~1427 行 |
| `src/catchup.rs` | "Catch Up" 功能——用户回到已更新 session 时自动生成 catch-up brief（mermaid 流程图 + 活动步骤 + 文件变更 + 工具调用统计 + 验证说明） | ~618 行 |
| `src/replay.rs` + `src/replay/` | 会话回放——`export_timeline()` 从 Session 生成 `TimelineEvent` 序列、`timeline_to_replay_events()` 转播放事件、`auto_edit_timeline()` 压缩死时间、swarm 多 pane 合成 | ~969 行 |
| `src/id.rs` / `src/util.rs` | ID 生成、HTTP 错误体读取 + anyhow 错误链格式化（均 re-export `jcode-core`） | 60 行 |

**Note**：replay 核心数据直接遍历 `session.messages`，与 session 持久化格式深度耦合，故归入本文档而非独立成篇。视频导出管线（SVG→PNG→MP4）见 [05-tui.md](05-tui.md) 关联模块的 `video_export.rs`。

## 回指
- Server 的 durable_state / reload_recovery / swarm_persistence：[04-server.md](04-server.md)
- TUI session picker / resume：[05-tui.md](05-tui.md)
