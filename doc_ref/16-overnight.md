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