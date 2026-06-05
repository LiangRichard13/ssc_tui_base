use super::{
    ActiveProvider, DEFAULT_CONTEXT_LIMIT, EventStream, ModelCatalogRefreshSummary, ModelRoute,
    MultiProvider, NativeCompactionResult, NativeToolResultSender, PremiumMode, Provider,
};
use crate::message::{Message, ToolDefinition};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::{Arc, RwLock};

const MCP_ONLY_MODEL: &str = "mcp-only";
const MCP_ONLY_ERROR: &str = "SAITEC login only grants permission for SAITEC MCP tools. It is not a base-model provider and does not expose an OpenAI-compatible chat endpoint. Configure a base model with `/login base-models` before sending normal chat prompts.";

pub struct JcodeProvider {
    base_model_provider: RwLock<Option<Arc<MultiProvider>>>,
}

impl JcodeProvider {
    pub fn new() -> Self {
        Self {
            base_model_provider: RwLock::new(None),
        }
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
            .unwrap_or("SAITEC")
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

    #[test]
    fn jcode_provider_does_not_enable_subscription_runtime_mode() {
        let _guard = crate::storage::lock_test_env();
        crate::subscription_catalog::clear_runtime_env();

        let provider = JcodeProvider::new();

        assert!(!crate::subscription_catalog::is_runtime_mode_enabled());
        assert!(provider.available_models_display().is_empty());

        crate::subscription_catalog::clear_runtime_env();
    }

    #[test]
    fn jcode_provider_name_and_model_are_mcp_only() {
        let _guard = crate::storage::lock_test_env();
        crate::subscription_catalog::clear_runtime_env();

        let provider = JcodeProvider::new();

        assert_eq!(provider.name(), "SAITEC");
        assert_eq!(provider.model(), "mcp-only");
        assert!(!crate::subscription_catalog::is_curated_model(
            &provider.model()
        ));

        crate::subscription_catalog::clear_runtime_env();
    }

    #[test]
    fn jcode_provider_is_mcp_only_and_does_not_enable_openrouter_runtime() {
        let _guard = crate::storage::lock_test_env();
        crate::subscription_catalog::clear_runtime_env();
        crate::env::remove_var("JCODE_OPENROUTER_MODEL");
        crate::env::remove_var("JCODE_ACTIVE_PROVIDER");
        crate::env::remove_var("JCODE_FORCE_PROVIDER");

        let provider = JcodeProvider::new();

        assert_eq!(provider.name(), "SAITEC");
        assert_eq!(provider.model(), "mcp-only");
        assert!(!crate::subscription_catalog::is_runtime_mode_enabled());
        assert!(std::env::var_os("JCODE_OPENROUTER_MODEL").is_none());
        assert!(std::env::var_os("JCODE_ACTIVE_PROVIDER").is_none());
        assert!(std::env::var_os("JCODE_FORCE_PROVIDER").is_none());
        assert!(provider.model_routes().is_empty());
    }

    #[tokio::test]
    async fn jcode_provider_without_base_model_rejects_chat_completion() {
        let _guard = crate::storage::lock_test_env();
        crate::subscription_catalog::clear_runtime_env();
        crate::env::remove_var("JCODE_OPENROUTER_MODEL");
        crate::env::remove_var("JCODE_ACTIVE_PROVIDER");
        crate::env::remove_var("JCODE_FORCE_PROVIDER");

        let provider = JcodeProvider::new();
        let error = match provider.complete(&[], &[], "", None).await {
            Ok(_) => {
                panic!("SAITEC provider must not send chat without an explicit base model");
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
        let _guard = crate::storage::lock_test_env();
        crate::subscription_catalog::clear_runtime_env();
        let previous_kimi_key = std::env::var_os("KIMI_API_KEY");
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

        if let Some(value) = previous_kimi_key {
            crate::env::set_var("KIMI_API_KEY", value);
        } else {
            crate::env::remove_var("KIMI_API_KEY");
        }
        crate::provider_catalog::force_apply_openai_compatible_profile_env(None);
        crate::subscription_catalog::clear_runtime_env();
    }
}
