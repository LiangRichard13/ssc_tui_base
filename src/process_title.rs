use crate::cli::args::{AmbientCommand, Args, Command};

const LINUX_PROCESS_TITLE_LIMIT: usize = 15;
const KILLALL_PROCESS_NAME: &str = "ssc-tui";

fn compact_process_title(prefix: &str, name: Option<&str>) -> String {
    let mut title = prefix.to_string();
    if let Some(name) = name.filter(|name| !name.is_empty()) {
        let remaining = LINUX_PROCESS_TITLE_LIMIT.saturating_sub(title.len());
        if remaining > 0 {
            title.push_str(&name.chars().take(remaining).collect::<String>());
        }
    }
    title
}

pub(crate) fn session_name(session_id: &str) -> String {
    crate::id::extract_session_name(session_id)
        .map(|name| name.to_string())
        .unwrap_or_else(|| session_id.to_string())
}

fn normalized_display_title(title: &str) -> Option<String> {
    let normalized = title.split_whitespace().collect::<Vec<_>>().join(" ");
    (!normalized.is_empty()).then_some(normalized)
}

fn capitalize_ascii_label(label: &str) -> String {
    let mut chars = label.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", truncated)
    } else {
        truncated
    }
}

pub(crate) fn terminal_session_label(session_name: &str, display_title: Option<&str>) -> String {
    let fallback = capitalize_ascii_label(session_name);
    let Some(title) = display_title.and_then(normalized_display_title) else {
        return fallback;
    };
    if title.eq_ignore_ascii_case(session_name) || title.eq_ignore_ascii_case(&fallback) {
        return fallback;
    }
    format!("{} ({})", truncate_chars(&title, 48), session_name)
}

pub(crate) fn terminal_session_label_for_id(session_id: &str) -> String {
    let session_name = session_name(session_id);
    let display_title = crate::session::Session::load_startup_stub(session_id)
        .ok()
        .and_then(|session| session.display_title().map(ToOwned::to_owned));
    match display_title.as_deref() {
        Some(title) => terminal_session_label(&session_name, Some(title)),
        None => session_name,
    }
}

pub(crate) fn set_title(title: impl AsRef<str>) {
    proctitle::set_title(title.as_ref());
    set_killall_process_name();
}

fn set_killall_process_name() {
    #[cfg(target_os = "linux")]
    unsafe {
        let mut name = [0u8; 16];
        let bytes = KILLALL_PROCESS_NAME.as_bytes();
        let len = bytes.len().min(name.len().saturating_sub(1));
        name[..len].copy_from_slice(&bytes[..len]);
        let _ = libc::prctl(libc::PR_SET_NAME, name.as_ptr(), 0, 0, 0);
    }
}

pub(crate) fn set_server_title(server_name: &str) {
    set_title(compact_process_title("ssc-tui:s:", Some(server_name)));
}

pub(crate) fn set_client_generic_title(is_selfdev: bool) {
    let prefix = if is_selfdev {
        "stui:selfdev"
    } else {
        "stui:client"
    };
    set_title(compact_process_title(prefix, None));
}

pub(crate) fn set_client_session_title(session_id: &str, is_selfdev: bool) {
    set_client_display_title(&session_name(session_id), is_selfdev);
}

pub(crate) fn set_client_display_title(session_name: &str, is_selfdev: bool) {
    let prefix = if is_selfdev { "stui:d:" } else { "stui:c:" };
    set_title(compact_process_title(prefix, Some(session_name)));
}

pub(crate) fn set_client_remote_display_title(
    server_name: &str,
    session_name: &str,
    is_selfdev: bool,
) {
    if server_name.is_empty() || server_name.eq_ignore_ascii_case("jcode") {
        set_client_display_title(session_name, is_selfdev);
        return;
    }
    let prefix = if is_selfdev { "stui:d:" } else { "stui:c:" };
    set_title(format!("{prefix}{server_name}/{session_name}"));
}

pub(crate) fn initial_title(args: &Args) -> String {
    match &args.command {
        Some(Command::Serve { .. }) => "ssc-tui:server".to_string(),
        Some(Command::Connect) => "ssc-tui:client".to_string(),
        Some(Command::Run { .. }) => "ssc-tui run".to_string(),
        Some(Command::Login { .. }) => "ssc-tui login".to_string(),
        Some(Command::Repl) => "ssc-tui repl".to_string(),
        Some(Command::Update) => "ssc-tui update".to_string(),
        Some(Command::Version { .. }) => "ssc-tui version".to_string(),
        Some(Command::Usage { .. }) => "ssc-tui usage".to_string(),
        Some(Command::SelfDev { .. }) => "stui:selfdev".to_string(),
        Some(Command::Debug { .. }) => "ssc-tui debug".to_string(),
        Some(Command::Auth(_)) => "ssc-tui auth".to_string(),
        Some(Command::Provider(_)) => "ssc-tui provider".to_string(),
        Some(Command::Memory(_)) => "ssc-tui memory".to_string(),
        Some(Command::Session(_)) => "ssc-tui session".to_string(),
        Some(Command::Ambient(subcommand)) => match subcommand {
            AmbientCommand::RunVisible => "ssc-tui ambient visible".to_string(),
            _ => "ssc-tui ambient".to_string(),
        },
        Some(Command::Pair { .. }) => "ssc-tui pair".to_string(),
        Some(Command::Permissions) => "ssc-tui permissions".to_string(),
        Some(Command::Transcript { .. }) => "ssc-tui transcript".to_string(),
        Some(Command::Dictate { .. }) => "ssc-tui dictate".to_string(),
        Some(Command::SetupHotkey {
            listen_macos_hotkey,
        }) => {
            if *listen_macos_hotkey {
                "ssc-tui hotkey listener".to_string()
            } else {
                "ssc-tui hotkey setup".to_string()
            }
        }
        Some(Command::Browser { .. }) => "ssc-tui browser".to_string(),
        Some(Command::Replay { .. }) => "ssc-tui replay".to_string(),
        Some(Command::Model(_)) => "ssc-tui model".to_string(),
        Some(Command::AuthTest { .. }) => "ssc-tui auth-test".to_string(),
        Some(Command::Restart { .. }) => "ssc-tui restart".to_string(),
        Some(Command::SetupLauncher) => "ssc-tui setup-launcher".to_string(),
        None => {
            if let Some(resume) = args.resume.as_deref().filter(|resume| !resume.is_empty()) {
                let prefix = if crate::cli::selfdev::client_selfdev_requested() {
                    "stui:d:"
                } else {
                    "stui:c:"
                };
                compact_process_title(prefix, Some(&session_name(resume)))
            } else if crate::cli::selfdev::client_selfdev_requested() {
                "stui:selfdev".to_string()
            } else {
                "stui:client".to_string()
            }
        }
    }
}

pub(crate) fn set_initial_title(args: &Args) {
    set_title(initial_title(args));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::Args;
    use crate::storage::lock_test_env;
    use clap::Parser;

    const SELFDEV_ENV: &str = crate::cli::selfdev::CLIENT_SELFDEV_ENV;

    fn with_selfdev_env_removed<T>(f: impl FnOnce() -> T) -> T {
        let _guard = lock_test_env();
        let previous = std::env::var_os(SELFDEV_ENV);
        crate::env::remove_var(SELFDEV_ENV);
        let result = f();
        if let Some(value) = previous {
            crate::env::set_var(SELFDEV_ENV, value);
        }
        result
    }

    #[test]
    fn initial_title_labels_server() {
        with_selfdev_env_removed(|| {
            let args = Args::parse_from(["jcode", "serve"]);
            assert_eq!(initial_title(&args), "ssc-tui:server");
        });
    }

    #[test]
    fn initial_title_labels_resume_client_with_short_name() {
        with_selfdev_env_removed(|| {
            let args = Args::parse_from(["jcode", "--resume", "session_fox_123"]);
            assert_eq!(initial_title(&args), "stui:c:fox");
        });
    }

    #[test]
    fn terminal_session_label_includes_custom_title_and_short_name() {
        assert_eq!(
            terminal_session_label("fox", Some("Release planning")),
            "Release planning (fox)"
        );
        assert_eq!(terminal_session_label("fox", Some("Fox")), "Fox");
        assert_eq!(terminal_session_label("fox", None), "Fox");
    }

    #[test]
    fn terminal_session_label_for_id_reads_custom_title_from_session() {
        let _guard = lock_test_env();
        let previous_home = std::env::var_os("JCODE_HOME");
        let temp = tempfile::tempdir().expect("temp dir");
        crate::env::set_var("JCODE_HOME", temp.path());

        let mut session = crate::session::Session::create_with_id(
            "session_fox_123".to_string(),
            None,
            Some("Generated title".to_string()),
        );
        session.rename_title(Some("Release planning".to_string()));
        session.save().expect("save session");

        assert_eq!(
            terminal_session_label_for_id("session_fox_123"),
            "Release planning (fox)"
        );

        if let Some(previous_home) = previous_home {
            crate::env::set_var("JCODE_HOME", previous_home);
        } else {
            crate::env::remove_var("JCODE_HOME");
        }
    }

    #[test]
    fn initial_title_labels_selfdev_command() {
        with_selfdev_env_removed(|| {
            let args = Args::parse_from(["jcode", "self-dev"]);
            assert_eq!(initial_title(&args), "stui:selfdev");
        });
    }
}
