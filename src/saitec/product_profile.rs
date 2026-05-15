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

pub fn prefer_text_startup_logo() -> bool {
    true
}

pub fn use_fixed_shell_layout() -> bool {
    true
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

pub fn unsupported_base_model_provider_message() -> String {
    "SAITEC-TUI only supports these base-model providers: openai, claude, zai, kimi, alibaba-cloud-coding.".to_string()
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
}
