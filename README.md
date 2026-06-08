# SAITEC-TUI

SAITEC-TUI 是一个 AI 原生的终端集成工具，面向大模型评测与治理、AIGC 内容检测、任务编排和后续数据处理/数据评估场景。它把 SAITEC 平台账号、基座模型账号、评测工具、检测工具、文件产物和任务状态统一放进一个 TUI 工作台里，让用户可以用自然语言完成从登录、配置模型到执行检测/评测的完整流程。

本项目不再是通用代码代理工具的 README。SAITEC-TUI 的产品定位是:

- 为 SAITEC 平台用户提供统一的 AI 能力入口。
- 让用户先登录 SAITEC 并获得平台 API Key，再登录或配置可用的基座模型。
- 内置 AIGC 文本、图片、视频检测能力。
- 内置大模型通用能力评测、安全评测和语料安全评测能力。
- 逐步承载数据处理、数据质检、数据评估、评测报告和治理工作流。

## 目录

- [产品定位](#产品定位)
- [核心能力](#核心能力)
- [使用流程](#使用流程)
- [安装与启动](#安装与启动)
- [登录与账号](#登录与账号)
- [内置评测与检测能力](#内置评测与检测能力)
- [数据与产物](#数据与产物)
- [常用命令](#常用命令)
- [本地配置与安全](#本地配置与安全)
- [开发与打包](#开发与打包)
- [路线图](#路线图)
- [故障排查](#故障排查)

## 产品定位

SAITEC-TUI 是 SAITEC 平台的本地交互入口。用户不需要分别记忆多个 API、脚本和评测工具，只需要在终端中启动 TUI，完成两类登录，然后通过自然语言或内置命令使用平台能力。

第一类登录是 SAITEC 登录。用户使用 SAITEC 账号登录后，系统会向 SAITEC Core 申请并保存一个业务 API Key。这个 API Key 用于访问平台侧的检测、评测、文件和任务接口。

第二类登录是基座模型登录。模型评测、Agent 推理和部分治理流程需要调用大模型。SAITEC-TUI 当前面向产品场景收敛到一组受支持的基座模型供应商，避免用户在过宽的供应商列表里迷路。

完成这两类登录后，用户可以在同一个 TUI 内执行:

- 检测一段文本是否可能由 AI 生成。
- 检测图片是否为 AI 生成、篡改或 DeepFake。
- 检测视频是否为 AI 生成或 DeepFake。
- 对大模型做通用能力评测。
- 对大模型做安全评测和攻击测试。
- 对文本语料做安全性评测。
- 上传数据集、图片、视频，查询任务状态，下载检测或评测产物。

## 核心能力

| 能力域 | 当前支持 | 说明 |
|---|---|---|
| SAITEC 平台登录 | 支持 | 登录 SAITEC 后自动创建并保存业务 API Key |
| 基座模型登录 | 支持 | 支持 OpenAI、Claude、Z.AI、Kimi、Alibaba Cloud Coding 等产品允许的模型入口 |
| AIGC 文本检测 | 支持 | 单条文本和批量文本检测 |
| AIGC 图片检测 | 支持 | AI 生成图片检测、篡改检测、可视化产物 |
| AIGC 视频检测 | 支持 | 视频 AIGC/DeepFake 检测，适合异步长任务 |
| 大模型安全评测 | 支持 | 评估模型在风险提示、攻击样本下的安全表现 |
| 语料安全评测 | 支持 | 不调用被测模型，直接评估语料文本是否有风险 |
| 通用能力评测 | 支持 | 使用 prompts 或 dataset 对模型能力做结构化评测 |
| 文件与产物管理 | 支持 | 上传图片、视频、dataset，下载报告和任务产物 |
| 数据处理与数据评估 | 规划中 | 未来纳入数据清洗、转换、质检、数据集评估和治理报告 |

## 使用流程

典型用户路径如下:

1. 启动 SAITEC-TUI。
2. 选择 SAITEC 登录，使用邮箱或手机号加密码完成平台登录。
3. 登录成功后，SAITEC-TUI 自动向 SAITEC Core 创建业务 API Key，并写入本地安全存储。
4. 选择基座模型登录，配置 OpenAI、Claude、Z.AI、Kimi 或 Alibaba Cloud Coding 等模型供应商。
5. 使用 `/model` 选择实际运行模型。
6. 用自然语言发起任务，例如检测文本、上传图片检测、评测模型安全性、用 JSONL 数据集跑通用评测。
7. 对异步任务使用任务查询能力查看状态，必要时下载报告、结果 JSON、可视化文件或其他产物。

一个最小上手流程:

```text
启动 SAITEC-TUI
-> /login
-> 选择 SAITEC
-> 填写邮箱或手机号以及密码
-> /login base-models
-> 选择并登录一个基座模型
-> /model
-> 选择可用模型
-> 输入你的检测或评测需求
```

## 安装与启动

### Windows 安装包

如果你已经拿到 `dist/saitec-tui` 目录，可以在 PowerShell 中执行:

```powershell
.\dist\saitec-tui\install.ps1
```

安装脚本会把 `saitec-tui.exe` 安装到用户目录下，并把启动目录加入用户 `PATH`。安装完成后打开新的终端窗口:

```powershell
saitec-tui
```

### 从源码启动开发版本

开发机需要准备:

- Rust 工具链。
- PowerShell。
- Python 3，用于本地 SAITEC-Skills MCP 运行环境。
- 能访问 SAITEC Core API 的网络环境。

在仓库根目录运行:

```powershell
.\scripts\dev_saitec_tui.ps1
```

这个脚本会:

- 准备 SAITEC-Skills 所需 Python 环境。
- 构建开发版本。
- 将运行时复制到 `dist\dev-saitec-tui`。
- 启动一个隔离的开发运行时。

如只想停止当前记录的开发运行时:

```powershell
.\scripts\dev_saitec_tui.ps1 -StopRunning -NoBuild
```

### 从源码打包

先构建 release 产物:

```powershell
cargo build --release
```

然后生成 SAITEC-TUI 分发目录:

```powershell
.\scripts\package_saitec.ps1
```

打包结果位于:

```text
dist\saitec-tui\
```

其中包含:

- `saitec-tui.exe`: 产品可执行文件。
- `install.ps1`: 自包含安装脚本。
- `SAITEC_logo.png`: 产品 Logo 资源，如果仓库中存在该文件。

如果本机资源不足导致构建被终止，可以使用仓库内的远程构建脚本:

```bash
scripts/remote_build.sh
```

## 登录与账号

SAITEC-TUI 的登录分为两层。

### 1. SAITEC 平台登录

SAITEC 登录用于访问平台侧的检测、评测、任务和文件能力。用户可以在 TUI 内执行:

```text
/login
```

然后选择 SAITEC 登录。登录表单支持:

- 邮箱 + 密码。
- 手机号 + 密码。

邮箱和手机号至少填写一个。登录成功后，SAITEC-TUI 会:

- 调用 SAITEC Core 登录接口获取用户身份令牌。
- 使用身份令牌创建一个业务 API Key。
- 将业务 API Key 和必要的用户元数据保存到本地。
- 将 API Key 注入 SAITEC-Skills 运行环境，供内置能力调用。

默认 SAITEC Core 地址可通过环境变量覆盖:

```powershell
$env:SAITEC_AUTH_BASE = "https://your-saitec-auth.example.com"
$env:CORE_API_BASE = "https://your-saitec-core.example.com"
```

也可以直接提供平台 API Key:

```powershell
$env:SAITEC_API_KEY = "your-saitec-api-key"
```

### 2. 基座模型登录

基座模型用于承载推理、评测、Judge 和 Agent 工作流。用户可以在 TUI 内执行:

```text
/login base-models
```

当前产品模式支持的基座模型入口:

| Provider | 用途 |
|---|---|
| OpenAI | 通用推理、评测、Judge、Agent 任务 |
| Claude | 通用推理、长上下文分析、评测辅助 |
| Z.AI | 国产模型入口与兼容模型调用 |
| Kimi | Kimi Code 等模型入口 |
| Alibaba Cloud Coding | 阿里云 Coding 计划模型入口 |

登录或配置完成后，使用:

```text
/model
```

选择具体模型。模型列表会结合已登录供应商、已验证模型和缓存模型目录展示。

## 内置评测与检测能力

SAITEC-TUI 内置的 SAITEC-Skills MCP 服务负责把用户意图转换为平台任务。用户通常不需要直接记忆工具名，只需要描述任务目标。Agent 会根据任务类型选择正确的检测或评测流程。

### AIGC 文本检测

适用于:

- 判断一段文章、评论、摘要是否疑似 AI 生成。
- 批量检测多段文本的 AIGC 概率。
- 生成结构化检测结果并关联任务记录。

示例:

```text
帮我检测这段文字是否 AI 生成，并给出置信度。
```

```text
批量检测下面 20 条评论是否是 AIGC 内容。
```

### AIGC 图片检测

适用于:

- 检测图片是否 AI 生成。
- 检测图片是否存在篡改、合成或修图痕迹。
- 生成检测框、可视化结果和报告产物。

本地图片需要先上传到平台文件服务，获得可供任务使用的 `storage_uri` 后再执行检测。SAITEC-TUI 会在工作流里处理这个步骤。

示例:

```text
帮我检测 C:\data\images\sample.jpg 是否为 AI 生成图片。
```

```text
检测这张图有没有被篡改，并下载检测报告。
```

### AIGC 视频检测

适用于:

- 检测视频是否由 AI 生成。
- 检测 DeepFake 或合成视频风险。
- 对视频文件执行异步分析并查询任务结果。

视频检测通常耗时更长，用户可以先提交任务，再查询状态和下载产物。

示例:

```text
帮我检测这个视频是不是 DeepFake: D:\videos\demo.mp4。
```

### 大模型安全评测

适用于:

- 评估模型是否会在攻击提示下输出有害内容。
- 测试模型安全边界和拒答能力。
- 使用被测模型与 Judge 模型形成自动评测闭环。

当评测需要调用第三方模型时，需要先配置对应模型的 API Key。SAITEC-TUI 会根据用户选择的模型和 Judge 模型提示所需凭证。

示例:

```text
用这些攻击样本评测 deepseek-chat 的安全性，并生成报告。
```

### 语料安全评测

适用于:

- 直接评估已有文本语料是否包含有害、违法、歧视、暴力等风险。
- 批量扫描 JSON 或 JSONL 数据集。
- 在不调用被测模型的情况下做内容治理。

示例:

```text
帮我评测 data/corpus.jsonl 里的文本安全性。
```

### 通用 LLM 能力评测

适用于:

- 使用 prompts 直接评估模型回答能力。
- 使用 dataset 对模型做批量评测。
- 按字段、任务类别或关键词汇总结果。
- 生成 Markdown 报告和结构化结果。

示例:

```text
用这个 cases.jsonl 评测 Kimi 的通用问答能力，并按 field 汇总得分。
```

## 数据与产物

SAITEC-TUI 的检测与评测任务通常会产生结构化结果和文件产物。

### 支持的输入

| 输入类型 | 说明 |
|---|---|
| 纯文本 | 可直接用于文本 AIGC 检测、语料安全评测、prompt 评测 |
| 图片文件 | 需要上传，支持图片 AIGC 检测和篡改检测 |
| 视频文件 | 需要上传，支持视频 AIGC/DeepFake 检测 |
| JSON/JSONL 数据集 | 需要上传，支持通用能力评测、模型安全评测和语料安全评测 |

### 产物类型

常见产物包括:

- 结构化结果 JSON。
- Markdown 评测报告。
- 图片或视频检测可视化文件。
- 任务 trace 和运行元数据。
- 数据集解析结果和样本级评分。

涉及本地文件时，推荐工作流是:

```text
确认本地文件路径
-> 上传文件
-> 获得 storage_uri
-> 创建检测或评测任务
-> 查询任务状态
-> 下载报告或结果产物
```

## 常用命令

| 命令 | 说明 |
|---|---|
| `/login` | 打开登录选择器，可选择 SAITEC 登录或基座模型配置 |
| `/login base-models` | 打开基座模型登录/配置选择器 |
| `/logout` | 清除 SAITEC 登录状态并回到登录流程 |
| `/model` | 打开模型选择器 |
| `/models` | `/model` 的别名 |
| `/refresh-model-list` | 刷新模型目录和可用路由 |
| `/clear` | 清空当前对话显示 |
| `/resume` | 恢复历史会话 |
| `/sessions` | 打开会话列表 |
| `/usage` | 查看用量信息 |
| `/version` | 查看版本信息 |
| `/help` | 查看帮助 |
| `/quit` | 退出 SAITEC-TUI |

## 本地配置与安全

### 本地目录

SAITEC-TUI 默认使用用户主目录下的专用目录:

```text
~/.saitec_tui/
```

常见文件:

| 路径 | 说明 |
|---|---|
| `~/.saitec_tui/auth.json` | SAITEC 平台登录信息、业务 API Key 和用户元数据 |
| `~/.saitec_tui/saitec.env` | SAITEC API Key 的环境变量桥接文件 |
| `~/.saitec_tui/mcp.json` | SAITEC-Skills MCP 服务器配置 |
| `~/.saitec_tui/config.toml` | 产品配置 |
| `~/.saitec_tui/logs/` | 运行日志 |
| `~/.saitec_tui/sessions/` | 会话记录 |

### 环境变量

| 变量 | 说明 |
|---|---|
| `SAITEC_API_KEY` | 平台 API Key，可用于非交互或外部注入 |
| `SAITEC_API_BASE` | SAITEC API 地址兼容变量 |
| `SAITEC_AUTH_BASE` | SAITEC 登录页或登录服务地址 |
| `CORE_API_BASE` | SAITEC Core API 地址，内置能力会访问该地址 |
| `SAITEC_TUI_PYTHON` | 指定 SAITEC-Skills MCP 使用的 Python 可执行文件 |
| `SAITEC_SKILLS_ROOT` | 指定 SAITEC-Skills 资源目录 |
| `SAITEC_TUI_HOME` | 运行时注入给 SAITEC-Skills 的本地产品目录 |

### 安全策略

- SAITEC API Key 只写入本地安全存储和运行时环境，不应出现在对话正文里。
- 本地凭证文件会尽量使用仅当前用户可读写的权限。
- SAITEC-Skills 请求通过 SAITEC Core 代理 API，携带平台 API Key 进行鉴权。
- 检测与评测任务由平台侧负责用户归属、任务归属和数据库事务管理。
- 对于图片、视频和 dataset，本地文件会先上传到平台文件服务，再通过平台侧 URI 创建任务。

## 开发与打包

### 快速检查

文档或小改动后可先运行:

```powershell
cargo check
```

涉及发布、运行时或产品行为变更时，完成后应构建源码:

```powershell
cargo build
```

如果构建耗时过长或被系统终止，优先检查本机资源，必要时使用:

```bash
scripts/remote_build.sh
```

### 开发运行

```powershell
.\scripts\dev_saitec_tui.ps1
```

常用参数:

```powershell
.\scripts\dev_saitec_tui.ps1 -Profile selfdev
.\scripts\dev_saitec_tui.ps1 -Profile release
.\scripts\dev_saitec_tui.ps1 -StopRunning -NoBuild
```

### Windows 打包

```powershell
cargo build --release
.\scripts\package_saitec.ps1
```

生成目录:

```text
dist\saitec-tui\
```

安装:

```powershell
.\dist\saitec-tui\install.ps1
```

## 路线图

SAITEC-TUI 的下一阶段重点会围绕数据处理与数据评估展开，计划包括:

- 数据集上传后的结构化预览、字段识别和格式校验。
- JSON/JSONL/CSV 等常见数据格式的清洗、抽样、切分和转换。
- 数据质量评估，包括重复率、缺失值、异常样本、标签一致性和分布偏移。
- 数据安全与合规扫描，包括敏感信息、风险内容和版权风险提示。
- 数据集版本管理和评测结果对比。
- 模型评测、语料评测、AIGC 检测结果的统一报告面板。
- 面向治理场景的审计记录、任务追踪和可导出报告。

## 故障排查

### 启动后要求登录

这是预期行为。SAITEC-TUI 在没有有效 SAITEC 登录状态时会阻止普通任务执行。运行:

```text
/login
```

完成 SAITEC 登录后再继续。

### SAITEC 登录失败

检查:

- 邮箱或手机号至少填写一个。
- 密码不为空。
- `CORE_API_BASE` 或 `SAITEC_AUTH_BASE` 是否指向正确服务。
- 网络是否能访问 SAITEC Core。
- 账号是否有创建 API Key 的权限。

### 基座模型不可用

运行:

```text
/login base-models
/refresh-model-list
/model
```

确认供应商已登录、API Key 有效、模型目录已刷新，并选择一个可用模型。

### SAITEC-Skills 未加载

检查:

- 仓库或安装包中是否包含 `SAITEC-Skills` 资源。
- `~/.saitec_tui/mcp.json` 是否存在并包含 `SAITEC-Skills`。
- Python 是否可用。
- `SAITEC_TUI_PYTHON` 是否指向正确 Python。
- `SAITEC_SKILLS_ROOT` 是否指向正确资源目录。

开发环境可重新运行:

```powershell
.\scripts\dev_saitec_tui.ps1
```

### 文件检测任务失败

检查:

- 文件路径是否为绝对路径。
- 文件是否存在且当前用户可读。
- 图片、视频或 dataset 是否先完成上传。
- dataset 是否为任务支持的 JSON 或 JSONL 格式。
- 任务参数中是否使用上传后返回的 `storage_uri`。

### 构建失败或被终止

先检查 CPU、内存、磁盘空间和杀毒软件占用。若本机资源不足，使用远程构建:

```bash
scripts/remote_build.sh
```

## 贡献说明

提交代码或文档时，请保持变更聚焦。完成一项功能或修复后及时提交，任务结束时推送到远端。涉及源码行为变化时，优先使用 `cargo check`、目标测试和开发构建快速迭代，收尾时执行完整构建。
