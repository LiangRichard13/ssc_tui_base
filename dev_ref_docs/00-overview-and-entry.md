# 00 · 总览与入口

> 子系统：程序入口执行流、workspace 全景、跨子系统鸟瞰。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

`jcode` 是一个 **51-crate Rust workspace（edition 2024）** 的多模型 AI 编程 agent，入口执行流 `main.rs → lib.rs::run() → cli::startup::run() → dispatch`，在 Windows 上需 8MB 栈线程防止 clap 命令树解析爆栈。

## 入口执行流

```
src/main.rs           fn main()
  └─ (Windows) 8MB 栈专用线程执行 run_app()，防止 clap 命令树爆栈
  └─ run_app():
       ├─ 配置 glibc arena 上限
       ├─ 安装 rustls crypto provider
       ├─ 构建 tokio multi-thread runtime
       └─ block_on(jcode::run())

src/lib.rs            pub async fn run()
  └─ cli::startup::run().await   // 一行，导出 ~70 个 pub mod

src/cli/startup.rs    pub async fn run()
  ├─ startup_profile::init()             // 启动耗时打点
  ├─ terminal::install_panic_hook()      // panic 时打印 session resume hint
  ├─ logging::init() + 清理旧日志
  ├─ platform::raise_nofile_limit_best_effort(8192)
  ├─ storage::harden_user_config_permissions()
  ├─ perf::init_background()
  ├─ telemetry::record_install_if_first_run() / record_upgrade_if_needed()
  ├─ parse_and_prepare_args()            // clap 解析 + process_title
  ├─ spawn_background_update_check()
  └─ dispatch::run_main(args).await      // 进入命令分发
```

## 关键文件清单

| 路径 | 职责 |
|---|---|
| `src/main.rs` | 二进制入口，Windows 8MB 栈线程 + runtime 初始化 |
| `src/lib.rs` | crate 根，`run()` 单行委托到 `cli::startup`，导出全部 `pub mod` |
| `src/cli/startup.rs` | 进程启动编排（日志/权限/遥测/参数/更新检查）→ `dispatch::run_main()` |
| `src/cli/dispatch.rs` | 命令分发核心：巨型 match on `Command`，含 `run_default_command()`（无子命令 TUI 路径）、`detect_bootstrap_credentials()`、server 孵化 |
| `src/platform.rs` | 跨平台 OS 抽象（fd 上限、symlink、权限、进程存活检查、`replace_process` exec/spawn） |
| `src/terminal_launch.rs` | 新终端窗口孵化（re-export `jcode-terminal-launch`） |
| `src/process_title.rs` | 进程标题管理（Linux `prctl(PR_SET_NAME)` 15 字节名） |
| `src/stdin_detect.rs` / `src/env.rs` | re-export `jcode-core` 的 stdin 检测 / 跨平台 env var 封装 |

## 子系统鸟瞰（跨文档地图）

```
用户输入
  ↓
[01 CLI] 参数解析 → dispatch
  ↓
[04 Server] accept loop（main/debug/gateway 三 socket）
  ↓ handle_client()
[02 Agent] run_turn() 循环
  ├─ build_system_prompt (split: static/dynamic)
  ├─ [03 Provider] complete_split() 流式响应
  ├─ 解析 tool calls → [02 Registry] 执行
  │      ├─ native tools (selfdev/communicate)
  │      └─ [09 MCP] call_tool (SAITEC-Skills 等)
  ├─ [07 Memory] 注入 / 提取
  └─ context-limit 自动 [02 compaction] + retry
  ↓
[11 Bus] 广播事件 (ToolEvent/TokenUsage/...)
  ↓
[05 TUI] RemoteConnection.next_event() → 渲染
  ↓
[08 Storage] snapshot+journal 持久化
```

横向支撑层：
- **[06 Auth]** 为 03 Provider 与 09 SAITEC 提供凭据
- **[10 Gateway/Transport]** 为 04 Server 提供 IPC 传输（Unix socket / Windows Named Pipe / WebSocket）
- **[11 Protocol]** 定义 04↔05 之间的 NDJSON wire 格式
- **[12 Workspace/Build/CI]** 定义 51 个 crate 的分层与构建链

## 依赖关系

- `main.rs` / `lib.rs` 是顶层，依赖全部 `src/*` 子模块。
- `cli::startup` → `cli::dispatch` → 各具体命令模块（`commands`、`login`、`tui_launch`、`selfdev`、`debug` 等）。
- 入口层刻意保持薄：只做进程级初始化（runtime/日志/权限/遥测），业务逻辑全在下游。

## 陷阱与设计约束

- **Windows 栈溢出防护**：clap 命令树已大到默认 Windows 主线程栈解析时会溢出，必须在 8MB 栈专用线程运行。新增子命令时留意命令树膨胀。
- **`run_default_command` 是最复杂路径**：串联 self-dev 检测 → restart restore → server 运行状态 → reload 状态等待 → provider bootstrap → server 孵化 → TUI 启动。新增「默认行为」需理解完整链路。
- **auto-update 与 TTY**：`should_auto_update` 仅在无 live terminal 时自动安装（后台/管道场景），有终端推迟以免打断用户。
- **`--headless` 是 `--no-browser` 的 alias**（Login 命令），文档中提及 `--headless` 实指 `--no-browser`。

## 回指

- 构建命令与 workspace 全景细节：见 [CLAUDE.md](../CLAUDE.md) 顶部「Build & Test Commands」与 [12-workspace-build-ci.md](12-workspace-build-ci.md)。
- 各子命令与 `auth_test` 框架：见 [01-cli.md](01-cli.md)。
