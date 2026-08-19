# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> **本文档仅作目录与索引。** 架构细节、子系统剖析、陷阱与历史修复记录都已迁移到 [`dev_ref_docs/`](dev_ref_docs/README.md) 下的分模块文档。需要细节时按下方索引跳转。

## Build & Test Commands

```powershell
cargo check                           # quick check (fastest)
cargo build                           # debug build
cargo build --release                 # release build
cargo test                            # all tests
cargo test <name> -- --nocapture      # single unit test
cargo test --test e2e <name> -- --exact --nocapture  # single e2e test
cargo test -p jcode-core              # specific crate
cargo fmt --all -- --check            # format check
cargo clippy --all-targets --all-features -- -D warnings
.\scripts\dev_ssc_tui.ps1 [-Profile selfdev]  # build dev/selfdev
.\scripts\dev_ssc_tui.ps1 -StopRunning -NoBuild  # stop dev instance
cargo build --release; .\scripts\package_ssc.ps1  # package dist
scripts/remote_build.sh               # remote build (low resources)
```

E2E tests: `tests/e2e/`（mock provider，无 API 调用）。Modules: `ambient`, `binary_integration`, `burst_spawn`, `provider_behavior`, `safety`, `session_flow`, `transport`, `windows_lifecycle`。
Unit tests: 各 source 内 inline `#[cfg(test)]`（~64 处）。
Budget scripts in `scripts/`（code size, panics, test size, warnings）— CI 中运行。详见 [dev_ref_docs/12](dev_ref_docs/12-workspace-build-ci.md)。

## 项目鸟瞰

**jcode** 是一个 **51-crate Rust workspace（edition 2024）** 的多模型 AI 编程 agent monorepo。入口执行流：`main.rs → lib.rs::run() → cli::startup::run() → dispatch`（Windows 上需 8MB 栈线程防 clap 爆栈）。

一个用户请求的流转：**[01 CLI](dev_ref_docs/01-cli.md)** 解析 → **[04 Server](dev_ref_docs/04-server.md)** 分发 → **[02 Agent](dev_ref_docs/02-agent-runtime.md)** turn 循环 → **[03 Provider](dev_ref_docs/03-provider.md)** 调 LLM → **[11 Bus/Protocol](dev_ref_docs/11-bus-message-protocol.md)** 事件回流 → **[05 TUI](dev_ref_docs/05-tui.md)** 渲染；持久化走 **[08 Storage](dev_ref_docs/08-storage-session.md)**。

## doc_ref 索引（按子系统）

| # | 文档 | 子系统 |
|---|---|---|
| 00 | [总览与入口](dev_ref_docs/00-overview-and-entry.md) | 入口执行流、workspace 全景、跨子系统鸟瞰 |
| 01 | [CLI](dev_ref_docs/01-cli.md) | clap 参数解析、子命令分发、`detect_bootstrap_credentials`、`auth_test` 验证框架 |
| 02 | [Agent Runtime](dev_ref_docs/02-agent-runtime.md) | `run_turn` 循环、split system prompt、CompactionManager、tool/skill Registry |
| 03 | [Provider](dev_ref_docs/03-provider.md) | 8 内置 + 30 OpenAI-compatible profile、`MultiProvider` facade、`JCODE_OPENROUTER_*` env、failover |
| 04 | [Server](dev_ref_docs/04-server.md) | 多进程运行时、三 accept loop、swarm 协调、headless、hot-reload exec、ambient |
| 05 | [TUI](dev_ref_docs/05-tui.md) | ratatui 0.30、`RemoteConnection` NDJSON、渲染管线、`App` God-object |
| 06 | [Auth / Login](dev_ref_docs/06-auth-login.md) | 两层 login 架构、`PendingLogin` 13 variant、AuthStatus 30s/5s 缓存、StartupGuide |
| 07 | [Memory](dev_ref_docs/07-memory.md) | 跨会话记忆、MemoryGraph、Sidecar、embedding、GRAPH_VERSION=2 |
| 08 | [Storage / Session](dev_ref_docs/08-storage-session.md) | JSON 持久化、snapshot+journal、`.bak` 恢复、PID 崩溃检测、restart_snapshot |
| 09 | [MCP / SSC](dev_ref_docs/09-mcp-ssc.md) | JSON-RPC、`SharedMcpPool`、SSC-Skills HTTP、凭据三件套、lifecycle sync |
| 10 | [Gateway / Transport](dev_ref_docs/10-gateway-transport.md) | WebSocket Gateway（TCP:7643）、Unix socket / Windows Named Pipe 抽象 |
| 11 | [Bus / Message / Protocol](dev_ref_docs/11-bus-message-protocol.md) | `Bus` broadcast(256) ~25 事件、`ServerEvent` ~40 variant、NDJSON wire 格式 |
| 12 | [Workspace / Build / CI](dev_ref_docs/12-workspace-build-ci.md) | 51-crate 分组、feature flags、build profiles、build.rs、CI 5 job、budget 脚本 |
| 13 | [Configuration](dev_ref_docs/13-config.md) | `config.toml` 加载、`OnceLock` 单例、80+ `JCODE_*` env override、`jcode-config-types` |
| 14 | [Telemetry](dev_ref_docs/14-telemetry.md) | 匿名遥测采集、Cloudflare Worker + D1、opt-out、`TELEMETRY.md` |
| 15 | [Update](dev_ref_docs/15-update.md) | 自动更新、stable(GitHub Release)/main(源码编译)双 channel、SHA256、`jcode-update-core` |
| 16 | [Overnight](dev_ref_docs/16-overnight.md) | 无人值守长时运行、安全系统（SafetySystem）、通知分发、ambient 高级变体 |

> 各现有文档末尾的「关联模块」小节以小表格列出归入该文档的辅助模块（goal/catchup/import/usage/side_panel/perf 等），保证可发现性，后续轮次深化。

入口导航：[dev_ref_docs/README.md](dev_ref_docs/README.md) 含按关注点的跨文档线索（用户请求流转、凭据与登录、持久化与恢复、远程客户端、多 agent 协作、无人值守运行）。

## 按关注点速查

- **想理解一个用户请求怎么流转**：01 → 04 → 02 → 03 → 11 → 05
- **想理解凭据与登录**：06 + 09（SSC 凭据三件套）+ 03（Provider 凭据探测）
- **想理解持久化与崩溃恢复**：08 + 04（durable_state / reload_recovery）
- **想理解远程客户端（iOS/Web）**：10 + 11 + 04（accept loop）
- **想理解多 agent 协作**：04（swarm*）+ 02（subagent Registry clone）+ 11（SwarmEvent）

## Target Platforms

- Windows x64（primary）、Windows ARM64
- Linux x86_64、macOS aarch64
- Mobile simulator（iOS via `jcode-mobile-core` + `jcode-mobile-sim`）

## Project Memory

接手项目后的 bug 修复记录与架构发现已迁移到 [`dev_ref_docs/`](dev_ref_docs/README.md) 各文档的「陷阱与历史修复」小节。汇总索引见 [dev_ref_docs/12-workspace-build-ci.md](dev_ref_docs/12-workspace-build-ci.md#历史修复记录汇总索引)。

| 修复 | 涉及 commit | 归档位置 |
|---|---|---|
| SSC-Skills HTTP transport（no local vendor） | — | [09](dev_ref_docs/09-mcp-ssc.md) |
| OpenAI-compatible: config vs credentials env hygiene | — | [03](dev_ref_docs/03-provider.md) |
| AuthTest deadlock from stale `auth-validation.json` | `b18a4c17` | [01](dev_ref_docs/01-cli.md) + [06](dev_ref_docs/06-auth-login.md) |
| MCP `notifications/initialized` 无 `id` + NDJSON reconnect storm + Unknown tool 链 | `fix/mcp-notification-id` | [09](dev_ref_docs/09-mcp-ssc.md) + [05](dev_ref_docs/05-tui.md) + [11](dev_ref_docs/11-bus-message-protocol.md) |
| OpenAI-compatible 200K / `anthropic/claude-sonnet-4` regression | `e05304a1` | [03](dev_ref_docs/03-provider.md) |
| Config.toml named provider `JCODE_OPENROUTER_MODEL` symmetric cleanup | after `e05304a1` | [03](dev_ref_docs/03-provider.md) |
| Restart endpoint reversion to localhost:11434 from stale Ollama marker | `dba79fc3` | [03](dev_ref_docs/03-provider.md) |
