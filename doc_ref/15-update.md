# 15 · Update（自动更新）

> 子系统：版本检测与安装。stable channel 从 GitHub Release 下载预编译二进制 + SHA256 校验；main channel 从源码 `git pull` + `cargo build --release`。前台交互与后台静默更新，经 Bus 事件通知 UI。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

检测并安装新版本：stable channel 从 GitHub Release 下载预编译二进制 + SHA256 校验，main channel 从源码 `git pull` + `cargo build --release`；支持前台交互与后台静默更新，通过 Bus 事件通知 UI。

## 关键文件清单

| 路径 | 职责 |
|---|---|
| `src/update.rs` | 主模块(~1126 行)：`check_and_maybe_update()` 入口、release 下载安装、main channel 源码编译、后台 session 更新、`UpdateMetadata` 持久化 |
| `crates/jcode-update-core/src/lib.rs` | 独立 crate(~425 行)：纯逻辑——版本比较、asset 名称、SHA256 校验、下载进度格式化、更新时长估算、git pull 错误摘要 |
| `RELEASING.md` | 发布流程文档：quick-release(本地 ~2.5 分钟)和 CI release(~16 分钟)、osxcross 交叉编译、GitHub Release 上传 |

## 核心类型与关键函数

- **`PreparedUpdate`** (`jcode-update-core`) — 枚举：`None` / `Stable { release, estimate }` / `MainSource { latest_sha, estimate }`。
- **`UpdateCheckResult`** — `NoUpdate` / `UpdateAvailable` / `UpdateInstalled` / `Error`。
- **`UpdateMetadata`** (`update.rs:69`) — 持久化到 `update_metadata.json`，记 last_check、installed_version、历史更新时长。
- **`check_and_maybe_update(auto_install)`** (`update.rs:904`) — 主入口：是否应检查 → 检查 → 可选自动安装。
- **`spawn_background_session_update(session_id)`** (`update.rs:447`) — 后台线程执行更新，经 `Bus` 事件通知 UI。
- **`download_and_install_blocking_with_progress()`** (`update.rs:747`) — 下载 + SHA256 校验 + 解压 tar.gz + 安装到版本目录 + 更新 symlink。
- **`prepare_update_blocking()`** — 按 `config.features.update_channel` 分发到 stable 或 main 路径。
- **`build_from_source()`** (`update.rs:654`) — main channel：`git clone/pull` → `cargo build --release`。
- **`BACKGROUND_UPDATE_THRESHOLD`** = 15s — 超此阈值的更新走后台路径。
- 工具函数：`version_is_newer()` / `parse_sha256sums()` / `verify_asset_checksum_text()`。

## 主控制流

**Stable channel**：
```
check_and_maybe_update()
  → 检查 should_auto_update() + UpdateMetadata.should_check()（60s 节流）
  → fetch_latest_release_blocking() 调 GitHub API 取最新 release
  → version_is_newer() 比较版本号
  → 下载 platform asset → 可选 SHA256SUMS 校验 → 解压 tar.gz → 安装到 builds/versions/<version>/ → 更新 stable/current/launcher symlink
```

**Main channel**：
```
  → 比较当前 binary 内嵌 git hash 与 GitHub main branch 最新 commit
  → 有 cargo: build_from_source() → git pull --ff-only(或 clone) → cargo build --release → 安装
  → 无 cargo: fallback 到 latest release
```

**后台更新**：
```
spawn_background_session_update() 启动新线程
  → 经 Bus::global().publish(BusEvent::SessionUpdateStatus(...)) 通知 UI 进度
```

## 依赖关系

- **依赖**：`crate::config::config().features.update_channel`、`crate::storage::jcode_dir()`/`builds_dir()`、`crate::build`(install_binary、symlink 管理)、`crate::bus::Bus`(事件通知)、`crate::platform`(权限设置)、`reqwest::blocking`、`sha2`、`flate2`、`tar`。
- **被依赖**：`src/cli/startup.rs`(启动时 auto check)、`src/cli/hot_exec.rs`(hot-reload 后检查)、`src/tui/app/state_ui_maintenance.rs`(UI 层触发)。

## 陷阱与设计约束

- **main channel 要求本地有 cargo**：没 cargo 会 fallback 到 latest release，但用户在 main channel 可能期望最新 commit——行为可能不符预期。
- **`should_auto_update()` 在非 release build 返回 false**：开发者不会触发自动更新。
- **60 秒检查间隔硬编码**：`UPDATE_CHECK_INTERVAL` 无法经 config 调整。
- **`JCODE_NO_AUTO_UPDATE` 环境变量**：与 telemetry 的 `JCODE_NO_TELEMETRY` 类似 opt-out 模式，但无对应文件 opt-out。
- **下载超时 120 秒**：`DOWNLOAD_TIMEOUT` 对大文件在慢网络下可能不够。
- **版本比较用简单 semver 三段比较**：不支持 pre-release 标签。
- **stroke 与 [01 CLI](01-cli.md) `hot_exec` 联动**：hot-reload 后会触发 update check；`reload` 用 exec 替换进程(见 [04-server.md](04-server.md))，update 用下载替换——两条替换路径要区分。

## 关联模块

| 模块 | 职责 | 归位说明 |
|---|---|---|
| `crate::build`(build.rs 注入的 `JCODE_*` 版本串) | 更新比较的版本来源 | 见 [12-workspace-build-ci.md](12-workspace-build-ci.md) 的 build.rs 小节 |

## 回指

- 启动时 `spawn_background_update_check` 在 [00-overview-and-entry.md](00-overview-and-entry.md) 的 startup 编排中
- `should_auto_update` 与 TTY 的关系(后台/管道场景才自动安装)见 [01-cli.md](01-cli.md)
- 版本串来源(build.rs 注入 `JCODE_VERSION`/`JCODE_SEMVER`)见 [12-workspace-build-ci.md](12-workspace-build-ci.md)