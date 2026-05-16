use super::{
    App, antigravity_input_requires_state_validation, save_tui_openai_compatible_api_base,
    save_tui_openai_compatible_key,
};
use crate::provider::Provider;
use crate::tui::TuiState;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use std::ffi::OsString;
use std::sync::Arc;

struct MockProvider;

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set_path(key: &'static str, value: &std::path::Path) -> Self {
        let previous = std::env::var_os(key);
        crate::env::set_var(key, value);
        Self { key, previous }
    }

    fn remove(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        crate::env::remove_var(key);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(ref value) = self.previous {
            crate::env::set_var(self.key, value);
        } else {
            crate::env::remove_var(self.key);
        }
    }
}

#[async_trait::async_trait]
impl Provider for MockProvider {
    async fn complete(
        &self,
        _messages: &[crate::message::Message],
        _tools: &[crate::message::ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<crate::provider::EventStream> {
        Err(anyhow::anyhow!(
            "Mock provider should not stream completions in auth tests"
        ))
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(Self)
    }
}

fn create_test_app() -> App {
    let provider: Arc<dyn Provider> = Arc::new(MockProvider);
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let registry = rt.block_on(crate::tool::Registry::new(provider.clone()));
    let mut app = App::new_for_test_harness(provider, registry);
    app.queue_mode = false;
    app.diff_mode = crate::config::DiffDisplayMode::Inline;
    app
}

fn create_isolated_test_app() -> App {
    let provider: Arc<dyn Provider> = Arc::new(MockProvider);
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let registry = rt.block_on(crate::tool::Registry::new(provider.clone()));
    let mut app = App::new_for_test_harness(provider, registry);
    app.queue_mode = false;
    app.diff_mode = crate::config::DiffDisplayMode::Inline;
    app
}

#[test]
fn antigravity_auto_callback_code_skips_manual_callback_parser() {
    assert!(!antigravity_input_requires_state_validation(
        "raw_authorization_code",
        Some("expected_state")
    ));
}

#[test]
fn antigravity_manual_callback_url_keeps_state_validation() {
    assert!(antigravity_input_requires_state_validation(
        "http://127.0.0.1:51121/oauth-callback?code=abc&state=expected_state",
        Some("expected_state")
    ));
}

#[test]
fn oauth_preflight_mentions_browser_fallback_and_doctor() {
    let message = App::record_oauth_preflight("openai", false, Some("localhost:1455"), Some(true));
    assert!(message.contains("could not open a browser"));
    assert!(message.contains("auth doctor openai"));
}

#[test]
fn oauth_preflight_mentions_manual_safe_callback_mode() {
    let message = App::record_oauth_preflight(
        "gemini",
        true,
        Some("http://127.0.0.1:0/oauth2callback"),
        Some(false),
    );
    assert!(message.contains("manual-safe paste completion"));
    assert!(message.contains("oauth2callback"));
}

#[test]
fn tui_openai_compatible_api_base_accepts_localhost_override() -> anyhow::Result<()> {
    let _env_guard = crate::storage::lock_test_env();
    let resolved = save_tui_openai_compatible_api_base("http://localhost:11434/v1")?;
    assert_eq!(resolved.api_base, "http://localhost:11434/v1");
    assert!(!resolved.requires_api_key);
    Ok(())
}

#[test]
fn tui_openai_compatible_local_key_save_allows_empty_key() -> anyhow::Result<()> {
    let _env_guard = crate::storage::lock_test_env();
    let resolved = save_tui_openai_compatible_key(crate::provider_catalog::OLLAMA_PROFILE, "")?;
    assert_eq!(resolved.api_base, "http://localhost:11434/v1");
    assert!(
        crate::provider_catalog::openai_compatible_profile_is_configured(
            crate::provider_catalog::OLLAMA_PROFILE
        )
    );
    assert!(
        crate::provider_catalog::load_api_key_from_env_or_config(
            &resolved.api_key_env,
            &resolved.env_file,
        )
        .is_none()
    );
    Ok(())
}

#[test]
fn saitec_pending_login_empty_submit_keeps_form_and_shows_validation_error() {
    let mut app = create_test_app();
    app.set_pending_saitec_login_for_tests();

    app.submit_input();

    match app.pending_login {
        Some(super::auth::PendingLogin::SaitecForm { ref form }) => {
            let error = form.error.as_deref().expect("validation error");
            assert!(error.contains("password"), "unexpected error: {error}");
            assert_eq!(form.focus, super::auth::SaitecLoginField::Submit);
            assert!(!form.submitting);
        }
        ref other => panic!("login form should stay pending after validation failure: {other:?}"),
    }
    assert!(
        TuiState::preserve_branded_startup_surface(&app),
        "validation error should keep the branded startup splash visible"
    );
}

#[test]
fn saitec_pending_login_tab_and_backtab_commit_and_restore_field_values() {
    let mut app = create_test_app();
    app.set_pending_saitec_login_for_tests();
    app.input = "user@example.com".to_string();
    app.cursor_pos = app.input.len();

    app.handle_key(KeyCode::Tab, KeyModifiers::empty())
        .expect("tab should move focus");

    match app.pending_login {
        Some(super::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(form.focus, super::auth::SaitecLoginField::Phone);
            assert_eq!(form.form.email, "user@example.com");
            assert_eq!(form.form.phone, "");
            assert_eq!(app.input, "");
        }
        ref other => panic!("expected saitec form after tab navigation: {other:?}"),
    }

    app.input = "13900139000".to_string();
    app.cursor_pos = app.input.len();
    app.handle_key(KeyCode::BackTab, KeyModifiers::empty())
        .expect("backtab should move focus backward");

    match app.pending_login {
        Some(super::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(form.focus, super::auth::SaitecLoginField::Email);
            assert_eq!(form.form.email, "user@example.com");
            assert_eq!(form.form.phone, "13900139000");
            assert_eq!(app.input, "user@example.com");
        }
        ref other => panic!("expected saitec form after reverse navigation: {other:?}"),
    }
}

#[test]
fn saitec_pending_login_enter_uses_keyboard_submit_path() {
    let mut app = create_test_app();
    app.set_pending_saitec_login_form_for_tests(
        crate::saitec::auth::SaitecLoginForm::new(
            "user@example.com".to_string(),
            "".to_string(),
            "".to_string(),
        ),
        super::auth::SaitecLoginField::Password,
        None,
        false,
    );
    app.input = "".to_string();
    app.cursor_pos = 0;

    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("enter should submit the saitec form");

    match app.pending_login {
        Some(super::auth::PendingLogin::SaitecForm { ref form }) => {
            let error = form.error.as_deref().expect("validation error");
            assert!(error.contains("password"), "unexpected error: {error}");
            assert_eq!(form.focus, super::auth::SaitecLoginField::Submit);
            assert_eq!(form.form.email, "user@example.com");
            assert_eq!(form.form.phone, "");
            assert_eq!(form.form.password, "");
            assert!(!form.submitting);
        }
        ref other => panic!("keyboard submit should keep the saitec form pending: {other:?}"),
    }
}

#[test]
fn saitec_pending_login_escape_closes_form() {
    let mut app = create_test_app();
    app.set_pending_saitec_login_for_tests();
    app.input = "user@example.com".to_string();
    app.cursor_pos = app.input.len();

    app.handle_key(KeyCode::Esc, KeyModifiers::empty())
        .expect("esc should close the saitec form");

    assert!(
        app.pending_login.is_none(),
        "saitec login form should be dismissed by esc"
    );
    let last = app
        .display_messages()
        .last()
        .expect("missing cancellation message");
    assert!(last.content.contains("Login cancelled."));
}

#[test]
fn saitec_pending_login_cancel_button_exits_form() {
    let mut app = create_test_app();
    app.set_pending_saitec_login_form_for_tests(
        crate::saitec::auth::SaitecLoginForm::new(
            "user@example.com".to_string(),
            "".to_string(),
            "secret-password".to_string(),
        ),
        super::auth::SaitecLoginField::Cancel,
        None,
        false,
    );
    app.input.clear();
    app.cursor_pos = 0;

    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("enter on cancel should close the saitec form");

    assert!(
        app.pending_login.is_none(),
        "saitec login form should be dismissed by cancel button"
    );
    let last = app
        .display_messages()
        .last()
        .expect("missing cancellation message");
    assert!(last.content.contains("Login cancelled."));
}

#[test]
fn saitec_pending_login_up_down_navigation_restores_active_field_input() {
    let mut app = create_test_app();
    app.set_pending_saitec_login_for_tests();
    app.input = "user@example.com".to_string();
    app.cursor_pos = app.input.len();

    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("down should move focus to phone");

    match app.pending_login {
        Some(super::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(form.focus, super::auth::SaitecLoginField::Phone);
            assert_eq!(form.form.email, "user@example.com");
            assert_eq!(app.input, "");
        }
        ref other => panic!("expected saitec form after down navigation: {other:?}"),
    }

    app.input = "13900139000".to_string();
    app.cursor_pos = app.input.len();
    app.handle_key(KeyCode::Up, KeyModifiers::empty())
        .expect("up should move focus back to email");

    match app.pending_login {
        Some(super::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(form.focus, super::auth::SaitecLoginField::Email);
            assert_eq!(form.form.email, "user@example.com");
            assert_eq!(form.form.phone, "13900139000");
            assert_eq!(app.input, "user@example.com");
        }
        ref other => panic!("expected saitec form after up navigation: {other:?}"),
    }
}

#[test]
fn saitec_pending_login_up_recalls_last_submitted_login_command_when_form_is_empty() {
    let mut app = create_test_app();
    app.input = "/login jcode".to_string();
    app.submit_input();

    app.input.clear();
    app.cursor_pos = 0;

    app.handle_key(KeyCode::Up, KeyModifiers::empty())
        .expect("up should recall last submitted input");

    assert_eq!(app.input(), "/login jcode");
}

#[test]
fn failed_plain_message_submission_can_be_recalled_with_up_arrow() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let _api_key = EnvVarGuard::remove(crate::subscription_catalog::JCODE_API_KEY_ENV);
    let mut app = create_isolated_test_app();
    app.input = "hello failed send".to_string();
    app.submit_input();

    assert_eq!(app.input(), "");

    app.handle_key(KeyCode::Up, KeyModifiers::empty())
        .expect("up should recall last submitted input after failed send");

    assert_eq!(app.input(), "hello failed send");
}

#[test]
fn recalled_failed_plain_message_returns_to_fresh_input_on_down_arrow() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let _api_key = EnvVarGuard::remove(crate::subscription_catalog::JCODE_API_KEY_ENV);
    let mut app = create_isolated_test_app();
    app.input = "hello failed send".to_string();
    app.submit_input();

    app.handle_key(KeyCode::Up, KeyModifiers::empty())
        .expect("up should recall last submitted input after failed send");
    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("down should leave recall mode");

    assert_eq!(app.input(), "");
}

#[test]
fn repeated_up_arrow_walks_back_through_multiple_history_entries() {
    let mut app = create_test_app();
    app.input = "first prompt".to_string();
    app.submit_input();
    app.input = "second prompt".to_string();
    app.submit_input();
    app.input = "third prompt".to_string();
    app.submit_input();

    app.handle_key(KeyCode::Up, KeyModifiers::empty())
        .expect("first up should recall the newest history entry");
    assert_eq!(app.input(), "third prompt");

    app.handle_key(KeyCode::Up, KeyModifiers::empty())
        .expect("second up should continue walking backward");
    assert_eq!(app.input(), "second prompt");

    app.handle_key(KeyCode::Up, KeyModifiers::empty())
        .expect("third up should reach the oldest entry");
    assert_eq!(app.input(), "first prompt");
}

#[test]
fn api_key_login_escape_closes_text_entry_overlay() {
    let mut app = create_test_app();
    app.set_pending_api_key_login_for_tests("zai", "Z.AI", "ZAI_API_KEY");
    app.input = "secret-api-key".to_string();
    app.cursor_pos = app.input.len();

    app.handle_key(KeyCode::Esc, KeyModifiers::empty())
        .expect("esc should close the API-key login overlay");

    assert!(app.pending_login.is_none(), "API-key login should be dismissed by esc");
    assert_eq!(app.input(), "");
    let last = app
        .display_messages()
        .last()
        .expect("missing cancellation message");
    assert!(last.content.contains("Login cancelled."));
}

#[test]
fn kimi_api_key_login_success_keeps_chat_input_available() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let mut app = create_test_app();

    app.start_login_provider(crate::provider_catalog::KIMI_LOGIN_PROVIDER);
    let pending = app.pending_login.take().expect("pending Kimi login");
    app.handle_login_input(pending, "kimi-test-key".to_string());

    assert!(
        app.pending_login.is_none(),
        "successful Kimi login should finish the pending login flow"
    );
    assert!(
        app.inline_interactive_state.is_none(),
        "successful Kimi login should not trap the user inside the model picker"
    );

    app.handle_key(KeyCode::Char('h'), KeyModifiers::empty())
        .expect("typing should still reach the chat input");
    assert_eq!(app.input(), "h");
}

#[test]
fn login_jcode_command_shows_visible_saitec_login_prompt_message() {
    let mut app = create_test_app();
    app.input = "/login jcode".to_string();
    app.submit_input();

    let last = app.display_messages().last().expect("missing login prompt");
    assert_eq!(last.role, "system");
    assert!(last.content.contains("Saitec Login"));
    assert!(last.content.contains("email or phone plus password"));
}

#[test]
fn saitec_login_failure_keeps_form_editable_on_password_field() {
    let mut app = create_test_app();
    app.set_pending_saitec_login_form_for_tests(
        crate::saitec::auth::SaitecLoginForm::new(
            "user@example.com".to_string(),
            "".to_string(),
            "secret-password".to_string(),
        ),
        super::auth::SaitecLoginField::Submit,
        None,
        true,
    );
    app.input.clear();
    app.cursor_pos = 0;

    app.handle_login_completed(crate::bus::LoginCompleted {
        provider: "jcode".to_string(),
        success: false,
        message: "Saitec login failed: Invalid credentials".to_string(),
    });

    match app.pending_login {
        Some(super::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(form.focus, super::auth::SaitecLoginField::Password);
            assert_eq!(form.form.email, "user@example.com");
            assert_eq!(form.form.password, "secret-password");
            assert!(!form.submitting);
            let error = form.error.as_deref().expect("login error should be shown");
            assert!(
                error.contains("Invalid credentials"),
                "unexpected error: {error}"
            );
            assert_eq!(app.input, "secret-password");
            assert_eq!(app.cursor_pos, "secret-password".len());
        }
        ref other => panic!("expected editable saitec form after login failure: {other:?}"),
    }
}
