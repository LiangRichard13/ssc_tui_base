# 01 · CLI 与程序入口

> 子系统：命令行参数解析（clap）、子命令分发、凭据探测、auth_test 端到端验证框架、平台/终端适配。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md) · 配套：[00-overview-and-entry.md](00-overview-and-entry.md)

## 职责一句话

CLI 子系统负责进程启动后的 clap 参数解析、子命令分发调度、凭据探测与引导式登录、后台自动更新、server 进程孵化、TUI 终端生命周期管理与进程标题/平台适配等横切关注点。

## 关键文件清单

| 路径 | 职责 |
|---|---|
| `src/cli/mod.rs` | 声明 `cli` 子模块结构（args, auth_test, commands, debug, dispatch, hot_exec, login, output, provider_init, selfdev, startup, terminal, tui_launch） |
| `src/cli/args.rs` | clap `Parser` 定义：顶层 `Args` + `Command` 子命令枚举（Serve/Connect/Run/Login/Repl/Update/Version/Usage/SelfDev/Debug/Auth/Provider/Memory/Session/Ambient/Pair/Permissions/Transcript/Dictate/SetupHotkey/SetupLauncher/Browser/Replay/Model/AuthTest/Restart 等） |
| `src/cli/startup.rs` | 进程启动编排 → `dispatch::run_main()`（详见 [00](00-overview-and-entry.md)） |
| `src/cli/dispatch.rs` | 命令分发核心；含 `run_default_command()`（无子命令 TUI 路径）、`detect_bootstrap_credentials()`、server 孵化 `spawn_server()` |
| `src/cli/commands.rs` | 非 TUI 子命令实现（run/version/usage/auth_status/model/memory/ambient/pair/browser/transcript 等） |
| `src/cli/login.rs` | 多 provider 登录流程分发（Claude/OpenAI/OpenRouter/Bedrock/Azure/OpenAI-Compatible/Cursor/Copilot/Gemini/Antigravity/Google）；scriptable login（`--print-auth-url`/`--callback-url`） |
| `src/cli/login/scriptable.rs` | scriptable login 辅助：headless 检测、auth URL 打印、pending login 磁盘读写 |
| `src/cli/auth_test.rs` + `auth_test/` | 端到端认证验证框架（types/run/probes/choice，用 `include!` 拆分） |
| `src/cli/provider_init.rs` | `ProviderChoice` 枚举（~30 provider 变体）+ `init_provider()` / `init_provider_and_registry()` |
| `src/cli/provider_init/external_auth.rs` | 外部 auth 自动导入（Claude Code / Codex / OpenCode / Pi 迁移凭据） |
| `src/cli/terminal.rs` | TUI 终端生命周期：panic hook、`init_tui_runtime()` / `cleanup_tui_runtime()`（crossterm raw mode/bracketed paste/mouse）、Unix signal watcher |
| `src/cli/tui_launch.rs` | TUI 客户端启动：`run_tui_client()`、`run_replay_command()`、`list_sessions()` picker、跨平台新终端窗口孵化 |
| `src/cli/output.rs` | quiet 模式控制 |
| `src/cli/debug.rs` | `debug` 子命令：经 debug socket 与运行中 server 通信 |
| `src/cli/hot_exec.rs` | 热更新/热重载/热重建/热重启（`hot_reload`/`hot_rebuild`/`hot_update`/`hot_restart`，均经 `platform::replace_process()`） |
| `src/cli/selfdev.rs` | Self-dev 模式：canary session、可选 build、孵化 server、启动 TUI client |

## 核心类型与关键函数

- **`Args`** (`args.rs`) — clap 顶层，全局 flag（`--provider`/`-p`、`--model`/`-m`、`--provider-profile`、`--cwd`、`--no-update`、`--trace`、`--quiet`、`--resume`、`--socket`、`--debug-socket` 等）+ `Command` 子命令枚举。
- **`Command`** 枚举 — ~30 子命令（见上表清单）。
- **`ProviderChoice`** (`provider_init.rs`) — ~30 provider 变体；含 deprecated `ClaudeSubprocess`（仍保留 alias/hide）。
- **`detect_bootstrap_credentials()`** (`dispatch.rs:720`) — 并发（`tokio::join!` + `spawn_blocking`）探测 6 类凭据：Claude OAuth、OpenAI OAuth/API key、OpenRouter、GitHub Copilot、Anthropic API key、磁盘 openai-compatible profile（含 env 文件扫描）。返回 `BootstrapCredentialState { has_any }`，`false` 且 provider 为 Auto 时触发交互式选择/登录。
- **`run_default_command()`** (`dispatch.rs:395`) — 无子命令时的 TUI 启动主路径（详见 [00](00-overview-and-entry.md) 陷阱）。
- **`init_provider()` / `init_provider_and_registry()`** — 构造 `Provider` 实例 + `Registry`（tools + skills）。

## 子命令概要

| 命令 | 用途 |
|---|---|
| `serve` | 后台 daemon（`--temporary-server`/`--owner_pid`/`--temp-idle-timeout-secs` 内部用） |
| `connect` | 连接已运行 server（简单 REPL） |
| `run <msg>` | 单消息执行后退出（`--json`/`--ndjson`，含 auto-poke） |
| `login` | 多 provider 登录（`--no-browser`/`--headless`/`--print-auth-url`/`--callback-url`/`--auth-code`/`--complete`） |
| `repl` | 无 TUI 的 REPL |
| `update` / `version` / `usage` | 更新/版本/用量（后两者支持 `--json`） |
| `self-dev [--build]` | self-dev 模式 |
| `debug <cmd>` | 与运行 server 的 debug socket 交互 |
| `auth status` / `auth doctor` | 认证状态/诊断 |
| `provider list/current/add` | provider 管理 |
| `memory list/search/export/import/stats` | 记忆管理 |
| `session rename` / `ambient status/log/trigger/stop` / `pair` / `permissions` | 各功能子命令 |
| `transcript` / `dictate` | 转录注入 / 语音听写 |
| `setup-hotkey` / `setup-launcher` / `browser setup` | 热键/launcher/浏览器自动化 |
| `replay <session>` | session 回放（`--swarm`/`--export`/`--video`/`--speed`） |
| `model list` | 模型列表 |
| `auth-test` | 端到端认证验证（`--login`/`--all-configured`/`--no-smoke`/`--no-tool-smoke`/`--prompt`/`--json`） |
| `restart save/restore/status/clear` | 重启快照管理 |

## auth_test 模块

`jcode auth-test` 子命令的端到端认证验证框架，流程：
1. **目标解析** `resolve_auth_test_targets`：按 `--provider` 或 `--all-configured` 确定目标（Detailed: Claude/OpenAI/Gemini/Antigravity/Google/Copilot/Cursor；Generic: 全部 openai-compatible preset）。
2. **可选登录** `--login`：先执行交互式登录。
3. **凭据探测** `probes.rs`：`credential_probe`（凭据文件有效性）+ `refresh_probe`（实际 refresh 端点调用）。
4. **Provider smoke**（`--no-smoke` 可跳过）：发 `"Reply with exactly AUTH_TEST_OK"` 验证响应。
5. **Tool smoke**（`--no-tool-smoke` 可跳过）：带 MCP tool 的 agent turn 验证。
6. **重试** `run_auth_test_with_retry`：transient 错误（429/5xx/timeout/reset）自动重试 2 次（3s + 8s）。
7. **持久化** `persist_auth_test_report`：写 `auth-validation.json`，供后续 provider 初始化参考。
8. **post-login 验证** `run_post_login_validation`：每次 login 成功后自动触发轻量验证。

## 依赖关系

- 依赖 [03 Provider](03-provider.md)（`init_provider`）、[06 Auth](06-auth-login.md)（`login.rs` 各 flow、`detect_bootstrap_credentials`）、[04 Server](04-server.md)（`spawn_server`）、[05 TUI](05-tui.md)（`tui_launch`）、[12 Workspace](12-workspace-build-ci.md)（`jcode-terminal-launch` / `jcode-core`）。
- `hot_exec.rs` 经 `platform::replace_process()` 调用 [04 Server](04-server.md) 的 reload 机制。

## 陷阱与历史修复

### auth_test 的 `include!` 宏拆分

`auth_test.rs` 用 `include!`（而非 `mod`）引入 `types.rs`/`run.rs`/`probes.rs`/`choice.rs`，共享同一模块作用域，减少 `pub(super)` 注解但让文件边界不明显，IDE 跳转可能不准。

### AuthTest deadlock from stale `auth-validation.json`（fixed `b18a4c17`）

**Symptom**：在 login picker 按 `R` 显示 `validation failed (just now)` 但无详情。读 `~/.saitec_tui/auth-validation.json` 看实际错误。

**Root cause**：stale `success: false` 行 → `state_for_provider` 返回 `Expired` → probe 在 smoke 前短路 → 写入新失败 → 锁死状态。

**Fix**（`src/cli/auth_test/probes.rs`）：对 `OpenAiCompatible` 目标，绕过 stale `Expired`，直接调 `openai_compatible_profile_is_configured()` 判定。详见 [06-auth-login.md](06-auth-login.md) 同名小节。

### `--headless` 是 `--no-browser` 的 alias

`args.rs` Login 命令 `#[arg(long, alias = "headless")]`，文档/脚本中 `--headless` 实指 `--no-browser`。

### `hot_rebuild` 含 `cargo test`

`hot_exec.rs` 中 `hot_rebuild` 先 `cargo build --release` 再 `cargo test --release`，测试失败中止重载——安全措施但 `/rebuild` 等待较久。

### Unix spawn lock 仅 Unix

`acquire_spawn_lock_or_wait` 用 `flock` 防并发 spawn server，仅在 `#[cfg(unix)]` 存在；Windows 并发启动客户端可能有竞态。

## 回指

- 入口执行流上下文：[00-overview-and-entry.md](00-overview-and-entry.md)
- 登录流程细节（`PendingLogin` 状态机、OAuth PKCE）：[06-auth-login.md](06-auth-login.md)
- server 孵化与 dispatch 调用的 server 运行状态：[04-server.md](04-server.md)
