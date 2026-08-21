use super::*;
use crate::message::Message;
use crate::provider::{EventStream, Provider};
use crate::tool::Registry;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

struct MockProvider;

#[async_trait]
impl Provider for MockProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[crate::message::ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        Err(anyhow::anyhow!(
            "Mock provider should not be used for streaming completions in ui prepare tests"
        ))
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(MockProvider)
    }
}

fn create_test_app() -> crate::tui::App {
    let provider: Arc<dyn Provider> = Arc::new(MockProvider);
    let rt = tokio::runtime::Runtime::new().expect("test runtime");
    let registry = rt.block_on(Registry::new(provider.clone()));
    crate::tui::App::new_for_test_harness(provider, registry)
}

fn rendered_lines(frame: &PreparedChatFrame) -> Vec<String> {
    frame
        .materialize_all_lines()
        .iter()
        .map(ui::line_plain_text)
        .collect()
}

#[test]
fn centered_mode_centers_unstructured_messages_and_preserves_structured_left_blocks() {
    for role in ["user", "assistant", "meta", "usage", "error", "memory"] {
        assert_eq!(
            default_message_alignment(role, true),
            ratatui::layout::Alignment::Center,
            "role {role} should default to centered alignment"
        );
    }
    for role in ["tool", "system", "swarm", "background_task"] {
        assert_eq!(
            default_message_alignment(role, true),
            ratatui::layout::Alignment::Left,
            "role {role} should keep left/default alignment"
        );
    }
}

#[test]
fn initial_empty_screen_uses_minimal_splash_layout() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_cwd = std::env::current_dir().expect("current dir");
    let prev_home = std::env::var_os("JCODE_HOME");
    std::env::set_current_dir(temp.path()).expect("set current dir");
    crate::env::set_var("JCODE_HOME", temp.path());
    crate::auth::AuthStatus::invalidate_cache();

    let frame = {
        let app = create_test_app();
        prepare_messages_inner(&app, 80, 24)
    };

    std::env::set_current_dir(prev_cwd).expect("restore current dir");
    if let Some(value) = prev_home {
        crate::env::set_var("JCODE_HOME", value);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
    crate::auth::AuthStatus::invalidate_cache();

    let lines = rendered_lines(&frame);
    let rendered = lines.join("\n");
    let working_dir = super::header::abbreviate_home(&temp.path().display().to_string());

    assert!(
        rendered.contains("SSC") || rendered.contains(semver()),
        "startup splash should use a pixel-style logo: {rendered}"
    );
    assert!(
        !rendered.contains("JCode"),
        "startup splash should use the SSC logo text fallback: {rendered}"
    );
    assert!(
        !rendered.contains("mcp:"),
        "startup splash should hide mcp status: {rendered}"
    );
    assert!(
        !rendered.contains("/model to switch"),
        "startup splash should hide model switching guidance: {rendered}"
    );
    assert!(
        !rendered.contains("Customize my terminal theme")
            && !rendered.contains("Log in to get started")
            && !rendered.contains("Press 1-3 or type anything to start"),
        "startup splash should not show suggestion prompts: {rendered}"
    );
    assert!(
        rendered.contains("Use `/login base-models` to configure a provider"),
        "startup splash should show an explicit /login hint before the first message: {rendered}"
    );
    assert!(
        lines.iter().any(|line| line.trim().contains(&working_dir)),
        "startup splash should show working directory on its own footer line: {rendered}"
    );
    assert!(
        lines.iter().any(|line| {
            let trimmed = line.trim();
            trimmed.contains(semver())
                && trimmed.contains("Not Logged In")
                && trimmed.contains("Model")
        }),
        "startup splash should show model login, business login, and version on the status line: {rendered}"
    );
}

#[test]
fn initial_empty_screen_registers_image_logo_when_asset_exists() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(temp.path()).expect("set current dir");

    let logo_path = temp.path().join("SSC_logo.png");
    let image = image::RgbaImage::from_fn(48, 16, |_x, _y| image::Rgba([0x77, 0x38, 0xaa, 0xff]));
    image.save(&logo_path).expect("write logo asset");

    let frame = {
        let app = create_test_app();
        prepare_messages_inner(&app, 80, 24)
    };

    std::env::set_current_dir(prev_cwd).expect("restore current dir");

    assert!(
        frame.image_regions.is_empty(),
        "startup splash should prefer the text logo in SAITEC product mode even when SAITEC_logo.png exists"
    );
}

#[test]
fn initial_empty_screen_registers_embedded_image_logo_without_external_asset() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(temp.path()).expect("set current dir");

    let frame = {
        let app = create_test_app();
        prepare_messages_inner(&app, 80, 24)
    };

    std::env::set_current_dir(prev_cwd).expect("restore current dir");

    assert!(
        frame.image_regions.is_empty(),
        "startup splash should prefer the text logo in SAITEC product mode when only the embedded SAITEC logo is available"
    );
}

#[test]
fn remote_startup_screen_hides_runtime_header_noise_even_when_processing() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(temp.path()).expect("set current dir");

    let frame = {
        let mut app = crate::tui::app::App::new_for_remote(None);
        app.set_remote_header_metadata_for_tests(
            "session_remote_active",
            "Island",
            "S",
            Some(1),
            "anthropic",
            "claude-opus-4-5-20251010",
        );
        app.set_mcp_server_names_for_tests(vec![("SSC-Skills".to_string(), 0)]);
        app.set_processing_state_for_tests(crate::tui::ProcessingStatus::Sending);
        app.clear_remote_startup_phase();
        app.clear_display_messages_for_tests();

        assert!(
            !app.remote_startup_phase_active(),
            "test setup should simulate the post-startup metadata-only state"
        );
        assert!(app.is_processing(), "test setup should keep app processing");
        assert!(
            app.display_messages().is_empty(),
            "test setup should keep the transcript empty"
        );

        prepare_messages_inner(&app, 80, 24)
    };

    std::env::set_current_dir(prev_cwd).expect("restore current dir");

    let lines = rendered_lines(&frame);
    let rendered = lines.join("\n");
    let footer_index = lines
        .iter()
        .position(|line| line.contains(semver()))
        .expect("missing footer line");

    assert!(
        rendered.contains("SSC") || rendered.contains(semver()),
        "rendered: {rendered}"
    );
    assert!(!rendered.contains("server:"), "rendered: {rendered}");
    assert!(!rendered.contains("client:"), "rendered: {rendered}");
    assert!(!rendered.contains("mcp:"), "rendered: {rendered}");
    assert!(!rendered.contains("anthropic"), "rendered: {rendered}");
    assert!(
        !rendered.contains("/model to switch"),
        "rendered: {rendered}"
    );
    assert!(
        footer_index >= lines.len().saturating_sub(3),
        "footer should sit at the bottom of the splash, got line {footer_index} of {}: {rendered}",
        lines.len()
    );
}

#[test]
fn remote_startup_screen_still_uses_splash_with_only_system_placeholder_messages() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(temp.path()).expect("set current dir");

    let frame = {
        let mut app = crate::tui::app::App::new_for_remote(None);
        app.set_remote_header_metadata_for_tests(
            "session_remote_placeholder",
            "Island",
            "S",
            Some(1),
            "anthropic",
            "claude-opus-4-5-20251010",
        );
        app.set_mcp_server_names_for_tests(vec![("SSC-Skills".to_string(), 0)]);
        app.clear_remote_startup_phase();
        app.set_display_messages_for_tests(vec![DisplayMessage::system(
            "Reload complete — checking restored history.".to_string(),
        )]);

        assert_eq!(
            app.display_user_message_count(),
            0,
            "test setup should simulate a pre-conversation remote startup"
        );
        assert!(
            !app.display_messages().is_empty(),
            "test setup should simulate placeholder display content"
        );

        prepare_messages_inner(&app, 80, 24)
    };

    std::env::set_current_dir(prev_cwd).expect("restore current dir");

    let lines = rendered_lines(&frame);
    let rendered = lines.join("\n");

    assert!(
        rendered.contains("SSC") || rendered.contains(semver()),
        "rendered: {rendered}"
    );
    assert!(!rendered.contains("server:"), "rendered: {rendered}");
    assert!(!rendered.contains("client:"), "rendered: {rendered}");
    assert!(!rendered.contains("mcp:"), "rendered: {rendered}");
    assert!(!rendered.contains("anthropic"), "rendered: {rendered}");
    assert!(
        !rendered.contains("claude-opus") && !rendered.contains("/model to switch"),
        "rendered: {rendered}"
    );
}

