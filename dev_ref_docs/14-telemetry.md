# 14 · Telemetry

> 子系统:匿名遥测采集(安装/升级/会话生命周期/工具调用/错误/token usage/onboarding/feedback),POST 到 Cloudflare Worker,支持 opt-out。
> 回指:[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

采集匿名安装/升级/会话生命周期/工具调用/错误/token usage/onboarding 步骤/feedback 事件，以 JSON payload POST 到 Cloudflare Worker 端点，支持 `JCODE_NO_TELEMETRY` 和 `DO_NOT_TRACK` opt-out；服务端为独立的 `telemetry-worker/`（Cloudflare Worker + D1）。

## 关键文件清单

| 路径 | 职责 |
|---|---|
| `src/telemetry.rs` | 主模块(~1713 行)：`SessionTelemetry`/`TurnTelemetry` 状态机、所有 `record_*` 公共 API、事件构造与发送、`is_enabled()` opt-out |
| `src/telemetry/lifecycle.rs` | `emit_lifecycle_event()` — session_end/session_crash 事件构造(~327 行)，workflow 分析、stop reason 推断 |
| `src/telemetry/state_support.rs` | 持久化辅助(~349 行)：telemetry_id 管理、install/upgrade 记录、active days tracking、session 并发监控、milestone 记录 |
| `src/telemetry_state.rs` | **与 `state_support.rs` 内容重复**(重构遗留) |
| `src/telemetry_tests.rs` / `src/telemetry/tests.rs` | 测试 |
| `TELEMETRY.md` | 详细文档(~17KB)：采集字段、隐私承诺、opt-out 方式 |
| `telemetry-worker/` | 服务端：Cloudflare Worker + D1 数据库(9 个 migration、schema.sql、health.sql) |

## 核心类型与关键函数

- **`SessionTelemetry`** (`telemetry.rs:93`) — 会话级状态 struct，~90 个字段，跟踪 session start 到 end 全部指标。
- **`TurnTelemetry`** (`telemetry.rs:40`) — 单轮对话指标，嵌入 `SessionTelemetry.current_turn`。
- **`is_enabled()`** (`telemetry.rs:256`) — 检查 `JCODE_NO_TELEMETRY`/`DO_NOT_TRACK` 环境变量和 `no_telemetry` 文件。
- **`record_install_if_first_run()`** / **`record_upgrade_if_needed()`** — 启动时调用，记录安装/升级事件。
- **`begin_session()` / `end_session()`** — 会话生命周期管理。
- **`record_turn()` / `record_tool_execution()` / `record_token_usage()`** — 运行时事件采集。
- **`emit_lifecycle_event()`** (`lifecycle.rs`) — session end 事件构造，含 workflow 分析和 stop reason 推断。
- **`send_payload()`** — 两种投递：`Background`(新线程，5s 超时)/`Blocking`(阻塞，800ms-1200ms 超时)。
- **`jcode_usage_types`** — 外部 crate，定义所有事件 struct(InstallEvent/SessionLifecycleEvent/TurnEndEvent 等)和辅助函数。

## 主控制流

```
应用启动
  → record_install_if_first_run()   // 首次安装 blocking 发送
  → record_upgrade_if_needed()
  → begin_session()                 // 初始化 SessionTelemetry 存入 SESSION_STATE 全局 Mutex
  → 运行时 record_turn() / record_tool_execution() / record_token_usage() 累积指标
  → 首次有活动时 maybe_emit_session_start() 发送 session_start 事件
  → 会话结束 end_session()
    → emit_lifecycle_event() 构造完整 session_end 事件，blocking 发送

每个事件自动附带 envelope：schema_version、build_channel、is_git_checkout、is_ci、ran_from_cargo
数据流:客户端 JSON → POST https://jcode-telemetry.jeremyhuang55555.workers.dev/v1/event → Cloudflare D1
```

## 依赖关系

- **依赖**：`crate::storage::jcode_dir()`、`crate::build`、`crate::cli::selfdev::CLIENT_SELFDEV_ENV`、`jcode_usage_types`(事件类型)、`chrono`、`uuid`、`reqwest::blocking`、`walkdir`。
- **被依赖**：24 个文件用 telemetry，覆盖 agent、tui、server、CLI、provider、memory、tool 模块——全项目散布**最广**的子系统之一。
- 服务端 `telemetry-worker/`（Cloudflare Worker + D1 + 9 个 migration）构成完整 client-server 架构。

## 陷阱与设计约束

- **`telemetry_state.rs` 与 `telemetry/state_support.rs` 内容完全重复**——两文件有相同函数(get_or_create_id、telemetry_id_path 等)，是模块迁移遗留。改一处时另一处不同步会埋坑。
- **Blocking 发送阻塞主线程**：`BLOCKING_LIFECYCLE_TIMEOUT` 800ms、`BLOCKING_INSTALL_TIMEOUT` 1200ms，慢网络下可能影响启动体验。
- **双重 opt-out 机制共存**：`no_telemetry` 文件(在 jcode_dir) + 环境变量(`JCODE_NO_TELEMETRY`/`DO_NOT_TRACK`)。排查「为什么没采集」时三个都查。
- **endpoint 硬编码**：`TELEMETRY_ENDPOINT` 常量直接写死 URL，无配置化——要切换收端点需改代码重编译。
- **独立 schema versioning**：当前 v5，migration 在 `telemetry-worker/`。客户端 envelope 的 `schema_version` 必须与服务端迁移版本对齐。
- **`SessionTelemetry` ~90 字段**：采集维度极广，隐私策略(opt-out/redaction/匿名 UUID)专门由 `TELEMETRY.md` 注明，改采集字段前务必同查该文档。

## 关联模块

| 模块 | 职责 | 归位说明 |
|---|---|---|
| `src/startup_profile.rs`(84 行) | 启动阶段耗时记录 mark | 基础设施,与 telemetry 同属「诊断/观测」层,但独立功能;归入 [12-workspace-build-ci.md](12-workspace-build-ci.md) |
| `src/process_memory.rs`(593 行) | 进程内存快照(RSS/PSS/jemalloc/OS) | 归入 [12-workspace-build-ci.md](12-workspace-build-ci.md) |
| `src/runtime_memory_log.rs` | 进程级内存采样日志 | 与 [07 Memory](07-memory.md) 的 RuntimeMemoryLog 同源,但属诊断层 |

## 回指

- 采集点分布:agent 的 `record_turn`/`record_tool_execution` 见 [02-agent-runtime.md](02-agent-runtime.md)
- token usage 采集与 [03 Provider](03-provider.md) 用量、[05 TUI](05-tui.md) usage overlay 联动
- 启动时的 install/upgrade 记录见 [00-overview-and-entry.md](00-overview-and-entry.md) 的 startup 编排