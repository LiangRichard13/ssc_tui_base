# 16 · Overnight（无人值守运行）

> 子系统：无人值守长时 agent 执行——manifest 创建、coordinator session 启动、资源采样、任务卡、preflight 检查、review HTML、安全审批。含 ambient 模式与安全系统。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

Overnight 子系统实现无人值守的长时 agent 运行：manifest 创建、coordinator 会话启动、资源采样、任务卡生成、preflight 检查、review HTML 生成；配套安全系统（动作分类、权限队列、审批历史、ambient transcript）和通知分发（ntfy/desktop/email）确保无人值守时 agent 不执行危险操作并通知用户。

## 关键文件清单

| 路径 | 职责 |
|---|---|
| `src/overnight.rs` | 主模块(~1275 行)：manifest 创建、coordinator session 启动、资源采样、任务卡、preflight、review HTML |
| `crates/jcode-overnight-core/` | 核心逻辑 crate：Overnight Start/Status/Cancel/Review 命令、任务卡系统、中断恢复、morning report |
| `src/safety.rs` | 无人值守安全系统(~702 行)：动作分类(AutoAllowed/RequiresPermission)、权限请求队列、审批决策历史、ambient transcript 记录；`SafetySystem` 管理权限队列和通知 |
| `src/notifications.rs` | ambient 模式通知调度器(~568 行)：ntfy.sh 推送、Desktop 通知(notify-send)、Email SMTP(lettre)；fire-and-forget，错误仅记录 |
| `src/tui/app/commands_overnight.rs` | TUI `/overnight` 命令处理(若存在) |

## 核心类型与关键函数

- **`SafetySystem`** (`src/safety.rs`) — 管理权限队列和通知，core 类。
- **动作分类**：`AutoAllowed` / `RequiresPermission`——agent 无人值守时执行工具前先过安全分类。
- **`CatchupBrief`** 见 [08-storage-session.md](08-storage-session.md) 的 catchup——overnight 跑完后用户回来接管时用 catch-up brief 快速了解进展。
- overnight 命令(`jcode-overnight-core`)：`Start` / `Status` / `Cancel` / `Review`。
- overnight manifest / 任务卡 / review HTML / morning report：见 crate。

## 主控制流

```
 Overnight Start
  → 创建 manifest（任务卡、资源预算、预期产出）
  → preflight 检查（凭据/磁盘/网络）
  → 启动 coordinator session（headless，见 04-server headless 模式）
  → ambient runner 接管循环（见 04-server ambient 系统）
  → 每个工具调用前过 SafetySystem 动作分类
       → AutoAllowed: 直接执行
       → RequiresPermission: 入权限队列 → 通知用户(ntfy/desktop/email) → 等审批
  → 资源采样 + milestone 记录
  → 结束生成 review HTML / morning report
  → 用户回来时 catch-up brief 接管
```

## 依赖关系

- **依赖**：[04 Server](04-server.md)（ambient runner 接管循环、coordinator session 用 headless 创建、swarm 成员管理）、[13 Config](13-config.md)（`SafetyConfig` 通知渠道）、[14 Telemetry](14-telemetry.md)（夜间运行事件采集）、[08 Storage](08-storage-session.md)（manifest/任务卡持久化、catch-up brief）、[11 Bus](11-bus-message-protocol.md)（`BackgroundTask*`/通知事件）。
- **被依赖**：`src/cli/commands.rs`(ambient 相关 CLI)、`src/tui/app/commands*.rs`(TUI `/overnight`/`/review` 等)。
- 与 [04 Server](04-server.md) 的 ambient 系统是「高级变体」关系：overnight 是定向的、带任务卡和 review 的 long-running ambient。

## 原生 jcode Safety System 原始设计参考

> **设计文档来源**：`docs/SAFETY_SYSTEM.md`（原生 jcode 设计，SAITEC-TUI 实现可能有差异，以下内容标注原生设计意图）

### 设计哲学

- 只有两个 tier：`AutoAllowed`（无需许可）和 `RequiresPermission`（需要许可）。
- **没有 "always denied"**：只要用户显式批准，agent 可以做任何事——Safety System 的责任是确保用户被询问，而非阻止行为。
- 核心原则：**任何与其他人类通信或在本地沙箱外留痕迹的行为都需要 permission**。

### Action Classification（原生详细分类）

**Tier 1: Auto-Allowed（无需 permission）**

| Action | Rationale |
|--------|-----------|
| Read files in project | Read-only, no side effects |
| Read git history / status | Read-only |
| Run tests (read-only) | Verification, no mutations |
| Memory operations (within per-cycle caps) | Local data, reversible |
| Create local branches / git worktrees | Local only, easily deleted |
| Write to ambient's own log/state files | Internal bookkeeping |
| Embed / similarity search | Computation only |
| Analyze sessions for extraction | Read-only analysis |

**Tier 2: Requires Permission（需用户批准）**

| Category | Action |
|----------|--------|
| **Human communication** | Send emails, submit assignments, post to Slack/Discord, create GitHub issues/PR comments |
| **Code modifications** | Modify code (must use worktree + PR), push to remote, create PRs, modify CI/CD |
| **System changes** | Install packages, modify dotfiles, start network services |
| **Deployment** | Deploy to any environment |
| **Data** | Delete files outside project sandbox, drop databases |
| **Financial/Account** | Purchases, change passwords/API keys, revoke tokens |

### Custom Rules（原生设计，SAITEC-TUI 可能未实现）

```toml
[safety.rules]
# Promote: allow ambient to create PRs without asking
allow_without_permission = ["create_pull_request"]

# Demote: always ask before running any tests
require_permission = ["run_tests"]

# Override: allow push to specific remotes
allow_push_to = ["origin"]
```

### Permission Request Tool（原生 `request_permission` 接口）

```rust
// request_permission tool
{
    "action": "create_pull_request",
    "description": "Create PR for ambient/fix-auth-tests branch with 3 test fixes",
    "rationale": "Found 3 failing tests in auth module...",
    "urgency": "low",     // "low" | "normal" | "high"
    "wait": false         // block until approved?
}
```

**返回**：
- `wait=true` 且用户响应：`{ "approved": true, "message": "looks good" }`
- `wait=true` 且超时：`{ "approved": false, "reason": "timeout", "timeout_minutes": 60 }`
- `wait=false`：`{ "queued": true, "request_id": "req_abc123" }`

**Agent 等待行为**：
- `wait: true`：agent 不阻塞整个 cycle——继续其他 ambient 工作；用户批准后 action 排队到下一 cycle（或当前 cycle 若仍在运行）。用户不响应则超时并记录。
- `wait: false`：request 入队，agent 不等待，批准后在下一 cycle 执行。

### Notification Channels（原生设计）

| 通道 | 实现 |
|------|------|
| Email | SMTP / SendGrid / SES |
| SMS | Twilio |
| Desktop | notify-send / Wayland |
| Webhook | Custom HTTP POST |
| TUI | In-app badge |

**配置示例**：
```toml
[safety.notifications]
email = true
sms = false
desktop = true
webhook = false

[safety.notifications.preferences]
min_urgency = "low"
batch_interval_seconds = 60   # 收集 60s 后批量发送
quiet_start = "23:00"         # 静默时段（除 high urgency）
quiet_end = "07:00"
```

### Session Transcript（原生 JSON schema）

每次 ambient cycle 后生成 transcript：

```json
{
    "session_id": "ambient-2026-02-08-143022",
    "actions": [
        {
            "type": "memory_consolidation",
            "description": "Merged 2 duplicate memories about dark mode preference",
            "tier": "auto_allowed"
        },
        {
            "type": "permission_request",
            "description": "Create PR for 3 auth test fixes",
            "tier": "requires_permission",
            "status": "pending",
            "request_id": "req_abc123"
        }
    ],
    "pending_permissions": 1,
    "scheduled_next": "2026-02-08T15:05:00Z"
}
```

### Review Queue 存储

```
~/.ssc_tui/safety/
├── queue.json              # Pending permission requests
├── history.json            # Past decisions (for learning patterns)
└── config.json             # Cached safety configuration
```

**Decision History** 用于学习模式——若用户反复批准某类 action，可建议 promote 到 auto-allowed。

### Integration API（原生 SafetySystem 接口设计）

```rust
pub struct SafetySystem {
    classifier: ActionClassifier,
    queue: ReviewQueue,
    notifier: NotificationDispatcher,
    logger: TranscriptLogger,
}

impl SafetySystem {
    pub fn is_auto_allowed(&self, action: &Action) -> bool;
    pub async fn request_permission(&self, request: PermissionRequest) -> PermissionResult;
    pub fn log_action(&self, action: &ActionLog);
    pub fn generate_summary(&self) -> SessionSummary;
    pub fn pending_requests(&self) -> Vec<PermissionRequest>;
    pub fn record_decision(&self, request_id: &str, decision: Decision) -> Result<()>;
}
```

### 实现阶段（原生设计 Phases 1-5）

| Phase | 内容 | SAITEC-TUI 状态 |
|-------|------|-----------------|
| 1 | Action classifier, Review queue, `request_permission` tool, Transcript logger | 已实现 (`src/safety.rs`) |
| 2 | Notification channels (desktop, email, webhook, SMS) | 部分（`src/notifications.rs` 有 email/desktop/ntfy） |
| 3 | Review interfaces (TUI panel, CLI commands, email approve/deny) | TBD |
| 4 | Configuration (custom rules, per-project overrides, notification config) | 部分（`SafetyConfig` 在 config） |
| 5 | Intelligence (decision history, pattern detection, urgency inference) | 未实现 |

## 原生 jcode Ambient Mode 原始设计参考

> **设计文档来源**：`docs/AMBIENT_MODE.md`（原生 jcode 设计。SAITEC-TUI 的 `src/ambient/runner.rs` / `scheduler.rs` 实现了部分调度和运行逻辑，但以下设计细节可能未同步实现。）

### 三大职责

1. **Garden**（园艺）—— consolidated、prune、strengthen memory graph
2. **Scout**（侦察）—— 分析最近 sessions、git history、memories，理解用户感兴趣什么
3. **Work**（工作）—— 主动完成用户会感激的 surprise work

**设计决策**：三者不是分离 phase——agent 在一次 cycle 中自然地同时处理这三者。

### 关键设计决策

1. **单 agent 单次**：同时只有一个 ambient 实例运行，无并行。
2. **Subscription-first**：默认用 OAuth（OpenAI/Anthropic），不用 API key 除非显式配置。
3. **User priority**：交互式 session 永远优先于 ambient。
4. **Strong models**：用 provider 最强可用模型，确保 agent 能推理出什么工作真正有用。
5. **Self-scheduling**：agent 自己决定何时唤醒，由自适应资源限制约束。

### `end_ambient_cycle` 工具（原生设计，SAITEC-TUI 可能使用不同接口）

每 cycle **必须**以该 tool call 结束：

```rust
{
    "summary": "Merged 3 duplicate memories, pruned 2 stale facts...",
    "memories_modified": 8,
    "compactions": 2,
    "proactive_work": null,
    "next_schedule": {
        "wake_in_minutes": 25,
        "context": "Verify 4 remaining stale facts"
    }
}
```

| 字段 | Required | 描述 |
|------|----------|------|
| `summary` | yes | 人类可读摘要（→ email + widget） |
| `memories_modified` | yes | 创建/合并/prune/更新数 |
| `compactions` | yes | 本 cycle 上下文压缩次数 |
| `proactive_work` | no | 主动代码变更描述 |
| `next_schedule` | no | 下次唤醒时间+上下文 |

### Unexpected Stop 处理

模型可能意外停止（output length、API error、random stop）——系统处理方式：

```
Running → Stopped → CheckTool{called end_ambient_cycle?}
  → Yes → Complete
  → No → Continuation（注入 continuation message）→ Running
  → Second stop without end_ambient_cycle → ForcedEnd → 生成 partial transcript + 默认调度
```

- Continuation message 格式：`You stopped unexpectedly without calling end_ambient_cycle...`
- 两次仍无 `end_ambient_cycle`：标记 `incomplete`，用系统 metrics 取 compaction count，调度默认 wake interval。
- 无 `schedule_ambient` 或 `next_schedule`：调度到 `max_interval_minutes`。

### 内存双层 Consolidation 架构

**Layer 1: Sidecar（每 turn，快速）**
- 只在已检索到的 memories 上操作，零额外延迟。
- Duplicate detection、contradiction detection、reinforcement。

**Layer 2: Ambient Garden（后台，深度）**
- 全图扫描，跨 session dedup，代码库事实校验，归因提取，prune dead memories，关系发现，embedding backfill，cluster 优化。

**Reinforcement Provenance**（每次强化记录 breadcrumb）：

```rust
pub struct Reinforcement {
    pub session_id: String,
    pub message_index: usize,
    pub timestamp: DateTime<Utc>,
}
// 每个 MemoryEntry 持 Vec<Reinforcement>，trace 回强化原因
```

### 自适应调度算法

两层级调度：agent 提出（`schedule_ambient` tool）→ 系统层约束（自适应计算器）。

自适应算法：
```
headroom = rate_limit - (user_usage_rate + ambient_usage_rate)
safe_interval = max(min_interval, target_budget_fraction / headroom)
```

策略：agent 说 "10m" 但系统算出 "30m 才安全" → push 到 30m；agent 说 "6h" 但 unused budget 充足 → pull forward 到 max_interval；用户活跃 → ambient pause 或重度 throttle。

**Event Triggers**（可提前唤醒，但仍过 resource gate）：

| 事件 | Priority | Rationale |
|------|----------|-----------|
| Session crashed | High | 可能遗漏 memory extraction |
| Session closed | Normal | 可能 unextracted memories |
| Git push | Low | Codebase 变化，facts 可能 stale |
| User idle > threshold | Low | Ambient 好时机 |
| 显式 `/ambient` 命令 | Immediate | 用户请求 |

### Provider / Model 选择优先级（原生设计）

```
OpenAI OAuth → Anthropic OAuth → OpenRouter/API key (opt-in) → Disabled
```

- **OpenAI first**：单独 rate limit pool，不与交互式 sessions 竞争。
- **Anthropic second**：subscription-based，无 per-token 成本。
- **OpenRouter/API keys last**：pay-per-token，仅 config opt-in 防烧钱。
- **Strong models**：ambient 需要 good judgment，弱模型会做错 proactive work。

### 原生 Ambient 配置

```toml
[ambient]
enabled = false
# allow_api_keys = false        # 默认 false，仅 OAuth
# min_interval_minutes = 5
# max_interval_minutes = 120
# pause_on_active_session = true
# proactive_work = true
# work_branch_prefix = "ambient/"
```

### Crash Safety & Recovery

- **Atomic writes**：memory graph / state 先写 temp file 再原子 rename，crash mid-write 不损数据。
- **Incremental checkpointing**：「last processed」marker 跟踪 cycle 进度，crash 后不重做已完成的。
- **Persistent queue**：scheduled queue 和 permission requests 在磁盘，非内存，重启存活。
- **Interrupted transcripts**：标记 `interrupted` 而非 `completed`。

**Restart recovery** 原则：
1. 不 replay missed cycles——just run one cycle examining current state。
2. 检查距上次运行时间——gap 大时 agent 自然发现 backlog（检查 current state 而非 diff）。
3. Expired scheduled items——still execute（context 仍有价值）。
4. Resume, don't restart——从 checkpoint 继续而非从头开始。

### Cold Start

- Start conservative：garden-only（no proactive work）。
- Build usage baseline：前几 cycle 仅观察和追踪用量模式。
- Proactive work 逐步解锁：N 次成功 garden cycle + user-approved 结果后。
- 或 user 通过 config opt-in 立即启用。

### 原生 Ambient 实现阶段（Phases 1-5）

| Phase | 内容 | SAITEC-TUI 状态 |
|-------|------|-----------------|
| 1 Foundation | Ambient agent loop, Single-instance guard, Basic scheduling, Provider selection chain, Config, Storage layout | 部分（`ambient/runner.rs`, `scheduler.rs`） |
| 2 Garden | Graph-wide dedup, Fact verification, Retroactive extraction, Pruning, Relationship discovery, Embedding backfill | 部分通过 memory 系统 |
| 3 Scheduling | `schedule_ambient` tool, Scheduled queue, Adaptive resource calculator, Usage history, Rate limit awareness, Event triggers | 部分 |
| 4 Proactive Work | Scout recent sessions + git, Infer priorities, Execute on separate branch | 未实现 |
| 5 Info Widget | Ambient status in TUI, Queue preview, Last cycle summary, Budget bar | 部分 |

## 陷阱与设计约束

- **Safety 系统专为 ambient/overnight 服务**：`SafetySystem` 的权限队列和 ambient transcript 直接关联 [04 Server](04-server.md) 的 ambient runner——不是通用运行时安全，只在无人值守场景激活。
- **通知 fire-and-forget**：`notifications.rs` 所有发送错误仅记录不阻塞——ntfy/desktop/email 任一通道失败不影响 agent 继续，但也意味着用户可能收不到关键审批请求。排查「没收到通知」时要查日志而非报错。
- **overnight 与 ambient 的边界**：ambient 是 server 无条件创建的 `AmbientRunnerHandle`（即使禁用也建），overnight 是其上的产品化封装。详见 [04-server.md](04-server.md) 的「与 Ambient 系统的关系」。
- **安全审批的异步性**：`RequiresPermission` 入队后等用户审批，期间 agent 阻塞——长任务深夜遇审批会一直挂起到早上，需在任务卡设计时预判会触发审批的工具。
- **`SafetyConfig` 通知渠道**：ntfy/email/telegram/discord 四通道配置在 [13-config.md](13-config.md)，配置缺失时对应通道静默跳过。

## 关联模块

| 模块 | 职责 | 归位说明 |
|---|---|---|
| `src/safety.rs`(702 行) | 无人值守安全系统（动作分类/权限队列/审批/transcript） | 本文档(overnight 专属安全) |
| `src/notifications.rs`(568 行) | ambient/overnight 通知分发(ntfy/desktop/email) | 本文档(ambient/overnight 通知通道) |
| `src/overnight.rs`(1275 行) + `jcode-overnight-core` | overnight 命令、manifest、任务卡、review | 本文档核心 |

## 回指

- ambient runner 基础设施(overnight 的底层)：[04-server.md](04-server.md)「与 Ambient 系统的关系」
- headless coordinator session 创建：[04-server.md](04-server.md) 的 `create_headless_session`
- 通知渠道配置：[13-config.md](13-config.md) 的 `SafetyConfig`
- 用户回来时的 catch-up brief：[08-storage-session.md](08-storage-session.md) 的 catchup 关联模块
- overnight 运行事件被 [14-telemetry.md](14-telemetry.md) 采集