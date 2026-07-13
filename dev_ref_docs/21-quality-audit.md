# 21 · 代码质量审计与计划

> 子系统：代码质量目标、审计报告、待办清单、重构路线图、依赖安全。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

本文件整合 jcode 的代码质量目标（10/10 计划）、2026-04-18 代码质量审计发现、完整的待办项目清单、重构路线图以及依赖安全扫描结果，为持续质量改进提供单一参考源。

## 依赖关系

**内部文档**：
- [00-overview-and-entry.md](00-overview-and-entry.md) — 代码结构鸟瞰
- [12-workspace-build-ci.md](12-workspace-build-ci.md) — CI guardrails 与 budget scripts
- [20-architecture-rfcs.md](20-architecture-rfcs.md) — 架构规划与 crate 拆分

**源文档**：
- `docs/CODE_QUALITY_10_10_PLAN.md` — 质量 10/10 计划
- `docs/CODE_QUALITY_AUDIT_2026-04-18.md` — 代码质量审计报告
- `docs/CODE_QUALITY_TODO.md` — 质量待办清单
- `docs/REFACTORING.md` — 重构路线图
- `docs/SECURITY_DEPENDENCIES.md` — 依赖安全扫描

---

## 1 · Code Quality 10/10 计划

> 源文档：`docs/CODE_QUALITY_10_10_PLAN.md` (326 行)

### 目标

将 jcode 从当前约 **7/10** 水平提升到 **9+/10**，持续工程标准。这不是"完美"，而是：
1. 缺陷预防比引入更容易
2. 贡献者能快速理解代码归属
3. 仓库抵抗架构漂移
4. 高风险区域经过良好测试和可观察
5. 质量不依赖记忆或英雄主义

### 当前问题

1. **超大模块**：`src/provider/openai.rs`、`src/provider/mod.rs`、`src/agent.rs`、`src/server.rs`、`src/tui/ui.rs`、`src/tui/info_widget.rs`、`tests/e2e/main.rs`
2. **警告和死代码债务**：容忍显著警告预算，多处宽泛 `allow(dead_code)`
3. **错误处理不一致**：生产代码中大量 `unwrap`、`expect`、`panic!`、`todo!`、`unimplemented!`
4. **测试集中化**：大型文件内集中测试
5. **Guardrails 不够严格**：需要收紧 CI

### 10/10 完成标准

**Build/lint**：
- `cargo check --all-targets --all-features` 通过
- `cargo clippy --all-targets --all-features -- -D warnings` 通过（或接近通过）
- `cargo fmt --all -- --check` 通过
- 警告数接近零

**结构性**：
- 无生产文件超过 **1200 LOC** 除非有文档理由
- 多数生产文件低于 **800 LOC**
- 多数函数低于 **100 LOC**
- 主要领域有清晰边界

**可靠性**：
- e2e 测试按 feature 拆分
- 关键状态转换有针对性测试
- reload/streaming/tool/swarm 有显式失败模式覆盖

**安全性**：
- 生产 `unwrap`/`expect` 大幅减少
- 宽泛 `allow(dead_code)` 消除或缩小为局部
- tool/shell/path/credential 边界显式且经过测试

### 分阶段执行

| 阶段 | 目标 | 状态 |
|---|---|---|
| Phase 0: Prevent Further Decay | 添加更严格 CI、设置文件大小目标 | **Done** |
| Phase 1: Warning/Dead-Code Burn-Down | 减少警告数、替换宽泛 allow | **Mostly Done**（clippy clean） |
| Phase 2: Decompose Biggest Files | 拆分超大文件 | **In Progress** |
| Phase 3: Strengthen Error Handling | 减少 unwrap/expect | **Planned** |
| Phase 4: Rebalance Test Pyramid | 按 feature 拆分 e2e | **In Progress** |
| Phase 5: Reliability Guardrails | 内存/压力/重连测试 | **Planned** |
| Phase 6: Finish Ratchet | 警告零容忍、自持续 | **Planned** |

### 六大非协商原则

1. 无大爆炸重写
2. 行为保持优先
3. 质量可强制执行（CI guardrail）
4. 积极删除死代码
5. 保持产品可发布

---

## 2 · 代码质量审计（2026-04-18）

> 源文档：`docs/CODE_QUALITY_AUDIT_2026-04-18.md` (561 行)

### 范围与方法

扫描了 `target`、`.git`、`node_modules` 外的所有 Rust 文件：
- 按 LOC 测量文件大小
- 通过 brace-balanced `fn` 块近似函数大小
- 统计 panic-prone 宏和方法
- 盘点 `allow(...)` 和 TODO/FIXME/HACK/XXX 标记

### 当前正面发现

- `cargo clippy --all-targets --all-features -- -D warnings` **通过**
- 无 `#[allow(dead_code)]` 剩余
- 格式化干净

### Repository 指标

| 指标 | 值 |
|---|---|
| Rust 文件扫描 | **455** |
| `src/` Rust 文件 | **429**（277,014 LOC） |
| `tests/` Rust 文件 | **11**（4,802 LOC） |
| `crates/` Rust 文件 | **14**（5,335 LOC） |
| 生产文件 > 1200 LOC | **50** |
| 生产文件 801-1200 LOC | **62** |
| >100 LOC 生产函数 | **304**（跨 165 文件） |

### `unwrap` / `expect` 分布

| 范围 | unwrap/expect 数量 |
|---|---|
| 生产文件 | **1258**（注：包含内嵌生产文件的 test-only 代码） |
| 测试文件 | **1334** |

**生产文件中最高的**：
| 数量 | 文件 |
|---|---|
| 136 | `src/tool/communicate.rs` |
| 62 | `src/build.rs` |
| 52 | `src/auth/cursor.rs` |
| 46 | `src/auth/codex.rs` |
| 42 | `src/provider/openai.rs` |
| 37 | `src/auth/claude.rs` |

### 最长的生产函数

| LOC | 函数 | 位置 |
|---|---|---|
| **1827** | `handle_remote_key_internal` | `src/tui/app/remote/key_handling.rs:93-1919` |
| **1658** | `handle_client` | `src/server/client_lifecycle.rs:669-2326` |
| **1121** | `handle_server_event` | `src/tui/app/remote/server_events.rs:5-1125` |
| **1016** | `run_turn_interactive` | `src/tui/app/turn.rs:23-1038` |
| 976 | `render_markdown_with_width` | `src/tui/markdown_render_full.rs:4-979` |
| 941 | `run_turn_streaming_mpsc` | `src/agent/turn_streaming_mpsc.rs:4-944` |
| 863 | `render_markdown_lazy` | `src/tui/markdown_render_lazy.rs:3-865` |
| 783 | `maybe_handle_swarm_read_command` | `src/server/debug_swarm_read.rs:21-803` |
| 780 | `execute` | `src/tool/communicate.rs:727-1506` |
| 771 | `run_turn_streaming` | `src/agent/turn_streaming_broadcast.rs:4-774` |

### 结构性债务：前 20 大生产文件

| LOC | 文件 |
|---|---|
| 3228 | `src/server/comm_control.rs` |
| 3165 | `src/tool/communicate.rs` |
| 2729 | `src/session.rs` |
| 2704 | `src/server/client_lifecycle.rs` |
| 2683 | `src/provider/openai.rs` |
| 2437 | `src/tui/ui.rs` |
| 2397 | `src/memory.rs` |
| 2365 | `src/provider/mod.rs` |
| 2217 | `src/telemetry.rs` |
| 2131 | `src/tui/ui_messages.rs` |
| 2115 | `src/tui/session_picker.rs` |
| 2041 | `src/tui/app/inline_interactive.rs` |
| 2023 | `src/tui/app/input.rs` |
| 2005 | `src/config.rs` |
| 1969 | `src/provider/anthropic.rs` |
| 1919 | `src/tui/app/remote/key_handling.rs` |
| 1912 | `src/tui/app/auth.rs` |
| 1900 | `src/usage.rs` |
| 1888 | `src/tui/session_picker/loading.rs` |
| 1881 | `src/cli/login.rs` |

### Suppression 清单

- Rust 文件含 `allow(...)`：**17** 文件，共 **28** 个
- 最常见：`clippy::too_many_arguments`（13 个）、`unused_mut`（7 个）
- 最严重的架构信号：`src/server/client_session.rs` 有 **5 个** `too_many_arguments`

### 最高价值改进主题

1. **拆分 mega 文件后再加逻辑** — 50 个文件 > 1200 LOC
2. **拆解怪兽函数** — 最大的维护风险是超长单函数
3. **减少 server 控制/会话代码的参数扇出** — 多处 `too_many_arguments`
4. **强化真正生产代码的失败路径** — 即使 clippy 干净，仍有大量 unwrap/expect
5. **移动或隔离巨型生产文件中的内嵌测试** — 拉高文件大小和 panic 计数
6. **减少测试集中** — `src/tui/app/tests.rs` 本身是巨型热点
7. **修剪 suppression 表面积** — server 代码的 `too_many_arguments` 是架构问题
8. **烧掉 deferred work markers** — TODO/FIXME 不多但仍需清理

---

## 3 · 完整待办清单

> 源文档：`docs/CODE_QUALITY_TODO.md` (480 行)

### Phase 0: Prevent Further Decay

- [x] Add CI job for `cargo check --all-targets --all-features`
- [x] Add CI job for `cargo clippy --all-targets --all-features -- -D warnings`
- [x] Keep warning policy on a downward ratchet
- [x] Add documented file-size and function-size targets

### Phase 1: Warning and Dead-Code Burn-Down

- [x] Inventory all `#![allow(dead_code)]` and remove/justify
- [x] Reduce baseline warning count
- [ ] Remove stale unused functions in `setup_hints.rs`
- [ ] Remove stale unused code in TUI support modules
- [ ] Audit broad suppressions → narrow local allowances

### Phase 2: Decompose Biggest Files

**Highest priority**：
- [x] Split `tests/e2e/main.rs` by feature area（2026-03-24 完成）
- [ ] Continue splitting `src/server.rs`（已提取 state/socket/reload_state/util 子模块）
- [ ] Split `src/agent.rs` into orchestration/stream/interrupt/tool-exec modules
- [ ] Split `src/provider/mod.rs` into traits/pricing/routes/HTTP helpers
- [ ] Split `src/provider/openai.rs` into request/stream/tool/response modules
- [ ] Split `src/tui/ui.rs` by render responsibility
- [ ] Split `src/tui/info_widget.rs` by widget/domain sections

**完整的 50 个超大文件拆分待办**见 `docs/CODE_QUALITY_TODO.md` 的结构性待办列表。

### Phase 3: Error Handling Hardening

- [x] Count production unwrap/expect separately from test-only（审计已完成）
- [ ] Replace easy production unwrap/expect hotspots with explicit errors
- [ ] Add better error context for provider stream parsing
- [ ] Add better error context for reload and socket lifecycle

**完整 harden 待办列表**含 ~70+ 文件，从 `src/tool/communicate.rs`（136 个 expect）开始。

### Phase 4: Test Strategy Improvements

- [x] Extract shared e2e test support helpers
- [ ] Add focused tests for reload state transitions
- [ ] Add focused tests for malformed provider stream chunks
- [ ] Add snapshot/golden tests for stable TUI render outputs
- [ ] Add property tests for protocol serialization and tool parsing

### Phase 5: Reliability and Performance Guardrails

- [ ] Add repeated reload reliability test coverage
- [ ] Add repeated attach/detach and reconnect coverage
- [ ] Track memory regression expectations
- [ ] Improve observability around reload/swarm/tool execution
- [ ] Execute compile-performance roadmap

### Suppression Cleanup 待办

| 文件 | 待清理 allow |
|---|---|
| `src/agent/turn_loops.rs` | `unused_variables` |
| `src/auth/mod.rs` | `unused_mut` |
| `src/cli/dispatch.rs` | `deprecated`, `unused_mut` × 2 |
| `src/server.rs` | `unused_mut` × 2 |
| `src/server/client_session.rs` | `too_many_arguments` × 5 |
| `src/server/client_lifecycle.rs` | `too_many_arguments` × 2 |
| `src/server/comm_session.rs` | `too_many_arguments` × 2 |

### Production `todo!` / `unimplemented!` 待办

| 文件 | 数量 |
|---|---|
| `src/tui/ui_header.rs` | 1 |
| `src/tui/app/remote.rs` | 1 |
| `src/tool/mod.rs` | 1 |
| `src/server/debug_command_exec.rs` | 1 |
| `src/server/debug.rs` | 1 |
| `src/server/client_state.rs` | 1 |
| `src/server/client_comm.rs` | 1 |
| `src/server/client_actions.rs` | 1 |
| `src/provider/gemini.rs` | 1 |
| `src/cli/selfdev.rs` | 1 |
| `src/ambient/runner.rs` | 1 |

---

## 4 · 重构路线图

> 源文档：`docs/REFACTORING.md` (77 行)

### 目标

- 保持现有会话和工作流稳定
- 使回归在早期可见
- 分阶段减少架构耦合

### 安全规则

1. 重构使用隔离环境：`scripts/refactor_shadow.sh`
2. 每次合并前运行：`scripts/refactor_phase1_verify.sh`
3. 警告数不可超过基线：`scripts/check_warning_budget.sh`
4. 合并前运行安全预检：`scripts/security_preflight.sh`
5. 先行为保持移动，再逻辑变更

### 6 阶段计划

| 阶段 | 内容 | 状态 |
|---|---|---|
| Phase 1: Safety + Hygiene | 添加隔离的 dev/run 工作流、可重复验证脚本、警告预算守卫 | **Done** |
| Phase 2: CLI Decomposition | main() 子命令 handler 移入 `src/cli/*`；main 保持 parse + dispatch | **In Progress** |
| Phase 3: Server Decomposition | server.rs 按责任拆分（session lifecycle、debug API、swarm、reload）；替换 stringly states 为 typed enum | **In Progress** |
| Phase 4: Agent Turn-Loop Unification | 合并重复 turn-loop 变体为单一共享引擎 + pluggable event sink | **Planned** |
| Phase 5: TUI State/Reducer Split | 分离 app state、command parsing、remote-event reduction、rendering control | **Planned** |
| Phase 6: Provider State Isolation | 将 caches 移入显式 state holders，减少全局可变状态 | **Planned** |

### 验证矩阵

| 检查 | 命令 |
|---|---|
| 编译 | `cargo check -q` |
| 编译计时 | `scripts/bench_compile.sh` |
| 警告 | `scripts/check_warning_budget.sh` |
| 安全 | `scripts/security_preflight.sh` |
| 单元+集成测试 | `cargo test -q` |
| E2E 测试 | `cargo test --test e2e -q` |
| 组合验证 | `scripts/refactor_phase1_verify.sh` |

---

## 5 · 依赖安全

> 源文档：`docs/SECURITY_DEPENDENCIES.md`（上次审查：2026-03-05）

### 当前漏洞

| Advisory | Crate | 依赖路径 | 受影响区域 | 严重程度 | 计划 |
|---|---|---|---|---|---|
| RUSTSEC-2025-0141 | `bincode` | `syntect → bincode` | Markdown 代码高亮 | 未维护的传递依赖 | 跟踪 syntect 升级 |
| RUSTSEC-2024-0436 | `paste` | 多重传递（ratatui/tokenizers/tract） | TUI 渲染、tokenizers、embedding | 广泛传递依赖 | 优先上游升级 |
| RUSTSEC-2026-0002 | `lru` | `ratatui → lru` | TUI 渲染缓存 | Unsoundness warning | 同步升级 ratatui |
| RUSTSEC-2023-0086 | `lexical-core` | `imap → imap-proto → lexical-core` | Gmail/IMAP 支持 | 旧未安全传递依赖（处理网络解析数据） | 最高优先级，调查升级或替换 imap |

### 优先级顺序

1. `lexical-core` via `imap-proto`（最高优先级，因涉及网络解析数据）
2. `lru` via `ratatui`
3. `bincode` via `syntect`
4. `paste` via 多重传递依赖

### 已处理的

- RUSTSEC-2024-0320 (`yaml-rust`) 已于 2026-03-05 通过裁剪 `syntect` 功能从依赖图中移除

### 依赖变更前验证命令

```
cargo check
cargo test -j 1
scripts/security_preflight.sh
```

---

## 回指

- 架构规划与 crate 拆分：[20-architecture-rfcs.md](20-architecture-rfcs.md)
- CI guardrails 与 budget scripts：[12-workspace-build-ci.md](12-workspace-build-ci.md)
- Refactoring shadows 与验证脚本：`scripts/` 目录
- Provider 子系统状态：[03-provider.md](03-provider.md)
- Workspace 全景：[00-overview-and-entry.md](00-overview-and-entry.md)
