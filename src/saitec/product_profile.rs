#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandVisibility {
    Public,
    HiddenCompatible,
    InternalOnly,
}

const PUBLIC_COMMANDS: &[&str] = &[
    "/help",
    "/?",
    "/commands",
    "/login",
    "/logout",
    "/auth",
    "/model",
    "/models",
    "/clear",
    "/resume",
    "/sessions",
    "/export",
    "/usage",
    "/version",
    "/quit",
];

const HIDDEN_COMPATIBLE_COMMANDS: &[&str] = &[
    "/git",
    "/selfdev",
    "/feedback",
    "/subscription",
    "/review",
    "/judge",
    "/swarm",
    "/memory",
    "/refactor",
    "/improve",
    "/autoreview",
    "/autojudge",
    "/observe",
    "/subagent",
    "/workspace",
    "/catchup",
    "/back",
    "/splitview",
    "/split-view",
    "/split",
    "/transfer",
    "/rebuild",
    "/restart",
    "/reload",
];

const ALLOWED_BASE_MODEL_PROVIDER_IDS: &[&str] =
    &["openai", "claude", "zai", "kimi", "alibaba-coding-plan"];

pub fn brand_header_label() -> &'static str {
    "🍇 SAITEC-TUI"
}

pub fn show_skills_in_ui() -> bool {
    false
}

pub fn emphasize_mcp_status() -> bool {
    true
}

pub fn expose_generic_openrouter_routes() -> bool {
    false
}

pub fn prefer_text_startup_logo() -> bool {
    true
}

pub fn use_fixed_shell_layout() -> bool {
    true
}

pub fn show_external_resume_sources() -> bool {
    false
}

pub fn allowed_base_model_provider_ids() -> &'static [&'static str] {
    ALLOWED_BASE_MODEL_PROVIDER_IDS
}

pub fn is_allowed_base_model_provider(provider_id: &str) -> bool {
    let normalized = provider_id.trim().to_ascii_lowercase();
    ALLOWED_BASE_MODEL_PROVIDER_IDS
        .iter()
        .any(|candidate| *candidate == normalized)
}

pub fn is_allowed_openai_compatible_profile(profile_id: &str) -> bool {
    matches!(
        profile_id.trim().to_ascii_lowercase().as_str(),
        "zai" | "kimi" | "alibaba-coding-plan"
    )
}

pub fn unsupported_base_model_provider_message() -> String {
    "SAITEC-TUI only supports these base-model providers: openai, claude, zai, kimi, alibaba-coding-plan.".to_string()
}

pub fn unsupported_base_model_route_message(model: &str) -> String {
    format!(
        "SAITEC-TUI cannot use `{}` because it is not routed through an allowed base-model provider. Use `/login base-models` to configure OpenAI, Anthropic/Claude, Z.AI, Kimi, or Alibaba Cloud Coding.",
        model.trim()
    )
}

fn normalized_provider_label(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '_', '-'], "")
}

fn is_openai_label(value: &str) -> bool {
    normalized_provider_label(value) == "openai"
}

fn is_claude_label(value: &str) -> bool {
    matches!(
        normalized_provider_label(value).as_str(),
        "anthropic" | "claude" | "anthropicclaude"
    )
}

fn is_saitec_subscription_label(value: &str) -> bool {
    normalized_provider_label(value) == "saitecsubscription"
}

fn openai_compatible_route_profile_id(provider: &str, api_method: &str) -> Option<String> {
    if let Some(("openai-compatible", profile_id)) = api_method.split_once(':') {
        let profile_id = profile_id.trim();
        if !profile_id.is_empty() {
            return Some(profile_id.to_string());
        }
    }

    if api_method == "openai-compatible" {
        return crate::provider_catalog::openai_compatible_profile_id_for_display_name(provider)
            .map(ToString::to_string);
    }

    None
}

fn model_matches_profile_model(model: &str, candidate: &str) -> bool {
    model.trim().eq_ignore_ascii_case(candidate.trim())
}

fn openai_compatible_profile_allows_model(profile_id: &str, model: &str) -> bool {
    let profile_id = profile_id.trim().to_ascii_lowercase();
    let model = model.trim();
    if model.is_empty() {
        return false;
    }

    if let Some(profile) = crate::provider_catalog::openai_compatible_profile_by_id(&profile_id)
        && crate::provider_catalog::openai_compatible_profile_static_models(profile)
            .iter()
            .any(|candidate| model_matches_profile_model(model, candidate))
    {
        return true;
    }

    crate::auth::validation::get(&profile_id)
        .filter(|record| record.success)
        .map(|record| {
            record
                .validated_models
                .iter()
                .any(|candidate| model_matches_profile_model(model, candidate))
        })
        .unwrap_or(false)
}

pub fn is_allowed_base_model_route(
    outer_provider: &str,
    model: &str,
    provider: &str,
    api_method: &str,
) -> bool {
    let api_method = api_method.trim();

    if let Some(profile_id) = openai_compatible_route_profile_id(provider, api_method) {
        return is_allowed_openai_compatible_profile(&profile_id)
            && openai_compatible_profile_allows_model(&profile_id, model);
    }

    if is_saitec_subscription_label(outer_provider)
        || is_saitec_subscription_label(provider)
        || api_method == "saitec"
    {
        return crate::subscription_catalog::is_curated_model(model.trim());
    }

    if api_method == "openai-oauth" || api_method == "openai-api-key" {
        return is_openai_label(provider);
    }

    if api_method == "claude-oauth" || api_method == "api-key" {
        return is_claude_label(provider);
    }

    if api_method == "current" {
        return is_openai_label(provider)
            || is_openai_label(outer_provider)
            || is_claude_label(provider)
            || is_claude_label(outer_provider);
    }

    false
}

pub fn command_visibility(command: &str) -> CommandVisibility {
    if PUBLIC_COMMANDS.contains(&command) {
        CommandVisibility::Public
    } else if HIDDEN_COMPATIBLE_COMMANDS.contains(&command) {
        CommandVisibility::HiddenCompatible
    } else {
        CommandVisibility::InternalOnly
    }
}

pub fn public_commands() -> Vec<&'static str> {
    PUBLIC_COMMANDS.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_command_list_contains_saitec_surface_commands() {
        let public = public_commands();

        assert!(public.contains(&"/help"));
        assert!(public.contains(&"/login"));
        assert!(public.contains(&"/logout"));
        assert!(public.contains(&"/auth"));
        assert!(public.contains(&"/model"));
        assert!(public.contains(&"/clear"));
        assert!(public.contains(&"/resume"));
        assert!(public.contains(&"/export"));
        assert!(public.contains(&"/usage"));
        assert!(public.contains(&"/version"));
        assert!(public.contains(&"/quit"));
    }

    #[test]
    fn hidden_compatible_commands_include_git_and_selfdev() {
        assert_eq!(
            command_visibility("/git"),
            CommandVisibility::HiddenCompatible
        );
        assert_eq!(
            command_visibility("/selfdev"),
            CommandVisibility::HiddenCompatible
        );
        assert_eq!(
            command_visibility("/improve"),
            CommandVisibility::HiddenCompatible
        );
    }

    #[test]
    fn saitec_brand_header_uses_grape_logo() {
        assert_eq!(brand_header_label(), "🍇 SAITEC-TUI");
    }

    #[test]
    fn product_mode_disables_skill_visibility() {
        assert!(!show_skills_in_ui());
    }

    #[test]
    fn product_mode_hides_external_resume_sources() {
        assert!(!show_external_resume_sources());
    }

    #[test]
    fn kimi_openai_compatible_route_only_allows_kimi_models() {
        assert!(is_allowed_base_model_route(
            "",
            "kimi-for-coding",
            "Kimi Code",
            "openai-compatible"
        ));
        assert!(!is_allowed_base_model_route(
            "",
            "claude-opus-4-6",
            "Kimi Code",
            "openai-compatible"
        ));
        assert!(!is_allowed_base_model_route(
            "",
            "claude-opus-4-6",
            "Kimi Code",
            "openai-compatible:kimi"
        ));
    }

    #[test]
    fn allowed_kimi_provider_allows_kimi_base_model_route() {
        assert!(is_allowed_base_model_route(
            "",
            "kimi-for-coding",
            "Kimi Code",
            "openai-compatible:kimi",
        ));
    }
}
