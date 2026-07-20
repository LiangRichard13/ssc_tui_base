use anyhow::Result;
use clap::Parser;
use std::io::IsTerminal;

use crate::{logging, perf, server, startup_profile, storage, telemetry, update};

use super::{
    args::{Args, Command},
    dispatch, hot_exec, output, terminal,
};

pub async fn run() -> Result<()> {
    startup_profile::init();

    terminal::install_panic_hook();
    startup_profile::mark("panic_hook");

    logging::init();
    startup_profile::mark("logging_init");
    logging::cleanup_old_logs();
    startup_profile::mark("log_cleanup");
    logging::info("jcode starting");
    crate::platform::raise_nofile_limit_best_effort(8_192);
    startup_profile::mark("nofile_limit");

    storage::harden_user_config_permissions();
    startup_profile::mark("perm_harden");

    perf::init_background();
    startup_profile::mark("perf_init");

    telemetry::record_install_if_first_run();
    telemetry::record_upgrade_if_needed();
    startup_profile::mark("telemetry_check");

    let args = parse_and_prepare_args()?;
    spawn_background_update_check(&args);

    if let Err(e) = dispatch::run_main(args).await {
        report_main_error(&e);
        return Err(e);
    }

    Ok(())
}

fn parse_and_prepare_args() -> Result<Args> {
    let args = Args::parse();
    startup_profile::mark("args_parse");

    output::set_quiet_enabled(args.quiet);

    if let Some(cwd) = &args.cwd {
        std::env::set_current_dir(cwd)?;
        logging::info(&format!("Changed working directory to: {}", cwd));
    }

    if args.trace {
        crate::env::set_var("JCODE_TRACE", "1");
    }

    if let Some(ref socket) = args.socket {
        server::set_socket_path(socket);
    }

    crate::process_title::set_initial_title(&args);

    Ok(args)
}

fn spawn_background_update_check(args: &Args) {
    if !should_spawn_background_update_check(args) {
        return;
    }

    // server 端不再做发布通道 update 检查 —— 见 `pub fn spawn_tui_update_check_for_client`。
    // 服务端 publish 不会跨进程到 client 端 App 的 Bus 订阅者，因此必须由 client 自己跑。
    // selfdev 路径额外做一次源码通道检查（git 状态）—— 仅日志，不影响 TUI banner。
    if !update::is_release_build() {
        spawn_hot_exec_update_check();
    }
}

/// 在持有 `tui::App` 的进程内 spawn SAITEC 后端 TUI 推送检查。
///
/// **必须** 在 `App` 已经 `Bus::subscribe` 之后调用 —— 该 spawn 任务 `Bus::publish`
/// 一个 `UpdateStatus::Available`，由 client/local TUI 的本地订阅者收到并调 `handle_update_status`。
/// 跨进程（如 server 进程 publish 给 client 进程）不可行：每个进程 `Bus::global()` 是独立的
/// `OnceLock<Bus>` 单例。
///
/// 调用方：
/// - `cli/commands.rs` —— `App::new(provider, registry)` 之后
/// - `cli/tui_launch.rs` —— `App::new_for_remote_with_options(...)` 之后
pub fn spawn_tui_update_check_for_client() {
    use crate::bus::{Bus, BusEvent, UpdateStatus};
    let current_raw = env!("JCODE_VERSION").to_string();
    let current = crate::saitec::tui_update::strip_prerelease(&current_raw).to_string();
    logging::info(&format!(
        "[tui-update] spawn check-update: current_version={} (raw JCODE_VERSION={})",
        current, current_raw
    ));
    tokio::spawn(async move {
        // 推迟一下让 TUI App 完成 Bus::subscribe —— broadcast::channel subscribe 之前的 send 会丢。
        // 2s 是稳的（HTTP 请求几百 ms 起，此延迟不会让用户感知到）。
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        let receiver_count = crate::bus::Bus::global().receiver_count();
        crate::logging::info(&format!(
            "[tui-update] about to publish (Bus receiver_count={})",
            receiver_count
        ));
        match crate::saitec::tui_update::check_tui_update(&current).await {
            Ok(Some(payload)) => {
                crate::logging::info(&format!(
                    "[tui-update] backend reports NEW version available: latest={} ({} MB), file={}, download_url={}",
                    payload.latest_version,
                    payload.size_bytes / 1_000_000,
                    payload.filename,
                    payload.download_url,
                ));
                Bus::global().publish(BusEvent::UpdateStatus(UpdateStatus::Available {
                    current,
                    latest: payload.latest_version.clone(),
                    payload,
                }));
            }
            Ok(None) => {
                crate::logging::info(&format!(
                    "[tui-update] backend reports up-to-date (is_new=false); latest={}",
                    current,
                ));
            }
            Err(e) => {
                crate::logging::warn(&format!("tui update check failed: {:#}", e));
            }
        }
    });
}

/// selfdev / 非 release build：保留 main channel（git pull + cargo build 源码）。
/// 用户启动时给一句日志提示存在更新，不自动 install —— selfdev 用户通常自己 git pull。
fn spawn_hot_exec_update_check() {
    let start = std::time::Instant::now();
    std::thread::spawn(move || {
        if let Some(true) = hot_exec::check_for_updates() {
            logging::info(
                "Self-dev build: new commits detected. Use /reload to rebuild from source.",
            );
        }
        logging::info(&format!(
            "[TIMING] hot_exec_update_check: total={}ms",
            start.elapsed().as_millis()
        ));
    });
}

fn should_spawn_background_update_check(args: &Args) -> bool {
    !args.quiet
        && !args.no_update
        && !matches!(
            args.command,
            Some(Command::Update) | Some(Command::Serve { .. })
        )
        && args.resume.is_none()
}

fn has_live_terminal_attached() -> bool {
    std::io::stdin().is_terminal()
        || std::io::stdout().is_terminal()
        || std::io::stderr().is_terminal()
}

fn should_auto_install_update(args: &Args, live_terminal_attached: bool) -> bool {
    args.auto_update && !live_terminal_attached
}

fn report_main_error(error: &anyhow::Error) {
    let error_str = format!("{:?}", error);
    logging::error(&error_str);

    if let Some(session_id) = terminal::get_current_session() {
        output::stderr_blank_line();
        output::stderr_info("\x1b[33mTo restore this session, run:\x1b[0m");
        output::stderr_info(format!("  jcode --resume {}", session_id));
        output::stderr_blank_line();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::{Args, Command};
    use clap::Parser;

    fn parse_args(argv: &[&str]) -> Args {
        Args::parse_from(argv)
    }

    #[test]
    fn auto_install_allowed_without_live_terminal() {
        let args = parse_args(&["jcode", "login"]);
        assert!(should_auto_install_update(&args, false));
    }

    #[test]
    fn auto_install_deferred_when_live_terminal_is_attached() {
        let args = parse_args(&["jcode", "login"]);
        assert!(!should_auto_install_update(&args, true));
    }

    #[test]
    fn auto_install_respects_explicit_disable_even_without_terminal() {
        let mut args = parse_args(&["jcode", "login"]);
        args.auto_update = false;
        assert!(!should_auto_install_update(&args, false));
    }

    #[test]
    fn update_command_still_skips_background_check_before_auto_install_logic() {
        let args = parse_args(&["jcode", "update"]);
        assert!(matches!(args.command, Some(Command::Update)));
        assert!(!should_auto_install_update(&args, true));
        assert!(should_auto_install_update(&args, false));
    }
}
