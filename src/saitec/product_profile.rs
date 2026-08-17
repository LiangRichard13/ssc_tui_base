//! Stage 2B (chore/ssc-tui-baseline): product restrictions lifted.
//!
//! The function bodies in this module now return permissive defaults
//! (all `is_allowed_*` queries return true, all `show_*` / `use_*` toggles
//! return the same value upstream jcode would have returned). Call sites
//! still compile against the same public symbols, so src/saitec/ stays
//! coherent until stage 2 subtask D removes it wholesale.
//!
//! The previous inline #[cfg(test)] module is also dropped here: it
//! asserted SAITEC-specific rules (grape brand, SAITEC-only base-model
//! allowlist, Kimi-only openai-compatible behaviour) that no longer hold.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandVisibility {
    Public,
    HiddenCompatible,
    InternalOnly,
}

pub fn brand_header_label() -> &'static str {
    "jcode"
}

pub fn show_skills_in_ui() -> bool {
    true
}

pub fn emphasize_mcp_status() -> bool {
    false
}

pub fn expose_generic_openrouter_routes() -> bool {
    true
}

pub fn prefer_text_startup_logo() -> bool {
    false
}

pub fn use_fixed_shell_layout() -> bool {
    false
}

pub fn show_external_resume_sources() -> bool {
    true
}

pub fn allowed_base_model_provider_ids() -> &'static [&'static str] {
    &[
        "openai",
        "claude",
        "openrouter",
        "zai",
        "kimi",
        "alibaba-coding-plan",
        "openai-compatible",
        "bedrock",
        "vertex",
        "azure",
        "copilot",
        "antigravity",
        "gemini",
        "cursor",
    ]
}

pub fn is_allowed_base_model_provider(_provider_id: &str) -> bool {
    true
}

pub fn is_allowed_openai_compatible_profile(_profile_id: &str) -> bool {
    true
}

pub fn is_allowed_base_model_route(
    _outer_provider: &str,
    _model: &str,
    _provider: &str,
    _api_method: &str,
) -> bool {
    true
}

pub fn unsupported_base_model_provider_message() -> String {
    "This provider is not currently supported.".to_string()
}

pub fn unsupported_base_model_route_message(model: &str) -> String {
    format!("Model `{}` is not available.", model.trim())
}

pub fn command_visibility(_command: &str) -> CommandVisibility {
    CommandVisibility::Public
}

pub fn public_commands() -> &'static [&'static str] {
    &[
        "/help", "/?", "/commands", "/login", "/logout", "/auth", "/model",
        "/models", "/clear", "/resume", "/sessions", "/export", "/usage",
        "/version", "/quit", "/download-latest",
    ]
}

/// Stage 2B: provider catalog now returns the union directly; this shim
/// only exists so the symbol stays available for the few in-tree callers
/// in `src/tui/app/state_ui_input_helpers.rs` and `src/tui/ui_header.rs`
/// that previously fed off SAITEC's curated visible list. They will be
/// replaced by the live catalog call in stage 2 subtask B-2 follow-up.
pub fn saitec_visible_base_model_providers() -> Vec<String> {
    Vec::new()
}
