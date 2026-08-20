# 12 · Workspace / 构建 / CI

> 子系统：51-crate workspace 全景、feature flags、build profiles、build.rs、CI pipeline、budget 脚本。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

`jcode` 是一个 **51-crate Rust workspace（edition 2024）** 的多模型 AI 编程 agent monorepo，根 package `jcode` v`1.0.1-alpha`，通过 `build.rs` 自动生成 git 版本串与 changelog，CI 用 5 个 job + budget 脚本 ratchet 机制守护质量。

## Workspace 总体

- **Edition**: 2024
- **根 package**: `jcode`，版本 `1.0.1-alpha`
- **workspace members**: 51 个 crate（根目录 `.` + 50 个 `crates/` 子目录）
- **autobins = false**，手动声明 6 个 binary target：`jcode`、`test_api`、`jcode-harness`、`session_memory_bench`、`mermaid_side_panel_probe`、`tui_bench`（后三者需 `dev-bins` feature）

## Crate 分组（51 crate）

### Provider（LLM 提供商抽象与适配，5 crate）

| Crate | 职责 |
|---|---|
| `jcode-provider-core` | Provider trait 抽象层，含 Anthropic/OpenAI schema、failover 决策、model catalog、pricing、selection |
| `jcode-provider-metadata` | Provider 元数据（Anthropic OAuth beta headers、model 能力、context limit、catalog refresh） |
| `jcode-provider-openai` | OpenAI Responses API request builder 与 encrypted content 处理 |
| `jcode-provider-openrouter` | OpenRouter 路由层，含 provider 缓存、Kimi fallback、provider alias |
| `jcode-provider-gemini` | Google Gemini / Cloud Code Assist 适配，含 model list 与 tier 定义 |

### TUI（终端 UI 组件库，11 crate）

| Crate | 职责 |
|---|---|
| `jcode-tui-core` | TUI 核心：copy selection、graph topology、keybind、stream buffer |
| `jcode-tui-markdown` | Markdown → ratatui Widget 渲染（pulldown-cmark + syntect 语法高亮 + Mermaid 识别） |
| `jcode-tui-mermaid` | Mermaid diagram → PNG → ratatui-image 终端渲染（Kitty/Sixel/iTerm2/halfblock 自适应） |
| `jcode-tui-messages` | 聊天消息列表的渲染与缓存 |
| `jcode-tui-render` | 通用渲染原语：rounded box、layout、chrome 元素 |
| `jcode-tui-style` | 主题色与样式系统 |
| `jcode-tui-tool-display` | Tool name → friendly display name 映射与 canonical 化 |
| `jcode-tui-session-picker` | 会话选择器数据结构（含多源会话 badge: Claude Code/Codex/Pi/OpenCode） |
| `jcode-tui-account-picker` | 账号选择器命令类型（Anthropic/OpenAI） |
| `jcode-tui-usage-overlay` | 用量 overlay widget（Good/Warning/Critical/Error 状态） |
| `jcode-tui-workspace` | Workspace map widget 与 color support（truecolor/indexed 自适应） |

### Types（共享数据类型，15 crate）

| Crate | 职责 |
|---|---|
| `jcode-auth-types` | AuthState / AuthCredentialSource 枚举 |
| `jcode-ambient-types` | 环境感知类型（PairedDevice、PairingCode） |
| `jcode-background-types` | 后台任务状态/进度类型 |
| `jcode-batch-types` | 批量 tool call 进度追踪（BatchSubcallState） |
| `jcode-config-types` | 配置类型（含 CompactionMode：Reactive/Proactive/Semantic） |
| `jcode-gateway-types` | Gateway 设备配对协议类型 |
| `jcode-memory-types` | Memory 系统活动状态、pipeline、graph 类型 |
| `jcode-message-types` | 核心消息模型：ToolCall、ToolDefinition、prompt cost 估算 |
| `jcode-selfdev-types` | 自我开发 build command、target、reload 恢复指令 |
| `jcode-session-types` | 会话渲染类型（RenderedMessage、RenderedImage、CompactedHistoryInfo） |
| `jcode-side-panel-types` | Side panel 页面格式/源（Markdown/Managed/LinkedFile/Ephemeral） |
| `jcode-task-types` | 目标/任务 scope（Global/Project）类型 |
| `jcode-usage-types` | Token 用量记录与 rate limit 信息 |
| `jcode-plan` | Swarm plan item（PlanItem）与 SwarmTaskProgress，版本化 plan graph |
| `jcode-tool-types` | ToolOutput / ToolImage 返回类型 |

### Core（核心运行时，6 crate）

| Crate | 职责 |
|---|---|
| `jcode-core` | 基础工具模块：env、fs、id 生成、panic_util、stdin_detect |
| `jcode-agent-runtime` | Agent runtime：SoftInterrupt queue、BackgroundToolSignal、GracefulShutdownSignal、异步 interrupt signal |
| `jcode-tool-core` | Tool trait 定义（async trait）、ToolContext、StdinInputRequest、intent schema |
| `jcode-protocol` | Client-server JSON-over-socket 协议（main socket + agent socket 双通道） |
| `jcode-storage` | 持久化存储（runtime_dir 发现、flock 平台感知、JSON 读写） |
| `jcode-compaction-core` | Context compaction 核心（200k token budget、80% 触发、95% critical、emergency truncation） |

### Mobile & Desktop（跨平台客户端，3 crate）

| Crate | 职责 |
|---|---|
| `jcode-mobile-core` | 移动模拟器共享 headless 核心（visual、protocol、scenario） |
| `jcode-mobile-sim` | Headless 移动模拟器 CLI（wgpu + winit GPU preview、hit test、screenshot diff） |
| `jcode-desktop` | wgpu + winit + glyphon 桌面 GUI 客户端（macOS/Windows） |

### Server / Swarm / Operations（服务端与运维，7 crate）

| Crate | 职责 |
|---|---|
| `jcode-swarm-core` | Swarm 协调核心（Agent/Coordinator/WorktreeManager 角色、completion report） |
| `jcode-overnight-core` | Overnight 长时间无人值守运行（Start/Status/Cancel/Review 命令） |
| `jcode-notify-email` | Email 通知（SMTP via lettre、IMAP reply 检测、permission decision 回复） |
| `jcode-update-core` | 自动更新（GitHub release 下载、SHA256 校验、background 更新阈值） |
| `jcode-build-support` | Build 版本管理（selfdev binary 路径、build progress、source state diff） |
| `jcode-azure-auth` | Azure Default Credential bearer token 获取 |
| `jcode-import-core` | Claude Code session 文件导入（sessions-index.json 解析、SHA256 去重） |

### Misc（杂项，4 crate）

| Crate | 职责 |
|---|---|
| `jcode-embedding` | 本地 ONNX embedding 推理（all-MiniLM-L6-v2、tract-onnx + tokenizers） |
| `jcode-pdf` | PDF 文本提取（pdf-extract 库封装） |
| `jcode-terminal-launch` | 跨平台外部终端启动（macOS Terminal.app / iTerm2 / Windows Terminal） |

## Feature Flags

| Feature | 说明 |
|---|---|
| **default** = `["pdf"]` | 默认仅启用 PDF 解析 |
| `pdf` | 拉入 `jcode-pdf` crate |
| `embeddings` | 拉入 `jcode-embedding`（163 个 transitive crate，tract-onnx + tokenizers，编译慢） |
| `jemalloc` | 启用 tikv-jemallocator（含 stats），长运行 server 减少内存碎片 |
| `jemalloc-prof` | jemalloc + profiling 支持 |
| `dev-bins` | 启用 bench/probe 专用 binary target |

`JCODE_DEV_FEATURE_PROFILE=full` 环境变量可启用 embeddings。

## Build Profiles

| Profile | 继承 | opt-level | debug | incremental | LTO | codegen-units |
|---|---|---|---|---|---|---|
| `dev` | - | default | 0 | true | - | - |
| `release` | - | 1 | 0 | true | - | 256 |
| `selfdev` | release | 0 | - | - | - | - |
| `release-lto` | release | - | - | false | thin | 16 |
| `test` | - | - | 0 | true | - | 256 |

关键点：
- `release` 用 `opt-level=1`（非 3）且 `codegen-units=256`，优先编译速度而非极致优化。
- `selfdev` 是开发时用的 profile（opt-level=0 继承 release 其他设置），快速 self-rebuild。
- `release-lto` 给正式分发：thin LTO + codegen-units=16。
- 所有 profile 关闭 debug info（debug=0）减少产物大小。

## 编译性能优化历程

> **来源**：原生 jcode `docs/COMPILE_PERFORMANCE_PLAN.md`（2026-05-05 snapshot）。SAITEC-TUI 作为二次开发，因 crate 裁剪可能未经历全部阶段，以下保留原生历程作为参考。

### 目标

- 全功能 build 保持可用（生产 + self-dev reload）
- 高频 self-dev edits 显著降低编译代价
- 减少因 customization 导致的重新编译需求
- 每阶段测后评估，停止没有收益的 churn

### 基准测量 (2026-03-24)

| 场景 | 耗时 |
|---|---|
| Warm `cargo check --quiet` | ~8.5s |
| Warm release `jcode` binary build | ~47.3s |
| Warm `selfdev-jcode` build（selfdev profile） | ~16.0s（2026-04-09 后） |

### 多阶段执行

**Phase 1 — 战术性构建速度优化**：
- `scripts/dev_cargo.sh`：自动启用 sccache、选 clang+lld/mold 快速 linker、selfdev profile 低内存保护（earlyoom 检测 → `CARGO_INCREMENTAL=0` + `CODEGEN_UNITS=16`）
- `JCODE_DEV_FEATURE_PROFILE` 环境变量：`minimal`/`pdf`/`embeddings`/`full` 快速切换 feature 集
- `autobins = false` + `dev-bins` feature：developer-only binary 不默认编译

**Phase 2 — 可重复测量**：
- `scripts/bench_compile.sh`：支持 `--runs N`、`--touch <path>`、`--json`、`-- <cargo args>`
- `scripts/bench_selfdev_checkpoints.sh`：cold/warm self-dev 标准检查点
- 强调 touched-file 测量（模拟真实编辑），而非纯热缓存无操作重跑

**Phase 3 — Workspace crate 边界重构（提议布局）**：
```
jcode-core          # protocol, ids, message types, config primitives
jcode-server        # server lifecycle, reload, socket, swarm
jcode-agent         # agent turn loop, tool orchestration
jcode-provider      # provider traits, shared provider types, routing
jcode-embedding     # ONNX/tokenizer 等重量级推理依赖
jcode-tui           # TUI rendering, widgets, state reduction
jcode-tui-core      # 低层 TUI helpers（stream buffer, keybind）
jcode-selfdev       # customization records, migration logic
jcode-build-support # build command, source state, channel paths
```

**Phase 4/4a — 落地的 crate 拆分（按时间线）**：

| 日期 | 拆分 | 边界决策 |
|---|---|---|
| 2026-03-24 | `crates/jcode-embedding` | ONNX/tokenizer 从主 crate 移出；后变为 opt-in feature |
| 2026-03-24 | `crates/jcode-pdf` | PDF 提取隔离；`--no-default-features` 时优雅降级 |
| 2026-03-24 | `crates/jcode-azure-auth` | Azure SDK 不再直接在主 crate |
| 2026-03-24 | `crates/jcode-notify-email` | lettre/imap/mail-parser/native-tls 隔离 |
| 2026-03-25 | `crates/jcode-provider-metadata` | Provider 元数据/目录/纯选择逻辑 |
| 2026-03-25 | `crates/jcode-provider-core` | 共享 HTTP client + route/cost/core 值类型 |
| 2026-03-25 | `crates/jcode-provider-openrouter` | OpenRouter 专属 catalog/cache/ranking |
| 2026-03-25 | `crates/jcode-provider-gemini` | Gemini Code Assist schema/types |
| 2026-03-30 | `crates/jcode-tui-workspace` | Workspace map 数据/模型 + widget 渲染 |
| 2026-05-03 | `crates/jcode-build-support` | 构建版本/self-dev 部署/源状态 |
| 2026-05-03 | `jcode-tui-core::keybind` | 纯 keybind 解析器从 TUI 移出 |
| 2026-05-05 | `jcode-message-types` | Message/ContentBlock/Role/StreamEvent/ToolDefinition |
| 2026-05-05 | `jcode-tool-types` / `jcode-tool-core` | ToolOutput/ToolImage 以及 Tool trait 合约 |
| 2026-05-05 | `jcode-plan` | PlanItem、swarm task control action 策略 |
| 2026-05-05 | `jcode-protocol` | Request::is_lightweight_control_request 判定 |

**Phase 5 — 减少失效压力**：收缩巨型热点文件；高 churn 代码远离稳定低层 crate；避免随改动共享广泛 fanout 类型。

**Phase 6 — 减少重新编译需求**（Issue #32）：通过 config/hooks/skills/prompt overlays/routing/theme/data 扩展点替代源码修改。已落地：`~/.ssc_tui/prompt-overlay.md` / `./.jcode/prompt-overlay.md` 系统 prompt 定制无需重编译。

### 关键测量数据

Warm touched-file 检查点（最终阶段）：

| 触发的文件 | Warm `cargo check` | Warm `selfdev-jcode` build |
|---|---|---|
| `src/tool/session_search.rs` | 7.009s | 12.874s |
| `src/agent.rs` | 7.318s | 30.928s |
| `src/tool/memory.rs` | 7.787s | 12.798s |
| `src/provider/mod.rs` | 9.772s | 17.917s |
| `src/tool/browser.rs` | 13.693s | 18.874s |

### 开发者工作流

```bash
# 本地开发 cargo wrapper
scripts/dev_cargo.sh check --quiet
scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode --quiet

# feature profile 切换
JCODE_DEV_FEATURE_PROFILE=minimal scripts/dev_cargo.sh check -p jcode --lib --quiet
JCODE_DEV_FEATURE_PROFILE=full scripts/dev_cargo.sh build --profile selfdev --quiet

# 编译时间测量（触碰文件模拟编辑）
scripts/bench_selfdev_checkpoints.sh --touch src/server.rs --runs 3
scripts/bench_compile.sh check --runs 3 --touch src/tool/read.rs

# 快速 linker 覆盖
JCODE_FAST_LINKER=mold scripts/dev_cargo.sh build --release -p jcode --bin jcode
```

### 停止条件

每个结构阶段后重新测量：warm check 时间是否实质改进？warm build 时间是否改进？常见 self-dev edit 的 rebuild scope 是否缩小？如果没有，停止以编译时间为由的高 churn 重构。

## build.rs 做什么

`build.rs`（405 行）：
1. **Version string 生成**：组合 `CARGO_PKG_VERSION` + git hash + dirty 标记，如 `v0.2.17-dev (abc1234, dirty)`。
2. **Auto patch bumping**：经 `target/jcode-build/patch-counters.txt` 记录每次 build 的 patch 号递增（文件锁保护，50ms polling，300s stale timeout），实现 `Cargo.toml` base version 之外的自动 patch bump。
3. **Git 元数据采集**：git hash、git date（完整 ISO datetime）、git tag、dirty 状态。
4. **Changelog 嵌入**：采集最近 700 条 git log（hash|timestamp|decorations|subject），编码为 ASCII record/unit separator 分隔字符串嵌入编译产物，用于 `/changelog` 命令。
5. **环境变量注入**：经 `cargo:rustc-env` 注入 `JCODE_GIT_HASH`/`JCODE_GIT_DATE`/`JCODE_VERSION`/`JCODE_SEMVER`/`JCODE_BASE_SEMVER`/`JCODE_UPDATE_SEMVER`/`JCODE_GIT_TAG`/`JCODE_CHANGELOG`。
6. **Rerun 条件**：`.git/HEAD`、`.git/index`、`Cargo.toml` 变化或 `JCODE_RELEASE_BUILD`/`JCODE_BUILD_SEMVER` 环境变量变化时重新执行。
7. **Override 支持**：`JCODE_BUILD_SEMVER` 环境变量可覆盖自动 patch bumping（CI release 用 `github.ref_name` 作为 semver）。

## CI Pipeline（GitHub Actions）

### `ci.yml` — 5+ job

| Job | 超时 | 运行环境 | 做什么 |
|---|---|---|---|
| **quality** | 45min | ubuntu-latest | `cargo fmt --check` + `cargo check --all-targets --all-features` + `cargo clippy -D warnings` + 5 个 budget 检查脚本（warning、code_size、test_size、panic、swallowed_error） |
| **build** | 35min | ubuntu + macos 矩阵 | `cargo build --release` + `cargo test --lib --bins --no-run` 编译检查 + provider_matrix 测试 + e2e 测试 + Linux warning budget + security preflight（cargo-audit） |
| **windows-build-test** | 150min | windows-latest | Windows release build + 11 个 targeted validation test（逐个 300s timeout）+ 2 个 e2e smoke test（420s timeout）+ Windows lifecycle e2e + binary launch 验证 + installer 验证 |
| **mobile-simulator** | 20min | ubuntu-latest | `cargo test -p jcode-mobile-core -p jcode-mobile-sim` + mobile simulator CLI smoke |
| **windows-cross-check** | 35min | ubuntu-latest | cargo-xwin 在 Linux 上交叉 check Windows x64 和 ARM64（ARM64 为 advisory / continue-on-error） |
| **fmt** | 10min | ubuntu-latest | 独立 `cargo fmt --check` |
| **powershell-syntax** | 10min | windows-latest | PowerShell 5.1 和 7 两个版本的 `.ps1` 脚本语法检查 |

### `release.yml` — tag push `v*` 触发

构建 6 个平台 release binary：Linux x86_64（manylinux2014 / CentOS 7 glibc 2.17 兼容容器）、Linux aarch64、macOS x86_64 + aarch64、Windows x86_64 + aarch64。构建后：SHA256SUMS 生成 → GitHub Release 创建 → Homebrew formula 自动更新 → AUR package 自动更新。

### `windows-smoke.yml` — 手动触发（`workflow_dispatch`）

可选 x64/arm64/both 目标的 Windows 冒烟测试，含完整 targeted test + e2e smoke + lifecycle test + installer 验证。

## 安全依赖审计

> **来源**：原生 jcode `docs/SECURITY_DEPENDENCIES.md`（最后审查 2026-03-05）。以下 CVE 记录于原生 jcode，SAITEC-TUI 作为衍生项目依赖树可能不同，但同类问题仍有参考意义。

### 当前已知 advisory

| Advisory | Crate | 依赖路径 | 影响范围 | 处理方案 |
|---|---|---|---|---|
| `RUSTSEC-2025-0141` | `bincode` | `syntect → bincode` | TUI Markdown/code 高亮 | 未维护的传递依赖。等待 `syntect` 升级或替换 |
| `RUSTSEC-2024-0436` | `paste` | `ratatui → paste`, `tokenizers → paste`, `tract-* → paste` | TUI 渲染、tokenizer、embedding | 广泛传递，优先依赖上游升级 |
| `RUSTSEC-2026-0002` | `lru` | `ratatui → lru` | TUI 渲染/缓存内部 | 非 soundness warning，不在 auth/provider 逻辑，但仍进程内 |
| `RUSTSEC-2023-0086` | `lexical-core` | `imap → imap-proto → lexical-core` | Gmail/IMAP 支持路径 | 处理网络数据的旧 unsound 传递依赖，优先级最高 |

### 处理优先级

1. `lexical-core`（via `imap-proto`）— 网络层数据，优先级最高
2. `lru`（via `ratatui`）
3. `bincode`（via `syntect`）
4. `paste`（via 多个传递依赖）

### 备注

- `RUSTSEC-2024-0320`（`yaml-rust`）已通过裁剪 `syntect` features（内置语法/主题而非 YAML 加载）于 2026-03-05 移除。
- 改动依赖前需执行：`cargo check` + `cargo test -j 1` + `scripts/security_preflight.sh`。

## scripts/ 关键脚本

### Budget 脚本（ratcheting 机制 — 只能变好不能变差）

| 脚本 | baseline 文件 | 用途 |
|---|---|---|
| `check_code_size_budget.py` | `code_size_budget.json` | 生产 Rust 文件 LOC > 1200 行的不能增长、不能新增。当前 49 个 oversized 文件（最大 `single_session.rs` 2517 行） |
| `check_test_size_budget.py` | `test_size_budget.json` | 测试 Rust 文件 LOC > 1200 行的不能增长/新增。当前 2 个 oversized |
| `check_panic_budget.py` | `panic_budget.json` | 统计 `.unwrap(`/`.expect(`/`panic!`/`todo!`/`unimplemented!`，总量不能增加。当前 total = 0（全量基线 0） |
| `check_warning_budget.sh` | `warning_budget.txt` | `cargo check -q` 的 warning 数不能超基线。当前基线 = 0 |
| `check_swallowed_error_budget.py` | `swallowed_error_budget.json` | 统计 `let _ =`/`.ok()`/`.unwrap_or_default()` 三种模式，总量不能增加。当前 total = 1998 |

所有 budget 脚本支持 `--update` 刷新基线。

### 开发/打包/远程脚本

| 脚本 | 用途 |
|---|---|
| `dev_cargo.sh` | 本地开发 cargo wrapper：自动启用 sccache、选 mold/lld 快速 linker、selfdev profile 低内存模式、feature profile（`JCODE_DEV_FEATURE_PROFILE`） |
| `dev_saitec_tui.ps1` | Windows 开发启动：build → 停止已有进程 → 复制 runtime → 启动 |
| `package_saitec.ps1` | Windows 打包：复制 build 产物到 `dist/saitec-tui/`，重命名为 `saitec-tui.exe` |
| `remote_build.sh` | SSH + rsync 远程构建（build/test/check/clippy），自动 sync-back 构建产物 |

## .cargo/config.toml 关键配置

```toml
[build]
jobs = 6
```

关键设计决策：
- **不硬编码 RUSTC_WRAPPER=sccache**，由 shell 环境或 `dev_cargo.sh`/CI 动态设置。
- **不硬编码 linker**，由 `dev_cargo.sh` 按平台选 mold/lld，CI 中 Linux 临时写入 `.cargo/config.toml` 覆盖。
- 注释说明 CI 会覆盖此文件。

## 依赖关系

- 全部子系统依赖此层的 crate（`jcode-core`/`jcode-protocol`/`jcode-storage`/各 `jcode-*-types` 等）。
- build.rs 为编译时注入版本/changelog；budget 脚本为 CI 守护质量。

## 关联模块（项目级基础设施与开发工具）

| 模块 | 路径 | 职责 | 规模 |
|---|---|---|---|
| `src/logging.rs` | 结构化日志框架——写 `~/.ssc_tui/logs/`、自动轮转、线程本地上下文（server/session/provider/model） | 319 行 |
| `src/process_memory.rs` | 进程内存快照——RSS、虚拟内存、OS 层（PSS/swap）、allocator 统计（jemalloc + 系统）；历史采样 + profiling 开关 | 593 行 |
| `src/startup_profile.rs` | 启动阶段耗时 mark 记录，生成冷启动时间报告 | 84 行 |
| `src/bin/harness.rs` | 确定性 tool 烟雾测试 harness，不调 LLM | 216 行 |
| `src/bin/tui_bench.rs` | TUI 渲染性能基准（含侧边面板子模块 `tui_bench/`）；需 `dev-bins` feature | 1655 行 |
| `src/bin/session_memory_bench.rs` | 会话内存归属与进程内存 benchmark；需 `dev-bins` | 250 行 |
| `src/bin/mermaid_side_panel_probe.rs` | 调试探查：mermaid 图在侧边面板渲染结果；需 `dev-bins` | 64 行 |
| `src/bin/test_api.rs` | **死代码**——测试废弃的 Claude CLI provider；需 `dev-bins` | 37 行 |

## 陷阱与设计约束

- **Oversized 文件债务严重**：code_size_budget 显示 49 个生产文件超 1200 行，最大 `src/server/client_lifecycle.rs`（2626 行）、`crates/jcode-desktop/src/single_session.rs`（2517 行）、`src/tui/ui.rs`（2618 行）。ratchet 机制阻止恶化但债务规模大。
- **Swallowed error patterns 数量庞大**：总计 1998 处 `let _ =`/`.ok()`/`.unwrap_or_default()`，其中 `let _ =` 877 处，散布 100+ 文件，大量集中在 streaming/server lifecycle/provider 等关键路径。
- **build.rs 的 auto patch bump 有竞态窗口**：用文件锁（create_new），但 lock 是用户态 spin-wait（200 次 * 50ms = 10s 超时），高并发 cargo build（CI matrix）时可能 counter 跳号；`save_patch_counters` 写入非原子（先写后 rename 才安全）。
- **Windows build timeout 极长**：`windows-build-test` 设 150 分钟超时，`windows-smoke` 150 分钟，反映 Windows 编译远慢于 Linux/macOS。
- **jcode-embedding 重量级依赖**：163 个 transitive crate（tract-onnx + tokenizers），编译极慢，故放 feature flag 后；`default` feature 仅含 `pdf`，开发者不知需 `--features embeddings` 可能困惑。
- **CI 中 git dependency 通过 SSH key 拉取**：`agentgrep` 和 `mermaid-rs-renderer` 来自 `git@github.com:1jehuang/*.git`，CI 需 `DEPLOY_KEY` secret 配 SSH agent；本地开发也需 SSH key 配置正确否则 `cargo build` 失败。
- **Cross-compile 的 ARM64 Windows 受限**：`windows-cross-check` 中 ARM64 标 `continue-on-error: true`，cargo-xwin 和 ring 间有已知兼容问题；Release 构建通过 native ARM64 runner 绕过。
- **根 crate `autobins = false`**：手动声明 6 个 bin target，新 bin 文件忘加到 Cargo.toml 则不编译。
- **Profile `dev` 设 `debug = 0`**：默认 dev profile 无调试信息，交互式调试（gdb/lldb）不友好，需开发者手动覆盖。

## 历史修复记录汇总索引

接手项目后的 bug 修复记录（原 CLAUDE.md Project Memory 小节）已迁移到各对应文档的「陷阱与历史修复」小节：

| 修复 | 涉及 commit | 归档位置 |
|---|---|---|
| SAITEC-Skills HTTP transport（no local vendor） | — | [09-mcp.md](09-mcp.md) |
| OpenAI-compatible: config vs credentials env hygiene | — | [03-provider.md](03-provider.md) |
| AuthTest deadlock from stale `auth-validation.json` | `b18a4c17` | [01-cli.md](01-cli.md) + [06-auth-login.md](06-auth-login.md) |
| MCP `notifications/initialized` 无 `id` + NDJSON reconnect storm + Unknown tool 链 | `fix/mcp-notification-id` | [09-mcp.md](09-mcp.md) + [05-tui.md](05-tui.md) + [11-bus-message-protocol.md](11-bus-message-protocol.md) |
| OpenAI-compatible 200K / `anthropic/claude-sonnet-4` regression | `e05304a1` | [03-provider.md](03-provider.md) |
| Config.toml named provider `JCODE_OPENROUTER_MODEL` symmetric cleanup | after `e05304a1` | [03-provider.md](03-provider.md) |
| Restart endpoint reversion to localhost:11434 from stale Ollama marker | `dba79fc3` | [03-provider.md](03-provider.md) |

## 回指

- 构建命令（高频使用，保留在 CLAUDE.md 顶部）：[CLAUDE.md](../CLAUDE.md)「Build & Test Commands」
- 各 crate 在子系统中的角色：见 [00](00-overview-and-entry.md)~[11](11-bus-message-protocol.md) 对应文档

## 原生 jcode Crate 边界与模块化设计原则

> **设计文档来源**：`docs/CRATE_OWNERSHIP_BOUNDARIES.md`（原生 jcode 设计决策和编译优化策略，SAITEC-TUI 继承自 jcode 但 crate 结构可能不同）

### 三支柱所有权规则

| 规则 | 内容 |
|------|------|
| **`*-types` crates 拥有稳定的数据合约** | plain 数据结构、序列化 shape、纯辅助方法；无 filesystem/network/process/TUI/provider/global state/storage 依赖；依赖限于 serde/chrono 和其他 type crates |
| **领域行为模块拥有根运行时行为** | root modules 保留 behavior 当需要 `storage`/`config`/`logging`/`server`/provider HTTP/tokio runtime/TUI rendering 时 |
| **`jcode-core` 是真正共享的原语** | 跨领域原语、极小的轻量 helper、临时 DTO 暂存（不应堆积，长集群了就 split） |

### 编译速度决策规则

优先 split 当能减少 root crate churn 或 dependency fan-out。**不要为 tidy 而 split**——需满足至少一项编译收益：

- Common root behavior 修改不再触及 stable type definitions。
- 纯类型修改只需编译小 type crate + 有限 dependents。
- Heavy dependencies 不进入 DTO crates。
- 多个下游 crate 可用小 contract 而不依赖 root crate。

### Re-export 迁移策略

1. 移动 type 到目标 crate。
2. 保留旧 root path 为 `pub use ...`。
3. 验证 focused tests + selfdev build/reload。
4. 待下游 crates 可直接依赖 domain crate 后，删除旧的 root re-exports。

### 迁移 Checklist

**1. Classify：**
- [ ] 是稳定的数据合约或纯 helper，而非 root runtime behavior？
- [ ] 有 inherent methods 吗？
- [ ] 它们需要 root-only API（storage/network/TUI/process/globals）吗？
- [ ] 若 behavior 也必须移动，能一起移动而不增加 fan-out 吗？

**2. Compatibility：**
- [ ] serde representation 一致？defaults/skips/renames/enum discriminants 保留？
- [ ] field visibilities 仍然合适？
- [ ] Root 可保留 compatibility re-export？

**3. Crate Health：**
- [ ] 目标 crate 已有需要的 dependency policy？
- [ ] 新依赖限于 `serde`/`serde_json`/`chrono`/sibling type crates？
- [ ] 目标 crate 仍无环？
- [ ] `cargo metadata` 确认未把 root/TUI/provider/storage/server/process 依赖拉进 type crate？

**4. Validate：**
- [ ] 有 focused test filter 覆盖？
- [ ] `cargo check --profile selfdev -p <type-crate> -p jcode --bin jcode` 通过？
- [ ] 相关 focused root tests 通过？
- [ ] `cargo fmt` 通过？
- [ ] 从 clean committed HEAD 起 selfdev build + reload 通过？

### Dependency Boundary Guard

```sh
python3 scripts/check_dependency_boundaries.py
```

阻止 `jcode-*-types` crates 直接依赖 root/runtime-heavy crates（`jcode`、`jcode-core`、provider crates、TUI crates、protocol/runtime crates、desktop/mobile crates）。Type crates 仅可依赖外部轻量库和其他 type crates。

### Test Policy — Focused Filters

已知 broad filter 陷阱（模块化过程中观察到）：

| Filter | 意外包含 | 建议 |
|--------|----------|------|
| `side_panel` | 侧边栏 pinned UI/layout + latency benchmark 测试 | 用 exact test names |
| `usage` | app-display 测试 | `cargo test --profile selfdev -p jcode usage::copilot_usage::tests --lib` 等 |
| `session::` | live-attach server tests + picker behavior | focus session persistence |
| `ambient` | TUI/helper integration tests + config + schedule state | `cargo test --profile selfdev -p jcode ambient::ambient_tests --lib` 等 |

### `jcode-core` Fan-out 审计

`jcode-core` 是**高风险 crate**（根 crate 的唯一直接依赖，root re-export 多模块）。一次 `jcode-core` 修改触发全量下游检查（~65s）。

已识别可迁出模块：

| 模块 | 现状 | 推荐去向 |
|------|------|----------|
| `ambient_usage_types` | 已迁到 `jcode-ambient-types` | 根保持 compat re-export |
| `copilot_usage_types` | 已迁到 `jcode-usage-types` | 同上 |
| `gateway_types` | 已迁到 `jcode-gateway-types` | 同上 |
| `memory_types` | 已迁到 `jcode-memory-types` | 同上 |
| `usage_types` | 已迁到 `jcode-usage-types` | 同上 |
| `goal_types` / `todo_types` / `catchup_types` | 仍在 `jcode-core` | 考虑合并到 `jcode-task-types` |
| `env` / `id` / `panic_util` / `stdin_detect` | 留在 `jcode-core` | 通用工具，不合适 domain crate |

### 大模块重构目标

| 模块 | 目标 split 结构 |
|------|----------------|
| `session.rs` | metadata/model + persistence/journal replay + startup stubs + memory profiling + render + crash recovery |
| `ambient.rs` | cycle context I/O + state persistence + directive persistence + schedule queue + prompt building + manager/runtime |
| `usage.rs` | API fetch + response parsing + caches/sync + display + account selection + public DTOs in `-types` |
| `gateway.rs` | registry + pairing/auth + HTTP routes + WebSocket auth + WebSocket relay + gateway DTOs in `-types` |

### "Optimal Enough" 定义

结构足够好当：
- 每 type crate 有清晰领域和最小依赖集。
- `jcode-core` 仅含真正原语或文档化的临时 staging 模块。
- Root modules 不再在大文件中混合 DTO 块、persistence、runtime orchestration、rendering。
- 每领域有 focused validation 命令。
- 每次结构变更后 selfdev build/reload 仍有意义。

## 原生 jcode Windows 平台实现细节

> **设计文档来源**：`docs/WINDOWS.md`

### 设计原则：Zero cost on Unix

`#[cfg]` compile-time gates + type aliases——Linux/macOS 编译产物 byte-for-byte 不变；Windows 走 `#[cfg(windows)]`。无 trait、无动态分发、无运行时分支。

### 传输层抽象（`src/transport/`）

```
src/transport/
  mod.rs        — conditional re-exports（cfg-gated）
  unix.rs       — type aliases wrapping tokio Unix sockets（zero-cost）
  windows.rs    — named pipe Listener/Stream with split support
```

**Unix**：直接 re-export tokio 类型——编译后二进制与原版本逐字节相同。

**Windows**：
- **`Listener`**：Wraps `NamedPipeServer`，accept loop 每连接创建新 pipe 实例（named pipes 是 single-client）。
- **`Stream`**：Enum over `NamedPipeServer`（accepted）或 `NamedPipeClient`（connected），实现 `AsyncRead + AsyncWrite`。
- **`ReadHalf` / `WriteHalf`**：`Arc<Mutex<Stream>>`——named pipe 不支持 native kernel-level splitting。
- **`SyncStream`**：以常规文件打开 named pipe 用于 blocking I/O。

Socket path 到 pipe name 转换：`/run/user/1000/jcode.sock` → `\\.\pipe\jcode`。

### Platform Module（`src/platform.rs`）

| 函数 | Unix | Windows |
|------|------|---------|
| `symlink_or_copy(src, dst)` | `symlink()` | Try `symlink_file/dir`, fallback copy |
| `atomic_symlink_swap(src, dst, temp)` | Create temp symlink + rename | Remove + copy (best effort) |
| `set_permissions_owner_only(path)` | `chmod 600` | No-op |
| `set_permissions_executable(path)` | `chmod 755` | No-op |
| `is_process_running(pid)` | `kill(pid, 0)` | Returns `true` (stub) |
| `replace_process(cmd)` | `exec()` (替换进程) | `spawn()` + `exit()` |

### 从应用文件迁移到 transport/platform 模块的记录

所有 OS-specific 代码从以下文件迁出：`src/server.rs`、`src/tui/backend.rs`、`src/tui/client.rs`、`src/tui/app.rs`、`src/tool/communicate.rs`、`src/tool/debug_socket.rs`、`src/main.rs`、`src/build.rs`、`src/update.rs`、`src/auth/oauth.rs`、`src/skill.rs`、`src/video_export.rs`、`src/ambient.rs`、`src/registry.rs`、`src/session.rs`。

### Windows 特有依赖

```toml
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.59", features = ["Win32_Foundation", "Win32_System_Threading"] }
```

### 跨平台部分（不因平台改变）

所有 provider code (HTTP)、大部分 tool 实现、TUI rendering (crossterm + ratatui)、agent logic、memory、sessions、config、MCP client/server protocol、JSON serialization、protocol handling。

### 原生 jcode Windows 剩余工作（设计时的记录）

1. **Windows CI** — 添加 GitHub Actions Windows runner，测试编译和基本 IPC。
2. **Shell tool** — 检测平台，Windows 用 `cmd.exe` 或 `pwsh.exe`。
3. **Self-update** — 处理 Windows exe 替换（不能 overwrite running binary）。
4. **Testing** — Windows 上跑完整测试套件。
