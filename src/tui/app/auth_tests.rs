use super::{
    App, antigravity_input_requires_state_validation, save_tui_openai_compatible_api_base,
    save_tui_openai_compatible_key,
};
use crate::provider::Provider;
use crate::tui::TuiState;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use std::ffi::OsString;
use std::sync::{Arc, Mutex};

struct MockProvider;

#[derive(Clone)]
struct ActivationSpecCaptureProvider {
    set_model_calls: Arc<Mutex<Vec<String>>>,
    routes: Vec<crate::provider::ModelRoute>,
}

#[derive(Clone)]
struct AuthChangedCaptureProvider {
    calls: Arc<Mutex<usize>>,
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        crate::env::set_var(key, value);
        Self { key, previous }
    }

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

#[async_trait::async_trait]
impl Provider for ActivationSpecCaptureProvider {
    async fn complete(
        &self,
        _messages: &[crate::message::Message],
        _tools: &[crate::message::ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<crate::provider::EventStream> {
        Err(anyhow::anyhow!(
            "ActivationSpecCaptureProvider should not stream completions in auth tests"
        ))
    }

    fn name(&self) -> &str {
        "activation-spec-capture"
    }

    fn model(&self) -> String {
        self.set_model_calls
            .lock()
            .expect("set_model calls")
            .last()
            .cloned()
            .unwrap_or_else(|| "gpt-5.5".to_string())
    }

    fn model_routes(&self) -> Vec<crate::provider::ModelRoute> {
        self.routes.clone()
    }

    async fn refresh_model_catalog(&self) -> Result<crate::provider::ModelCatalogRefreshSummary> {
        Ok(crate::provider::ModelCatalogRefreshSummary::default())
    }

    fn set_model(&self, model: &str) -> Result<()> {
        self.set_model_calls
            .lock()
            .expect("set_model calls")
            .push(model.to_string());
        Ok(())
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(self.clone())
    }
}

#[async_trait::async_trait]
impl Provider for AuthChangedCaptureProvider {
    async fn complete(
        &self,
        _messages: &[crate::message::Message],
        _tools: &[crate::message::ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<crate::provider::EventStream> {
        Err(anyhow::anyhow!(
            "AuthChangedCaptureProvider should not stream completions in auth tests"
        ))
    }

    fn name(&self) -> &str {
        "auth-changed-capture"
    }

    fn on_auth_changed(&self) {
        *self.calls.lock().expect("auth changed calls") += 1;
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(self.clone())
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

fn create_auth_changed_capture_app() -> (App, Arc<Mutex<usize>>) {
    let calls = Arc::new(Mutex::new(0));
    let provider: Arc<dyn Provider> = Arc::new(AuthChangedCaptureProvider {
        calls: Arc::clone(&calls),
    });
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let registry = rt.block_on(crate::tool::Registry::new(provider.clone()));
    let mut app = App::new_for_test_harness(provider, registry);
    app.queue_mode = false;
    (app, calls)
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

fn create_activation_spec_capture_test_app(
    routes: Vec<crate::provider::ModelRoute>,
) -> (App, Arc<Mutex<Vec<String>>>) {
    let set_model_calls = Arc::new(Mutex::new(Vec::new()));
    let provider: Arc<dyn Provider> = Arc::new(ActivationSpecCaptureProvider {
        set_model_calls: set_model_calls.clone(),
        routes,
    });
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let registry = rt.block_on(crate::tool::Registry::new(provider.clone()));
    let mut app = App::new_for_test_harness(provider, registry);
    app.queue_mode = false;
    app.diff_mode = crate::config::DiffDisplayMode::Inline;
    (app, set_model_calls)
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
fn openai_compatible_key_save_does_not_mark_hosted_provider_validated_by_presence_only()
-> anyhow::Result<()> {
    let _env_guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());

    let resolved =
        save_tui_openai_compatible_key(crate::provider_catalog::ZAI_PROFILE, "zai-test-key")?;
    assert_eq!(resolved.id, "zai");
    assert!(
        crate::provider_catalog::openai_compatible_profile_is_configured(
            crate::provider_catalog::ZAI_PROFILE
        ),
        "saving the key should still count as configured"
    );
    assert!(
        crate::auth::validation::get("zai").is_none(),
        "saving a hosted API key should not silently fabricate a successful validation record"
    );
    Ok(())
}

#[test]
fn failed_openai_compatible_validation_keeps_saved_key_flow_closed_and_records_failure() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    crate::auth::AuthStatus::invalidate_cache();

    let mut app = create_test_app();
    app.start_login_provider(crate::provider_catalog::ZAI_LOGIN_PROVIDER);
    let pending = app.pending_login.take().expect("pending Z.AI login");
    app.handle_login_input(pending, "zai-test-key".to_string());

    app.handle_login_completed(crate::bus::LoginCompleted {
        provider: "zai".to_string(),
        success: false,
        message: "Post-login validation failed for Z.AI. Credentials were saved, but jcode could not verify runtime readiness.".to_string(),
    });

    assert!(
        app.pending_login.is_none(),
        "runtime validation failure after saving a hosted API key should not reopen the API-key prompt"
    );
    assert_eq!(
        app.status_notice(),
        Some("Validation: Z.AI failed".to_string())
    );
    assert!(
        app.display_messages()
            .iter()
            .any(|message| message.role == "error"
                && message.content.contains("Post-login validation failed")),
        "validation failure should be surfaced as an error message"
    );
}

#[test]
fn failed_openai_compatible_activation_keeps_saved_key_flow_closed_and_records_failure() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    crate::auth::AuthStatus::invalidate_cache();

    let mut app = create_test_app();
    app.start_login_provider(crate::provider_catalog::KIMI_LOGIN_PROVIDER);
    let pending = app.pending_login.take().expect("pending Kimi Code login");
    app.handle_login_input(pending, "kimi-test-key".to_string());

    app.handle_login_completed(crate::bus::LoginCompleted {
        provider: "Kimi Code".to_string(),
        success: false,
        message: "Fetched the model catalog, but it contained no selectable Kimi Code models and failed to switch to the documented default `kimi-for-coding`: This provider does not support model switching".to_string(),
    });

    assert!(
        app.pending_login.is_none(),
        "post-save model activation failure should not reopen the API-key prompt"
    );
    assert_eq!(
        app.status_notice(),
        Some("Validation: Kimi Code failed".to_string())
    );
    assert!(
        app.display_messages().iter().any(|message| {
            message.role == "error"
                && message
                    .content
                    .contains("contained no selectable Kimi Code models")
        }),
        "model activation failure should be surfaced as an error message"
    );
}

#[test]
fn provider_validation_completion_refreshes_open_login_picker_status() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());

    crate::provider_catalog::save_env_value_to_env_file(
        "ZHIPU_API_KEY",
        "zai.env",
        Some("zai-test-key"),
    )
    .expect("save Z.AI key");

    let mut app = create_test_app();
    app.open_saitec_base_model_login_picker();
    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("move selection to OpenAI");
    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("move selection to Z.AI");

    crate::auth::validation::save(
        "zai",
        crate::auth::validation::ProviderValidationRecord {
            checked_at_ms: chrono::Utc::now().timestamp_millis(),
            success: true,
            provider_smoke_ok: Some(true),
            tool_smoke_ok: Some(true),
            validated_models: Vec::new(),
            summary: "tool_smoke: AUTH_TEST_OK".to_string(),
        },
    )
    .expect("save passing validation");

    super::local::handle_bus_event(
        &mut app,
        Ok(crate::bus::BusEvent::ProviderValidationCompleted(
            crate::bus::ProviderValidationCompleted {
                provider: "zai".to_string(),
                provider_display_name: "Z.AI".to_string(),
                success: true,
                message: "Runtime validation passed for Z.AI.".to_string(),
            },
        )),
    );

    assert!(
        app.login_picker_overlay.is_some(),
        "revalidation should refresh the open picker instead of closing it"
    );
    assert_eq!(
        app.status_notice(),
        Some("Validation: Z.AI ready".to_string())
    );
    assert!(
        app.display_messages()
            .iter()
            .any(|message| message.role == "system"
                && message
                    .content
                    .contains("Runtime validation passed for Z.AI")),
        "successful revalidation should be surfaced to the user"
    );
}

#[test]
fn post_login_validation_failure_does_not_reopen_base_model_picker() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    crate::auth::AuthStatus::invalidate_cache();

    let mut app = create_test_app();
    app.start_login_provider(crate::provider_catalog::KIMI_LOGIN_PROVIDER);
    let pending = app.pending_login.take().expect("pending Kimi Code login");
    app.handle_login_input(pending, "kimi-test-key".to_string());

    assert!(
        app.login_picker_overlay.is_none(),
        "submitting the API-key form should not leave the base-model picker open"
    );

    super::local::handle_bus_event(
        &mut app,
        Ok(crate::bus::BusEvent::ProviderValidationCompleted(
            crate::bus::ProviderValidationCompleted {
                provider: "kimi".to_string(),
                provider_display_name: "Kimi Code".to_string(),
                success: false,
                message: "Fetched the model catalog, but it contained no selectable Kimi Code models and failed to switch to the documented default `kimi-for-coding`: This provider does not support model switching".to_string(),
            },
        )),
    );

    assert!(
        app.login_picker_overlay.is_none(),
        "post-login validation failure should not reopen the base-model picker"
    );
    assert_eq!(
        app.status_notice(),
        Some("Validation: Kimi Code failed".to_string())
    );
    assert!(
        app.display_messages().iter().any(|message| {
            message.role == "error"
                && message
                    .content
                    .contains("contained no selectable Kimi Code models")
        }),
        "post-login validation failure should still surface an error message"
    );
}

#[test]
fn failed_text_entry_login_does_not_preserve_branded_startup_surface_after_error() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    crate::auth::AuthStatus::invalidate_cache();

    let mut app = create_test_app();
    app.start_login_provider(crate::provider_catalog::KIMI_LOGIN_PROVIDER);
    let pending = app.pending_login.take().expect("pending Kimi Code login");
    app.handle_login_input(pending, "kimi-test-key".to_string());

    super::local::handle_bus_event(
        &mut app,
        Ok(crate::bus::BusEvent::ProviderValidationCompleted(
            crate::bus::ProviderValidationCompleted {
                provider: "kimi".to_string(),
                provider_display_name: "Kimi Code".to_string(),
                success: false,
                message: "Fetched the model catalog, but it contained no selectable Kimi Code models and failed to switch to the documented default `kimi-for-coding`: This provider does not support model switching".to_string(),
            },
        )),
    );

    assert!(
        !TuiState::preserve_branded_startup_surface(&app),
        "once an API-key login surfaces an error message, the branded startup splash should not keep hiding it"
    );
}

#[test]
fn openai_compatible_post_login_activation_uses_provider_prefixed_kimi_spec() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let (mut app, set_model_calls) =
        create_activation_spec_capture_test_app(vec![crate::provider::ModelRoute {
            model: "kimi-for-coding".to_string(),
            provider: "Kimi Code".to_string(),
            api_method: "openai-compatible:kimi".to_string(),
            available: true,
            detail: "https://api.kimi.com/coding/v1".to_string(),
            cheapness: None,
        }]);
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let _enter = runtime.enter();

    app.start_openai_compatible_post_login_activation("Kimi Code".to_string());

    let start = std::time::Instant::now();
    loop {
        if !set_model_calls.lock().expect("set_model calls").is_empty() {
            break;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "timed out waiting for post-login activation to select a model"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(
        set_model_calls.lock().expect("set_model calls").as_slice(),
        ["kimi:kimi-for-coding"]
    );
}

#[test]
fn openai_compatible_post_login_activation_prefers_documented_kimi_default_over_unrelated_route() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let (mut app, set_model_calls) =
        create_activation_spec_capture_test_app(vec![crate::provider::ModelRoute {
            model: "anthropic/claude-sonnet-4".to_string(),
            provider: "OpenRouter".to_string(),
            api_method: "openai-compatible:openrouter".to_string(),
            available: true,
            detail: "https://openrouter.ai/api/v1".to_string(),
            cheapness: None,
        }]);
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let _enter = runtime.enter();

    app.start_openai_compatible_post_login_activation("Kimi Code".to_string());

    let start = std::time::Instant::now();
    loop {
        if !set_model_calls.lock().expect("set_model calls").is_empty() {
            break;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "timed out waiting for post-login activation to select Kimi default"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(
        set_model_calls.lock().expect("set_model calls").as_slice(),
        ["kimi:kimi-for-coding"]
    );
}

#[test]
fn remote_openai_compatible_post_login_activation_queues_documented_default_model() {
    let mut app = App::new_for_remote(None);
    app.start_openai_compatible_post_login_activation("Kimi Code".to_string());

    assert_eq!(
        app.pending_model_switch.as_deref(),
        Some("kimi:kimi-for-coding")
    );
}

#[test]
fn saitec_provider_openai_compatible_post_login_activation_selects_kimi_default() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let _kimi_key = EnvVarGuard::set("KIMI_API_KEY", "test-kimi-key");
    crate::subscription_catalog::clear_runtime_env();
    crate::provider_catalog::force_apply_openai_compatible_profile_env(None);

    let provider = Arc::new(crate::provider::jcode::JcodeProvider::new());
    let provider_for_app: Arc<dyn Provider> = provider.clone();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let registry = runtime.block_on(crate::tool::Registry::new(provider_for_app.clone()));
    let mut app = App::new_for_test_harness(provider_for_app, registry);
    app.queue_mode = false;
    let _enter = runtime.enter();

    app.start_openai_compatible_post_login_activation("Kimi Code".to_string());

    let start = std::time::Instant::now();
    loop {
        if provider.model() == "kimi-for-coding" {
            break;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "timed out waiting for SAITEC provider to activate Kimi default; current model={}",
            provider.model()
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(provider.name(), "OpenRouter");
    assert!(!crate::subscription_catalog::is_runtime_mode_enabled());
    crate::provider_catalog::force_apply_openai_compatible_profile_env(None);
    crate::subscription_catalog::clear_runtime_env();
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
fn saitec_pending_login_repeated_up_continues_history_recall_after_first_match() {
    let mut app = create_test_app();
    app.input = "first prompt".to_string();
    app.submit_input();
    app.input = "/login jcode".to_string();
    app.submit_input();

    app.input.clear();
    app.cursor_pos = 0;

    app.handle_key(KeyCode::Up, KeyModifiers::empty())
        .expect("first up should recall the newest history entry");
    assert_eq!(app.input(), "/login jcode");

    app.handle_key(KeyCode::Up, KeyModifiers::empty())
        .expect("second up should continue walking backward inside the login form");
    assert_eq!(app.input(), "first prompt");
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

    assert!(
        app.pending_login.is_none(),
        "API-key login should be dismissed by esc"
    );
    assert_eq!(app.input(), "");
    let last = app
        .display_messages()
        .last()
        .expect("missing cancellation message");
    assert!(last.content.contains("Login cancelled."));
}

#[test]
fn api_key_login_down_then_enter_activates_validate_button() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let mut app = create_test_app();
    app.set_pending_api_key_login_for_tests("zai", "Z.AI", "ZAI_API_KEY");
    app.input = "secret-api-key".to_string();
    app.cursor_pos = app.input.len();

    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("down should move focus to clear");
    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("second down should move focus to validate");
    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("enter on validate should submit the overlay");

    assert!(
        app.pending_login.is_none(),
        "API-key login should be submitted by the validate button"
    );
    let saved = std::fs::read_to_string(
        crate::storage::app_config_dir()
            .expect("config dir")
            .join("test.env"),
    )
    .expect("saved env file");
    assert!(
        saved.contains("ZAI_API_KEY=secret-api-key"),
        "validate button should save the API key, got:\n{saved}"
    );
}

#[test]
fn api_key_login_down_twice_then_enter_activates_cancel_button() {
    let mut app = create_test_app();
    app.set_pending_api_key_login_for_tests("zai", "Z.AI", "ZAI_API_KEY");
    app.input = "secret-api-key".to_string();
    app.cursor_pos = app.input.len();

    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("first down should move focus to clear");
    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("second down should move focus to validate");
    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("third down should move focus to cancel");
    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("enter on cancel should dismiss the overlay");

    assert!(
        app.pending_login.is_none(),
        "API-key login should be dismissed by the cancel button"
    );
    assert_eq!(app.input(), "");
    let last = app
        .display_messages()
        .last()
        .expect("missing cancellation message");
    assert!(last.content.contains("Login cancelled."));
}

#[test]
fn api_key_login_prefills_saved_key_when_reopened() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let _key_env = EnvVarGuard::remove("KIMI_API_KEY");
    save_tui_openai_compatible_key(crate::provider_catalog::KIMI_PROFILE, "saved-kimi-key")
        .expect("save Kimi key");

    let mut app = create_test_app();
    app.input = "/account kimi login".to_string();
    app.submit_input();

    match app.pending_login.as_ref() {
        Some(super::auth::PendingLogin::ApiKeyProfile {
            provider_id,
            provider,
            ..
        }) => {
            assert_eq!(provider_id, "kimi");
            assert_eq!(provider, "Kimi Code");
        }
        other => panic!("expected Kimi API-key login, got: {other:?}"),
    }
    assert_eq!(app.input(), "saved-kimi-key");
    assert_eq!(app.cursor_pos, "saved-kimi-key".len());
}

#[test]
fn api_key_login_clear_button_removes_saved_key_and_keeps_form_open() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let _key_env = EnvVarGuard::remove("KIMI_API_KEY");
    let resolved =
        save_tui_openai_compatible_key(crate::provider_catalog::KIMI_PROFILE, "saved-kimi-key")
            .expect("save Kimi key");

    let mut app = create_test_app();
    app.input = "/account kimi login".to_string();
    app.submit_input();
    assert_eq!(app.input(), "saved-kimi-key");

    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("down should focus clear");
    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("enter on clear should clear the saved key");

    assert_eq!(app.input(), "");
    assert_eq!(app.cursor_pos, 0);
    match app.pending_login.as_ref() {
        Some(super::auth::PendingLogin::ApiKeyProfile { provider_id, .. }) => {
            assert_eq!(provider_id, "kimi");
        }
        other => panic!("clear should keep the Kimi API-key login open, got: {other:?}"),
    }
    assert!(
        crate::provider_catalog::load_env_value_from_env_or_config(
            &resolved.api_key_env,
            &resolved.env_file
        )
        .is_none(),
        "clear should remove the saved API key"
    );
    let saved = std::fs::read_to_string(
        crate::storage::app_config_dir()
            .expect("config dir")
            .join(&resolved.env_file),
    )
    .expect("saved env file");
    assert!(
        !saved.contains("KIMI_API_KEY="),
        "clear should remove the env-file entry, got:\n{saved}"
    );
}

#[test]
fn api_key_login_submit_does_not_enter_submitted_input_history() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let mut app = create_test_app();

    app.input = "first prompt".to_string();
    app.submit_input();

    app.set_pending_api_key_login_for_tests("zai", "Z.AI", "ZAI_API_KEY");
    app.input = "secret-api-key".to_string();
    app.cursor_pos = app.input.len();
    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("down should move focus to clear");
    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("second down should move focus to validate");
    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("enter on validate should submit the overlay");

    app.handle_key(KeyCode::Up, KeyModifiers::empty())
        .expect("history recall should still work after login");
    assert_eq!(
        app.input(),
        "first prompt",
        "secondary form input should not be stored in submitted input history"
    );
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
fn saitec_login_success_does_not_refresh_base_model_provider() {
    let _guard = crate::storage::lock_test_env();
    crate::subscription_catalog::clear_runtime_env();
    crate::subscription_catalog::apply_runtime_env();
    assert!(crate::subscription_catalog::is_runtime_mode_enabled());

    let (mut app, calls) = create_auth_changed_capture_app();

    app.handle_login_completed(crate::bus::LoginCompleted {
        provider: "jcode".to_string(),
        success: true,
        message: "Saitec login successful.".to_string(),
    });

    assert_eq!(
        *calls.lock().expect("auth changed calls"),
        0,
        "SAITEC login grants MCP permissions and must not mutate the base-model provider"
    );
    assert!(!crate::subscription_catalog::is_runtime_mode_enabled());
    assert!(std::env::var_os("JCODE_OPENROUTER_API_BASE").is_none());
    assert!(std::env::var_os("JCODE_OPENROUTER_API_KEY_NAME").is_none());
    assert_eq!(app.status_notice(), Some("Login: jcode ready".to_string()));

    crate::subscription_catalog::clear_runtime_env();
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
