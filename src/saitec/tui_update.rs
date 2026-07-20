//! TUI 客户端更新推送：与 SAITEC 后端 `/api/v1/tui/check-update` + `/api/v1/tui/download`
//! 对接。后端分发目录的 `vX.Y(.Z).exe` 由后端扫描、自动选最高版本。
//!
//! 取代旧有 GitHub Release 自更新通道（`src/update.rs` + `crates/jcode-update-core`
//! 中的 GitHub 部分）。selfdev profile 仍保留 `hot_exec` 源码编译通道。
//!
//! ## 流程
//!
//! 1. 启动后台 `tokio::spawn` 一次调用 [`check_tui_update`]，命中 `is_new=true` 时
//!    `Bus::publish(UpdateStatus::Available { payload, ... })`。
//! 2. TUI `App.pending_update` 收到后渲染底部 banner；按 `U` 调
//!    [`start_tui_update_download`](crate::tui::app::App::start_tui_update_download) 触发下载。
//! 3. 下载调用 [`download_tui_update`] 流式写盘至 `<jcode_home>/downloads/SAITEC-TUI-v{version}.exe`，
//!    鉴权用 `runtime_api_key()` 注入 `X-API-Key` header；每 ~256KB 推一次 `DownloadProgress`。
//! 4. 下载完成发布 `UpdateStatus::Downloaded { version, path }` → TUI 提示用户手动退出 + 跑此 exe。

use anyhow::{Context, Result, anyhow, bail};
use futures::TryStreamExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

use crate::bus::TuiUpdatePayload;
use crate::saitec::auth::{ApiEnvelope, core_api_base, runtime_api_key};

// ── 全局 TUI 更新状态（绕过 &dyn TuiState trait dispatch）────────────
//
// `handle_update_status` 在 `tui/app/auth.rs`（tui 侧）会把 payload 写入 App 字段，
// 但 `draw_status` 通过 `&dyn TuiState` 读取时可能因 trait object dispatch 返回 None。
// 此全局静态直接从 Bus 事件写入（tui 侧）并由 draw_status 读取（ui 侧），完全避开 trait。
use std::sync::{OnceLock, RwLock};

/// 后端 check-update 响应的最新 payload（没有更新时为 None）。
/// 由 `tui/app/auth.rs::handle_update_status` 在 Available 分支写入，
/// 由 `tui/ui_input.rs::draw_status` 直接读取。
///
/// `RwLock` 而非 `Mutex`：读路径在 60fps redraw 中高频（splash + 主布局两处 `draw_status`
/// 都访问），写路径只在 Available / 清理时发生一次。多读单写场景 RwLock 不会相互阻塞。
pub static TUI_PENDING_UPDATE: OnceLock<RwLock<Option<TuiUpdatePayload>>> = OnceLock::new();

fn tui_pending_update_lock() -> &'static RwLock<Option<TuiUpdatePayload>> {
    TUI_PENDING_UPDATE.get_or_init(|| RwLock::new(None))
}

/// 设置全局 pending update（供 handle_update_status 调用）。
pub fn set_global_pending_update(payload: TuiUpdatePayload) {
    *tui_pending_update_lock().write().unwrap() = Some(payload);
}

/// 清除全局 pending update（下载开始后 / 错误后 / 完成后）。
pub fn clear_global_pending_update() {
    *tui_pending_update_lock().write().unwrap() = None;
}

/// `check-update` 公开接口超时。
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
/// `download` 大文件（~100MB）需要更宽松的超时。
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);
/// 流式下载的进度回调粒度（256KB）。每次累计写入超过这个阈值时回调一次。
const PROGRESS_STEP_BYTES: u64 = 256 * 1024;
/// 预留给磁盘空间检查的 buffer。
#[allow(dead_code)]
const DISK_HEADROOM_BYTES: u64 = 16 * 1024 * 1024;

/// 给定版本号返回默认下载落地路径：
/// `<jcode_home>/downloads/SAITEC-TUI-v{strip_prerelease(version)}.exe`
///
/// 自动创建 `downloads/` 子目录。如未来要允许用户自定义 download dir，
/// 可加 env override `JCODE_DOWNLOAD_DIR`。
pub fn available_dest_path(version: &str) -> Result<PathBuf> {
    let dir = downloads_dir()?;
    Ok(dir.join(format!("SAITEC-TUI-v{}.exe", strip_prerelease(version))))
}

/// 本地 downloads 目录：`<jcode_home>/downloads/`。
fn downloads_dir() -> Result<PathBuf> {
    let dir = crate::storage::jcode_dir()?.join("downloads");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating downloads dir {}", dir.display()))?;
    Ok(dir)
}

/// 剥掉 `JCODE_VERSION` 里所有非核心 semver 字符，返回三段数字前缀。
///
/// `JCODE_VERSION` 由 `build.rs:105-115` 注入，可能形如：
///   - release:        `"v1.0.1-alpha (abcdef12)"` 或 `"v1.0.1 (abcdef12)"`
///   - selfdev dirty:  `"v1.0.1+dirty.1700000000 (abcdef12)"`
///   - selfdev clean:  `"v1.0.0-dev (abcdef12)"`
///
/// 后端 `/api/v1/tui/check-update` 只识三段数字；任何 `(-prerelease)`、`(+build)`、
/// ` (git-hash)` 后缀都必须剥掉，否则 `current_version=1.0.1 (abcdef12)` 会被后端解析失败。
pub fn strip_prerelease(version: &str) -> &str {
    let s = version.trim().trim_start_matches('v');
    // 先找到第一处 `-` 或 `+`，砍掉 prerelease / build metadata。
    let cut1 = s.find(|c: char| c == '-' || c == '+').unwrap_or(s.len());
    let major_minor_patch = s[..cut1].trim_end();
    // clean release 没有 `-` / `+`，需继续剥尾部 ` (git-hash)` 部分。
    let cut2 = major_minor_patch
        .find(|c: char| c == '(' || c == ' ' || c == '\t')
        .unwrap_or(major_minor_patch.len());
    &major_minor_patch[..cut2]
}

/// 逐段比较 `a > b`。`"1.10"` 视为 `"1.10.0"`，所以 `"1.10" > "1.9"`。
/// 后缀版本（`-alpha`、`+build`）先剥后比。
pub fn version_is_newer(a: &str, b: &str) -> bool {
    fn parse(s: &str) -> Vec<u32> {
        strip_prerelease(s)
            .split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .collect()
    }
    parse(a).cmp(&parse(b)) == std::cmp::Ordering::Greater
}

/// `GET /api/v1/tui/check-update?current_version=<>` —— 公开接口，无鉴权。
///
/// # 返回
/// - `Ok(Some(payload))` —— `is_new=true`，有更新可下
/// - `Ok(None)` —— `is_new=false`，已是最新
/// - `Err(_)` —— 网络失败 / HTTP 非 2xx / 后端 `success=false`（调用方一般 `warn!` 静默）
pub async fn check_tui_update(current_version: &str) -> Result<Option<TuiUpdatePayload>> {
    let base = core_api_base();
    let url = format!(
        "{}/api/v1/tui/check-update?current_version={}",
        base.trim_end_matches('/'),
        url_encode_version(strip_prerelease(current_version)),
    );

    let client = reqwest::Client::builder()
        .timeout(CHECK_TIMEOUT)
        .build()
        .context("building reqwest client for check-update")?;

    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {}", url))?;
    if !resp.status().is_success() {
        bail!("check-update HTTP {}", resp.status());
    }
    let env: ApiEnvelope<TuiUpdatePayload> = resp
        .json()
        .await
        .context("parsing /api/v1/tui/check-update JSON envelope")?;
    if !env.success {
        bail!("backend reported failure: {}", env.message);
    }
    let result = if env.data.is_new {
        Some(env.data)
    } else {
        crate::logging::info(&format!(
            "[tui-update] backend reports up-to-date (is_new=false); latest={}",
            env.data.latest_version,
        ));
        None
    };
    Ok(result)
}

/// `GET /api/v1/tui/download[?version=]` —— 需 `X-API-Key` 鉴权。
///
/// # 参数
/// - `url` —— 后端响应的 `download_url` 字段（或拼出的 `${base}/api/v1/tui/download?version=...`）
/// - `dest` —— 落地 `.exe` 路径
/// - `api_key` —— 当前用户 API key。可为 `None`（未登录），将返回 401 → `bail!("unauthorized")`
/// - `on_progress` —— `(downloaded, total)`，每 ~256KB 触发。`total=0` 表示后端未给 Content-Length
/// - `cancel_rx` —— `watch::Receiver<bool>`，置 `true` 后下次循环检查时清理半成品 + `bail!("cancelled")`
///
/// # 返回
/// - `Ok(dest)` —— 下载完成且落盘成功
/// - `Err(_)` —— 网络 / 鉴权 / 写盘 / 取消
pub async fn download_tui_update<F>(
    url: &str,
    dest: &Path,
    api_key: Option<&str>,
    on_progress: F,
    mut cancel_rx: watch::Receiver<bool>,
) -> Result<PathBuf>
where
    F: Fn(u64, u64) + Send + 'static,
{
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating parent dir {}", parent.display()))?;
    }

    let mut default_headers = reqwest::header::HeaderMap::new();
    if let Some(key) = api_key {
        let value = reqwest::header::HeaderValue::from_str(key)
            .map_err(|_| anyhow!("api key contains invalid header character"))?;
        default_headers.insert("X-API-Key", value);
    }

    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .default_headers(default_headers)
        .build()
        .context("building reqwest client for download")?;

    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {}", url))?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        cleanup_partial(dest).await;
        bail!("unauthorized (HTTP 401): please log in first");
    }
    if !resp.status().is_success() {
        cleanup_partial(dest).await;
        bail!("download HTTP {}", resp.status());
    }
    let total = resp.content_length().unwrap_or(0);

    let mut file = tokio::fs::File::create(dest)
        .await
        .with_context(|| format!("creating {}", dest.display()))?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_progress: u64 = 0;

    loop {
        tokio::select! {
            changed = cancel_rx.changed() => {
                if changed.is_ok() && *cancel_rx.borrow() {
                    drop(file);
                    cleanup_partial(dest).await;
                    bail!("cancelled by user");
                }
                // sender 已被 drop（false 不会再变）—— 继续读流
            }
            next = stream.try_next() => {
                match next {
                    Ok(Some(chunk)) => {
                        file.write_all(&chunk)
                            .await
                            .with_context(|| format!("writing to {}", dest.display()))?;
                        downloaded = downloaded.saturating_add(chunk.len() as u64);
                        if downloaded.saturating_sub(last_progress) >= PROGRESS_STEP_BYTES {
                            on_progress(downloaded, total);
                            last_progress = downloaded;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        drop(file);
                        cleanup_partial(dest).await;
                        return Err(anyhow::Error::from(e))
                            .with_context(|| format!("streaming {}", url));
                    }
                }
            }
        }
    }

    on_progress(downloaded, total);
    drop(file);
    Ok(dest.to_path_buf())
}

/// 取当前活跃 SAITEC API key。下载接口强制要求 —— 无 key 时返回 `None` 让调用方决定如何提示。
///
/// 直接 re-export auth 模块，避免下游每次写 `crate::saitec::auth::runtime_api_key()`。
pub fn current_api_key() -> Option<String> {
    runtime_api_key()
}

async fn cleanup_partial(dest: &Path) {
    let _ = tokio::fs::remove_file(dest).await;
}

/// 极简版本号 URL 编码 —— 版本只含数字 + `.`，安全字符透传。
fn url_encode_version(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '0'..='9' | 'a'..='z' | 'A'..='Z' | '.' | '-' | '~' => out.push(c),
            other => out.push_str(&format!("%{:02X}", other as u32)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_prerelease_basic() {
        assert_eq!(strip_prerelease("1.0.1-alpha"), "1.0.1");
        assert_eq!(strip_prerelease("v2.0+build.5"), "2.0");
        assert_eq!(strip_prerelease("1.2.3"), "1.2.3");
        assert_eq!(strip_prerelease("v1.0"), "1.0");
        assert_eq!(strip_prerelease(""), "");
    }

    /// build.rs:105-115 注入的 `JCODE_VERSION` 实际格式不止 "1.0.1-alpha"。
    /// release / selfdev-dirty / selfdev-clean / 含 git hash 括号四种都要稳定剥成三段数字，
    /// 否则后端 `current_version` 拿到 hash 就会误判。
    /// 任何对 build.rs `format!("v{} ({})", ...)` 的改动都会被这组测试挡住。
    #[test]
    fn strip_prerelease_handles_build_rs_formats() {
        // release build: "v1.0.1-alpha (abcdef12)"
        assert_eq!(strip_prerelease("v1.0.1-alpha (abcdef12)"), "1.0.1");
        // release 干净: "v1.0.1 (abcdef12)"
        assert_eq!(strip_prerelease("v1.0.1 (abcdef12)"), "1.0.1");
        // selfdev dirty: "v1.0.1+dirty.1700000000 (abcdef12)"
        assert_eq!(
            strip_prerelease("v1.0.1+dirty.1700000000 (abcdef12)"),
            "1.0.1"
        );
        // selfdev dev: "v1.0.0-dev (abcdef12)"
        assert_eq!(strip_prerelease("v1.0.0-dev (abcdef12)"), "1.0.0");
        // 不带括号（CARGO_PKG_VERSION 直接使用场景）
        assert_eq!(strip_prerelease("1.0.1-alpha"), "1.0.1");
        // 完整 JCODE_SEMVER（不带 v 前缀、不带 hash）
        assert_eq!(strip_prerelease("1.0.1-alpha"), "1.0.1");
    }

    #[test]
    fn version_is_newer_basic() {
        assert!(version_is_newer("1.0.1", "1.0.0"));
        assert!(version_is_newer("1.10", "1.9"));
        assert!(version_is_newer("2.0", "1.99.99"));
        assert!(!version_is_newer("1.0.0", "1.0.0"));
        assert!(!version_is_newer("1.0.0", "1.0.1"));
    }

    #[test]
    fn version_is_newer_strips_prerelease_before_compare() {
        // "1.0.1-alpha" 应被剥成 "1.0.1"，与 "1.0.0" 比 → newer
        assert!(version_is_newer("1.0.1-alpha", "1.0.0"));
        // 两边都有 pre-release 时仍按主版本比
        assert!(!version_is_newer("1.0.0-alpha", "1.0.0"));
    }

    #[test]
    fn url_encode_passthrough_safe_chars() {
        assert_eq!(url_encode_version("1.0.1"), "1.0.1");
        assert_eq!(url_encode_version("1.10.0-beta"), "1.10.0-beta");
    }
}
