# 22 · Browser Provider 协议设计

> 子系统：Browser Provider 协议规范、agent 与 browser 交互、provider 能力协商。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

定义 jcode 的 browser tool 与多种浏览器自动化后端（Firefox Agent Bridge、Chrome Agent Bridge、CDP、WebDriver/BiDi、Safari 等）之间的标准化协议——包括核心操作集、能力协商、会话模型、传输信封、错误模型和认证指南。

## 依赖关系

**内部文档**：
- [02-agent-runtime.md](02-agent-runtime.md) — Agent 如何通过 browser tool 调用 provider
- [03-provider.md](03-provider.md) — Provider 子系统中的 browser provider 实现
- [20-architecture-rfcs.md](20-architecture-rfcs.md) — 架构规划上下文

**源文档**：
- `docs/BROWSER_PROVIDER_PROTOCOL.md` — Browser Provider Protocol 完整设计

---

## 设计目标

1. **jcode 中一个一等公民 `browser` tool** — 模型使用单一 `browser` tool
2. **多 provider 实现** — Firefox、Chrome、Safari、Edge、WebDriver 等多种后端
3. **能力协商** — jcode 知道每个 provider 的能力和限制
4. **扩展而不碎片化** — 标准核心 + provider 特定命令
5. **稳定会话和元素引用** — 模型可快照页面，然后对返回的引用操作
6. **传输无关语义** — 语义协议在 in-process、stdio、socket 或 wrapper adapter 中保持一致

### 非目标

- 标准化每个低级浏览器原语
- 要求所有 provider 支持深层 DOM、网络或 JS 内省
- 要求 provider 附加到用户现有浏览器配置
- 将 provider 特定命令纳入必需核心

---

## 术语

| 术语 | 定义 |
|---|---|
| **browser tool** | 用户/模型面向的 jcode tool |
| **provider** | 满足此协议的后端实现 |
| **bridge** | 外部浏览器集成（如 Firefox Agent Bridge） |
| **adapter** | 将 bridge 原生 API 翻译为此协议的胶水代码 |
| **browser session** | jcode session 的 provider 隔离 session 或 attachment scope |
| **page** | session 下的 tab、target 或浏览表面 |
| **element ref** | provider 发放的不透明 handle，用于可操作元素 |

---

## 符合性模型

Provider 不须实现所有功能。

### 核心必需操作（Certification 认证用）

| 方法 | 描述 |
|---|---|
| `provider.describe` | 返回 provider 元数据 |
| `provider.status` | 返回当前可用性和设置状态 |
| `session.ensure` | 创建或复用浏览器 session |
| `session.close` | 关闭或分离 provider session |
| `page.open` | 打开 URL |
| `page.snapshot` | 返回当前页面的规范化视图（最重要方法） |
| `page.click` | 点击元素 |
| `page.type` | 输入文本 |
| `page.wait` | 等待条件 |
| `page.screenshot` | 截取屏幕截图 |

### 可选推荐

`page.go_back`、`page.go_forward`、`page.reload`、`tab.list`、`tab.activate`、`tab.close`、`page.eval`、`page.press`、`page.scroll`、`page.select`、`download.list`

### Provider 特定扩展

Provider 可暴露额外命令（如 `firefox.install_extension`、`chrome.attach_debug_target`、`cdp.send`、`webdriver.perform_actions`），但它们不属于必需核心。

---

## 传输模型

该协议定义消息语义，非单一 wire 格式。支持：
- 直接 Rust trait 调用
- stdio JSON request/response
- 本地 socket RPC
- 包装的远程 API

外部集成推荐 JSON-RPC 风格的信封格式。

### 请求信封

```json
{
  "protocol_version": "0.1",
  "id": "req_123",
  "method": "page.open",
  "params": {
    "session_id": "sess_abc",
    "url": "https://example.com"
  }
}
```

### 成功响应

```json
{
  "protocol_version": "0.1",
  "id": "req_123",
  "ok": true,
  "result": { "page_id": "page_1", "url": "...", "title": "..." },
  "warnings": []
}
```

### 错误响应

```json
{
  "protocol_version": "0.1",
  "id": "req_123",
  "ok": false,
  "error": {
    "code": "unsupported_method",
    "message": "...",
    "retryable": false,
    "details": {}
  }
}
```

### 事件信封

```json
{
  "protocol_version": "0.1",
  "event": "page.navigated",
  "payload": { "session_id": "sess_abc", "page_id": "page_1", "url": "..." }
}
```

---

## 关键操作详情

### `provider.describe` — 能力协商

返回 provider 元数据，包含：
- `provider_id`、`provider_label`、`provider_version`
- `protocol_version`、`browser_families`、`transport`
- `certification_tier`（candidate / certified / compatible / experimental）
- `capabilities.core_methods`、`capabilities.optional_methods`、`capabilities.features`
- `capabilities.custom_methods`（每个有 name/description/stability/input_schema/output_schema）

### `provider.status` — 可用性诊断

返回：
- `availability`: `ready | degraded | unavailable`
- `browser_detected`、`browser_running`、`setup_state`: `complete | partial | required | broken`
- `requires_manual_setup`、`recommended_browser`
- `diagnostics` 数组（每条有 level/code/message/manual_steps）

### `page.snapshot` — 最关键的模型方法

返回规范化页面视图。Provider 可用不同内部表示，但应归一化为通用最小格式：

```json
{
  "snapshot": {
    "format": "jcode.page_snapshot.v1",
    "root": { "node_id": "n1", "role": "document", "name": "...", "children": ["n2", "n3"] },
    "nodes": [
      { "node_id": "n2", "role": "heading", "name": "...", "text": "...", "element_ref": "el_1", "actionable": false },
      { "node_id": "n3", "role": "link", "name": "...", "text": "...", "element_ref": "el_2", "actionable": true }
    ]
  }
}
```

还包含扁平化的 `elements` 列表（带 element_ref/role/name/text/actionable/enabled/selector_hint）供模型便捷使用。

### `page.click` — 多点支持

请求支持多种定位模式：`element_ref`、`selector`、`text_query`、`position`。至少须提供一种。

### `page.screenshot` — 截图

可返回内联 base64 或 provider 管理的图片引用（根据传输约束）。

---

## 会话模型

jcode 不关心 provider 内部使用 tab/context/profile/remote target，只需可复用的稳定 handle。

**`session.ensure`** 参数：
- `client_session_id`、`browser_preference`: `auto`
- `isolation`: `per_jcode_session`、`attach`: `prefer`、`persist`: `true`
- `metadata.owner`: `agent`

**`session.close`** 可选择关闭 tab、分离 target、或仅释放 provider 侧状态。

---

## 能力 schema

### Methods（可调用操作）
`page.open`、`page.snapshot`、`tab.list` 等。

### Features（影响 jcode 行为的语义/质量）
- `element_refs`、`a11y_snapshot`、`dom_snapshot`、`html_snapshot`
- `full_page_screenshot`、`attach_existing_browser`
- `persistent_profile`、`isolated_contexts`
- `js_eval`、`network_observe`、`console_observe`
- `file_upload`、`download_observe`
- `manual_setup_required`、`extension_required`、`remote_debugging_required`

### Stability 标签
`stable` / `experimental` / `deprecated`

---

## 设置与诊断

Browser provider 通常需要手动设置。协议使设置信息机器可读。

```json
{
  "level": "warning",
  "code": "extension_missing",
  "message": "Firefox extension is not installed",
  "manual_steps": ["Open Firefox", "Install the extension from /path/to/bridge.xpi"]
}
```

推荐方法：`provider.status`、`provider.setup_guide`（可选）、`provider.verify`（可选）。

---

## 错误模型

标准错误码：
`unsupported_method`、`unsupported_target`、`invalid_request`、`invalid_selector`、`element_not_found`、`element_not_actionable`、`navigation_timeout`、`not_ready`、`setup_required`、`permission_denied`、`browser_not_running`、`session_not_found`、`page_not_found`、`internal_error`。

---

## 扩展性模型 4 规则

1. **Provider 可暴露自定义方法**，但应使用命名空间（如 `firefox.install_extension`、`cdp.send`）
2. **Provider 必须广告自定义方法**（在 `provider.describe.capabilities.custom_methods`）
3. **jcode core 默认只依赖规范化方法**；provider 特定方法仅在用户显式要求、adapter 识别、或高级/调试模式下使用
4. **Provider-native passthrough 允许但应显式标记**为高级/调试行为

---

## 版本控制

- `protocol_version` 标识语义协议版本
- 小型增量更改不应破坏现有认证 provider
- 破坏性变更要求新协议版本
- 当前版本：`"0.1"`

---

## 认证分级

| 等级 | 要求 |
|---|---|
| **Certified** | 通过核心必需方法的一致性测试；稳定标识符和规范化结果；正确报告设置/诊断；可预测行为 |
| **Compatible** | 支持部分或大多数规范化方法；可能有缺少的功能或部分行为 |
| **Experimental** | adapter 存在但语义不完整或不稳定 |

---

## 最小一致性测试场景

1. `provider.describe` 成功
2. `provider.status` 报告一致状态
3. `session.ensure` 创建或复用 session
4. `page.open` 导航到测试页面
5. `page.snapshot` 返回可用文本和至少一个可操作引用
6. `page.click` 激活已知元素
7. `page.type` 填充已知输入
8. `page.wait` 观察到确定性页面变化
9. `page.screenshot` 返回图片
10. `session.close` 清理干净

---

## 推荐集成策略

jcode `browser` tool 应：
1. 优先使用规范化核心方法
2. 基于用户偏好、可用性和能力质量选择 provider
3. 仅在显式高级路径后暴露 provider 特定方法
4. 无可用 provider 时返回设置指南
5. 避免在核心 tool API 中硬编码 Firefox/Chrome 特定假设

---

## 开放问题

1. Screenshots 应始终内联还是可返回文件/图片 handle？
2. 事件流是否应对高级集成必需？
3. 原始 HTML/DOM 中有多少应规范化 vs. 作为 provider data 返回？
4. `page.snapshot` 应支持 `jcode.page_snapshot.v1` 之外的命名格式吗？
5. Provider 特定方法应在同一 `browser` tool 中调用还是仅通过调试模式？

---

## 回指

- Agent 如何通过 browser tool 与 provider 交互：[02-agent-runtime.md](02-agent-runtime.md)
- Provider 子系统中 browser provider 的运行时实现：[03-provider.md](03-provider.md)
- 模块化架构上下文：[20-architecture-rfcs.md](20-architecture-rfcs.md)
