# SSC-TUI

SSC-TUI 是一个**多模型 AI 编程 Agent 的终端基线**：51-crate Rust workspace，把 LLM 对话、工具调用、MCP 服务接入、多 Provider 管理和终端 UI 集成在一个可二次开发的工程里。它开箱可用，同时也是各部门定制自己 AI 工作台的起点——定制指南见 [EXTENDING.md](EXTENDING.md)。

## 目录

- [这是什么](#这是什么)
- [快速开始](#快速开始)
- [登录与模型配置](#登录与模型配置)
- [MCP 服务配置](#mcp-服务配置)
- [常用命令](#常用命令)
- [本地目录与环境变量](#本地目录与环境变量)
- [开发与测试](#开发与测试)
- [深入文档](#深入文档)

## 这是什么

一句话：**在终端里与多个大模型对话、让模型调用工具干活、并接入你自己业务服务的 AI Agent 基线。**

- **多模型**：8 个内置 Provider（OpenAI / Claude / Gemini / Copilot / Bedrock / Azure …）+ 30 个 OpenAI-compatible profile，支持多账号切换与故障转移；
- **工具与 Agent**：内置 read/write/edit/bash/webfetch 等工具集，心跳任务（schedule）、子 agent、技能（skills）系统；
- **MCP 客户端**：完整 JSON-RPC 2.0 实现，支持 HTTP 远程与 stdio 本地两种 MCP 服务接入（[配置见下](#mcp-服务配置)）；- **终端 UI**：ratatui 全屏界面，会话管理、模型切换、diff 渲染、图表、内存估算等；
- **工程质量**：三平台 CI、单元 + e2e 测试、代码预算守卫。

一个用户请求的流转：CLI 解析 → Server 分发 → Agent turn 循环 → Provider 调 LLM → 事件总线回流 → TUI 渲染。

## 快速开始

### 环境要求

- Rust 工具链（stable）
- Windows 开发机额外需要 PowerShell（开发脚本为 PowerShell 编写）

### 从源码启动开发版本

```powershell
.\scripts\dev_ssc_tui.ps1
```

脚本会构建开发版本、复制运行时到 `dist\dev-ssc-tui` 并启动隔离的开发运行时。停止运行中的实例：

```powershell
.\scripts\dev_ssc_tui.ps1 -StopRunning -NoBuild
```

### 直接构建

```powershell
cargo build                  # debug
cargo build --release        # release
```

### Windows 打包分发

```powershell
cargo build --release
.\scripts\package_ssc_tui.ps1     # 产物在 dist\ssc-tui\（含 ssc-tui.exe + install.ps1）
```

本机资源不足时可用 `scripts/remote_build.sh` 远程构建。

## 登录与模型配置

首次启动后，用 `/login base-models` 打开基座模型选择器，支持 OAuth（OpenAI / Claude / Gemini / Copilot 等）和 API Key（各 OpenAI-compatible 服务商）两种方式。

**零代码接入自建网关**——`~/.ssc_tui/config.toml`：

```toml
[provider.dept_gateway]
api_base = "http://gateway.internal:8000/v1"
api_key_env = "DEPT_API_KEY"
default_model = "dept-model-v1"
```

多账号管理用 `/account`；内置 profile 或批量分发方案见 [EXTENDING.md D 节](EXTENDING.md#d-接入模型网关-provider)。

## MCP 服务配置

在 `~/.ssc_tui/mcp.json`（全局）或项目目录 `./.jcode/mcp.json`（项目级）中声明 MCP 服务：

```json
{
  "servers": {
    "my-service": {
      "type": "http",
      "url": "http://your-server:8000/mcp",
      "headers": { "X-API-Key": "sk-your-key" },
      "shared": true
    }
  }
}
```

stdio 本地服务把 `type` 换成缺省并使用 `command`/`args`/`env` 字段。修改配置后重启 TUI 生效，或让 Agent 执行 mcp 工具的 reload（如"重新加载 MCP 服务"）。连接成功后工具以 `mcp__my-service__<tool>` 前缀注册给 Agent。两种模式的选型、服务端实现骨架与凭据保护实践见 [EXTENDING.md A 节](EXTENDING.md#a-接入业务-mcp-服务)。

## 常用命令

| 命令 | 说明 |
|---|---|
| `/login base-models` | 打开基座模型登录/配置选择器 |
| `/account` | 账号中心：多账号登录/切换/登出、默认 provider 设置 |
| `/model`、`/models` | 打开模型选择器 |
| `/refresh-model-list` | 刷新模型目录和可用路由 |
| `/resume`、`/sessions` | 恢复历史会话 / 会话列表 |
| `/usage` | 查看当前 provider 用量 |
| `/export`、`/help`、`/quit` 等 | 见 TUI 内 `/help` |

## 本地目录与环境变量

```text
~/.ssc_tui/              # 用户数据根（可用 JCODE_HOME 覆盖）
├── config.toml          # 产品配置
├── mcp.json             # MCP 服务配置
├── jcode.env            # 订阅 API Key 环境变量桥接
├── logs/                # 运行日志
├── sessions/            # 会话记录
└── skills/              # 全局技能目录
```

| 环境变量 | 说明 |
|---|---|
| `JCODE_HOME` | 覆盖用户数据根目录（默认 `~/.ssc_tui`） |
| `JCODE_API_KEY` / `JCODE_API_BASE` | 订阅凭据（非交互/外部注入场景） |
| `JCODE_SUBSCRIPTION_ACTIVE` | 显式启用订阅模式 |
| `DEPT_API_KEY` 等 | 各 provider 的 API Key 按所选 profile 的 `api_key_env` 注入 |

`JCODE_*` 前缀与 `jcode` 是项目代号（crate 名、内部标识符），与产品品牌无关，定制时建议保留以降低上游合并成本——品牌化触点清单见 [EXTENDING.md E 节](EXTENDING.md#e-品牌化你的-fork)。

## 开发与测试

```powershell
cargo check                            # 快速检查
cargo test -p jcode --lib              # 单元测试
cargo test --test e2e <name> -- --exact # 单条 e2e（mock provider，无真实 API）
cargo fmt --all -- --check             # 格式检查
cargo clippy --all-targets --all-features -- -D warnings
```

CI（push 到 main 自动触发）包含三平台构建测试、格式/clippy 守卫、预算脚本（代码体积/panic 使用/吞错误）。各 job 的取舍建议见 [EXTENDING.md F 节](EXTENDING.md#f-测试与质量规范)。

## 深入文档

| 文档 | 内容 |
|---|---|
| [EXTENDING.md](EXTENDING.md) | **定制改造指南**：MCP 接入、命令/工具扩展、Provider、品牌化 |
| [CLAUDE.md](CLAUDE.md) | 构建命令速查 + 架构文档索引 |
| [dev_ref_docs/](dev_ref_docs/README.md) | 23 篇按子系统切分的架构参考（CLI / Agent / Provider / MCP / TUI / Storage …） |

---

SSC-TUI 基线由 jcode 内核定制剥离而来。各部门的定制实现（含源码级参考索引）见 EXTENDING.md 附录。
