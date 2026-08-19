#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaitecLoginField {
    Email,
    Phone,
    Password,
    Submit,
    Cancel,
}

// Stage 2D: SaitecPendingForm and PendingLogin::SaitecForm retired — the
// SAITEC business login form is gone with src/saitec/.

#[derive(Debug, Clone)]
pub(crate) struct SaitecPendingForm {
    pub form: SaitecLoginForm,
    pub focus: SaitecLoginField,
    pub error: Option<String>,
    pub submitting: bool,
}

/// Stage 2D (chore/ssc-tui-baseline): the SAITEC backend login form shape is
/// kept so the TUI's PendingLogin::SaitecForm plumbing compiles; the actual
/// submission is a no-op because src/saitec/ was retired.
#[derive(Debug, Clone, Default)]
pub(crate) struct SaitecLoginForm {
    pub email: String,
    pub phone: String,
    pub password: String,
}

impl SaitecLoginForm {
    pub fn new(email: String, phone: String, password: String) -> Self {
        Self {
            email,
            phone,
            password,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.email.trim().is_empty() && self.phone.trim().is_empty() {
            return Err("Enter an email or phone number.".to_string());
        }
        if self.password.is_empty() {
            return Err("Enter your password.".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) enum PendingLogin {
    /// Stage 2D (chore/ssc-tui-baseline): kept as a plumbing stub — the SAITEC
    /// backend login is retired but the form state type still exists so the
    /// TUI input/overlay code compiles. Submitting always fails.
    SaitecForm { form: SaitecPendingForm },
    /// Waiting for user to paste Claude OAuth code for a specific stored account
    ClaudeAccount {
        verifier: String,
        label: String,
        redirect_uri: Option<String>,
    },
    /// Waiting for user to paste an OpenAI OAuth callback URL/query for a specific stored account.
    OpenAiAccount {
        verifier: String,
        label: String,
        expected_state: String,
        redirect_uri: String,
    },
    /// Waiting for user to paste a Gemini OAuth callback URL/query or auth code.
    Gemini {
        verifier: String,
        expected_state: Option<String>,
        redirect_uri: String,
    },
    /// Waiting for user to paste an Antigravity OAuth callback URL/query.
    Antigravity {
        verifier: String,
        expected_state: String,
        redirect_uri: String,
    },
    /// Waiting for user to paste an API key for an OpenAI-compatible provider.
    ApiKeyProfile {
        provider_id: String,
        provider: String,
        auth_method: String,
        docs_url: String,
        env_file: String,
        key_name: String,
        default_model: Option<String>,
        endpoint: Option<String>,
        api_key_optional: bool,
        openai_compatible_profile: Option<crate::provider_catalog::OpenAiCompatibleProfile>,
    },
    /// Waiting for the user to paste a custom OpenAI-compatible API base.
    OpenAiCompatibleApiBase {
        profile: crate::provider_catalog::OpenAiCompatibleProfile,
    },
    /// Waiting for the user to enter an optional model name after saving
    /// API Key for an OpenAI-compatible provider that has no default_model.
    OpenAiCompatibleModelName {
        provider: String,
        provider_id: String,
        env_file: String,
        profile: crate::provider_catalog::OpenAiCompatibleProfile,
    },
    /// Waiting for user to paste a Cursor API key.
    CursorApiKey,
    /// GitHub Copilot device flow in progress (polling in background)
    Copilot,
    /// Waiting for the user to choose which external auth sources to import.
    AutoImportSelection {
        candidates: Vec<crate::cli::provider_init::ExternalAuthReviewCandidate>,
    },
}

impl PendingLogin {
    pub(crate) fn telemetry_context(&self) -> Option<(String, String)> {
        match self {
            Self::SaitecForm { .. } => Some(("jcode".to_string(), "password".to_string())),
            Self::ClaudeAccount { .. } => Some(("claude".to_string(), "oauth".to_string())),
            Self::OpenAiAccount { .. } => Some(("openai".to_string(), "oauth".to_string())),
            Self::Gemini { .. } => Some(("gemini".to_string(), "oauth".to_string())),
            Self::Antigravity { .. } => Some(("antigravity".to_string(), "oauth".to_string())),
            Self::ApiKeyProfile {
                provider_id,
                auth_method,
                ..
            } => Some((provider_id.clone(), auth_method.clone())),
            Self::OpenAiCompatibleApiBase { profile } => {
                let resolved = crate::provider_catalog::resolve_openai_compatible_profile(*profile);
                Some((
                    resolved.id,
                    if resolved.requires_api_key {
                        "api_key".to_string()
                    } else {
                        "local_endpoint".to_string()
                    },
                ))
            }
            Self::CursorApiKey => Some(("cursor".to_string(), "api_key".to_string())),
            Self::Copilot => Some(("copilot".to_string(), "device_code".to_string())),
            Self::OpenAiCompatibleModelName { .. } => None,
            Self::AutoImportSelection { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum PendingAccountInput {
    NewAccountLabel {
        provider_id: String,
        display_name: String,
    },
    CommandValue {
        prompt: String,
        command_prefix: String,
        empty_value: Option<String>,
        status_notice: String,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum AccountCommand {
    OpenOverlay {
        provider_filter: Option<String>,
    },
    Doctor {
        provider_id: Option<String>,
    },
    ShowSettings {
        provider_id: String,
    },
    Login {
        provider_id: String,
    },
    Add {
        provider_id: String,
        label: Option<String>,
    },
    Switch {
        provider_id: String,
        label: String,
    },
    SwitchShorthand {
        label: String,
    },
    Remove {
        provider_id: String,
        label: String,
    },
    SetDefaultProvider(Option<String>),
    SetDefaultModel(Option<String>),
    SetOpenAiTransport(Option<String>),
    SetOpenAiEffort(Option<String>),
    SetOpenAiFast(bool),
    SetCopilotPremium(Option<String>),
    SetOpenAiCompatApiBase(Option<String>),
    SetOpenAiCompatApiKeyName(Option<String>),
    SetOpenAiCompatEnvFile(Option<String>),
    SetOpenAiCompatDefaultModel(Option<String>),
}
