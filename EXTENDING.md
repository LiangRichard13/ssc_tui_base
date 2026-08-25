# SSC-TUI 定制改造指南

> 本文档写给**基于 SSC-TUI 基线进行二次开发的团队**：如何接入自己的 MCP 业务服务、扩展斜杠命令、给 Agent 添加内置工具、接入自己的模型网关，以及品牌化与测试规范。
>
> 指南中的方案与代码骨架都来自真实项目验证，文末附源码级参考实现索引。

## 目录

- [改造路线总览](#改造路线总览)
- [A. 接入业务 MCP 服务](#a-接入业务-mcp-服务)
- [B. 扩展斜杠命令](#b-扩展斜杠命令)
- [C. 给 Agent 添加内置工具](#c-给-agent-添加内置工具)
- [D. 接入模型网关](#d-接入模型网关-provider)
- [E. 品牌化你的 Fork](#e-品牌化你的-fork)
- [F. 测试与质量规范](#f-测试与质量规范)
- [G. 跟随基线上游更新](#g-跟随基线上游更新)
- [H. 参考实现索引](#h-参考实现索引)

---

## 改造路线总览

拿到基线后，典型改造按"离业务由近到远"分五个扩展面：

| 扩展面 | 改什么 | 是否需要改 Rust 代码 | 典型场景 |
|---|---|---|---|
| **A. MCP 服务** | TUI 外部的 MCP server + 一份 `mcp.json` | **否**（纯配置） | 接入业务 API（检测/评估/工单…） |
| **B. 斜杠命令** | `src/tui/app/commands.rs` 等 | 是（Rust） | `/export` 导出、`/download-latest` 更新 |
| **C. 内置工具** | `src/tool/` 实现 `Tool` trait | 是（Rust） | `cancel_schedule` 任务生命周期补全 |
| **D. Provider** | `crates/jcode-provider-metadata` 或 config.toml | 可选 | 接入模型网关 / 新模型供应商 |
| **E. 品牌化** | logo 字模、窗口标题、存储路径等 | 是（定点修改） | 把 SSC-TUI 变成 `XX-TUI` |

**选型建议**：业务能力优先走 **MCP（扩展面 A）**——零客户端代码、独立部署、跨 Agent 复用（Claude Code 等标准 MCP 客户端也能连）；只有需要深度集成 TUI 内部状态（会话数据、UI、进程内调度）的能力才下沉为内置工具（扩展面 C）。

---

## A. 接入业务 MCP 服务

基线实现了完整的 MCP 客户端（JSON-RPC 2.0，`src/mcp/`），**接入任何标准 MCP server 都不需要改客户端代码**，只写一份配置。

### A.1 两种模式与选型

| | 模式一：HTTP + API Key（远程） | 模式二：stdio（本地） |
|---|---|---|
| 部署 | 服务端集中部署 | 用户本地拉起子进程 |
| SKILL 文档/提示词 | **留在服务端**，按需经工具下发，不落客户端磁盘 | 打包随客户端分发，**无法保密** |
| 版本管理 | 服务端统一升级，全体用户即时生效 | 依赖用户更新 |
| 凭据 | API Key 经 HTTP header 传输，可接网关审计 | env 注入子进程 |
| 适用 | 有保密要求 / 多用户 / 需要统一管控 | 无保密要求 / 纯本地工具 |

若你的业务提示词或文档有保密要求、或需要集中管控版本，建议选模式一：SKILL 文档（业务操作规范、参数规则等）只存在服务端，Agent 通过 `list_skills` / `get_skill_doc` 工具按需读取，客户端磁盘不留副本；业务工具全部经 MCP server 转发到后端 Core API（鉴权 + 数据库事务），MCP server 本身保持无状态薄代理。

### A.2 模式一实战：HTTP 远程 MCP

#### 第 1 步：实现 MCP 服务端（Python FastMCP）

服务端最小骨架（Python `mcp` SDK）：

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

三个关键工程实践：

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

   SKILL 文档里写的是**给 Agent 的操作规范**，例如：必须先 `list_files` 再引用路径（保证服务端文件可达）、array/object/bool 参数必须传原生 JSON 类型而非字符串、执行业务工具前先读对应文档等。

3. **工具可见性开关**：按部署环境裁剪工具面：

   ```python
   # api_tools/tool_config.py
   TOOL_ENABLED = {"detect_text": True, "detect_video": False, ...}
   # server.py 启动时对 disabled 的调用 mcp.remove_tool(name)
   ```

   对文件上传/下载类长耗时操作，可加一层**短期令牌**（token → (api_key, expires_at)，TTL 5 分钟），避免长会话中 key 失效。

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

1. 重启 TUI（或让 Agent 执行 mcp 工具的 reload），连接成功的 server 工具以 `mcp__Dept-Skills__<tool>` 前缀注册给 Agent；
2. 对 Agent 说"列出可用的 skills"——应看到 `mcp__Dept-Skills__list_skills` 被调用并返回服务端文档列表；
3. 排错：`~/.ssc_tui/logs/` 下有 MCP 连接日志；30 秒失败冷却、并发连接去重在 `src/mcp/pool.rs`。

#### 客户端协议实现（供深度排错参考，无需修改）

| 组件 | 位置 | 说明 |
|---|---|---|
| `McpConfig::load` | `src/mcp/protocol.rs` | 多源合并（全局 + 项目 + `~/.claude/mcp.json` 导入 + codex toml） |
| `HttpMessageTransport` | `src/mcp/transport.rs` | POST JSON-RPC、`Mcp-Session-Id` 会话粘连、SSE 响应解析 |
| `StdioMessageTransport` | `src/mcp/transport.rs` | 子进程 stdin/stdout 逐行 NDJSON |
| `SharedMcpPool` | `src/mcp/pool.rs` | 全局连接池、引用计数、并发去重、30s 冷却 |

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

### A.4 凭据轮换与密钥保护（可选进阶）

若需要"登录换 key → 热重连 MCP"，核心是三条原则 + 一条链路：

**原则 1：密钥永不落盘。** 推荐的 `mcp.json` 落盘形状只有 `type/url/shared`，不含 `X-API-Key`：
- 配置引导逻辑在每次 `McpConfig::load` 时**主动剥离**任何误持久化的 `X-API-Key` header；
- 真实凭据仅加载配置后在**内存中**注入 headers——磁盘上的配置文件泄露也拿不到密钥。

**原则 2：重连必须真正释放池句柄。** `pool.disconnect_server(name)` 若只标记不摘除句柄，重连后 Agent 沿用旧工具表会报 Unknown tool（这是一个真实发生过的缺陷形态，修复见参考实现索引）。

**原则 3：三层同步。** MCP 工具在 server 进程执行、TUI 是瘦客户端，凭据变化要三层都刷新：

```
(1) server 进程:  凭据变化 → reconnect/disconnect（pool 断开 → 重读配置 → 重连）
                  登出时对每个 agent drop_tool_prefix("mcp__Dept-Skills__") + unlock_tools()
(2) Pool 层:      pool.disconnect_server(name) → McpConfig::load()（含新 key）→ connect_server
(3) TUI 进程:     manager.reacquire_pool_handle(name) + registry 重注册 mcp__ 前缀工具
```

---

## B. 扩展斜杠命令

### B.1 命令系统三层结构

一条 `/xxx` 命令生效需要三个接入点（外加可选的帮助文本）：

1. **处理函数**：`src/tui/app/commands.rs` 中 `handle_xxx_command(app, trimmed) -> bool`（返回 `true` 表示已消费）；
2. **dispatch 串接**：同文件的大 match/串链中加入 `|| handle_xxx_command(app, trimmed)`；
3. **命令注册**：`src/tui/app/state_ui_input_helpers.rs` 的 `RegisteredCommand::public("/xxx", "一句话描述")`——进补全建议与合法命令表；
4. （可选）**帮助文本**：`src/tui/app/input_help.rs` 增加 `"xxx" => "..."` 分支（`/help xxx` 显示）。

### B.2 完整案例：`/export`（纯本地同步命令模板）

`/export [path]` 把当前会话 Q&A 导出为 Markdown，共 4 处改动：

**① 处理函数**（`src/tui/app/commands.rs`）：

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

要点：入口先判断前缀不匹配则 `return false`（让 dispatch 链继续）；所有分支（含错误）必须 `return true` 防止落入"未知命令"；用户反馈用 `push_display_message`（气泡）+ `set_status_notice`（状态栏）。若你的产品有远程（瘦客户端）模式，导出内容应优先取屏幕可见消息，会话存储为兜底——两端数据源可能不同步。

**② dispatch 串接**（`commands.rs`）：

```rust
        || handle_export_command(app, trimmed)
        || handle_transcript_command(app, trimmed)
        // ...其余 handler
```

**③ 命令注册**（`state_ui_input_helpers.rs`）：

```rust
RegisteredCommand::public("/export", "Export Q&A pairs to a Markdown file"),
```

**④ 帮助文本**（`input_help.rs`）：`"export" => "`/export [path]`\nExport the current session's Q&A pairs..."`。

**⑤ 测试**（模式见 [F.3](#f3-常用测试模式速查)）：在 `src/tui/app/tests/` 下加用例：构造 `create_test_app()` → `app.input = "/export".into(); app.submit_input();` → 断言 `display_messages` 最后一条含成功/失败消息。

### B.3 进阶案例：`/download-latest`（命令触发后台任务 + 事件回推 UI）

当命令需要**异步长任务**（轮询远端、下载），采用"命令 → spawn 后台任务 → Bus 事件 → UI 渲染"四段式：

1. **命令入口**：匹配 `/download-latest`（可带 `/tui-download` 别名），从全局待更新状态读 payload，无更新则提示并退出；
2. **后台轮询**：TUI 启动时 `tokio::spawn`，**先 sleep 2s 等 App 完成 Bus 订阅**（broadcast 通道无回放，早发的事件会丢）再 `GET {后端}/check-update?current_version=x.y.z` 比对版本；
3. **事件回推**：`Bus::global().publish(BusEvent::UpdateStatus(UpdateStatus::Available { current, latest, payload }))`——事件枚举应覆盖 Checking/Available/Downloading/DownloadProgress/Downloaded/Error 全生命周期（定义于 `src/bus.rs`，Bus 机制见 `dev_ref_docs/11-bus-message-protocol.md`）；
4. **UI 渲染**：状态栏 banner 读全局 pending-update 静态（RwLock）——**刻意绕过 `&dyn TuiState` trait 派发**，因为 draw_status 是 60fps 热路径，每帧走 trait 虚表 + Option 包装开销不划算；
5. **下载执行**：`u` 快捷键或命令触发 → `reqwest` 流式下载（默认 headers 注入鉴权、**每 256KB publish 一次 DownloadProgress**、`watch::Receiver` 支持 Esc 取消、401 主动清理半成品文件并提示重新登录）。

完整源码级参考见 [H](#h-参考实现索引)（更新通道约 700 行）。

**可拆卸性警示**：这类后端推送通道与客户端耦合较深（启动钩子、Bus 事件、全局状态、UI banner、快捷键五处触点），一旦引入，后续想移除或替换的成本远高于当初接入。实现前建议先评估：是否可以退化为"客户端定期 `GET` 一个版本清单 + 手动 `/download`"的松耦合形态，把推送通道作为增强而非依赖。**决策指南**：

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

### C.2 完整案例：补全 `schedule` 任务生命周期

基线的 `schedule` 工具只能**创建**心跳轮询任务（查询/取消要手改队列文件）。补齐 create → list → cancel 闭环的完整做法（三件套均在基线中，`src/tool/mod.rs:187-198`）。以 `cancel_schedule` 为例：

```rust
// src/tool/ambient.rs
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

### C.4 编写要点

- **输入用强类型**：`#[derive(Deserialize)]` 一个私有 struct，`serde_json::from_value` 一次解出，字段类型错误让模型自己重试；
- **description 写"何时用"而非"是什么"**：模型靠它决策调用时机，写清前置条件（如 "by its id (returned by the schedule tool)"）；
- **错误信息给模型可行动的下一步**（"not found, call list_schedule first" 优于 "error"）；
- **无状态工具每次 execute 现场加载**：schedule 工具族不持有常驻 manager，每次 `AmbientManager::new()?` 从磁盘 load 队列——这样 list/cancel **立即对其他会话创建的任务生效**，保证跨会话一致；
- **工具 id 在创建时返回给模型**：`schedule` 返回 `sched_{8位hex}`，`cancel_schedule` 的 schema 描述里明写这个来源——模型能把两次调用串起来；
- **新工具必须带回归测试**（TDD：先写失败测试再实现）。

---

## D. 接入模型网关（Provider）

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

### D.2 内置 profile（批量分发时）

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

之后 `/login base-models` 列表、`/account dept-gateway login`、env 文件持久化全部自动生效。注意已知陷阱：切换 named profile 时要清理旧 profile 遗留的 env 变量（历史修复记录见 `dev_ref_docs/03-provider.md` 的"陷阱与历史修复"小节）。

---

## E. 品牌化你的 Fork

把 SSC-TUI 变成 `XX-TUI` 的定点修改清单：

| 触点 | 位置 | 说明 |
|---|---|---|
| 启动像素 logo | `src/tui/ui_header.rs:303` `startup_logo_text_lines` | `█` 块拼字母，full（5 行）/ compact（3 行）/ 窄终端 fallback 三档 |
| 会话 header 品牌行 | `src/tui/ui_header.rs` `animated_brand_header_line_for` | 无会话名时显示的品牌文本（当前 "SSC-TUI"） |
| 窗口标题 | `src/cli/tui_launch.rs:21` `SSC_WINDOW_TITLE` | 终端窗口标题常量（3 处使用点） |
| 存储根目录 | `crates/jcode-storage/src/lib.rs:76` `home.join(".ssc_tui")` | 用户配置/会话/日志根；**改这里即可全局生效**（其余代码都走 `jcode_dir()`） |
| 登录入口显示名 | `crates/jcode-provider-metadata/src/lib.rs` `JCODE_LOGIN_PROVIDER.display_name` | 若使用平台登录骨架 |
| 遥测文档链接 | `src/telemetry.rs` `TELEMETRY.md` 链接 | 指向你自己的仓库 |
| README / CLAUDE.md / dev_ref_docs 标题 | 各 md | `grep -ri "ssc" --include="*.md"` 扫尾 |

**可以不改**：`JCODE_*` 环境变量族、`jcode.env`、crate 名（`jcode-storage` 等）、`~/.ssc_tui/jcode.env` 文件名——这些是项目代号不是品牌，保留它们能让你持续低成本合并上游更新。

**登录门禁（可选）**：若需要"必须登录部门账号才能使用 TUI"，参考 [H](#h-参考实现索引) 中的登录定制实现；不需要门禁时，凭据可直接经 `mcp.json` headers / provider env 文件下发，无需任何代码。

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

`feat(scope):` / `fix(scope):` / `chore(scope):` / `test(scope):`，正文写动机 + 行为变化 + 验证方式。大型功能建议拆小步提交（每步可编译可验证），便于日后 cherry-pick 与回溯。

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

基线仓库会持续演进（bug 修复、新能力）。同步策略：

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

## H. 参考实现索引

本基线源自一个真实定制项目的剥离。该定制项目（含全部源码与 commit 历史）可作为各扩展面的**源码级参考**：

- 仓库：`https://github.com/LiangRichard13/SAITEC-TUI`，分支 `feat/saitec-mcp-http-transport`，与基线的共同祖先为 `340fb04c`；
- 下文表格中的行号均基于该仓库（与基线同源，大部分可直接对照）。

| 扩展面 | 参考内容 | 位置 / commit |
|---|---|---|
| A. HTTP MCP transport | `McpTransport` enum → `MessageTransport` trait → client 分发 → HTTP 配置引导，共约 450 行（小步提交序列，每步可编译） | `1897ef56` → `bd78112e` → `3b604f66` → `8fbaeddb` → `4dd93191` |
| A. 凭据三层重连 | 登录/登出同步 MCP 生命周期；池句柄释放缺陷的修复 | `81b77707`、`11247bf0`、`0f49c226` |
| A. 协议排错 | 协议错误 → 重连风暴 → Unknown tool 链的 4 个修复 | `e561ee2c` |
| A. MCP 服务端 | FastMCP 服务端（工具分域注册、API Key contextvar、SKILL 文档下发、令牌 TTL） | 内部仓库 SAITEC-Skills |
| B. `/export` | 会话 Q&A 导出（含远程模式可见消息优先的修复） | `22b70c82`、`f76f3e57` |
| B. 更新通道 | 后端轮询 → `BusEvent` → banner → 流式下载/取消，约 700 行 | `f147e9fd`、`7c700630`、`44be444a` |
| C. 任务生命周期 | schedule create → list → cancel 补全（TDD 全覆盖；该三件套已包含在基线中） | `340fb04c` |
| D. Provider 修复 | openai-compatible profile 切换的 env 清理与上下文窗口修复 | `e05304a1`、`dba79fc3`、`251a07f6`、`635154d8` |
| E. 品牌化 | 按模块分批替换品牌文案的提交序列 | `c4c3ae03` → `42358f97` → `3fe29808` → `03c46037` |
| 登录门禁 | 平台账号登录门禁 + 表单登录 + 凭据管理（约 1750 行，基线未包含，按需参考） | `src/saitec/auth.rs` |
