# doc_ref — SAITEC-TUI 架构参考文档

本目录是 SAITEC-TUI（根 crate `jcode`）的架构参考文档集，按子系统切分，**一个模块对应一个 md**。
项目根的 `CLAUDE.md` 仅作目录与索引，引导到此处的各文档；细节描述都在这里。

## 阅读约定

- 每篇文档统一模板：**职责一句话** → **关键文件清单** → **核心类型/函数** → **主控制流** → **依赖关系** → **陷阱与历史修复** → **回指 CLAUDE.md**。
- 文档为**骨架级**：先建立大致框架，后续会持续优化补充。
- 路径用正斜杠 `/`（项目跑在 Windows，但跨平台代码与文档统一用 Unix 风格）。
- 技术术语、代码标识符、作者姓名、版本号保留英文原文。

## 文档索引

| 编号 | 文档 | 子系统 |
|---|---|---|
| 00 | [00-overview-and-entry.md](00-overview-and-entry.md) | 总览、入口执行流、workspace 全景 |
| 01 | [01-cli.md](01-cli.md) | CLI 参数解析、子命令分发、auth_test 验证框架 |
| 02 | [02-agent-runtime.md](02-agent-runtime.md) | Agent 对话循环、compaction、tool/skill Registry |
| 03 | [03-provider.md](03-provider.md) | LLM Provider 抽象、8 内置 + 30 OpenAI-compatible profile、failover |
| 04 | [04-server.md](04-server.md) | Server 多进程运行时、swarm 协调、headless、hot-reload、ambient |
| 05 | [05-tui.md](05-tui.md) | ratatui 终端 UI、RemoteConnection、渲染管线 |
| 06 | [06-auth-login.md](06-auth-login.md) | 两层 Login 架构、OAuth PKCE、AuthStatus 缓存、StartupGuide |
| 07 | [07-memory.md](07-memory.md) | 跨会话记忆、MemoryGraph、Sidecar、embedding |
| 08 | [08-storage-session.md](08-storage-session.md) | JSON 持久化、snapshot+journal、崩溃恢复、PID 检测 |
| 09 | [09-mcp-saitec.md](09-mcp-saitec.md) | MCP JSON-RPC、SharedMcpPool、SAITEC-Skills HTTP 集成 |
| 10 | [10-gateway-transport.md](10-gateway-transport.md) | WebSocket Gateway、Unix socket / Windows Named Pipe 抽象 |
| 11 | [11-bus-message-protocol.md](11-bus-message-protocol.md) | 事件总线、消息类型、wire protocol（NDJSON） |
| 12 | [12-workspace-build-ci.md](12-workspace-build-ci.md) | 51-crate workspace、build.rs、CI、budget 脚本 |
| 13 | [13-config.md](13-config.md) | 全局配置加载、OnceLock 单例、80+ `JCODE_*` env override |
| 14 | [14-telemetry.md](14-telemetry.md) | 匿名遥测采集、Cloudflare Worker + D1、opt-out |
| 15 | [15-update.md](15-update.md) | 自动更新、stable/main 双 channel、SHA256 校验 |
| 16 | [16-overnight.md](16-overnight.md) | 无人值守运行、安全系统、通知分发、ambient 高级变体 |
| 17 | [17-desktop-app.md](17-desktop-app.md) | Desktop 原生 GPU 客户端、wgpu+winit 渲染、Niri 风格多 session 空间化工作台 |
| 18 | [18-mobile-client.md](18-mobile-client.md) | iOS 移动客户端、Mobile Agent Simulator、Rust 共享核心、Tailscale+WebSocket 接入 |
| 19 | [19-cross-platform.md](19-cross-platform.md) | 跨平台策略（TUI/Desktop/Mobile）、代码共享划分、各客户端对比 |
| 20 | [20-architecture-rfcs.md](20-architecture-rfcs.md) | 模块化架构 RFC、Client-Core/Presentation 拆分、多会话客户端、Server/Service 拆分 |
| 21 | [21-quality-audit.md](21-quality-audit.md) | 代码质量计划、审计、待办、重构路线图、依赖安全 |
| 22 | [22-browser-provider.md](22-browser-provider.md) | Browser Provider 协议设计与 agent 交互 |

## 阅读约定补充

- **关联模块**：除独立成篇的子系统外，`src/` 下大量辅助/功能模块（goal、catchup、import、usage、side_panel、perf、background、prompt、todo、soft_interrupt_store 等）已**就近归入**相关现有文档的「关联模块」小节（小表格：模块名 | 职责 | 归位说明）。这些小节优先保证可发现性，后续优化轮次再逐个深化。

## 跨文档线索（按关注点）

- **想理解一个用户请求怎么流转**：01 (CLI 入口) → 04 (Server 分发) → 02 (Agent turn 循环) → 03 (Provider 调用) → 11 (事件回流) → 05 (TUI 渲染)
- **想理解凭据与登录**：06 (Auth 全景) + 09 (SAITEC 凭据三件套) + 03 (Provider 凭据探测)
- **想理解持久化与崩溃恢复**：08 (Storage/Session) + 04 (Server durable_state / reload_recovery)
- **想理解远程客户端（iOS/Web）**：10 (Gateway/Transport) + 11 (Protocol) + 04 (Server accept loop)
- **想理解多 agent 协作**：04 (swarm*) + 02 (subagent Registry clone) + 11 (SwarmEvent)
- **想理解架构规划与方向**：20 (架构 RFC、crate 拆分、Client-Core/Presentation 拆分方案) + 12 (workspace 构建)
- **想理解代码质量改进进展**：21 (质量审计、计划、待办、依赖安全) + 12 (budget 脚本与 CI guardrails)
- **想理解浏览器自动化集成**：22 (Browser Provider 协议与 provider 设计) + 02 (agent browser tool) + 03 (browser provider 实现)
- **想理解无人值守/夜间运行**：16 (overnight/safety/notifications) + 04 (ambient 系统) + 13 (SafetyConfig 通知渠道)
- **想理解 Desktop 原生客户端**：17 (Desktop 架构全景) + 11 (共享 NDJSON protocol) + 04 (Server 连接)
- **想理解 iOS 移动客户端**：18 (Mobile 架构) + 10 (Gateway/Transport WebSocket 接入) + 11 (Protocol) + 04 (Server accept loop)
- **想理解各客户端关系**：19 (跨平台策略/共享层划分) + 05 (TUI 特性参考) + 17 (Desktop 设计) + 18 (Mobile 设计)

## 历史修复记录归档

接手项目后的 bug 修复记录（原 CLAUDE.md 的 Project Memory 小节）已迁移到各对应文档的「陷阱与历史修复」小节，并在 `12-workspace-build-ci.md` 末尾设有汇总索引。
