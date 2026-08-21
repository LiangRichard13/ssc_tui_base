use super::{
    ActiveProvider, DEFAULT_CONTEXT_LIMIT, EventStream, ModelCatalogRefreshSummary, ModelRoute,
    MultiProvider, NativeCompactionResult, NativeToolResultSender, PremiumMode, Provider,
};
use crate::message::{Message, ToolDefinition};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::{Arc, RwLock};

const MCP_ONLY_MODEL: &str = "mcp-only";
const MCP_ONLY_ERROR: &str = "SSC login only grants permission for SSC MCP tools. It is not a base-model provider and does not expose an OpenAI-compatible chat endpoint. Configure a base model with `/login base-models` before sending normal chat prompts.";

pub struct JcodeProvider {
    base_model_provider: RwLock<Option<Arc<MultiProvider>>>,
}

impl JcodeProvider {
    pub fn new() -> Self {
        let provider = Self {
            base_model_provider: RwLock::new(None),
        };
        provider.try_activate_configured_base_model();
        provider
    }

    fn base_model_provider(&self) -> Option<Arc<MultiProvider>> {
        self.base_model_provider
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn delegated_provider_name(provider: &MultiProvider) -> &'static str {
        match provider.active_provider() {
            ActiveProvider::Claude => "Claude",
            ActiveProvider::OpenAI => "OpenAI",
            ActiveProvider::Copilot => "Copilot",
            ActiveProvider::Antigravity => "Antigravity",
            ActiveProvider::Gemini => "Gemini",
            ActiveProvider::Cursor => "Cursor",
            ActiveProvider::Bedrock => "Bedrock",
            ActiveProvider::OpenRouter => "OpenRouter",
        }
    }

    fn openai_compatible_default_model_spec_for_profile(profile_id: &str) -> Option<String> {
        let profile = crate::provider_catalog::openai_compatible_profile_by_id(&profile_id)?;
        let resolved = crate::provider_catalog::resolve_openai_compatible_profile(profile);
        if resolved.requires_api_key
            && !crate::provider_catalog::openai_compatible_profile_is_configured(profile)
        {
            return None;
        }
        resolved
            .default_model
            .map(|model| format!("{profile_id}:{model}"))
    }

    fn runtime_validated_default_model_spec_for_profile(
        profile: crate::provider_catalog::OpenAiCompatibleProfile,
    ) -> Option<(String, i64)> {
        let resolved = crate::provider_catalog::resolve_openai_compatible_profile(profile);
        let model = resolved.default_model.as_deref()?;
        let spec = Self::openai_compatible_default_model_spec_for_profile(&resolved.id)?;
        let record = crate::auth::validation::get(&resolved.id)?;
        if !record.success
            || (record.provider_smoke_ok != Some(true) && record.tool_smoke_ok != Some(true))
        {
            return None;
        }
        if !record
            .validated_models
            .iter()
            .any(|validated| validated.trim().eq_ignore_ascii_case(model.trim()))
        {
            return None;
        }
        Some((spec, record.checked_at_ms))
    }

    fn configured_openai_compatible_default_model_spec() -> Option<String> {
        if let Some(spec) = crate::provider_catalog::active_openai_compatible_profile_id()
            .as_deref()
            .and_then(Self::openai_compatible_default_model_spec_for_profile)
        {
            return Some(spec);
        }

        let profiles = crate::provider_catalog::openai_compatible_profiles()
            .iter()
            .copied()
            .filter(|profile| {
                Self::openai_compatible_default_model_spec_for_profile(profile.id).is_some()
            })
            .collect::<Vec<_>>();

        let mut validated_specs = profiles
            .iter()
            .copied()
            .filter_map(Self::runtime_validated_default_model_spec_for_profile)
            .collect::<Vec<_>>();
        validated_specs.sort_by(|a, b| b.1.cmp(&a.1));
        if validated_specs.len() == 1
            || validated_specs
                .first()
                .zip(validated_specs.get(1))
                .is_some_and(|(first, second)| first.1 > second.1)
        {
            return validated_specs.into_iter().next().map(|(spec, _)| spec);
        }

        let mut specs = profiles
            .iter()
            .filter_map(|profile| {
                Self::openai_compatible_default_model_spec_for_profile(profile.id)
            })
            .collect::<Vec<_>>();

        if specs.len() == 1 { specs.pop() } else { None }
    }

    fn try_activate_configured_base_model(&self) {
        if self.base_model_provider().is_some() {
            return;
        }
        let Some(model_spec) = Self::configured_openai_compatible_default_model_spec() else {
            return;
        };
        let provider = Arc::new(MultiProvider::new_fast());
        match provider.set_model(&model_spec) {
            Ok(()) => {
                *self
                    .base_model_provider
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(provider);
            }
            Err(error) => {
                crate::logging::warn(&format!(
                    "Failed to activate configured base model `{}`: {}",
                    model_spec, error
                ));
                // Roll back the *runtime* env vars only — leave the env file's
                // JCODE_OPENAI_COMPAT_API_BASE / DEFAULT_MODEL intact so a
                // transient activation failure does not wipe the user's
                // configured endpoint and model name on the next launch.
                crate::provider_catalog::clear_openai_compatible_runtime_env_keep_config();
            }
        }
    }
}

impl Default for JcodeProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for JcodeProvider {
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system: &str,
        resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        if let Some(provider) = self.base_model_provider() {
            provider
                .complete(messages, tools, system, resume_session_id)
                .await
        } else {
            let _ = (messages, tools, system, resume_session_id);
            Err(anyhow::anyhow!(MCP_ONLY_ERROR))
        }
    }

    async fn complete_split(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_static: &str,
        system_dynamic: &str,
        resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        if let Some(provider) = self.base_model_provider() {
            provider
                .complete_split(
                    messages,
                    tools,
                    system_static,
                    system_dynamic,
                    resume_session_id,
                )
                .await
        } else {
            let _ = (
                messages,
                tools,
                system_static,
                system_dynamic,
                resume_session_id,
            );
            Err(anyhow::anyhow!(MCP_ONLY_ERROR))
        }
    }

    fn name(&self) -> &str {
        self.base_model_provider()
            .as_deref()
            .map(Self::delegated_provider_name)
            .unwrap_or("SSC")
    }

    fn model(&self) -> String {
        self.base_model_provider()
            .map(|provider| provider.model())
            .unwrap_or_else(|| MCP_ONLY_MODEL.to_string())
    }

    fn set_model(&self, model: &str) -> Result<()> {
        let provider = self
            .base_model_provider()
            .unwrap_or_else(|| Arc::new(MultiProvider::new_fast()));
        provider.set_model(model)?;
        *self
            .base_model_provider
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(provider);
        Ok(())
    }

    fn supports_image_input(&self) -> bool {
        self.base_model_provider()
            .is_some_and(|provider| provider.supports_image_input())
    }

    fn available_models(&self) -> Vec<&'static str> {
        self.base_model_provider()
            .map(|provider| provider.available_models())
            .unwrap_or_default()
    }

    fn available_models_display(&self) -> Vec<String> {
        self.base_model_provider()
            .map(|provider| provider.available_models_display())
            .unwrap_or_default()
    }

    fn available_models_for_switching(&self) -> Vec<String> {
        self.base_model_provider()
            .map(|provider| provider.available_models_for_switching())
            .unwrap_or_default()
    }

    fn available_providers_for_model(&self, model: &str) -> Vec<String> {
        self.base_model_provider()
            .map(|provider| provider.available_providers_for_model(model))
            .unwrap_or_default()
    }

    fn provider_details_for_model(&self, model: &str) -> Vec<(String, String)> {
        self.base_model_provider()
            .map(|provider| provider.provider_details_for_model(model))
            .unwrap_or_default()
    }

    fn preferred_provider(&self) -> Option<String> {
        self.base_model_provider()
            .and_then(|provider| provider.preferred_provider())
    }

    fn model_routes(&self) -> Vec<ModelRoute> {
        self.base_model_provider()
            .map(|provider| provider.model_routes())
            .unwrap_or_default()
    }

    async fn prefetch_models(&self) -> Result<()> {
        if let Some(provider) = self.base_model_provider() {
            provider.prefetch_models().await
        } else {
            Ok(())
        }
    }

    async fn refresh_model_catalog(&self) -> Result<ModelCatalogRefreshSummary> {
        if let Some(provider) = self.base_model_provider() {
            provider.refresh_model_catalog().await
        } else {
            Ok(ModelCatalogRefreshSummary::default())
        }
    }

    fn on_auth_changed(&self) {
        if let Some(provider) = self.base_model_provider() {
            provider.on_auth_changed();
        } else {
            self.try_activate_configured_base_model();
        }
    }

    fn reasoning_effort(&self) -> Option<String> {
        self.base_model_provider()
            .and_then(|provider| provider.reasoning_effort())
    }

    fn set_reasoning_effort(&self, effort: &str) -> Result<()> {
        if let Some(provider) = self.base_model_provider() {
            provider.set_reasoning_effort(effort)
        } else {
            anyhow::bail!("This provider does not support reasoning effort")
        }
    }

    fn available_efforts(&self) -> Vec<&'static str> {
        self.base_model_provider()
            .map(|provider| provider.available_efforts())
            .unwrap_or_default()
    }

    fn native_compaction_mode(&self) -> Option<String> {
        self.base_model_provider()
            .and_then(|provider| provider.native_compaction_mode())
    }

    fn native_compaction_threshold_tokens(&self) -> Option<usize> {
        self.base_model_provider()
            .and_then(|provider| provider.native_compaction_threshold_tokens())
    }

    fn transport(&self) -> Option<String> {
        self.base_model_provider()
            .and_then(|provider| provider.transport())
    }

    fn set_transport(&self, transport: &str) -> Result<()> {
        if let Some(provider) = self.base_model_provider() {
            provider.set_transport(transport)
        } else {
            anyhow::bail!("This provider does not support transport switching")
        }
    }

    fn available_transports(&self) -> Vec<&'static str> {
        self.base_model_provider()
            .map(|provider| provider.available_transports())
            .unwrap_or_default()
    }

    fn handles_tools_internally(&self) -> bool {
        self.base_model_provider()
            .is_some_and(|provider| provider.handles_tools_internally())
    }

    async fn invalidate_credentials(&self) {
        if let Some(provider) = self.base_model_provider() {
            provider.invalidate_credentials().await;
        }
    }

    fn set_premium_mode(&self, mode: PremiumMode) {
        if let Some(provider) = self.base_model_provider() {
            provider.set_premium_mode(mode);
        }
    }

    fn premium_mode(&self) -> PremiumMode {
        self.base_model_provider()
            .map(|provider| provider.premium_mode())
            .unwrap_or(PremiumMode::Normal)
    }

    fn supports_compaction(&self) -> bool {
        self.base_model_provider()
            .is_some_and(|provider| provider.supports_compaction())
    }

    fn uses_jcode_compaction(&self) -> bool {
        self.base_model_provider()
            .is_some_and(|provider| provider.uses_jcode_compaction())
    }

    async fn native_compact(
        &self,
        messages: &[Message],
        existing_summary_text: Option<&str>,
        existing_openai_encrypted_content: Option<&str>,
    ) -> Result<NativeCompactionResult> {
        if let Some(provider) = self.base_model_provider() {
            provider
                .native_compact(
                    messages,
                    existing_summary_text,
                    existing_openai_encrypted_content,
                )
                .await
        } else {
            anyhow::bail!("This provider does not support native compaction")
        }
    }

    fn context_window(&self) -> usize {
        self.base_model_provider()
            .map(|provider| provider.context_window())
            .unwrap_or(DEFAULT_CONTEXT_LIMIT)
    }

    fn fork(&self) -> Arc<dyn Provider> {
        let forked = Self::new();
        if let Some(provider) = self.base_model_provider() {
            let _ = forked.set_model(&provider.model());
        }
        Arc::new(forked)
    }

    fn native_result_sender(&self) -> Option<NativeToolResultSender> {
        self.base_model_provider()
            .and_then(|provider| provider.native_result_sender())
    }

    fn drain_startup_notices(&self) -> Vec<String> {
        self.base_model_provider()
            .map(|provider| provider.drain_startup_notices())
            .unwrap_or_default()
    }

    fn switch_active_provider_to(&self, provider_name: &str) -> Result<()> {
        if let Some(provider) = self.base_model_provider() {
            provider.switch_active_provider_to(provider_name)
        } else {
            anyhow::bail!("This provider does not support active provider switching")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROVIDER_TEST_ENV_KEYS: &[&str] = &[
        "OPENROUTER_API_KEY",
        "DEEPSEEK_API_KEY",
        "ZHIPU_API_KEY",
        "ZAI_API_KEY",
        "KIMI_API_KEY",
        "JCODE_OPENROUTER_API_BASE",
        "JCODE_OPENROUTER_API_KEY_NAME",
        "JCODE_OPENROUTER_ENV_FILE",
        "JCODE_OPENROUTER_CACHE_NAMESPACE",
        "JCODE_OPENROUTER_PROVIDER_FEATURES",
        "JCODE_OPENROUTER_ALLOW_NO_AUTH",
        "JCODE_OPENROUTER_MODEL_CATALOG",
        "JCODE_OPENROUTER_MODEL",
        "JCODE_OPENROUTER_STATIC_MODELS",
        "JCODE_OPENROUTER_AUTH_HEADER",
        "JCODE_OPENROUTER_AUTH_HEADER_NAME",
        "JCODE_OPENROUTER_DYNAMIC_BEARER_PROVIDER",
        "JCODE_OPENROUTER_PROVIDER",
        "JCODE_OPENROUTER_NO_FALLBACK",
        "JCODE_ACTIVE_PROVIDER",
        "JCODE_FORCE_PROVIDER",
        "JCODE_NAMED_PROVIDER_PROFILE",
        "JCODE_PROVIDER_PROFILE_ACTIVE",
        "JCODE_PROVIDER_PROFILE_NAME",
    ];

    struct ProviderTestEnvGuard {
        previous_home: Option<std::ffi::OsString>,
        saved_env: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl ProviderTestEnvGuard {
        fn isolate() -> (tempfile::TempDir, Self) {
            let temp = tempfile::tempdir().expect("tempdir");
            let previous_home = std::env::var_os("JCODE_HOME");
            let saved_env = PROVIDER_TEST_ENV_KEYS
                .iter()
                .map(|key| (*key, std::env::var_os(key)))
                .collect::<Vec<_>>();

            crate::env::set_var("JCODE_HOME", temp.path());
            for (key, _) in &saved_env {
                crate::env::remove_var(key);
            }
            crate::provider_catalog::force_apply_openai_compatible_profile_env(None);
            crate::subscription_catalog::clear_runtime_env();

            (
                temp,
                Self {
                    previous_home,
                    saved_env,
                },
            )
        }
    }

    impl Drop for ProviderTestEnvGuard {
        fn drop(&mut self) {
            crate::provider_catalog::force_apply_openai_compatible_profile_env(None);
            crate::subscription_catalog::clear_runtime_env();

            if let Some(value) = self.previous_home.take() {
                crate::env::set_var("JCODE_HOME", value);
            } else {
                crate::env::remove_var("JCODE_HOME");
            }

            for (key, value) in self.saved_env.drain(..) {
                if let Some(value) = value {
                    crate::env::set_var(key, value);
                } else {
                    crate::env::remove_var(key);
                }
            }
        }
    }

    #[test]
    fn jcode_provider_does_not_enable_subscription_runtime_mode() {
        let _lock = crate::storage::lock_test_env();
        let (_temp, _env_guard) = ProviderTestEnvGuard::isolate();

        let provider = JcodeProvider::new();

        assert!(!crate::subscription_catalog::is_runtime_mode_enabled());
        assert!(provider.available_models_display().is_empty());
    }

    #[test]
    fn jcode_provider_name_and_model_are_mcp_only() {
        let _lock = crate::storage::lock_test_env();
        let (_temp, _env_guard) = ProviderTestEnvGuard::isolate();

        let provider = JcodeProvider::new();

        assert_eq!(provider.name(), "SSC");
        assert_eq!(provider.model(), "mcp-only");
        assert!(!crate::subscription_catalog::is_curated_model(
            &provider.model()
        ));
    }

    #[test]
    fn jcode_provider_is_mcp_only_and_does_not_enable_openrouter_runtime() {
        let _lock = crate::storage::lock_test_env();
        let (_temp, _env_guard) = ProviderTestEnvGuard::isolate();

        let provider = JcodeProvider::new();

        assert_eq!(provider.name(), "SSC");
        assert_eq!(provider.model(), "mcp-only");
        assert!(!crate::subscription_catalog::is_runtime_mode_enabled());
        assert!(std::env::var_os("JCODE_OPENROUTER_MODEL").is_none());
        assert!(std::env::var_os("JCODE_ACTIVE_PROVIDER").is_none());
        assert!(std::env::var_os("JCODE_FORCE_PROVIDER").is_none());
        assert!(provider.model_routes().is_empty());
    }

    #[tokio::test]
    async fn jcode_provider_without_base_model_rejects_chat_completion() {
        let _lock = crate::storage::lock_test_env();
        let (_temp, _env_guard) = ProviderTestEnvGuard::isolate();

        let provider = JcodeProvider::new();
        let error = match provider.complete(&[], &[], "", None).await {
            Ok(_) => {
                panic!("SSC provider must not send chat without an explicit base model");
            }
            Err(error) => error,
        };

        assert!(error.to_string().contains("MCP tools"));
        assert!(error.to_string().contains("/login base-models"));
        assert!(std::env::var_os("JCODE_OPENROUTER_MODEL").is_none());
        assert!(std::env::var_os("JCODE_ACTIVE_PROVIDER").is_none());
        assert!(std::env::var_os("JCODE_FORCE_PROVIDER").is_none());
    }

    #[test]
    fn jcode_provider_allows_configured_kimi_profile_prefixed_model() {
        let _lock = crate::storage::lock_test_env();
        let (_temp, _env_guard) = ProviderTestEnvGuard::isolate();
        crate::env::set_var("KIMI_API_KEY", "test-kimi-key");
        crate::auth::validation::save(
            "kimi",
            crate::auth::validation::ProviderValidationRecord {
                checked_at_ms: chrono::Utc::now().timestamp_millis(),
                success: true,
                provider_smoke_ok: Some(true),
                tool_smoke_ok: Some(true),
                validated_models: vec!["kimi-for-coding".to_string()],
                summary: "tool_smoke: AUTH_TEST_OK".to_string(),
            },
        )
        .expect("save passing Kimi validation");
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

        runtime.block_on(async {
            let provider = JcodeProvider::new();
            provider
                .set_model("kimi:kimi-for-coding")
                .expect("configured Kimi base model should be selectable from SAITEC provider");
            assert_eq!(provider.name(), "OpenRouter");
            assert_eq!(provider.model(), "kimi-for-coding");
            assert!(!crate::subscription_catalog::is_runtime_mode_enabled());
            assert!(
                provider
                    .model_routes()
                    .iter()
                    .any(|route| route.model == "kimi-for-coding"
                        && route.provider == "Kimi Code"
                        && route.api_method == "openai-compatible:kimi"),
                "Kimi route should be visible after base-model activation"
            );
        });
    }

    #[test]
    fn jcode_provider_auto_activates_single_configured_kimi_default_model() {
        let _lock = crate::storage::lock_test_env();
        let (_temp, _env_guard) = ProviderTestEnvGuard::isolate();
        crate::provider_catalog::save_env_value_to_env_file(
            "KIMI_API_KEY",
            "kimi.env",
            Some("test-kimi-key"),
        )
        .expect("save Kimi key");

        let provider = JcodeProvider::new();

        assert_eq!(provider.name(), "OpenRouter");
        assert_eq!(provider.model(), "kimi-for-coding");
        assert!(!crate::subscription_catalog::is_runtime_mode_enabled());
        assert!(
            provider
                .model_routes()
                .iter()
                .any(|route| route.model == "kimi-for-coding"
                    && route.provider == "Kimi Code"
                    && route.api_method == "openai-compatible:kimi"),
            "Kimi route should be visible after automatic base-model activation"
        );
    }

    #[test]
    fn jcode_provider_auto_activates_runtime_validated_kimi_when_other_keys_exist() {
        let _lock = crate::storage::lock_test_env();
        let (_temp, _env_guard) = ProviderTestEnvGuard::isolate();
        crate::provider_catalog::save_env_value_to_env_file(
            "KIMI_API_KEY",
            "kimi.env",
            Some("test-kimi-key"),
        )
        .expect("save Kimi key");
        crate::provider_catalog::save_env_value_to_env_file(
            "ZHIPU_API_KEY",
            "zai.env",
            Some("test-zai-key"),
        )
        .expect("save Z.AI key");
        crate::auth::validation::save(
            "kimi",
            crate::auth::validation::ProviderValidationRecord {
                checked_at_ms: chrono::Utc::now().timestamp_millis(),
                success: true,
                provider_smoke_ok: Some(true),
                tool_smoke_ok: Some(true),
                validated_models: vec!["kimi-for-coding".to_string()],
                summary: "tool_smoke: AUTH_TEST_OK".to_string(),
            },
        )
        .expect("save passing Kimi validation");

        let provider = JcodeProvider::new();

        assert_eq!(provider.name(), "OpenRouter");
        assert_eq!(provider.model(), "kimi-for-coding");
        assert!(!crate::subscription_catalog::is_runtime_mode_enabled());
    }
}
