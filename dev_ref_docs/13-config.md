# 13 · Configuration

> 子系统：全局配置加载（`~/.ssc_tui/config.toml`）、`OnceLock` 单例、环境变量逐字段覆盖、外部 auth trust 管理。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

从 `~/.ssc_tui/config.toml`（或 `$JCODE_HOME/config.toml`）加载全局配置，通过 `OnceLock` 单例暴露，支持 80+ 个 `JCODE_*` 环境变量逐字段覆盖文件值，覆盖 keybinding/display/feature/provider/auth/safety/ambient/gateway/compaction 等全部用户可调参数。

## 关键文件清单

| 路径 | 职责 |
|---|---|
| `src/config.rs` | 入口：`Config` 顶层 struct、`OnceLock<Config>` 单例 `config()`、re-export `jcode_config_types` 全部子类型、定义 `DictationConfig` |
| `src/config/config_file.rs` | `Config::load()`/`save()`/`path()`/`set_default_model()` 等持久化方法；外部 auth trust 管理（allow/revoke） |
| `src/config/env_overrides.rs` | `apply_env_overrides()` 逐字段读 `JCODE_*` 环境变量覆盖 toml 值（~80+ env var） |
| `src/config/default_file.rs` | `create_default_config_file()` 生成带注释的默认 config.toml 模板（~245 行） |
| `src/config/display_summary.rs` | `display_string()` 生成人类可读配置摘要（`/config` 命令用） |
| `src/config_tests.rs` | 配置单元测试（默认值、env override、external auth 等） |
| `crates/jcode-config-types/src/lib.rs` | 独立 crate：所有可序列化配置类型定义（~775 行） |

## 核心类型与关键函数

- **`Config`** (`src/config.rs:28`) — 顶层配置 struct，聚合 14 个子配置。
- **`config()`** (`src/config.rs:22`) — `OnceLock` 单例，`get_or_init(Config::load)`，首次调用加载文件 + env override。
- **`Config::load()`** (`config_file.rs:13`) — 文件加载 → legacy compat → env override。
- **`apply_env_overrides()`** (`env_overrides.rs:9`) — 逐字段 `JCODE_*` 覆盖。
- **配置类型**（`jcode-config-types`）：`DisplayConfig`、`ProviderConfig`、`AuthConfig`、`FeatureConfig`、`SafetyConfig`、`CompactionConfig`、`KeybindingsConfig`、`NamedProviderConfig` 等。
- **`NamedProviderConfig`** — 自定义 OpenAI-compatible provider 定义（`[providers.xxx]`）。
- **`CompactionConfig`** — reactive/proactive/semantic 三压缩模式参数。
- **`SafetyConfig`** — ntfy/email/telegram/discord 通知渠道配置。

## 主控制流

```
首次调用 config()
  → Config::load()
    → 从 jcode_dir()/config.toml 读 TOML，toml::from_str 反序列化
    → display.apply_legacy_compat()  // show_diffs → diff_mode 迁移
    → apply_env_overrides()          // 逐字段 JCODE_* 覆盖
  → 结果存入 OnceLock<Config>，后续调用返回同一引用

写操作（set_default_model 等）
  → load-patch-save 模式：读文件 → 改 → 写文件
  → 不更新单例（重启生效）
```

## 依赖关系

- **依赖**：`crate::storage::jcode_dir()`（路径）、`crate::protocol::TranscriptMode`、`crate::logging`、`toml`、`serde`。
- **被依赖**：几乎所有模块（搜索到 20+ 文件用 `crate::config`）——provider、tui、agent、server、CLI、notifications、sidecar 等。是名副其实的「基础设施层」。

## 陷阱与设计约束

- **OnceLock 不可变**：`set_default_model` 保存了文件但无法更新单例，注释说明「生效到下次重启」。长运行 session 中可能造成困惑——改了配置但当前 session 不生效。
- **`Config::load()` vs `config()` 路径不一致**：外部 auth trust 的 `external_auth_source_allowed` 每次调 `Config::load()` 重读文件（不缓存），而 `external_auth_source_allowed_for_path_cached` 用 `config()` 缓存。两路径行为不一致，调试时易混淆。
- **env override 没有反向同步**：`copilot_premium` 是唯一 config→env 反向写入的字段（`env_overrides.rs:431-440`），其他字段只有 env→config 单向。改这个字段后要留意进程 env 被 mutate。
- **80+ 环境变量**：`apply_env_overrides` 覆盖面极大，调试「为什么配置不生效」时优先查 `JCODE_*` env var 是否被设置（尤其 CI/容器环境）。

## 关联模块

| 模块 | 职责 | 归位说明 |
|---|---|---|
| — | Configuration 本身无独立辅助模块；keybindings 消费方在 [05 TUI](05-tui.md) 的 `src/tui/keybind.rs` | — |

## 回指

- keybindings 配置消费：[05-tui.md](05-tui.md)
- `NamedProviderConfig` 与 OpenAI-compatible profile 的关系：[03-provider.md](03-provider.md)（`apply_named_provider_profile_env`）
- `CompactionConfig` 三模式驱动 [02 Agent](02-agent-runtime.md) 的 CompactionManager
- SafetyConfig 通知渠道被 [16-overnight.md](16-overnight.md) 的 ambient/overnight 安全系统使用