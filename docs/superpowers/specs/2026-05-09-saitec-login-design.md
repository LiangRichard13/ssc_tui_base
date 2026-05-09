# Saitec Login Flow Design

## Goal

基于现有 JCode TUI，增加一个 Saitec 专用登录门禁，使程序在未登录或令牌失效时不能正常使用；登录状态与模型 API 配置统一存放在用户目录下的 `~/.saitec_tui/` 中；支持 TUI 内的 `/logout`；同时保留后续把 mock 登录接口替换成真实接口的扩展点。

## Scope

本次设计覆盖以下内容：

- 启动时登录门禁
- Saitec 专用本地状态目录与文件格式
- 模拟浏览器登录与本地回调接收
- 模拟 token 校验
- `/logout` 命令
- 基座模型 API 配置读取
- Windows 本地打包与安装产物命名

本次设计不覆盖以下内容：

- 真实生产认证接口对接
- 多账号切换
- MCP 集成改造
- UI 全量重装

## Recommendation

推荐方案为“复用现有 JCode 认证/TUI 骨架，新增 Saitec 专用登录层”，而不是绕开现有体系重写一套认证系统。

原因如下：

- 现有仓库已经具备浏览器打开、OAuth 回调监听、TUI 登录状态提示、命令注册、认证状态缓存与测试结构。
- 我们只需要把这些能力收束成 Saitec 单一入口，而不需要保留原有多 provider 的完整交互复杂度。
- 后续接真实后端时，只需替换登录 URL、token 提取规则和校验请求，而不需要推翻交互骨架。

## User Experience

### Startup behavior

程序启动后立即检查 `~/.saitec_tui/auth.json`：

- 文件不存在：视为未登录
- 文件存在但缺少 `auth_token`：视为未登录
- 文件存在且有 `auth_token`：执行 mock 校验
- mock 校验失败：视为已失效，需要重新登录

当处于未登录或已失效状态时：

- 程序不进入正常会话工作流
- TUI 显示登录说明、当前状态、登录 URL、回调等待提示
- 程序尝试自动打开浏览器
- 同时启动本地 loopback 回调监听

回调成功并校验通过后：

- 将 token 写入 `~/.saitec_tui/auth.json`
- 刷新内存中的认证状态
- 进入正常 TUI 工作流

### Logout behavior

用户在 TUI 输入 `/logout` 后：

- 清除内存中的 Saitec 登录状态
- 删除或重写 `~/.saitec_tui/auth.json`
- 清空与登录状态相关的缓存
- 在当前界面显示“已登出”
- 重新进入登录门禁状态，而不是直接退出程序

## Storage Design

### Base directory

统一使用用户目录下的 `~/.saitec_tui/` 作为 Saitec 版程序的本地根目录。

在 Windows 上对应：

- `C:\Users\<user>\.saitec_tui\`

在默认运行模式下，不再把核心认证与配置状态写到 `~/.jcode/`。

### Files

核心文件：

- `~/.saitec_tui/auth.json`
- `~/.saitec_tui/config.toml`

可继续沿用现有 JCode 其他会话型目录结构时，统一挂在 `~/.saitec_tui/` 下，例如：

- `~/.saitec_tui/logs/`
- `~/.saitec_tui/sessions/`
- `~/.saitec_tui/builds/`

### auth.json format

初版使用最小结构：

```json
{
  "auth_token": "mock-token-value",
  "token_type": "Bearer",
  "issued_at": "2026-05-09T14:00:00Z",
  "expires_at": null,
  "user_id": "mock-user",
  "last_validated_at": "2026-05-09T14:00:05Z"
}
```

说明：

- `auth_token` 是唯一必需字段
- `token_type` 默认写入 `Bearer`
- `expires_at` 在 mock 阶段允许为空
- `last_validated_at` 用于 UI 诊断与后续优化
- 字段命名要尽量中性，便于后续接真实接口

### config.toml format

初版配置至少支持 Saitec 基座模型 API：

```toml
[provider]
default_provider = "openai-compatible"
default_model = "saitec-chat"

[providers.saitec]
type = "openai-compatible"
base_url = "https://api.saitec.local/v1"
api_key_env = "SAITEC_API_KEY"
```

初版不强制定义完整字段集合；只要与现有 `Config` 结构兼容即可。关键点是配置文件路径改为 `~/.saitec_tui/config.toml`。

## Login Flow

### Mock authorize URL

初版使用模拟登录地址，例如：

`https://auth.saitec.local/login?redirect_uri=http://127.0.0.1:1455/auth/callback`

设计要求：

- URL 生成逻辑独立封装
- 回调端口可复用现有本地监听能力
- 后续能替换为真实域名与参数

### Local callback

程序本地监听：

- 地址：`127.0.0.1`
- 路径：`/auth/callback`

回调请求示例：

`http://127.0.0.1:1455/auth/callback?auth_token=mock-token-value`

回调处理逻辑：

1. 校验请求路径正确
2. 从 query 中提取 `auth_token`
3. 若缺少 token，返回失败页面并继续等待
4. 将 token 暂存到内存
5. 立即执行 mock 校验
6. 校验通过后写入 `auth.json`
7. 返回“登录成功，可关闭此页面”的 HTML

### Mock validation request

初版使用模拟校验接口，例如：

- Method: `GET`
- URL: `https://auth.saitec.local/api/v1/session`
- Header: `Authorization: Bearer <auth_token>`
- Header: `X-Saitec-Client: saitec-tui`

Mock 阶段的实现策略：

- 默认通过本地模拟逻辑返回“有效”
- 保留一个明确的函数边界，后续可以直接替换成真实 `reqwest` 请求
- 测试中允许通过 token 内容触发失败分支，例如 token 以 `invalid-` 开头则视为失效

### Validation result contract

初版内部统一使用如下校验结果：

```rust
struct SaitecValidationResult {
    is_valid: bool,
    user_id: Option<String>,
    expires_at: Option<String>,
    message: Option<String>,
}
```

这样后面替换真实接口时，只需要做响应到该结构的转换。

## TUI Integration

### Command strategy

保留现有 `/login` 命令，但将它收束为 Saitec 登录入口。

新增 `/logout` 命令。

初版推荐交互：

- `/login`：显示登录状态并重新触发浏览器登录
- `/logout`：清除本地认证状态并回到登录门禁
- `/auth`：显示 Saitec 当前登录状态，而不是多 provider 总览

### Gate behavior

登录门禁不应只是建议提示，而应真正阻止正常使用。

初版行为：

- 未登录时，不允许发送正常业务 prompt
- 未登录时，输入区仍允许使用 `/login`、`/logout`、`/help`、`/quit`
- 其他命令统一提示“请先登录”

### UI messaging

未登录界面至少包含：

- 当前状态：未登录 / token 已失效
- 登录链接
- 浏览器自动打开结果
- 回调等待状态
- 失败时的重试提示

成功后至少包含：

- 登录成功
- 当前用户标识（若有）
- 最近校验时间

## Architecture

### New modules

建议新增以下模块：

- `src\saitec\mod.rs`
- `src\saitec\paths.rs`
- `src\saitec\auth.rs`
- `src\saitec\config.rs`
- `src\saitec\mock_server.rs` 或将 mock 校验逻辑放入 `auth.rs`

职责划分：

- `paths.rs`：统一管理 `~/.saitec_tui` 路径
- `auth.rs`：读写 `auth.json`、回调处理、登录门禁、logout、mock 校验
- `config.rs`：对接 `config.toml` 位置与加载
- `mod.rs`：公共导出

### Existing files likely to change

- `crates\jcode-storage\src\lib.rs`
- `src\config\config_file.rs`
- `src\cli\startup.rs`
- `src\tui\app\auth.rs`
- `src\tui\app\auth_account_commands.rs`
- `src\tui\app\state_ui_input_helpers.rs`
- `src\provider_catalog.rs`
- `src\lib.rs`
- `src\main.rs` 或启动分发链中的相邻入口
- `scripts\install.ps1`

### Path strategy

优先采用“存储根目录可切换”的方式，而不是在业务层到处硬编码 `.saitec_tui`。

推荐做法：

- 在 `jcode-storage` 中增加统一的产品 Home 解析逻辑
- 允许通过环境变量或 Saitec 产品模式返回 `~/.saitec_tui`
- 现有 `jcode_dir()` 的使用点尽量不大改，让底层路径解析完成产品切换

这样可以减少对会话、日志、构建、缓存等大量调用点的侵入。

## Error Handling

需要覆盖以下失败场景：

- 本地目录创建失败
- `auth.json` 损坏或不可解析
- 浏览器无法自动打开
- 监听端口被占用
- 回调缺少 `auth_token`
- mock 校验失败
- `config.toml` 不存在或格式错误

期望行为：

- TUI 中给出明确诊断信息
- 不因单次登录失败导致程序崩溃
- 允许用户再次执行 `/login`
- 对损坏的 `auth.json` 采用“忽略旧值并要求重新登录”的策略

## Security Notes

即使当前是 mock 阶段，也保持最基本的安全约束：

- `auth.json` 使用现有 secret file 权限收紧逻辑写入
- 只监听 `127.0.0.1`
- 只接受明确的 callback path
- 后续若加入 state 参数，当前接口边界不应阻碍扩展

初版可以不强制实现完整 OAuth state 校验，但代码结构要预留位置。

## Testing Strategy

### Unit tests

至少补以下测试：

- 无 `auth.json` 时判定未登录
- `auth.json` 缺少 `auth_token` 时判定未登录
- token 读取成功时触发 mock 校验
- `invalid-*` token 触发失效分支
- `/logout` 删除登录态
- `config.toml` 路径切到 `~/.saitec_tui/config.toml`
- 未登录时命令门禁生效

### TUI behavior tests

优先沿用现有 `src\tui\app\tests\...` 风格，补以下行为测试：

- 启动时未登录给出 `/login` 提示
- `/login` 进入 Saitec 登录流程而非旧 provider picker
- `/logout` 后重新回到未登录状态

### Manual verification

至少手工验证以下流程：

1. 删除 `~/.saitec_tui/auth.json`
2. 启动程序
3. 观察登录门禁界面
4. 触发 `/login`
5. 模拟回调写入 token
6. 进入正常会话
7. 执行 `/logout`
8. 再次回到登录门禁

## Packaging Plan

初版打包目标以 Windows 本地产物为主。

建议方式：

- 先执行 `cargo build --release`
- 生成二开品牌二进制
- 如需安装脚本，同时调整 `scripts\install.ps1` 的默认目录与品牌文案

初版产物策略：

- 可以继续复用 `jcode` crate 名称与主二进制构建链
- 对外分发时改用 Saitec 品牌命名，例如安装目录、文案、可执行文件别名

如果当前仓库改名成本过高，允许本轮先产出：

- 一个成功编译的 release 二进制
- 一个面向 Saitec 的 Windows 打包目录

后续再决定是否彻底把 crate/bin 名称从 `jcode` 改为 `saitec-tui`

## Implementation Notes

实现时遵循以下顺序：

1. 先把存储根目录切换到 `~/.saitec_tui`
2. 再补 `auth.json` 读写与 mock 校验
3. 再把启动门禁接入主入口
4. 再把 `/login` 与 `/logout` 接入 TUI
5. 最后做 release 构建与打包

这样可以降低大范围改动时的定位成本。

## Acceptance Criteria

满足以下条件则视为本次登录逻辑二开完成：

- 程序默认使用 `~/.saitec_tui/` 作为本地根目录
- 未登录时不能正常使用 TUI 会话
- `/login` 可以触发浏览器登录与本地回调
- 回调拿到 `auth_token` 后会写入 `auth.json`
- 启动时会校验 token 是否有效
- token 无效时会阻止使用并要求重新登录
- `/logout` 可用
- `config.toml` 从 `~/.saitec_tui/config.toml` 读取
- release 构建成功
- 至少一组针对登录门禁的自动化测试通过
