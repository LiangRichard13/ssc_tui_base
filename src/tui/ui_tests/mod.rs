use super::*;
use crate::tui::session_picker;
use crate::tui::ui::tools_ui;
use std::sync::{Mutex, OnceLock};

fn viewport_snapshot_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn buffer_to_text(terminal: &ratatui::Terminal<ratatui::backend::TestBackend>) -> Vec<String> {
    let buf = terminal.backend().buffer();
    let width = buf.area.width as usize;
    let height = buf.area.height as usize;
    let mut lines = Vec::with_capacity(height);
    for y in 0..height {
        let mut line = String::with_capacity(width);
        for x in 0..width {
            line.push_str(buf[(x as u16, y as u16)].symbol());
        }
        lines.push(line);
    }
    lines
}

#[test]
fn parse_changelog_from_supports_timestamped_entries() {
    let changelog = concat!(
        "abc123\x1ev1.2.2\x1e1711234500\x1eCut release\x1f",
        "def456\x1e\x1e1711234600\x1eFix follow-up"
    );

    let entries = parse_changelog_from(changelog);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].hash, "abc123");
    assert_eq!(entries[0].tag, "v1.2.2");
    assert_eq!(entries[0].timestamp, Some(1711234500));
    assert_eq!(entries[0].subject, "Cut release");
    assert_eq!(entries[1].timestamp, Some(1711234600));
}

#[test]
fn group_changelog_entries_includes_release_times() {
    let entries = vec![
        ChangelogEntry {
            hash: "aaa111",
            tag: "",
            timestamp: Some(1711235600),
            subject: "Latest unreleased fix",
        },
        ChangelogEntry {
            hash: "bbb222",
            tag: "v1.2.2",
            timestamp: Some(1711234500),
            subject: "Cut release",
        },
        ChangelogEntry {
            hash: "ccc333",
            tag: "",
            timestamp: Some(1711234400),
            subject: "Earlier release commit",
        },
    ];

    let groups = group_changelog_entries(&entries, "v1.2.3 (deadbee)", "2024-03-23 16:46:40 +0000");

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].version, "v1.2.3 (unreleased)");
    assert_eq!(
        groups[0].released_at.as_deref(),
        Some("2024-03-23 16:46 UTC")
    );
    assert_eq!(groups[0].entries, vec!["Latest unreleased fix"]);

    assert_eq!(groups[1].version, "v1.2.2");
    assert_eq!(
        groups[1].released_at.as_deref(),
        Some("2024-03-23 22:55 UTC")
    );
    assert_eq!(
        groups[1].entries,
        vec!["Cut release", "Earlier release commit"]
    );
}

#[test]
fn parse_changelog_from_supports_legacy_entries_without_timestamps() {
    let entries = parse_changelog_from("abc123:v1.2.2:Legacy entry");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].hash, "abc123");
    assert_eq!(entries[0].tag, "v1.2.2");
    assert_eq!(entries[0].timestamp, None);
    assert_eq!(entries[0].subject, "Legacy entry");
}

#[test]
fn split_native_scrollbar_area_reserves_one_column_when_enabled() {
    let (content, scrollbar) = split_native_scrollbar_area(Rect::new(3, 4, 20, 8), true);
    assert_eq!(content, Rect::new(3, 4, 19, 8));
    assert_eq!(scrollbar, Some(Rect::new(22, 4, 1, 8)));
}

#[test]
fn split_native_scrollbar_area_skips_tiny_regions() {
    let (content, scrollbar) = split_native_scrollbar_area(Rect::new(1, 2, 1, 5), true);
    assert_eq!(content, Rect::new(1, 2, 1, 5));
    assert!(scrollbar.is_none());
}

#[test]
fn left_aligned_content_inset_only_applies_when_not_centered() {
    assert_eq!(left_aligned_content_inset(40, true), 0);
    assert_eq!(left_aligned_content_inset(40, false), 1);
    assert_eq!(left_aligned_content_inset(1, false), 0);
}

#[test]
fn native_scrollbar_visibility_requires_overflow() {
    assert!(!native_scrollbar_visible(false, 20, 5));
    assert!(!native_scrollbar_visible(true, 0, 5));
    assert!(!native_scrollbar_visible(true, 5, 5));
    assert!(!native_scrollbar_visible(true, 4, 5));
    assert!(native_scrollbar_visible(true, 6, 5));
}

#[test]
fn startup_splash_footer_renders_on_bottom_row() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState::default();

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("startup splash draw should succeed");

    let layout = last_layout_snapshot().expect("missing layout snapshot");
    let lines = buffer_to_text(&terminal);
    let footer_row = lines
        .iter()
        .rposition(|line| line.contains(semver()))
        .expect("missing footer row");

    assert_eq!(
        footer_row,
        (layout.messages_area.y + layout.messages_area.height).saturating_sub(1) as usize,
        "footer row should render on the bottom of the messages area, got row {footer_row}"
    );
}

#[test]
fn startup_splash_shows_prompt_above_footer() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState::default();

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("startup splash draw should succeed");

    let layout = last_layout_snapshot().expect("missing layout snapshot");
    let lines = buffer_to_text(&terminal);
    let prompt_row = lines
        .iter()
        .rposition(|line| line.contains("> "))
        .expect("missing startup prompt row");
    let footer_row = lines
        .iter()
        .rposition(|line| line.contains(semver()))
        .expect("missing footer row");

    assert!(
        prompt_row < footer_row,
        "prompt should render above footer, got prompt row {prompt_row}, footer row {footer_row}"
    );
}

#[test]
fn startup_splash_keeps_prompt_close_to_logo() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState::default();

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("startup splash draw should succeed");

    let layout = last_layout_snapshot().expect("missing layout snapshot");
    let lines = buffer_to_text(&terminal);
    let prompt_row = lines
        .iter()
        .rposition(|line| line.contains("> "))
        .expect("missing startup prompt row");
    let footer_row = lines
        .iter()
        .rposition(|line| line.contains(semver()))
        .expect("missing footer row");
    let logo_bottom_row = lines
        .iter()
        .rposition(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.contains("> ") && !trimmed.contains(semver())
        })
        .expect("missing startup logo rows");

    assert!(
        prompt_row <= logo_bottom_row + 2,
        "prompt should sit directly below the logo, got logo bottom row {logo_bottom_row}, prompt row {prompt_row}"
    );
    assert!(
        prompt_row < footer_row,
        "footer should remain below the prompt, got prompt row {prompt_row}, footer row {footer_row}"
    );
}

#[test]
fn startup_splash_shows_login_tab_completion_hint_before_first_message() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        input: "/login".to_string(),
        cursor_pos: "/login".len(),
        command_suggestions: vec![
            (
                "/login".to_string(),
                "Choose SAITEC login or base-model configuration",
            ),
            (
                "/login jcode".to_string(),
                crate::provider_catalog::JCODE_LOGIN_PROVIDER.menu_detail,
            ),
            (
                "/login base-models".to_string(),
                "Open the filtered base-model provider picker",
            ),
        ],
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("startup splash draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(
        rendered.contains("/login - Choose SAITEC login or base-model configuration"),
        "startup splash should show the /login completion hint when /login is typed before the first message: {rendered}"
    );
    assert!(
        rendered.contains("Tab: +2 more"),
        "startup splash should show the Tab cycling hint for additional /login completions: {rendered}"
    );
    assert!(
        !rendered.contains('›'),
        "startup splash should avoid the Windows-hostile prompt glyph: {rendered}"
    );
}

#[test]
fn startup_splash_multiple_suggestions_use_ascii_separator() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        input: "/lo".to_string(),
        cursor_pos: "/lo".len(),
        command_suggestions: vec![
            ("/login".to_string(), "Login"),
            ("/logout".to_string(), "Logout"),
            ("/logs".to_string(), "Logs"),
        ],
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("startup splash draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(
        rendered.contains("/login (Login)"),
        "startup splash should show the first suggestion, got: {rendered}"
    );
    assert!(
        rendered.contains(" | /logout"),
        "startup splash should use an ASCII separator between suggestions, got: {rendered}"
    );
    assert!(
        !rendered.contains("鈹?"),
        "startup splash should not show corrupted separator text, got: {rendered}"
    );
}

#[test]
fn startup_splash_new_session_hint_uses_ascii_arrow() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        input: "draft prompt".to_string(),
        cursor_pos: "draft prompt".len(),
        next_prompt_new_session_armed: true,
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("startup splash draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(
        rendered.contains("-> Next prompt opens a new session"),
        "startup splash should show the ASCII new-session hint, got: {rendered}"
    );
    assert!(
        !rendered.contains("鈫?"),
        "startup splash should not show corrupted arrow text, got: {rendered}"
    );
}

fn sample_model_picker() -> crate::tui::InlineInteractiveState {
    crate::tui::InlineInteractiveState {
        kind: crate::tui::PickerKind::Model,
        entries: vec![crate::tui::PickerEntry {
            name: "kimi-k2-test".to_string(),
            options: vec![crate::tui::PickerOption {
                provider: "Kimi".to_string(),
                api_method: "openai-compatible".to_string(),
                available: true,
                detail: "configured".to_string(),
                estimated_reference_cost_micros: None,
            }],
            action: crate::tui::PickerAction::Model,
            selected_option: 0,
            is_current: true,
            is_default: false,
            recommended: false,
            recommendation_rank: usize::MAX,
            old: false,
            created_date: None,
            effort: None,
        }],
        filtered: vec![0],
        selected: 0,
        column: 0,
        filter: String::new(),
        preview: false,
    }
}

#[test]
fn startup_splash_does_not_hide_inline_model_picker() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        inline_interactive_state: Some(sample_model_picker()),
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("inline picker draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(
        rendered.contains("kimi-k2-test"),
        "startup splash should not hide an active inline model picker: {rendered}"
    );
}

#[test]
fn startup_splash_renders_png_logo_without_external_asset_file() {
    let _guard = viewport_snapshot_test_lock();
    let _env_guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(temp.path()).expect("set current dir");

    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState::default();

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("startup splash draw should succeed");

    std::env::set_current_dir(prev_cwd).expect("restore current dir");

    let lines = buffer_to_text(&terminal);
    let fallback_logo_lines = crate::tui::ui::header::startup_logo_lines(80);
    let rendered = lines.join("\n");

    for fallback in fallback_logo_lines {
        assert!(
            !lines.iter().any(|line| line.trim() == fallback.trim()),
            "startup splash should render the PNG logo instead of the text fallback line `{fallback}`: {rendered}"
        );
    }
}

#[test]
fn remote_error_message_suppresses_startup_splash() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        remote_startup_phase_active: true,
        display_messages: vec![DisplayMessage::error(
            "Validation failed for Kimi Code.".to_string(),
        )],
        messages_version: 1,
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("remote error draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(
        rendered.contains("Validation failed for Kimi Code."),
        "remote error feedback should remain visible instead of being hidden behind the startup splash: {rendered}"
    );
}

#[test]
fn startup_splash_footer_shows_three_segments() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(100, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        auth_status: crate::auth::AuthStatus {
            openai: crate::auth::AuthState::Available,
            openai_has_api_key: true,
            ..Default::default()
        },
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("startup splash draw should succeed");

    let lines = buffer_to_text(&terminal);
    let footer = lines
        .iter()
        .find(|line| line.contains(semver()))
        .expect("missing footer row");

    assert!(footer.contains("Not Logged In"), "footer: {footer}");
    assert!(footer.contains("Model Configured"), "footer: {footer}");
    assert!(footer.contains('●'), "footer: {footer}");
    assert!(footer.contains(semver()), "footer: {footer}");
}

#[test]
fn conversation_screen_keeps_branded_footer_fixed_while_messages_grow() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(100, 28);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        display_messages: vec![
            DisplayMessage::user("hello"),
            DisplayMessage::assistant("world"),
        ],
        messages_version: 2,
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("conversation draw should succeed");

    let layout = last_layout_snapshot().expect("missing layout snapshot");
    let lines = buffer_to_text(&terminal);
    let footer_row = lines
        .iter()
        .rposition(|line| line.contains(semver()))
        .expect("missing footer row during conversation");
    let message_row = lines
        .iter()
        .position(|line| line.contains("world"))
        .expect("missing assistant message");

    assert_eq!(
        footer_row,
        (layout.messages_area.y + layout.messages_area.height).saturating_sub(1) as usize,
        "footer should stay pinned to the bottom row even during conversation, got row {footer_row}"
    );
    assert!(
        message_row < footer_row,
        "messages should render above the fixed footer, got message row {message_row} and footer row {footer_row}"
    );
    assert!(
        layout.messages_area.y > 0,
        "conversation layout should reserve a fixed branded region above the message viewport"
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("mock-model") || line.contains("/model to switch")),
        "conversation screen should not fall back to the old chat header chrome"
    );
}

#[test]
fn saitec_login_overlay_renders_masked_password_over_startup_splash() {
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        input: "secret".to_string(),
        cursor_pos: "secret".len(),
        pending_saitec_login_form: Some(crate::tui::app::SaitecPendingForm {
            form: crate::saitec::auth::SaitecLoginForm::new(
                "user@example.com".to_string(),
                "".to_string(),
                "secret".to_string(),
            ),
            focus: crate::tui::app::SaitecLoginField::Password,
            error: Some("password cannot be empty".to_string()),
            submitting: false,
        }),
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("saitec login overlay draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(rendered.contains("Saitec Login"), "rendered: {rendered}");
    assert!(rendered.contains("Email"), "rendered: {rendered}");
    assert!(rendered.contains("Phone"), "rendered: {rendered}");
    assert!(rendered.contains("Password"), "rendered: {rendered}");
    assert!(
        rendered.contains("user@example.com"),
        "rendered: {rendered}"
    );
    assert!(rendered.contains("******"), "rendered: {rendered}");
}

#[test]
fn saitec_login_overlay_hides_live_password_from_startup_prompt() {
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        input: "secret-password".to_string(),
        cursor_pos: "secret-password".len(),
        pending_saitec_login_form: Some(crate::tui::app::SaitecPendingForm {
            form: crate::saitec::auth::SaitecLoginForm::new(
                "user@example.com".to_string(),
                "".to_string(),
                "".to_string(),
            ),
            focus: crate::tui::app::SaitecLoginField::Password,
            error: None,
            submitting: false,
        }),
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("saitec login overlay draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(
        !rendered.contains("secret-password"),
        "startup prompt should not leak the live password while the login form is focused: {rendered}"
    );
    assert!(rendered.contains("***************"), "rendered: {rendered}");
}

#[test]
fn saitec_login_overlay_uses_live_input_and_moves_cursor_into_overlay() {
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        input: "13900139000".to_string(),
        cursor_pos: "13900139000".len(),
        pending_saitec_login_form: Some(crate::tui::app::SaitecPendingForm {
            form: crate::saitec::auth::SaitecLoginForm::new(
                "user@example.com".to_string(),
                "".to_string(),
                "".to_string(),
            ),
            focus: crate::tui::app::SaitecLoginField::Phone,
            error: None,
            submitting: false,
        }),
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("saitec login overlay draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(rendered.contains("13900139000"), "rendered: {rendered}");

    let cursor = terminal
        .backend_mut()
        .get_cursor_position()
        .expect("cursor position should be available");
    let layout = last_layout_snapshot().expect("missing layout snapshot");
    let prompt_area = layout.input_area.expect("missing prompt area");
    assert!(
        cursor.y > prompt_area.y + prompt_area.height.saturating_sub(1),
        "cursor should move into the login overlay instead of staying on the startup prompt: cursor={cursor:?}, prompt_area={prompt_area:?}"
    );
}

#[test]
fn saitec_login_overlay_does_not_render_placeholder_asterisk_for_empty_password() {
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        input: String::new(),
        cursor_pos: 0,
        pending_saitec_login_form: Some(crate::tui::app::SaitecPendingForm {
            form: crate::saitec::auth::SaitecLoginForm::new(
                "user@example.com".to_string(),
                "".to_string(),
                "".to_string(),
            ),
            focus: crate::tui::app::SaitecLoginField::Password,
            error: None,
            submitting: false,
        }),
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("saitec login overlay draw should succeed");

    let lines = buffer_to_text(&terminal);
    let password_row = lines
        .iter()
        .find(|line| line.contains("Password"))
        .expect("password row should be present");

    assert!(
        !password_row.contains('*'),
        "empty password row should not render a placeholder asterisk: {password_row}"
    );
}

#[test]
fn saitec_login_overlay_wraps_long_error_messages() {
    let backend = ratatui::backend::TestBackend::new(72, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let long_error = "Saitec login failed: Invalid credentials. Please verify whether the email or phone matches the account and try again with the correct password.".to_string();
    let state = TestState {
        pending_saitec_login_form: Some(crate::tui::app::SaitecPendingForm {
            form: crate::saitec::auth::SaitecLoginForm::new(
                "user@example.com".to_string(),
                "".to_string(),
                "secret".to_string(),
            ),
            focus: crate::tui::app::SaitecLoginField::Password,
            error: Some(long_error.clone()),
            submitting: false,
        }),
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("saitec login overlay draw should succeed");

    let lines = buffer_to_text(&terminal);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Saitec login failed:")),
        "rendered output should include the beginning of the long login error: {}",
        lines.join("\n")
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("email or phone matches")),
        "rendered output should wrap and include the middle of the long login error: {}",
        lines.join("\n")
    );
    assert!(
        lines.iter().any(|line| line.contains("correct"))
            && lines.iter().any(|line| line.contains("password.")),
        "rendered output should include the tail of the long login error: {}",
        lines.join("\n")
    );
}

#[test]
fn saitec_login_overlay_stays_visible_after_chat_history_exists() {
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        display_messages: vec![DisplayMessage::user("hello")],
        pending_saitec_login_form: Some(crate::tui::app::SaitecPendingForm {
            form: crate::saitec::auth::SaitecLoginForm::new(
                "user@example.com".to_string(),
                "".to_string(),
                "".to_string(),
            ),
            focus: crate::tui::app::SaitecLoginField::Email,
            error: None,
            submitting: false,
        }),
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("saitec login overlay draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(rendered.contains("Saitec Login"), "rendered: {rendered}");
    assert!(rendered.contains("Email"), "rendered: {rendered}");
    assert!(rendered.contains("[ Submit ]"), "rendered: {rendered}");
    assert!(rendered.contains("[ Cancel ]"), "rendered: {rendered}");
}

#[test]
fn saitec_login_overlay_hides_live_password_from_conversation_input() {
    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        input: "secret-password".to_string(),
        cursor_pos: "secret-password".len(),
        display_messages: vec![DisplayMessage::user("hello")],
        pending_saitec_login_form: Some(crate::tui::app::SaitecPendingForm {
            form: crate::saitec::auth::SaitecLoginForm::new(
                "user@example.com".to_string(),
                "".to_string(),
                "".to_string(),
            ),
            focus: crate::tui::app::SaitecLoginField::Password,
            error: None,
            submitting: false,
        }),
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("saitec login overlay draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(
        !rendered.contains("secret-password"),
        "conversation input should not leak the live password while the login form is focused: {rendered}"
    );
    assert!(rendered.contains("***************"), "rendered: {rendered}");
}

#[derive(Clone, Default)]
struct TestState {
    input: String,
    cursor_pos: usize,
    display_messages: Vec<DisplayMessage>,
    messages_version: u64,
    streaming_text: String,
    batch_progress: Option<crate::bus::BatchProgress>,
    queued_messages: Vec<String>,
    pending_soft_interrupts: Vec<String>,
    interleave_message: Option<String>,
    status: ProcessingStatus,
    queue_mode: bool,
    active_skill: Option<String>,
    centered_mode: bool,
    anim_elapsed: f32,
    time_since_activity: Option<Duration>,
    remote_startup_phase_active: bool,
    inline_view_state: Option<crate::tui::InlineViewState>,
    inline_interactive_state: Option<crate::tui::InlineInteractiveState>,
    changelog_scroll: Option<usize>,
    help_scroll: Option<usize>,
    chat_native_scrollbar: bool,
    auth_status: crate::auth::AuthStatus,
    pending_saitec_login_form: Option<crate::tui::app::SaitecPendingForm>,
    command_suggestions: Vec<(String, &'static str)>,
    next_prompt_new_session_armed: bool,
}

impl crate::tui::TuiState for TestState {
    fn display_messages(&self) -> &[DisplayMessage] {
        &self.display_messages
    }
    fn display_user_message_count(&self) -> usize {
        self.display_messages
            .iter()
            .filter(|message| message.role == "user")
            .count()
    }
    fn has_display_edit_tool_messages(&self) -> bool {
        self.display_messages.iter().any(|message| {
            message
                .tool_data
                .as_ref()
                .map(|tool| tools_ui::is_edit_tool_name(&tool.name))
                .unwrap_or(false)
        })
    }
    fn side_pane_images(&self) -> Vec<crate::session::RenderedImage> {
        Vec::new()
    }
    fn display_messages_version(&self) -> u64 {
        self.messages_version
    }
    fn streaming_text(&self) -> &str {
        &self.streaming_text
    }
    fn input(&self) -> &str {
        &self.input
    }
    fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }
    fn is_processing(&self) -> bool {
        !matches!(self.status, ProcessingStatus::Idle)
    }
    fn queued_messages(&self) -> &[String] {
        &self.queued_messages
    }
    fn interleave_message(&self) -> Option<&str> {
        self.interleave_message.as_deref()
    }
    fn pending_soft_interrupts(&self) -> &[String] {
        &self.pending_soft_interrupts
    }
    fn scroll_offset(&self) -> usize {
        0
    }
    fn auto_scroll_paused(&self) -> bool {
        false
    }
    fn provider_name(&self) -> String {
        "mock".to_string()
    }
    fn provider_model(&self) -> String {
        "mock-model".to_string()
    }
    fn upstream_provider(&self) -> Option<String> {
        None
    }
    fn connection_type(&self) -> Option<String> {
        None
    }
    fn status_detail(&self) -> Option<String> {
        None
    }
    fn mcp_servers(&self) -> Vec<(String, usize)> {
        Vec::new()
    }
    fn available_skills(&self) -> Vec<String> {
        Vec::new()
    }
    fn streaming_tokens(&self) -> (u64, u64) {
        (0, 0)
    }
    fn streaming_cache_tokens(&self) -> (Option<u64>, Option<u64>) {
        (None, None)
    }
    fn output_tps(&self) -> Option<f32> {
        None
    }
    fn streaming_tool_calls(&self) -> Vec<ToolCall> {
        Vec::new()
    }
    fn elapsed(&self) -> Option<Duration> {
        None
    }
    fn status(&self) -> ProcessingStatus {
        self.status.clone()
    }
    fn command_suggestions(&self) -> Vec<(String, &'static str)> {
        self.command_suggestions.clone()
    }
    fn active_skill(&self) -> Option<String> {
        self.active_skill.clone()
    }
    fn subagent_status(&self) -> Option<String> {
        None
    }
    fn batch_progress(&self) -> Option<crate::bus::BatchProgress> {
        self.batch_progress.clone()
    }
    fn time_since_activity(&self) -> Option<Duration> {
        self.time_since_activity
    }
    fn total_session_tokens(&self) -> Option<(u64, u64)> {
        None
    }
    fn is_remote_mode(&self) -> bool {
        false
    }
    fn is_canary(&self) -> bool {
        false
    }
    fn is_replay(&self) -> bool {
        false
    }
    fn diff_mode(&self) -> crate::config::DiffDisplayMode {
        crate::config::DiffDisplayMode::Inline
    }
    fn current_session_id(&self) -> Option<String> {
        None
    }
    fn session_display_name(&self) -> Option<String> {
        None
    }
    fn server_display_name(&self) -> Option<String> {
        None
    }
    fn server_display_icon(&self) -> Option<String> {
        None
    }
    fn server_sessions(&self) -> Vec<String> {
        Vec::new()
    }
    fn connected_clients(&self) -> Option<usize> {
        None
    }
    fn status_notice(&self) -> Option<String> {
        None
    }
    fn remote_startup_phase_active(&self) -> bool {
        self.remote_startup_phase_active
    }
    fn pending_saitec_login_form(&self) -> Option<&crate::tui::app::SaitecPendingForm> {
        self.pending_saitec_login_form.as_ref()
    }
    fn dictation_key_label(&self) -> Option<String> {
        None
    }
    fn animation_elapsed(&self) -> f32 {
        self.anim_elapsed
    }
    fn rate_limit_remaining(&self) -> Option<Duration> {
        None
    }
    fn queue_mode(&self) -> bool {
        self.queue_mode
    }
    fn next_prompt_new_session_armed(&self) -> bool {
        self.next_prompt_new_session_armed
    }
    fn has_stashed_input(&self) -> bool {
        false
    }
    fn context_info(&self) -> crate::prompt::ContextInfo {
        Default::default()
    }
    fn context_limit(&self) -> Option<usize> {
        None
    }
    fn client_update_available(&self) -> bool {
        false
    }
    fn server_update_available(&self) -> Option<bool> {
        None
    }
    fn info_widget_data(&self) -> info_widget::InfoWidgetData {
        Default::default()
    }
    fn render_streaming_markdown(&self, _width: usize) -> Vec<Line<'static>> {
        markdown::render_markdown_with_width(&self.streaming_text, Some(_width))
    }
    fn centered_mode(&self) -> bool {
        self.centered_mode
    }
    fn auth_status(&self) -> crate::auth::AuthStatus {
        self.auth_status.clone()
    }
    fn update_cost(&mut self) {}
    fn diagram_mode(&self) -> crate::config::DiagramDisplayMode {
        Default::default()
    }
    fn diagram_focus(&self) -> bool {
        false
    }
    fn diagram_index(&self) -> usize {
        0
    }
    fn diagram_scroll(&self) -> (i32, i32) {
        (0, 0)
    }
    fn diagram_pane_ratio(&self) -> u8 {
        50
    }
    fn diagram_pane_animating(&self) -> bool {
        false
    }
    fn diagram_pane_enabled(&self) -> bool {
        false
    }
    fn diagram_pane_position(&self) -> crate::config::DiagramPanePosition {
        Default::default()
    }
    fn diagram_zoom(&self) -> u8 {
        100
    }
    fn diff_pane_scroll(&self) -> usize {
        0
    }
    fn diff_pane_scroll_x(&self) -> i32 {
        0
    }
    fn side_panel_image_zoom_percent(&self) -> u8 {
        100
    }
    fn diff_pane_focus(&self) -> bool {
        false
    }
    fn side_panel(&self) -> &crate::side_panel::SidePanelSnapshot {
        static EMPTY: std::sync::LazyLock<crate::side_panel::SidePanelSnapshot> =
            std::sync::LazyLock::new(crate::side_panel::SidePanelSnapshot::default);
        &EMPTY
    }
    fn pin_images(&self) -> bool {
        false
    }
    fn diff_line_wrap(&self) -> bool {
        true
    }
    fn inline_interactive_state(&self) -> Option<&crate::tui::InlineInteractiveState> {
        self.inline_interactive_state.as_ref()
    }
    fn inline_view_state(&self) -> Option<&crate::tui::InlineViewState> {
        self.inline_view_state.as_ref()
    }
    fn changelog_scroll(&self) -> Option<usize> {
        self.changelog_scroll
    }
    fn help_scroll(&self) -> Option<usize> {
        self.help_scroll
    }
    fn session_picker_overlay(&self) -> Option<&std::cell::RefCell<session_picker::SessionPicker>> {
        None
    }
    fn login_picker_overlay(
        &self,
    ) -> Option<&std::cell::RefCell<crate::tui::login_picker::LoginPicker>> {
        None
    }
    fn account_picker_overlay(
        &self,
    ) -> Option<&std::cell::RefCell<crate::tui::account_picker::AccountPicker>> {
        None
    }
    fn usage_overlay(
        &self,
    ) -> Option<&std::cell::RefCell<crate::tui::usage_overlay::UsageOverlay>> {
        None
    }
    fn working_dir(&self) -> Option<String> {
        None
    }
    fn now_millis(&self) -> u64 {
        0
    }
    fn copy_badge_ui(&self) -> crate::tui::CopyBadgeUiState {
        Default::default()
    }
    fn copy_selection_mode(&self) -> bool {
        false
    }
    fn copy_selection_range(&self) -> Option<crate::tui::CopySelectionRange> {
        None
    }
    fn copy_selection_status(&self) -> Option<crate::tui::CopySelectionStatus> {
        None
    }
    fn suggestion_prompts(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    fn cache_ttl_status(&self) -> Option<crate::tui::CacheTtlInfo> {
        None
    }
    fn chat_native_scrollbar(&self) -> bool {
        self.chat_native_scrollbar
    }
    fn side_panel_native_scrollbar(&self) -> bool {
        false
    }
}

fn reset_prompt_viewport_state_for_test() {
    TEST_PROMPT_VIEWPORT_STATE.with(|state| {
        *state.borrow_mut() = PromptViewportState::default();
    });
}

#[path = "basic.rs"]
mod basic;
#[path = "diagrams.rs"]
mod diagrams;
#[path = "prepare.rs"]
mod prepared_messages_tests;
#[path = "rendering.rs"]
mod rendering;
#[path = "tools.rs"]
mod tools;
