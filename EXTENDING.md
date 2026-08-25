# SSC-TUI 定制改造指南

> 本文档写给**基于 SSC-TUI 基线进行二次开发的部门团队**：如何在基线上接入自己的 MCP 业务服务、扩展斜杠命令、给 Agent 添加内置工具、接入自己的模型网关，以及品牌化与测试规范。

> **关于案例的定位**：文中案例取自 SAITEC 部门的改造记录（153 个 commit，共同祖先 `340fb04c`）——它是一条**已经走过的路，不是标准答案**。SAITEC 的实现经历过多次返工：MCP 集成踩过协议错误引发的重连风暴、句柄泄漏导致的 Unknown tool（都是自己引入后修复的）；schedule 工具族最初只有创建、连自己人都要手改队列文件；更新通道与登录门禁最终因耦合过深被整体拆除。本指南尽量同时给出"怎么做"和"哪里返工过"，供各部门按自身情况取舍——**不必照抄 SAITEC 的选择**。

## 目录

- [改造路线总览](#改造路线总览)
- [A. 接入部门 MCP 服务](#a-接入部门-mcp-服务)
- [B. 扩展斜杠命令](#b-扩展斜杠命令)
- [C. 给 Agent 添加内置工具](#c-给-agent-添加内置工具)
- [D. 接入部门模型网关](#d-接入部门模型网关-provider)
- [E. 品牌化你的 Fork](#e-品牌化你的-fork)
- [F. 测试与质量规范](#f-测试与质量规范)
- [G. 跟随基线上游更新](#g-跟随基线上游更新)
- [附录：SAITEC-TUI 改造 commit 索引](#附录saitec-tui-改造-commit-索引)

---

## 改造路线总览

拿到基线后，典型改造按"离业务由近到远"分五个扩展面：

| 扩展面 | 改什么 | 是否需要改 Rust 代码 | 典型场景 |
|---|---|---|---|
| **A. MCP 服务** | TUI 外部的 MCP server + 一份 `mcp.json` | **否**（纯配置） | 接入部门业务 API（检测/评估/工单…） |
| **B. 斜杠命令** | `src/tui/app/commands.rs` 等 | 是（Rust） | `/export` 导出、`/download-latest` 更新 |
| **C. 内置工具** | `src/tool/` 实现 `Tool` trait | 是（Rust） | `cancel_schedule` 任务生命周期补全 |
| **D. Provider** | `crates/jcode-provider-metadata` 或 config.toml | 可选 | 接入部门模型网关 / 新模型供应商 |
| **E. 品牌化** | logo 字模、窗口标题、存储路径等 | 是（定点修改） | 把 SSC-TUI 变成 `XX-TUI` |

**选型建议**：业务能力优先走 **MCP（扩展面 A）**——零客户端代码、独立部署、跨 Agent 复用（Claude Code 等标准 MCP 客户端也能连）；只有需要深度集成 TUI 内部状态（会话数据、UI、进程内调度）的能力才下沉为内置工具（扩展面 C）。这条优先级是 SAITEC 一路返工后的体会，也是可商榷的起点。

---

## A. 接入部门 MCP 服务

基线实现了完整的 MCP 客户端（JSON-RPC 2.0，`src/mcp/`），**接入任何标准 MCP server 都不需要改客户端代码**，只写一份配置。

### A.1 两种模式与选型

| | 模式一：HTTP + API Key（远程） | 模式二：stdio（本地） |
|---|---|---|
| 部署 | 服务端集中部署在部门服务器 | 用户本地拉起子进程 |
| SKILL 文档/提示词 | **留在服务端**，按需经工具下发，不落客户端磁盘 | 打包随客户端分发，**无法保密** |
| 版本管理 | 服务端统一升级，全体用户即时生效 | 依赖用户更新 |
| 凭据 | API Key 经 HTTP header 传输，可接网关审计 | env 注入子进程 |
| 适用 | 有保密要求 / 多用户 / 需要统一管控 | 无保密要求 / 纯本地工具 |

SAITEC 在自己的场景里选择了**模式一**（有文档保密和集中管控需求）：MCP server 部署在部门服务器，SKILL 文档（含业务操作规范、参数规则等敏感提示词）只存在服务端，Agent 通过 `list_skills` / `get_skill_doc` 工具按需读取，本地磁盘不留副本；业务工具全部经服务端转发到 Core API（鉴权 + 数据库事务），MCP server 本身是无状态薄代理。

### A.2 模式一参考实现：HTTP 远程 MCP（以 SAITEC-Skills 为例）

#### 第 1 步：实现 MCP 服务端（Python FastMCP）

服务端最小骨架（摘自真实实现，Python `mcp` SDK）：

```python
# server.py — FastMCP 启动入口
from mcp.server.fastmcp import FastMCP

mcp = FastMCP("Dept-Skills", host="0.0.0.0")   # 服务名即工具前缀来源

from api_tools.text_tools import register_text_tools
register_text_tools(mcp)                        # 按业务域分文件注册

if __name__ == "__main__":
    mcp.run(transport="streamable-http")        # Streamable HTTP 协议
```

工具定义模式（`@mcp.tool()` 装饰器，docstring 即给 Agent 看的描述）：

```python
# api_tools/text_tools.py
def register_text_tools(mcp: FastMCP):

    @mcp.tool()
    def detect_text(text: str) -> dict:
        """Detect whether a text is AI-generated. ..."""
        resp = httpx.post(
            f"{CORE_API_BASE}/api/v1/skills/text-detect",
            json={"text": text},
            headers=_headers(),          # 见下文 API Key 透传
            timeout=60,
        )
        resp.raise_for_status()
        return resp.json()
```

三个值得交代的实现细节（其中两个是踩坑后才补的）：

1. **API Key 请求级透传**：用 `ContextVar` 而非 `os.environ` 存当前请求的 key，避免并发请求互相覆盖：

   ```python
   # api_key_context.py
   from contextvars import ContextVar
   api_key_var: ContextVar[str] = ContextVar("api_key", default="")

   def get_api_key() -> str:
       # contextvar 请求级（无并发竞态）；env 兜底
       return api_key_var.get() or os.environ.get("DEPT_API_KEY", "")
   ```

   中间件把 `X-API-Key` 请求头写入 contextvar；每个工具的 `_headers()` 统一经 `get_api_key()` 取值转发给 Core API。

2. **SKILL 文档服务端持有、动态下发**（文档保护的实现方式）：

   ```python
   @mcp.tool()
   def list_skills() -> dict:
       """List all available skill documentations."""
       # 读服务端 skills/*.md 目录，返回 name + 一行摘要

   @mcp.tool()
   def get_skill_doc(skill_name: str) -> dict:
       """Get the full content of a skill documentation."""
       # 返回服务端 markdown 全文；md 中写明 Agent 行为规范
   ```

   SKILL 文档里写的是**给 Agent 的操作规范**（真实样例）：必须先 `list_files` 再引用路径（云端文件可达性）、array/object/bool 参数必须传原生 JSON 类型而非字符串、先读文档再执行业务工具等。

3. **工具可见性开关**：按部署环境裁剪工具面：

   ```python
   # api_tools/tool_config.py
   TOOL_ENABLED = {"detect_text": True, "detect_video": False, ...}
   # server.py 启动时对 disabled 的调用 mcp.remove_tool(name)
   ```

   另外对文件上传/下载类长耗时操作实现了**短期令牌**（token → (api_key, expires_at)，TTL 5 分钟），避免长会话中 key 失效。

#### 第 2 步：TUI 侧配置（无需改代码）

编辑 `~/.ssc_tui/mcp.json`（全局）或项目目录 `./.jcode/mcp.json`（项目级，优先级更高，二者合并）：

```json
{
  "servers": {
    "Dept-Skills": {
      "type": "http",
      "url": "http://your-server:8000/mcp",
      "headers": { "X-API-Key": "sk-your-key" },
      "shared": true
    }
  }
}
```

字段说明（完整定义见 `src/mcp/protocol.rs` 的 `McpServerConfig`）：

| 字段 | 说明 |
|---|---|
| `type` | `"http"` 或 `"stdio"`（缺省按 stdio 兼容旧配置） |
| `url` | HTTP 模式必填，MCP 端点 |
| `headers` | 原样持久化、原样发送（基线不做任何改写/注入——见 `src/mcp/protocol_tests.rs:4` 的 verbatim 契约测试） |
| `shared` | 是否跨会话共享连接（无状态服务设 `true`） |

#### 第 3 步：验证

1. 启动 TUI，输入 `/mcp`（或触发重连）后，连接成功的 server 工具以 `mcp__Dept-Skills__<tool>` 前缀注册给 Agent；
2. 对 Agent 说"列出可用的 skills"——应看到 `mcp__Dept-Skills__list_skills` 被调用并返回服务端文档列表；
3. 排错：`~/.ssc_tui/logs/` 下有 MCP 连接日志；30 秒失败冷却、并发连接去重在 `src/mcp/pool.rs`。

#### 客户端协议实现（供深度排错参考，无需修改）

| 组件 | 位置 | 说明 |
|---|---|---|
| `McpConfig::load` | `src/mcp/protocol.rs` | 多源合并（全局 + 项目 + `~/.claude/mcp.json` 导入 + codex toml） |
| `HttpMessageTransport` | `src/mcp/transport.rs` | POST JSON-RPC、`Mcp-Session-Id` 会话粘连、SSE 响应解析 |
| `StdioMessageTransport` | `src/mcp/transport.rs` | 子进程 stdin/stdout 逐行 NDJSON |
| `SharedMcpPool` | `src/mcp/pool.rs` | 全局连接池、引用计数、leader/waiter 并发去重、30s 冷却 |

### A.3 模式二：stdio 本地 MCP（简要）

`mcp.json` 条目换成：

```json
{
  "servers": {
    "local-tool": {
      "command": "python",
      "args": ["-m", "my_mcp_server"],
      "env": { "MY_TOKEN": "xxx" }
    }
  }
}
```

TUI 按 stdio 子进程拉起并逐行交换 NDJSON。适合纯本地工具（文件处理、本地数据库查询）；**注意**：任何随客户端分发的提示词/文档都无保密性可言。

### A.4 凭据轮换与密钥保护（可选进阶，SAITEC 实战）

若部门需要"登录换 key → 热重连 MCP"（SAITEC 模式），先看我们返工后留下的三条教训，再决定是否采用同一条链路：

**教训 1：密钥永不落盘。** SAITEC 的 `mcp.json` 落盘形状只有 `type/url/shared`，不含 `X-API-Key`：
- `ensure_bootstrap`（每次 `McpConfig::load` 先执行）会**主动剥离**任何误持久化的 `X-API-Key` header；
- 真实凭据仅经 `apply_runtime_env` 在**内存中**注入 headers——磁盘上的配置文件泄露也拿不到密钥。

**教训 2：重连必须真正释放池句柄。** `pool.disconnect_server(name)` 若只标记不摘除句柄，重连后 Agent 沿用旧工具表会报 Unknown tool——这是我们集成时自己引入的 bug，`0f49c226` 才修复。

**教训 3：三层同步。** MCP 工具在 server 进程执行、TUI 是瘦客户端，凭据变化要三层都刷新：

```
(1) server 进程:  凭据变化 → reconnect/disconnect_saitec_mcp()（pool 断开 → 重读配置 → 重连）
                  登出时对每个 agent drop_tool_prefix("mcp__Dept-Skills__") + unlock_tools()
(2) Pool 层:      pool.disconnect_server(name) → McpConfig::load()（含新 key）→ connect_server
(3) TUI 进程:     manager.reacquire_pool_handle(name) + registry 重注册 mcp__ 前缀工具
```

完整参考实现：SAITEC-TUI 仓库 `src/saitec/mcp.rs`（273 行）+ commit `81b77707` / `11247bf0` / `0f49c226`。

---

## B. 扩展斜杠命令

### B.1 命令系统三层结构

一条 `/xxx` 命令生效需要三个接入点（外加可选的帮助文本）：

1. **处理函数**：`src/tui/app/commands.rs` 中 `handle_xxx_command(app, trimmed) -> bool`（返回 `true` 表示已消费）；
2. **dispatch 串接**：同文件的大 match/串链中加入 `|| handle_xxx_command(app, trimmed)`；
3. **命令注册**：`src/tui/app/state_ui_input_helpers.rs` 的 `RegisteredCommand::public("/xxx", "一句话描述")`——进补全建议与合法命令表；
4. （可选）**帮助文本**：`src/tui/app/input_help.rs` 增加 `"xxx" => "..."` 分支（`/help xxx` 显示）。

### B.2 案例：`/export`（纯本地同步命令的做法参考）

SAITEC-TUI 的 `/export [path]` 把当前会话 Q&A 导出为 Markdown（commits `22b70c82`、`f76f3e57`——后者修复远程模式下优先导出"可见"消息），共 4 处改动：

**① 处理函数**（`src/tui/app/commands.rs:1240`，SAITEC 版）：

```rust
fn handle_export_command(app: &mut App, trimmed: &str) -> bool {
    if trimmed != "/export" && !trimmed.starts_with("/export ") {
        return false;                       // 不是本命令，放行给下一个 handler
    }
    let raw_path = trimmed.strip_prefix("/export").unwrap_or_default().trim();
    let path = match resolve_export_path(app, raw_path) {
        Ok(path) => path,
        Err(message) => {
            app.push_display_message(DisplayMessage::error(message));
            return true;                    // 已消费，错误已提示
        }
    };
    let (markdown, pair_count) = build_qa_export_markdown(app);
    // ... create_dir_all(parent) + std::fs::write(&path, markdown) ...
    match std::fs::write(&path, markdown) {
        Ok(()) => {
            app.push_display_message(DisplayMessage::system(export_success_message(&path, pair_count)));
            app.set_status_notice("Q&A exported");
        }
        Err(error) => app.push_display_message(DisplayMessage::error(format!(
            "Failed to export Q&A to `{}`: {}", path.display(), error
        ))),
    }
    true
}
```

要点：入口先判断前缀不匹配则 `return false`（让 dispatch 链继续）；所有分支（含错误）必须 `return true` 防止落入"未知命令"；用户反馈用 `push_display_message`（气泡）+ `set_status_notice`（状态栏）。

**② dispatch 串接**（`commands.rs:1355`）：

```rust
        || handle_export_command(app, trimmed)
        || handle_transcript_command(app, trimmed)
        // ...其余 handler
```

**③ 命令注册**（`state_ui_input_helpers.rs:59`）：

```rust
RegisteredCommand::public("/export", "Export Q&A pairs to a Markdown file"),
```

**④ 帮助文本**（`input_help.rs`）：`"export" => "`/export [path]`\nExport the current session's Q&A pairs..."`。

**⑤ 测试**（模式见 [F.3](#f3-常用测试模式速查)）：在 `src/tui/app/tests/` 下加用例：构造 `create_test_app()` → `app.input = "/export".into(); app.submit_input();` → 断言 `display_messages` 最后一条含成功/失败消息。

### B.3 进阶案例：`/download-latest`（命令触发后台任务 + 事件回推 UI）

当命令需要**异步长任务**（轮询远端、下载），可采用"命令 → spawn 后台任务 → Bus 事件 → UI 渲染"四段式。SAITEC 的 TUI 更新通道（commit `f147e9fd`，约 700 行）是一份可拆解的参考实现——注意这条通道后来在基线剥离中被整体移除（耦合较深），借鉴其结构时建议同时评估可拆卸性：

1. **命令入口**（`src/tui/app/input.rs:2382`，SAITEC 版）：匹配 `/download-latest`（别名 `/tui-download`），从全局待更新状态读 payload，无更新则提示并退出；
2. **后台轮询**（SAITEC 版 `src/saitec/tui_update.rs::check_tui_update`）：TUI 启动时 `tokio::spawn`，**先 sleep 2s 等 App 完成 Bus 订阅**（broadcast 通道无回放，早发的事件会丢——真实踩坑）再 `GET {后端}/check-update?current_version=x.y.z` 比对版本；
3. **事件回推**：`Bus::global().publish(BusEvent::UpdateStatus(UpdateStatus::Available { current, latest, payload }))`（事件枚举含 Checking/Available/Downloading/DownloadProgress/Downloaded/Error 全生命周期，定义于 `src/bus.rs`）；
4. **UI 渲染**：状态栏 banner 读全局 `TUI_PENDING_UPDATE` 静态（RwLock）——**刻意绕过 `&dyn TuiState` trait 派发**，因为 draw_status 是 60fps 热路径，每帧走 trait 虚表 + Option 包装开销不划算（原实现注释）；
5. **下载执行**：`u` 快捷键或命令触发 → `reqwest` 流式下载（默认 headers 注入鉴权、**每 256KB publish 一次 DownloadProgress**、`watch::Receiver` 支持 Esc 取消、401 主动清理半成品文件并提示重新登录）。

改造时替换第 2 步的后端 URL 与鉴权即可复用整条链路。**决策指南**：

| 需求形态 | 推荐做法 |
|---|---|
| 纯本地、毫秒级（导出/统计/开关） | B.2 同步模板 |
| 远端轮询/下载/长任务 | B.3 后台任务 + `BusEvent` 模板 |
| 需要用户选择（多 provider/多选项） | AccountPicker overlay（参考 `open_base_model_login_picker`，`src/tui/app/auth.rs:52`） |

---

## C. 给 Agent 添加内置工具

内置工具是注册进 Agent `Registry` 的进程内能力（模型以 function-calling 调用），与 MCP 工具对立统一：**MCP 管业务面（可部署、可复用），内置工具管集成面（要碰 TUI 内部状态）**。

### C.1 Tool trait

定义在 `jcode-tool-core` crate（经 `src/tool/mod.rs:42` re-export），四个必须实现的方法：

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;                  // 工具名（Agent 调用标识）
    fn description(&self) -> &str;           // 给模型看的功能描述
    fn parameters_schema(&self) -> Value;    // JSON Schema
    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolOutput>;
}
```

### C.2 案例：补全 `schedule` 任务生命周期（commit `340fb04c`）

这是对早期设计的补课：`schedule` 工具最初只有创建路径，查看和取消要**手工编辑队列文件**——直到实际使用不便才补齐 create → list → cancel（三个工具现均已在基线中，`src/tool/mod.rs:187-198`）。以 `cancel_schedule` 为例：

```rust
// src/tool/ambient.rs（SAITEC 版 340fb04c 新增，基线已包含）
pub struct CancelScheduleTool;

#[derive(Deserialize)]
struct CancelScheduleToolInput {
    task_id: String,
}

#[async_trait]
impl Tool for CancelScheduleTool {
    fn name(&self) -> &str { "cancel_schedule" }

    fn description(&self) -> &str {
        "Cancel a pending scheduled task by its id (returned by the schedule tool)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["task_id"],
            "properties": {
                "intent": super::intent_schema_property(),
                "task_id": {
                    "type": "string",
                    "description": "The id of the scheduled task to cancel, e.g. sched_1a2b3c4d"
                }
            }
        })
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolOutput> {
        let params: CancelScheduleToolInput = serde_json::from_value(input)?;
        // 委托 AmbientManager::cancel(id)，not-found 时返回明确错误
    }
}
```

配套的领域层改动：`ScheduledQueue::remove(id)`（按 id 删除并持久化队列文件）、`AmbientManager::cancel(id)` 委托。`list_schedule` 同理（返回 id/context/priority/due/target 列表）。

### C.3 注册进 Registry

`src/tool/mod.rs` 的 `Registry::new` 工具表（一行一个）：

```rust
// src/tool/mod.rs:187-198（基线现状）
Self::insert_tool_timed(&mut m, &mut timings, "schedule", ambient::ScheduleTool::new);
Self::insert_tool_timed(&mut m, &mut timings,
    "cancel_schedule", ambient::CancelScheduleTool::new);
Self::insert_tool_timed(&mut m, &mut timings,
    "list_schedule", ambient::ListScheduleTool::new);
```

### C.4 编写要点（来自实录的经验）

- **输入用强类型**：`#[derive(Deserialize)]` 一个私有 struct，`serde_json::from_value` 一次解出，字段类型错误让模型自己重试；
- **description 写"何时用"而非"是什么"**：模型靠它决策调用时机，写清前置条件（如 "by its id (returned by the schedule tool)"）；
- **错误信息给模型可行动的下一步**（"not found, call list_schedule first" 优于 "error"）；
- **无状态工具每次 execute 现场加载**：schedule 工具族不持有常驻 manager，每次 `AmbientManager::new()?` 从磁盘 load 队列——这样 list/cancel **立即对其他会话创建的任务生效**，跨会话一致（此前用户只能手改队列文件）；
- **工具 id 在创建时返回给模型**：`schedule` 返回 `sched_{8位hex}`，`cancel_schedule` 的 schema 描述里明写这个来源——模型能把两次调用串起来；
- **新工具必须带回归测试**（该 commit 全部新行为均有 TDD 测试覆盖，见 commit message）。

---

## D. 接入部门模型网关（Provider）

两条路径，按需选择：

### D.1 零代码：config.toml named provider

任何 OpenAI-compatible 网关不改代码即可接入——用户 `~/.ssc_tui/config.toml`：

```toml
[provider.dept_gateway]
api_base = "http://gateway.internal:8000/v1"
api_key_env = "DEPT_API_KEY"       # key 放 env，不进配置文件
default_model = "dept-model-v1"
```

详见 `dev_ref_docs/03-provider.md` 与 `dev_ref_docs/13-config.md`。

### D.2 内置 profile（部门批量分发时）

要把网关做成开箱即用（用户 `/login` 列表直接可见），在 `crates/jcode-provider-metadata/src/lib.rs` 加一个 `OpenAiCompatibleProfile`（struct 定义 `:119`，profile 表 `openai_compatible_profiles()` `:1132`）：

```rust
OpenAiCompatibleProfile {
    id: "dept-gateway",
    display_name: "Dept Gateway",
    api_base: "http://gateway.internal:8000/v1",
    api_key_env: "DEPT_API_KEY",
    env_file: "dept.env",
    default_model: Some("dept-model-v1"),
    requires_api_key: true,
    // ...
}
```

之后 `/login base-models` 列表、`/account dept-gateway login`、env 文件持久化全部自动生效。注意坑：切换 named profile 时清理旧 profile 的 env（历史修复 commit `635154d8` / `e05304a1`，详见 `dev_ref_docs/03-provider.md` 的"陷阱与历史修复"）。

---

## E. 品牌化你的 Fork

把 SSC-TUI 变成 `XX-TUI` 的定点修改清单（SAITEC 替换时实际触碰过的文件；两次替换的教训：**分小批做、每批保持可编译**——我们曾在一个 commit 里删了 PNG 却把引用它的代码留在下一个 commit，造成该 commit 单独 checkout 无法编译；还曾把存储路径改错方向、后续再返工修正）：

| 触点 | 位置 | 说明 |
|---|---|---|
| 启动像素 logo | `src/tui/ui_header.rs:303` `startup_logo_text_lines` | `█` 块拼字母，full（5 行）/ compact（3 行）/ 窄终端 fallback 三档 |
| 会话 header 品牌行 | `src/tui/ui_header.rs` `animated_brand_header_line_for` | 无会话名时显示的品牌文本（当前 "SSC-TUI"） |
| 窗口标题 | `src/cli/tui_launch.rs:21` `SSC_WINDOW_TITLE` | 终端窗口标题常量（3 处使用点） |
| 存储根目录 | `crates/jcode-storage/src/lib.rs:76` `home.join(".ssc_tui")` | 用户配置/会话/日志根；**改这里即可全局生效**（其余代码都走 `jcode_dir()`） |
| 登录入口显示名 | `crates/jcode-provider-metadata/src/lib.rs` `JCODE_LOGIN_PROVIDER.display_name` | 若保留平台登录骨架 |
| 遥测文档链接 | `src/telemetry.rs` `TELEMETRY.md` 链接 | 指向你自己的仓库 |
| README / CLAUDE.md / dev_ref_docs 标题 | 各 md | `grep -ri "ssc" --include="*.md"` 扫尾 |

**可以不改**：`JCODE_*` 环境变量族、`jcode.env`、crate 名（`jcode-storage` 等）、`~/.ssc_tui/jcode.env` 文件名——这些是项目代号不是品牌，保留它们能让你持续低成本合并上游更新。

**登录门禁（可选）**：SAITEC 曾实现"必须登录部门账号才能用 TUI"（启动登录门禁 + 表单登录 + 平台凭据），该能力在基线剥离中移除。若部门需要，参考 SAITEC-TUI 仓库 `src/saitec/auth.rs`（1749 行）与 `dev_ref_docs/06-auth-login.md` 的历史描述；不需要门禁时，凭据可直接经 `mcp.json` headers / provider env 文件下发。

---

## F. 测试与质量规范

### F.1 测试组织

| 层级 | 位置 | 运行 |
|---|---|---|
| 单元测试 | 各源文件内 `#[cfg(test)]`（~64 处） | `cargo test -p jcode --lib` |
| E2E | `tests/e2e/`（mock provider，无真实 API） | `cargo test --test e2e <name> -- --exact` |
| 预算守卫 | `scripts/`（code size / panics / test size / warnings） | CI 自动运行 |

CI 五个 job 的结构见 `dev_ref_docs/12-workspace-build-ci.md`。**新增功能的验收线：`cargo check -p jcode --tests` 0 error + 新增行为有回归测试**。

### F.2 提交规范

参考基线 git 历史：`feat(scope):` / `fix(scope):` / `chore(scope):` / `test(scope):`，正文写动机 + 行为变化 + 验证方式。`340fb04c` 的写法可供参考（"Previously schedule only had a create path... All new behavior is covered by TDD regression tests"）。

### F.3 常用测试模式速查

```rust
// 1. 环境隔离（所有碰 env/HOME 的测试必用）
let _lock = crate::storage::lock_test_env();          // 全局串行锁
let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());

// 2. 命令行为测试
let mut app = create_test_app();
app.input = "/export".to_string();
app.submit_input();
assert!(app.display_messages().last().unwrap().content.contains("exported"));

// 3. UI 渲染断言（TestBackend 离屏渲染）
let backend = ratatui::backend::TestBackend::new(80, 24);
let mut terminal = ratatui::Terminal::new(backend).unwrap();
terminal.draw(|frame| crate::tui::ui::draw(frame, &state)).unwrap();
let rendered = buffer_to_text(&terminal).join("\n");
assert!(rendered.contains("..."));
```

---

## G. 跟随基线上游更新

基线仓库会持续演进（bug 修复、新能力）。你的 fork 与基线的**共同祖先是 `340fb04c`**。同步策略：

```bash
git remote add base https://github.com/LiangRichard13/ssc_tui_base.git
git fetch base
git log --oneline HEAD..base/main          # 看基线新增了什么
git cherry-pick <commit>                   # 按需摘取（推荐：小步、逐个）
# 或整体合并（定制面大时冲突较多，需人工审）
git merge base/main
```

冲突高发区预判：品牌化触点（E 节清单中的文件）、命令注册列表、`commands.rs` dispatch 链——这些地方你的定制与上游演进最容易碰撞，cherry-pick 时优先小粒度。

---

## 附录：SAITEC-TUI 改造 commit 索引

SAITEC 定制期共 153 个 commit（`f1deb6bf..340fb04c`，仓库 [LiangRichard13/SAITEC-TUI](https://github.com/LiangRichard13/SAITEC-TUI) 分支 `feat/saitec-mcp-http-transport`）。按扩展面分组的代表作：

**A. MCP / HTTP transport 演进链**（小步提交序列，每步可独立编译验证；链上的多个 commit 是对前面引入 bug 的修复，一并列出供对照）：

| Commit | 内容 |
|---|---|
| `1897ef56` | `McpServerConfig` 增加 `McpTransport` enum（stdio \| http） |
| `bd78112e` | 抽象 `MessageTransport` trait |
| `3b604f66` | `McpClient` 经 trait 分发（transport 解耦） |
| `8fbaeddb` | bootstrap 写入 HTTP transport 配置（X-API-Key header） |
| `4dd93191` | HTTP transport closed-flag + 池化句柄重取 |
| `11247bf0` | MCP 生命周期与服务器鉴权变更同步 + mcp.json 路径迁移 |
| `81b77707` | 登录/登出三层 MCP 重连 + 凭据转发 |
| `e561ee2c` | 协议错误 → 重连风暴 → Unknown tool 链的 4 个修复 |
| `0f49c226` | shared disconnect 真正移除池句柄（重连后工具表刷新） |

**B. 命令 / 更新通道**：`22b70c82` / `f76f3e57`（/export 两连）、`f147e9fd`（TUI 更新推送通道全链路）、`7c700630`（banner 与 spinner 冲突修复）、`44be444a`（本地 mock server 用于更新端点开发）

**C. 内置工具**：`340fb04c`（schedule 三件套补全，TDD 全覆盖范本）

**D. Provider / 模型目录 / 登录（~65 个，量大）**：`80e333f8`（自定义 base-model 登录）、`e147098d`（无默认模型时提示输入）、`e05304a1` / `dba79fc3` / `251a07f6` / `635154d8`（openai-compatible 环境变量与 profile 四连修复）、Kimi 系列与 model picker 系列（各 ~10 个，见仓库 `git log --oneline f1deb6bf..340fb04c | grep -i kimi`）、`81b77707` / `11247bf0`（登录登出 MCP 三层同步）

**E. 品牌化**：`c4c3ae03` → `42358f97` → `3fe29808` → `03c46037`（按 auth/prompts/CLI/setup-hints 分批替换 jcode→SAITEC-TUI 的四连提交——品牌替换应分批做、每批可编译可验证）

**剥离（反向工程）**：`2e6f65c1..ec271540`（基线仓库 main 的前 24 个 commit）——SAITEC 定制被逐层移除的过程，可作为"哪些定制容易剥离、哪些（如登录门禁、更新通道）耦合较深、剥离代价大"的一份对照材料
