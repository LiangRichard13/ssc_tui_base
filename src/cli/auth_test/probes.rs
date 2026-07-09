fn generic_credential_paths_for_provider(
    provider: crate::provider_catalog::LoginProviderDescriptor,
) -> Vec<String> {
    let Ok(config_dir) = crate::storage::app_config_dir() else {
        return Vec::new();
    };

    match provider.target {
        crate::provider_catalog::LoginProviderTarget::Jcode => {
            vec![config_dir.join(crate::subscription_catalog::JCODE_ENV_FILE)]
        }
        crate::provider_catalog::LoginProviderTarget::OpenRouter => {
            vec![config_dir.join("openrouter.env")]
        }
        crate::provider_catalog::LoginProviderTarget::OpenAiApiKey => {
            vec![config_dir.join("openai.env")]
        }
        crate::provider_catalog::LoginProviderTarget::Azure => {
            vec![config_dir.join(crate::auth::azure::ENV_FILE)]
        }
        crate::provider_catalog::LoginProviderTarget::Bedrock => {
            vec![config_dir.join(crate::provider::bedrock::ENV_FILE)]
        }
        crate::provider_catalog::LoginProviderTarget::OpenAiCompatible(profile) => {
            let resolved = crate::provider_catalog::resolve_openai_compatible_profile(profile);
            vec![config_dir.join(resolved.env_file)]
        }
        _ => Vec::new(),
    }
    .into_iter()
    .map(|path| path.display().to_string())
    .collect()
}

fn auth_state_label(state: crate::auth::AuthState) -> &'static str {
    match state {
        crate::auth::AuthState::Available => "available",
        crate::auth::AuthState::Expired => "expired",
        crate::auth::AuthState::NotConfigured => "not_configured",
    }
}

fn probe_generic_provider_auth(
    provider: crate::provider_catalog::LoginProviderDescriptor,
    report: &mut AuthTestProviderReport,
) {
    // Keep generic provider probes provider-local. A DeepSeek/Z.AI/OpenRouter
    // auth-test should never be delayed or wedged by an unrelated Cursor/Gemini
    // external auth probe.
    //
    // For openai-compatible presets (DeepSeek/Z.AI/Kimi/...), `AuthStatus`
    // considers a stale `auth-validation.json` `success: false` record as
    // `Expired` (see `auth::state_for_provider` for OpenAiCompatible). That
    // makes `credential_probe` fail immediately, which short-circuits the
    // provider smoke (`maybe_run_auth_test_smoke` checks `report.success`),
    // and the new run writes another `success: false` — a deadlock that
    // the user cannot recover from without deleting `auth-validation.json`
    // by hand. We avoid that here by checking the on-disk configuration
    // (`openai_compatible_profile_is_configured`) directly: if the API key is
    // still present in the env file, the credential is configured regardless
    // of any past validation record. The provider_smoke step then runs and
    // actually exercises the endpoint; if it fails, the user sees a real
    // error (HTTP status, model not supported, etc.) instead of a generic
    // "auth status is expired" that masks the truth.
    let (state, detail) = match provider.target {
        crate::provider_catalog::LoginProviderTarget::OpenAiCompatible(profile) => {
            if crate::provider_catalog::openai_compatible_profile_is_configured(profile) {
                let status = crate::auth::AuthStatus::check_fast();
                let detail = status.method_detail_for_provider(provider);
                (crate::auth::AuthState::Available, detail)
            } else {
                let status = crate::auth::AuthStatus::check_fast();
                let state = status.state_for_provider(provider);
                let detail = status.method_detail_for_provider(provider);
                (state, detail)
            }
        }
        _ => {
            let status = crate::auth::AuthStatus::check_fast();
            let state = status.state_for_provider(provider);
            let detail = status.method_detail_for_provider(provider);
            (state, detail)
        }
    };
    report.push_step(
        "credential_probe",
        state == crate::auth::AuthState::Available,
        format!(
            "{} auth status is {} ({detail}).",
            provider.display_name,
            auth_state_label(state),
        ),
    );
    report.push_step(
        "refresh_probe",
        true,
        "Skipped: provider does not expose a dedicated refresh probe in jcode today.".to_string(),
    );
}

async fn probe_claude_auth(report: &mut AuthTestProviderReport) {
    if let Some(creds) = push_result_step(
        report,
        "credential_probe",
        crate::auth::claude::load_credentials(),
        |creds| {
            format!(
                "Loaded Claude credentials (expires_at={}).",
                creds.expires_at
            )
        },
    ) {
        push_result_step(
            report,
            "refresh_probe",
            crate::auth::oauth::refresh_claude_tokens(&creds.refresh_token).await,
            |tokens| {
                format!(
                    "Claude token refresh succeeded (new_expires_at={}).",
                    tokens.expires_at
                )
            },
        );
    }
}

async fn probe_openai_auth(report: &mut AuthTestProviderReport) {
    if let Some(creds) = push_result_step(
        report,
        "credential_probe",
        crate::auth::codex::load_credentials(),
        |creds| {
            if creds.refresh_token.trim().is_empty() {
                "Loaded OpenAI API key credentials (no refresh token present).".to_string()
            } else {
                format!(
                    "Loaded OpenAI OAuth credentials (expires_at={:?}).",
                    creds.expires_at
                )
            }
        },
    ) {
        if creds.refresh_token.trim().is_empty() {
            report.push_step(
                "refresh_probe",
                true,
                "Skipped: OpenAI is using API key auth, not OAuth.",
            );
        } else {
            push_result_step(
                report,
                "refresh_probe",
                crate::auth::oauth::refresh_openai_tokens(&creds.refresh_token).await,
                |tokens| {
                    format!(
                        "OpenAI token refresh succeeded (new_expires_at={}).",
                        tokens.expires_at
                    )
                },
            );
        }
    }
}

async fn probe_gemini_auth(report: &mut AuthTestProviderReport) {
    if push_result_step(
        report,
        "credential_probe",
        crate::auth::gemini::load_tokens(),
        |tokens| {
            format!(
                "Loaded Gemini tokens{} (expires_at={}).",
                auth_email_suffix(tokens.email.as_deref()),
                tokens.expires_at
            )
        },
    )
    .is_some()
    {
        push_result_step(
            report,
            "refresh_probe",
            crate::auth::gemini::load_or_refresh_tokens().await,
            |tokens| {
                format!(
                    "Gemini token load/refresh succeeded (expires_at={}).",
                    tokens.expires_at
                )
            },
        );
    }
}

async fn probe_antigravity_auth(report: &mut AuthTestProviderReport) {
    if push_result_step(
        report,
        "credential_probe",
        crate::auth::antigravity::load_tokens(),
        |tokens| {
            format!(
                "Loaded Antigravity OAuth tokens{} (expires_at={}).",
                auth_email_suffix(tokens.email.as_deref()),
                tokens.expires_at
            )
        },
    )
    .is_some()
    {
        push_result_step(
            report,
            "refresh_probe",
            crate::auth::antigravity::load_or_refresh_tokens().await,
            |tokens| {
                format!(
                    "Antigravity token load/refresh succeeded (expires_at={}).",
                    tokens.expires_at
                )
            },
        );
    }
}

async fn probe_google_auth(report: &mut AuthTestProviderReport) {
    let creds_result = crate::auth::google::load_credentials();
    let tokens_result = crate::auth::google::load_tokens();
    match (creds_result, tokens_result) {
        (Ok(creds), Ok(tokens)) => {
            report.push_step(
                "credential_probe",
                true,
                format!(
                    "Loaded Google credentials (client_id={}...) and Gmail tokens{}.",
                    &creds.client_id[..20.min(creds.client_id.len())],
                    auth_email_suffix(tokens.email.as_deref())
                ),
            );
            match crate::auth::google::get_valid_token().await {
                Ok(_) => report.push_step(
                    "refresh_probe",
                    true,
                    "Google/Gmail token load/refresh succeeded.".to_string(),
                ),
                Err(err) => report.push_step("refresh_probe", false, err.to_string()),
            }
        }
        (Err(err), _) => report.push_step("credential_probe", false, err.to_string()),
        (_, Err(err)) => report.push_step("credential_probe", false, err.to_string()),
    }
}

async fn probe_copilot_auth(report: &mut AuthTestProviderReport) {
    if let Some(token) = push_result_step(
        report,
        "credential_probe",
        crate::auth::copilot::load_github_token(),
        |token| {
            format!(
                "Loaded GitHub OAuth token for Copilot ({} chars).",
                token.len()
            )
        },
    ) {
        let client = reqwest::Client::new();
        push_result_step(
            report,
            "refresh_probe",
            crate::auth::copilot::exchange_github_token(&client, &token).await,
            |api_token| {
                format!(
                    "Exchanged GitHub token for Copilot API token (expires_at={}).",
                    api_token.expires_at
                )
            },
        );
    }
}

async fn probe_cursor_auth(report: &mut AuthTestProviderReport) {
    let has_api_key = crate::auth::cursor::has_cursor_api_key();
    let has_auth_file = crate::auth::cursor::has_cursor_auth_file_token();
    let has_vscdb = crate::auth::cursor::has_cursor_vscdb_token();
    let ok = has_api_key || has_auth_file || has_vscdb;
    report.push_step(
        "credential_probe",
        ok,
        format!(
            "Cursor native auth sources: api_key={}, auth_json={}, vscdb_token={}",
            has_api_key, has_auth_file, has_vscdb
        ),
    );
    report.push_step(
        "refresh_probe",
        true,
        "Skipped: Cursor provider does not expose a native refresh-token probe in jcode today."
            .to_string(),
    );
}

#[cfg(test)]
mod probes_tests {
    use super::*;
    use crate::auth::validation::{ProviderValidationRecord, save};

    /// `probe_generic_provider_auth` must NOT be wedged by a stale
    /// `success: false` row in `auth-validation.json` while the underlying
    /// configuration is still present on disk. Otherwise the auth-test smoke
    /// short-circuits at the credential_probe step and writes another failure,
    /// which the user cannot recover from without deleting the file by hand.
    #[test]
    fn probe_generic_provider_auth_ignores_stale_failure_when_key_present() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::TempDir::new().expect("create temp dir");
        let prev_home = std::env::var_os("JCODE_HOME");
        crate::env::set_var("JCODE_HOME", temp.path());

        // Plant a real API key so `openai_compatible_profile_is_configured`
        // returns true on the synthetic JCODE_HOME. `app_config_dir()` joins
        // `$JCODE_HOME/config/jcode` when JCODE_HOME is set.
        let config_dir = temp.path().join("config").join("jcode");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        std::fs::write(
            config_dir.join("openai-compatible.env"),
            "JCODE_OPENAI_COMPAT_API_BASE=https://api.deepseek.com\n\
             OPENAI_COMPAT_API_KEY=test-key-not-real\n\
             JCODE_OPENAI_COMPAT_DEFAULT_MODEL=deepseek-chat\n",
        )
        .expect("write openai-compatible.env");

        // Plant a stale `success: false` row — this is the deadlock trigger.
        let stale = ProviderValidationRecord {
            checked_at_ms: chrono::Utc::now().timestamp_millis(),
            success: false,
            provider_smoke_ok: None,
            tool_smoke_ok: None,
            validated_models: Vec::new(),
            summary: "credential_probe: OpenAI-compatible auth status is expired \
                      (not configured)."
                .to_string(),
        };
        save("openai-compatible", stale).expect("plant stale record");

        // AuthStatus has a short TTL; invalidate to be sure we observe the
        // planted record (otherwise the cached `Available` from a previous
        // test would mask the regression).
        crate::auth::AuthStatus::invalidate_cache();

        let mut report =
            AuthTestProviderReport::new_generic("openai-compatible".to_string(), Vec::new());
        probe_generic_provider_auth(
            crate::provider_catalog::OPENAI_COMPAT_LOGIN_PROVIDER,
            &mut report,
        );

        let credential_probe = report
            .steps
            .iter()
            .find(|step| step.name == "credential_probe")
            .expect("credential_probe step recorded");
        assert!(
            credential_probe.ok,
            "credential_probe should be Available when key is on disk, got: {}",
            credential_probe.detail
        );
        assert!(
            report.success,
            "report should remain viable so the provider smoke can run; steps: {:?}",
            report.steps
        );

        if let Some(prev_home) = prev_home {
            crate::env::set_var("JCODE_HOME", prev_home);
        } else {
            crate::env::remove_var("JCODE_HOME");
        }
        crate::auth::AuthStatus::invalidate_cache();
    }
}
