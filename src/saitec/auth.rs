use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const DEFAULT_CALLBACK_PORT: u16 = 1455;
pub const CALLBACK_PATH: &str = "/auth/callback";
pub const DEFAULT_AUTH_BASE: &str = "http://101.133.153.37:8080";
pub const DEFAULT_CORE_API_BASE: &str = "http://101.133.153.37:8080";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaitecSession {
    #[serde(default)]
    pub auth_token: Option<String>,
    pub api_key: String,
    pub token_type: String,
    pub user_id: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub api_key_id: Option<String>,
    #[serde(default)]
    pub api_key_name: Option<String>,
    #[serde(default)]
    pub api_key_created_at: Option<String>,
    #[serde(default)]
    pub api_key_expires_at: Option<String>,
    pub last_validated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct LegacySaitecSession {
    #[serde(default)]
    auth_token: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaitecValidationResult {
    pub is_valid: bool,
    pub user_id: Option<String>,
    pub expires_at: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaitecApiKeyResult {
    pub id: String,
    pub name: String,
    pub raw_key: String,
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
}

fn normalize_optional_token(value: Option<String>) -> Option<String> {
    value.and_then(|token| {
        let trimmed = token.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn normalized_auth_token(session: &SaitecSession) -> Option<String> {
    normalize_optional_token(session.auth_token.clone())
}

fn build_session(
    auth_token: Option<String>,
    api_key: String,
    token_type: String,
    user_profile: UserProfileData,
    api_key_result: Option<&SaitecApiKeyResult>,
) -> SaitecSession {
    SaitecSession {
        auth_token: normalize_optional_token(auth_token),
        api_key,
        token_type,
        user_id: Some(user_profile.user_id),
        email: user_profile.email,
        phone: user_profile.phone,
        display_name: user_profile.display_name,
        api_key_id: api_key_result.map(|value| value.id.clone()),
        api_key_name: api_key_result.map(|value| value.name.clone()),
        api_key_created_at: api_key_result.and_then(|value| value.created_at.clone()),
        api_key_expires_at: api_key_result.and_then(|value| value.expires_at.clone()),
        last_validated_at: Some(chrono::Utc::now().to_rfc3339()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaitecLoginForm {
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

    pub fn validate(&self) -> Result<()> {
        if self.password.trim().is_empty() {
            anyhow::bail!("password cannot be empty");
        }

        if self.email.trim().is_empty() && self.phone.trim().is_empty() {
            anyhow::bail!("email and phone cannot both be empty");
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct ApiEnvelope<T> {
    success: bool,
    message: String,
    data: T,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct LoginData {
    token: String,
    user_id: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    phone: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct UserProfileData {
    user_id: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    avatar_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct ApiKeyData {
    id: String,
    name: String,
    is_active: bool,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    revoked_at: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    raw_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
struct SaitecRuntimeConfig {
    #[serde(default)]
    auth_base: Option<String>,
    #[serde(default)]
    core_api_base: Option<String>,
}

pub fn callback_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}{CALLBACK_PATH}")
}

pub fn authorize_url(port: u16) -> String {
    let base = auth_base();
    let trimmed_base = base.trim_end_matches('/');
    let callback_url = callback_url(port);
    let callback = urlencoding::encode(&callback_url);
    format!("{trimmed_base}/?redirect_uri={callback}")
}

pub fn bind_callback_listener(port: u16) -> Result<tokio::net::TcpListener> {
    crate::auth::oauth::bind_callback_listener(port)
}

pub fn load_session() -> Result<Option<SaitecSession>> {
    let path = crate::saitec::paths::auth_file()?;
    if !path.exists() {
        return Ok(None);
    }

    match crate::storage::read_json(&path) {
        Ok(session) => Ok(Some(session)),
        Err(err) => {
            let legacy = crate::storage::read_json::<LegacySaitecSession>(&path);
            match legacy {
                Ok(legacy_session) => {
                    let has_api_key = legacy_session
                        .api_key
                        .as_deref()
                        .map(|value| !value.trim().is_empty())
                        .unwrap_or(false);
                    let looks_like_legacy_auth = legacy_session
                        .auth_token
                        .as_deref()
                        .map(|value| !value.trim().is_empty())
                        .unwrap_or(false)
                        && legacy_session
                            .token_type
                            .as_deref()
                            .map(|value| !value.trim().is_empty())
                            .unwrap_or(false);
                    if has_api_key || !looks_like_legacy_auth {
                        Err(err)
                    } else {
                        Ok(None)
                    }
                }
                Err(_) => Err(err),
            }
        }
    }
}

pub fn save_session(session: &SaitecSession) -> Result<()> {
    let path = crate::saitec::paths::auth_file()?;
    crate::storage::write_json_secret(&path, session)?;

    crate::provider_catalog::save_env_value_to_env_file(
        crate::subscription_catalog::JCODE_API_KEY_ENV,
        crate::subscription_catalog::JCODE_ENV_FILE,
        Some(session.api_key.as_str()),
    )?;

    Ok(())
}

pub fn clear_session() -> Result<()> {
    let path = crate::saitec::paths::auth_file()?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }

    let env_path =
        crate::storage::app_config_dir()?.join(crate::subscription_catalog::JCODE_ENV_FILE);
    if env_path.exists() {
        crate::storage::upsert_env_file_value(
            &env_path,
            crate::subscription_catalog::JCODE_API_KEY_ENV,
            None,
        )?;
    }
    crate::env::remove_var(crate::subscription_catalog::JCODE_API_KEY_ENV);
    Ok(())
}

pub fn auth_base() -> String {
    std::env::var("SAITEC_AUTH_BASE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            load_runtime_config()
                .ok()
                .and_then(|cfg| cfg.auth_base)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| DEFAULT_AUTH_BASE.to_string())
}

pub fn core_api_base() -> String {
    std::env::var("CORE_API_BASE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var(crate::subscription_catalog::JCODE_API_BASE_ENV).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            crate::subscription_catalog::configured_api_base()
                .map(|value| value.trim().trim_end_matches("/v1").to_string())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| {
            load_runtime_config()
                .ok()
                .and_then(|cfg| cfg.core_api_base)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| DEFAULT_CORE_API_BASE.to_string())
}

pub(crate) fn generate_api_key_name_for_time(now: chrono::DateTime<chrono::Utc>) -> String {
    format!("SAITEC-TUI-{}", now.format("%Y%m%d-%H%M%S"))
}

pub fn generate_api_key_name() -> String {
    generate_api_key_name_for_time(chrono::Utc::now())
}

fn login_request_body(form: &SaitecLoginForm) -> serde_json::Value {
    serde_json::json!({
        "email": if form.email.trim().is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(form.email.trim().to_string())
        },
        "phone": if form.phone.trim().is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(form.phone.trim().to_string())
        },
        "password": form.password,
    })
}

async fn login_with_password(form: &SaitecLoginForm) -> Result<LoginData> {
    form.validate()?;

    let url = format!(
        "{}/api/v1/auth/login",
        core_api_base().trim_end_matches('/')
    );
    let response = reqwest::Client::new()
        .post(url)
        .json(&login_request_body(form))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("login failed with {}: {}", status, body);
    }

    let payload: ApiEnvelope<serde_json::Value> = response.json().await?;
    if !payload.success {
        anyhow::bail!("{}", payload.message);
    }

    Ok(serde_json::from_value(payload.data)?)
}

pub async fn validate_api_key(api_key: &str) -> Result<SaitecValidationResult> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return Ok(SaitecValidationResult {
            is_valid: false,
            user_id: None,
            expires_at: None,
            message: Some("missing API key".to_string()),
        });
    }

    let url = format!("{}/api/v1/users/me", core_api_base().trim_end_matches('/'));
    let client = reqwest::Client::new();
    let response = client.get(url).bearer_auth(trimmed).send().await?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Ok(SaitecValidationResult {
            is_valid: false,
            user_id: None,
            expires_at: None,
            message: Some("API key unauthorized".to_string()),
        });
    }

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("API key validation failed with {}: {}", status, body);
    }

    let payload: ApiEnvelope<serde_json::Value> = response.json().await?;
    if !payload.success {
        return Ok(SaitecValidationResult {
            is_valid: false,
            user_id: None,
            expires_at: None,
            message: Some(payload.message),
        });
    }

    let profile: UserProfileData = serde_json::from_value(payload.data)?;
    Ok(SaitecValidationResult {
        is_valid: true,
        user_id: Some(profile.user_id),
        expires_at: None,
        message: None,
    })
}

pub async fn validate_token(token: &str) -> Result<SaitecValidationResult> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Ok(SaitecValidationResult {
            is_valid: false,
            user_id: None,
            expires_at: None,
            message: Some("missing auth token".to_string()),
        });
    }

    let profile = fetch_user_profile(trimmed).await;
    match profile {
        Ok(profile) => Ok(SaitecValidationResult {
            is_valid: true,
            user_id: Some(profile.user_id),
            expires_at: None,
            message: None,
        }),
        Err(err) => {
            let message = err.to_string();
            if message.contains("401") || message.to_ascii_lowercase().contains("unauthorized") {
                Ok(SaitecValidationResult {
                    is_valid: false,
                    user_id: None,
                    expires_at: None,
                    message: Some("auth token unauthorized".to_string()),
                })
            } else {
                Err(err)
            }
        }
    }
}

pub async fn create_api_key(token: &str) -> Result<SaitecApiKeyResult> {
    let url = format!("{}/api/v1/api-keys", core_api_base().trim_end_matches('/'));
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "name": generate_api_key_name(),
    });
    let response = client
        .post(url)
        .bearer_auth(token.trim())
        .json(&body)
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        anyhow::bail!("token unauthorized");
    }

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("api key creation failed with {}: {}", status, body);
    }

    let payload: ApiEnvelope<serde_json::Value> = response.json().await?;
    if !payload.success {
        anyhow::bail!("{}", payload.message);
    }

    let data: ApiKeyData = serde_json::from_value(payload.data)?;
    let raw_key = data
        .raw_key
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("api key response missing raw_key"))?;
    Ok(SaitecApiKeyResult {
        id: data.id,
        name: data.name,
        raw_key,
        created_at: data.created_at,
        expires_at: data.expires_at,
    })
}

pub async fn refresh_session_from_api_key(session: &SaitecSession) -> Result<SaitecSession> {
    if let Some(auth_token) = normalized_auth_token(session) {
        let validation = validate_token(&auth_token).await?;
        if !validation.is_valid {
            anyhow::bail!(
                "{}",
                validation
                    .message
                    .unwrap_or_else(|| "auth token validation failed".to_string())
            );
        }

        let user_profile = fetch_user_profile(&auth_token).await?;
        return Ok(SaitecSession {
            auth_token: Some(auth_token),
            api_key: session.api_key.clone(),
            token_type: session.token_type.clone(),
            user_id: Some(user_profile.user_id),
            email: user_profile.email,
            phone: user_profile.phone,
            display_name: user_profile.display_name,
            api_key_id: session.api_key_id.clone(),
            api_key_name: session.api_key_name.clone(),
            api_key_created_at: session.api_key_created_at.clone(),
            api_key_expires_at: session.api_key_expires_at.clone(),
            last_validated_at: Some(chrono::Utc::now().to_rfc3339()),
        });
    }

    let validation = validate_api_key(&session.api_key).await?;
    if !validation.is_valid {
        anyhow::bail!(
            "{}",
            validation
                .message
                .unwrap_or_else(|| "API key validation failed".to_string())
        );
    }

    Ok(SaitecSession {
        auth_token: None,
        api_key: session.api_key.clone(),
        token_type: session.token_type.clone(),
        user_id: session.user_id.clone().or(validation.user_id),
        email: session.email.clone(),
        phone: session.phone.clone(),
        display_name: session.display_name.clone(),
        api_key_id: session.api_key_id.clone(),
        api_key_name: session.api_key_name.clone(),
        api_key_created_at: session.api_key_created_at.clone(),
        api_key_expires_at: session.api_key_expires_at.clone(),
        last_validated_at: Some(chrono::Utc::now().to_rfc3339()),
    })
}

pub async fn submit_business_login(form: &SaitecLoginForm) -> Result<SaitecSession> {
    form.validate()?;

    let login = login_with_password(form).await?;
    let api_key = create_api_key(&login.token).await?;
    let user_profile = fetch_user_profile(&login.token).await?;

    Ok(build_session(
        Some(login.token),
        api_key.raw_key.clone(),
        "Bearer".to_string(),
        user_profile,
        Some(&api_key),
    ))
}

pub async fn refresh_saved_session_if_present() -> Result<Option<SaitecSession>> {
    let Some(session) = load_session()? else {
        return Ok(None);
    };

    match refresh_session_from_api_key(&session).await {
        Ok(refreshed) => {
            save_session(&refreshed)?;
            Ok(Some(refreshed))
        }
        Err(err) => {
            clear_session()?;
            Err(err)
        }
    }
}

pub async fn session_from_auth_token(token: &str) -> Result<SaitecSession> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Callback input missing auth_token");
    }

    let validation = validate_token(trimmed).await?;
    if !validation.is_valid {
        anyhow::bail!(
            "{}",
            validation
                .message
                .unwrap_or_else(|| "token validation failed".to_string())
        );
    }

    let user_profile = fetch_user_profile(trimmed).await?;
    let api_key = create_api_key(trimmed).await?;
    Ok(build_session(
        Some(trimmed.to_string()),
        api_key.raw_key.clone(),
        "Bearer".to_string(),
        user_profile,
        Some(&api_key),
    ))
}

pub fn ensure_logged_in() -> Result<()> {
    match load_session() {
        Ok(Some(session)) if !session.api_key.trim().is_empty() => return Ok(()),
        Ok(_) => {}
        Err(err) => {
            if crate::subscription_catalog::configured_api_key().is_some() {
                return Ok(());
            }
            return Err(err);
        }
    }

    if crate::subscription_catalog::configured_api_key().is_some() {
        return Ok(());
    }

    anyhow::bail!("Saitec login required. Run `/login` in the TUI.");
}

pub async fn wait_for_auth_callback(listener: tokio::net::TcpListener) -> Result<SaitecSession> {
    use tokio::io::{AsyncWriteExt, BufReader};

    loop {
        let (stream, _) = listener.accept().await?;
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut request_line = String::new();
        let maybe_line = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut request_line),
        )
        .await;
        match maybe_line {
            Ok(Ok(0)) | Err(_) => continue,
            Ok(Ok(_)) => {
                if request_line.trim().is_empty() {
                    continue;
                }
                let mut header_line = String::new();
                loop {
                    header_line.clear();
                    match tokio::time::timeout(
                        Duration::from_secs(5),
                        tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut header_line),
                    )
                    .await
                    {
                        Ok(Ok(0)) | Err(_) => break,
                        Ok(Ok(_)) if header_line.trim().is_empty() => break,
                        Ok(Ok(_)) => {}
                        Ok(Err(err)) => return Err(err.into()),
                    }
                }

                let parts: Vec<&str> = request_line.split_whitespace().collect();
                if parts.len() < 2 {
                    let _ = writer
                        .write_all(
                            b"HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nConnection: close\r\nContent-Length: 26\r\n\r\nInvalid callback request.\n",
                        )
                        .await;
                    continue;
                }

                let path = parts[1];
                let callback = format!("http://localhost{path}");
                match session_from_callback_input(&callback).await {
                    Ok(session) => {
                        let body = "<html><body><h1>Saitec login complete</h1><p>You can close this window and return to the TUI.</p></body></html>";
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        writer.write_all(response.as_bytes()).await?;
                        return Ok(session);
                    }
                    Err(err) => {
                        let message = format!(
                            "<html><body><h1>Saitec login not completed</h1><p>{}</p><p>You can close this window and retry the login flow.</p></body></html>",
                            err
                        );
                        let response = format!(
                            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                            message.len(),
                            message
                        );
                        let _ = writer.write_all(response.as_bytes()).await;
                        continue;
                    }
                }
            }
            Ok(Err(err)) => return Err(err.into()),
        }
    }
}

pub async fn session_from_callback_input(input: &str) -> Result<SaitecSession> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Callback input is empty");
    }

    let candidate = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.starts_with('?') {
        format!("http://localhost{CALLBACK_PATH}{trimmed}")
    } else if trimmed.contains("auth_token=") {
        format!("http://localhost{CALLBACK_PATH}?{trimmed}")
    } else {
        anyhow::bail!("Callback input must contain auth_token");
    };

    let url = url::Url::parse(&candidate)?;
    let auth_token = url
        .query_pairs()
        .find(|(key, _)| key == "auth_token")
        .map(|(_, value)| value.to_string())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Callback input missing auth_token"))?;
    session_from_auth_token(&auth_token).await
}

async fn fetch_user_profile(token: &str) -> Result<UserProfileData> {
    let url = format!("{}/api/v1/users/me", core_api_base().trim_end_matches('/'));
    let client = reqwest::Client::new();
    let response = client.get(url).bearer_auth(token.trim()).send().await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("profile request failed with {}: {}", status, body);
    }
    let payload: ApiEnvelope<serde_json::Value> = response.json().await?;
    if !payload.success {
        anyhow::bail!("{}", payload.message);
    }
    Ok(serde_json::from_value(payload.data)?)
}

fn load_runtime_config() -> Result<SaitecRuntimeConfig> {
    let path = crate::config::Config::path().ok_or_else(|| anyhow::anyhow!("No config path"))?;
    if !path.exists() {
        return Ok(SaitecRuntimeConfig::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))?;
    let providers = value
        .get("providers")
        .and_then(toml::Value::as_table)
        .and_then(|providers| providers.get("saitec"))
        .and_then(toml::Value::as_table);
    let auth_base = providers
        .and_then(|table| table.get("auth_base"))
        .and_then(toml::Value::as_str)
        .map(str::to_string);
    let core_api_base = providers
        .and_then(|table| table.get("core_api_base"))
        .or_else(|| providers.and_then(|table| table.get("base_url")))
        .and_then(toml::Value::as_str)
        .map(str::to_string);
    Ok(SaitecRuntimeConfig {
        auth_base,
        core_api_base,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set_path(key: &'static str, value: &std::path::Path) -> Self {
            let previous = std::env::var_os(key);
            crate::env::set_var(key, value);
            Self { key, previous }
        }

        fn set_value(key: &'static str, value: &str) -> Self {
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
            if let Some(previous) = &self.previous {
                crate::env::set_var(self.key, previous);
            } else {
                crate::env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn load_session_returns_none_when_auth_file_missing() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        assert!(load_session().expect("load session").is_none());
    }

    #[test]
    fn load_session_treats_legacy_auth_file_without_business_api_key_as_logged_out() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let _api_key_guard = EnvVarGuard::remove(crate::subscription_catalog::JCODE_API_KEY_ENV);
        let auth_path = crate::saitec::paths::auth_file().expect("auth path");
        std::fs::create_dir_all(auth_path.parent().expect("auth parent")).expect("create auth dir");
        std::fs::write(
            &auth_path,
            r#"{"auth_token":"legacy-token","token_type":"Bearer","user_id":"legacy-user"}"#,
        )
        .expect("write legacy auth file");

        assert!(load_session().expect("load session").is_none());

        let error = ensure_logged_in().expect_err("legacy auth file should require login");
        assert!(error.to_string().contains("Saitec login required"));
    }

    #[test]
    fn load_session_preserves_parse_error_for_parseable_but_non_legacy_auth_file() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let _api_key_guard = EnvVarGuard::remove(crate::subscription_catalog::JCODE_API_KEY_ENV);
        let auth_path = crate::saitec::paths::auth_file().expect("auth path");
        std::fs::create_dir_all(auth_path.parent().expect("auth parent")).expect("create auth dir");
        std::fs::write(&auth_path, r#"{"foo":"bar"}"#).expect("write parseable invalid auth file");

        let error = load_session().expect_err("parseable but non-legacy auth file should error");

        assert!(
            error.to_string().contains("missing field `api_key`"),
            "unexpected parse error: {error:#}"
        );
    }

    #[test]
    fn load_session_preserves_parse_error_for_corrupted_current_format_auth_file() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let _api_key_guard = EnvVarGuard::remove(crate::subscription_catalog::JCODE_API_KEY_ENV);
        let auth_path = crate::saitec::paths::auth_file().expect("auth path");
        std::fs::create_dir_all(auth_path.parent().expect("auth parent")).expect("create auth dir");
        std::fs::write(
            &auth_path,
            r#"{"api_key":"sk-live","token_type":"Bearer","user_id":"mock-user""#,
        )
        .expect("write corrupted auth file");

        let error = load_session().expect_err("corrupted current-format auth file should error");

        assert!(
            error.to_string().contains("EOF") || error.to_string().contains("expected"),
            "unexpected parse error: {error:#}"
        );
    }

    #[test]
    fn validate_api_key_marks_empty_tokens_as_missing_api_key() {
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = runtime
            .block_on(validate_api_key(""))
            .expect("validate api key");

        assert!(!result.is_valid);
        assert_eq!(result.message.as_deref(), Some("missing API key"));
    }

    #[test]
    fn validate_api_key_marks_unauthorized_api_keys_as_invalid() {
        let _lock = crate::storage::lock_test_env();
        let server = spawn_single_json_response_server(
            401,
            "Unauthorized",
            r#"{"success":false,"message":"unauthorized","data":{}}"#,
        );
        let _core_api = EnvVarGuard::set_value("CORE_API_BASE", &server);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

        let result = runtime
            .block_on(validate_api_key("sk-invalid"))
            .expect("validate api key");

        assert!(!result.is_valid);
        assert_eq!(result.user_id, None);
        assert_eq!(result.message.as_deref(), Some("API key unauthorized"));
    }

    #[test]
    fn validate_api_key_treats_success_false_response_with_empty_data_as_invalid() {
        let _lock = crate::storage::lock_test_env();
        let server = spawn_single_json_response_server(
            200,
            "OK",
            r#"{"success":false,"message":"api key invalid","data":{}}"#,
        );
        let _core_api = EnvVarGuard::set_value("CORE_API_BASE", &server);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

        let result = runtime
            .block_on(validate_api_key("sk-invalid"))
            .expect("success:false response should be treated as invalid, not as an error");

        assert!(!result.is_valid);
        assert_eq!(result.user_id, None);
        assert_eq!(result.message.as_deref(), Some("api key invalid"));
    }

    #[test]
    fn login_form_validation_requires_password_and_one_account_identifier() {
        let error = SaitecLoginForm::new("".to_string(), "".to_string(), "".to_string())
            .validate()
            .expect_err("empty form should fail")
            .to_string();
        assert!(error.contains("password"), "unexpected error: {error}");

        let error = SaitecLoginForm::new("".to_string(), "".to_string(), "secret".to_string())
            .validate()
            .expect_err("missing email and phone should fail")
            .to_string();
        assert!(error.contains("email"), "unexpected error: {error}");
        assert!(error.contains("phone"), "unexpected error: {error}");
    }

    #[test]
    fn login_with_password_rejects_success_false_response_with_empty_data() {
        let _lock = crate::storage::lock_test_env();
        let server = spawn_single_json_response_server(
            200,
            "OK",
            r#"{"success":false,"message":"invalid credentials","data":{}}"#,
        );
        let _core_api = EnvVarGuard::set_value("CORE_API_BASE", &server);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

        let error = runtime
            .block_on(login_with_password(&SaitecLoginForm::new(
                "user@example.com".to_string(),
                "".to_string(),
                "wrong-password".to_string(),
            )))
            .expect_err("success:false login response should be handled as a clean error");

        let message = error.to_string();
        assert!(
            message.contains("invalid credentials"),
            "unexpected error: {message}"
        );
        assert!(
            !message.contains("missing field"),
            "login failure should not leak deserialization details: {message}"
        );
    }

    #[test]
    fn generated_api_key_name_uses_saitec_prefix_and_timestamp_shape() {
        let name = generate_api_key_name_for_time(
            chrono::DateTime::parse_from_rfc3339("2026-05-14T15:30:00Z")
                .expect("parse")
                .with_timezone(&chrono::Utc),
        );

        assert_eq!(name, "SAITEC-TUI-20260514-153000");
    }

    #[test]
    fn create_api_key_uses_generated_name_in_request_body() {
        let _lock = crate::storage::lock_test_env();
        let server = spawn_api_key_creation_server();
        let _core_api = EnvVarGuard::set_value("CORE_API_BASE", &server);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

        let result = runtime
            .block_on(create_api_key("jwt-token"))
            .expect("api key creation should succeed");

        assert_eq!(result.id, "key-123");
        assert!(
            result.name.starts_with("SAITEC-TUI-"),
            "returned name should carry generated prefix: {}",
            result.name
        );
    }

    #[test]
    fn create_api_key_rejects_success_false_response_with_empty_data() {
        let _lock = crate::storage::lock_test_env();
        let server = spawn_single_json_response_server(
            200,
            "OK",
            r#"{"success":false,"message":"api key quota reached","data":{}}"#,
        );
        let _core_api = EnvVarGuard::set_value("CORE_API_BASE", &server);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

        let error = runtime
            .block_on(create_api_key("jwt-token"))
            .expect_err("success:false api key response should be handled as a clean error");

        let message = error.to_string();
        assert!(
            message.contains("api key quota reached"),
            "unexpected error: {message}"
        );
        assert!(
            !message.contains("missing field"),
            "api key creation failure should not leak deserialization details: {message}"
        );
    }

    #[test]
    fn fetch_user_profile_rejects_success_false_response_with_empty_data() {
        let _lock = crate::storage::lock_test_env();
        let server = spawn_single_json_response_server(
            200,
            "OK",
            r#"{"success":false,"message":"user profile unavailable","data":{}}"#,
        );
        let _core_api = EnvVarGuard::set_value("CORE_API_BASE", &server);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

        let error = runtime
            .block_on(fetch_user_profile("sk-live"))
            .expect_err("success:false users/me response should be handled as a clean error");

        let message = error.to_string();
        assert!(
            message.contains("user profile unavailable"),
            "unexpected error: {message}"
        );
        assert!(
            !message.contains("missing field"),
            "profile failure should not leak deserialization details: {message}"
        );
    }

    #[test]
    fn refresh_session_from_auth_token_updates_profile_fields_after_valid_users_me() {
        let _lock = crate::storage::lock_test_env();
        let server = spawn_refresh_session_server();
        let _core_api = EnvVarGuard::set_value("CORE_API_BASE", &server);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

        let original = SaitecSession {
            auth_token: Some("jwt-refresh-token".to_string()),
            api_key: "sk-business-live".to_string(),
            token_type: "Bearer".to_string(),
            user_id: Some("stale-user".to_string()),
            email: Some("stale@example.com".to_string()),
            phone: None,
            display_name: Some("Old Name".to_string()),
            api_key_id: Some("key-keep".to_string()),
            api_key_name: Some("SAITEC-TUI-20260514-120000".to_string()),
            api_key_created_at: Some("2026-05-14T12:00:00Z".to_string()),
            api_key_expires_at: Some("2026-06-14T12:00:00Z".to_string()),
            last_validated_at: Some("2026-05-14T12:30:00Z".to_string()),
        };

        let refreshed = runtime
            .block_on(refresh_session_from_api_key(&original))
            .expect("refresh should succeed");

        assert_eq!(refreshed.api_key, original.api_key);
        assert_eq!(refreshed.token_type, original.token_type);
        assert_eq!(refreshed.api_key_id, original.api_key_id);
        assert_eq!(refreshed.api_key_name, original.api_key_name);
        assert_eq!(refreshed.api_key_created_at, original.api_key_created_at);
        assert_eq!(refreshed.api_key_expires_at, original.api_key_expires_at);
        assert_eq!(refreshed.user_id.as_deref(), Some("user-456"));
        assert_eq!(refreshed.email.as_deref(), Some("fresh@example.com"));
        assert_eq!(refreshed.phone.as_deref(), Some("13800138000"));
        assert_eq!(refreshed.display_name.as_deref(), Some("Fresh User"));
        assert!(
            refreshed.last_validated_at.is_some(),
            "refresh should stamp validation time"
        );
        assert_ne!(refreshed.last_validated_at, original.last_validated_at);
    }

    #[test]
    fn submit_business_login_returns_session_with_business_api_key_metadata_and_user_info_and_persists_auth_token()
     {
        let _lock = crate::storage::lock_test_env();
        let server = spawn_business_login_flow_server();
        let _core_api = EnvVarGuard::set_value("CORE_API_BASE", &server);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

        let session = runtime
            .block_on(submit_business_login(&SaitecLoginForm::new(
                " user@example.com ".to_string(),
                "".to_string(),
                "secret-password".to_string(),
            )))
            .expect("business login should succeed");

        assert_eq!(session.auth_token.as_deref(), Some("jwt-session-token"));
        assert_eq!(session.api_key, "sk_business_live_key");
        assert_eq!(session.token_type, "Bearer");
        assert_eq!(session.user_id.as_deref(), Some("user-789"));
        assert_eq!(session.email.as_deref(), Some("user@example.com"));
        assert_eq!(session.phone.as_deref(), Some("13900139000"));
        assert_eq!(session.display_name.as_deref(), Some("Business User"));
        assert_eq!(session.api_key_id.as_deref(), Some("key-789"));
        assert_eq!(
            session.api_key_name.as_deref(),
            Some("SAITEC-TUI-20260514-153000")
        );
        assert_eq!(
            session.api_key_created_at.as_deref(),
            Some("2026-05-14T15:30:00Z")
        );
        assert_eq!(
            session.api_key_expires_at.as_deref(),
            Some("2026-06-14T15:30:00Z")
        );
        assert!(
            session.last_validated_at.is_some(),
            "business login should stamp validation time"
        );

        let serialized = serde_json::to_string(&session).expect("serialize session");
        assert!(
            serialized.contains("jwt-session-token"),
            "session should persist auth token material for startup validation: {serialized}"
        );
    }

    #[test]
    fn login_request_body_trims_identifiers_and_uses_null_for_blank_fields() {
        let body = login_request_body(&SaitecLoginForm::new(
            " user@example.com ".to_string(),
            "   ".to_string(),
            "secret".to_string(),
        ));

        assert_eq!(
            body,
            serde_json::json!({
                "email": "user@example.com",
                "phone": null,
                "password": "secret",
            })
        );
    }

    #[test]
    fn save_and_reload_session_round_trips_business_api_key_and_auth_token() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());

        let session = SaitecSession {
            auth_token: Some("jwt-stored".to_string()),
            api_key: "sk-live".to_string(),
            token_type: "Bearer".to_string(),
            user_id: Some("mock-user".to_string()),
            email: Some("mock@example.com".to_string()),
            phone: None,
            display_name: Some("Mock User".to_string()),
            api_key_id: Some("key-1".to_string()),
            api_key_name: Some("SAITEC-TUI".to_string()),
            api_key_created_at: Some("2026-05-11T10:00:00Z".to_string()),
            api_key_expires_at: None,
            last_validated_at: None,
        };

        save_session(&session).expect("save session");
        let loaded = load_session()
            .expect("load session")
            .expect("stored session");

        assert_eq!(loaded.user_id.as_deref(), Some("mock-user"));
        assert_eq!(loaded.auth_token.as_deref(), Some("jwt-stored"));
        assert_eq!(loaded.api_key, "sk-live");
        assert_eq!(
            crate::provider_catalog::load_api_key_from_env_or_config(
                crate::subscription_catalog::JCODE_API_KEY_ENV,
                crate::subscription_catalog::JCODE_ENV_FILE,
            )
            .as_deref(),
            Some("sk-live")
        );
    }

    #[test]
    fn ensure_logged_in_fails_when_session_missing() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let _api_key_guard = EnvVarGuard::remove(crate::subscription_catalog::JCODE_API_KEY_ENV);

        let error = ensure_logged_in().expect_err("missing session should fail");

        assert!(error.to_string().contains("Saitec login required"));
    }

    #[test]
    fn ensure_logged_in_fails_when_api_key_is_missing() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let _api_key_guard = EnvVarGuard::remove(crate::subscription_catalog::JCODE_API_KEY_ENV);

        save_session(&SaitecSession {
            auth_token: None,
            api_key: String::new(),
            token_type: "Bearer".to_string(),
            user_id: Some("mock-user".to_string()),
            email: None,
            phone: None,
            display_name: None,
            api_key_id: None,
            api_key_name: None,
            api_key_created_at: None,
            api_key_expires_at: None,
            last_validated_at: None,
        })
        .expect("save session");

        let error = ensure_logged_in().expect_err("missing api key should fail");

        assert!(error.to_string().contains("Saitec login required"));
    }

    #[test]
    fn ensure_logged_in_accepts_env_backed_configured_api_key_when_session_file_is_missing() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let _api_key_guard = EnvVarGuard::remove(crate::subscription_catalog::JCODE_API_KEY_ENV);

        crate::provider_catalog::save_env_value_to_env_file(
            crate::subscription_catalog::JCODE_API_KEY_ENV,
            crate::subscription_catalog::JCODE_ENV_FILE,
            Some("sk-env-configured"),
        )
        .expect("save env backed api key");

        ensure_logged_in().expect("env-backed configured API key should count as logged in");
    }

    #[test]
    fn ensure_logged_in_accepts_env_backed_configured_api_key_when_session_file_is_invalid() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let _api_key_guard = EnvVarGuard::remove(crate::subscription_catalog::JCODE_API_KEY_ENV);
        let auth_path = crate::saitec::paths::auth_file().expect("auth path");
        std::fs::create_dir_all(auth_path.parent().expect("auth parent")).expect("create auth dir");
        std::fs::write(&auth_path, r#"{"api_key":"legacy-non-empty"}"#)
            .expect("write invalid auth file");

        assert!(
            load_session().is_err(),
            "test precondition: malformed session should error"
        );

        crate::provider_catalog::save_env_value_to_env_file(
            crate::subscription_catalog::JCODE_API_KEY_ENV,
            crate::subscription_catalog::JCODE_ENV_FILE,
            Some("sk-env-configured"),
        )
        .expect("save env backed api key");

        ensure_logged_in()
            .expect("env-backed configured API key should override invalid session file");
    }

    #[test]
    fn refresh_saved_session_if_present_rewrites_refreshed_identity_fields() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let server = spawn_refresh_session_server();
        let _core_api = EnvVarGuard::set_value("CORE_API_BASE", &server);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

        save_session(&SaitecSession {
            auth_token: Some("jwt-refresh-token".to_string()),
            api_key: "sk-business-live".to_string(),
            token_type: "Bearer".to_string(),
            user_id: Some("stale-user".to_string()),
            email: Some("stale@example.com".to_string()),
            phone: None,
            display_name: Some("Old Name".to_string()),
            api_key_id: Some("key-keep".to_string()),
            api_key_name: Some("SAITEC-TUI-20260514-120000".to_string()),
            api_key_created_at: Some("2026-05-14T12:00:00Z".to_string()),
            api_key_expires_at: Some("2026-06-14T12:00:00Z".to_string()),
            last_validated_at: None,
        })
        .expect("save stale session");

        let refreshed = runtime
            .block_on(refresh_saved_session_if_present())
            .expect("refresh should succeed")
            .expect("session should remain available");

        assert_eq!(refreshed.user_id.as_deref(), Some("user-456"));
        let loaded = load_session()
            .expect("load refreshed session")
            .expect("stored refreshed session");
        assert_eq!(loaded.email.as_deref(), Some("fresh@example.com"));
        assert_eq!(loaded.display_name.as_deref(), Some("Fresh User"));
        assert!(loaded.last_validated_at.is_some());
    }

    #[test]
    fn refresh_saved_session_if_present_clears_invalid_saved_session() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let server = spawn_single_json_response_server(
            401,
            "Unauthorized",
            r#"{"success":false,"message":"unauthorized","data":{}}"#,
        );
        let _core_api = EnvVarGuard::set_value("CORE_API_BASE", &server);
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

        save_session(&SaitecSession {
            auth_token: Some("expired-jwt".to_string()),
            api_key: "sk-invalid".to_string(),
            token_type: "Bearer".to_string(),
            user_id: Some("stale-user".to_string()),
            email: Some("stale@example.com".to_string()),
            phone: None,
            display_name: Some("Old Name".to_string()),
            api_key_id: None,
            api_key_name: None,
            api_key_created_at: None,
            api_key_expires_at: None,
            last_validated_at: None,
        })
        .expect("save invalid session");

        let error = runtime
            .block_on(refresh_saved_session_if_present())
            .expect_err("invalid saved session should fail refresh");
        assert!(
            error.to_string().contains("unauthorized"),
            "unexpected refresh error: {error}"
        );
        assert!(
            load_session()
                .expect("load session after invalid refresh")
                .is_none()
        );
    }

    #[test]
    fn core_api_base_defaults_to_saitec_server_when_no_override_exists() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let _core_api = EnvVarGuard {
            key: "CORE_API_BASE",
            previous: std::env::var_os("CORE_API_BASE"),
        };
        crate::env::remove_var("CORE_API_BASE");
        let _saitec_api = EnvVarGuard {
            key: crate::subscription_catalog::JCODE_API_BASE_ENV,
            previous: std::env::var_os(crate::subscription_catalog::JCODE_API_BASE_ENV),
        };
        crate::env::remove_var(crate::subscription_catalog::JCODE_API_BASE_ENV);

        assert_eq!(core_api_base(), DEFAULT_CORE_API_BASE);
        assert_eq!(DEFAULT_CORE_API_BASE, "http://101.133.153.37:8080");
    }

    #[test]
    fn authorize_url_uses_saitec_homepage_with_local_callback_redirect() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let _auth_base = EnvVarGuard {
            key: "SAITEC_AUTH_BASE",
            previous: std::env::var_os("SAITEC_AUTH_BASE"),
        };
        crate::env::remove_var("SAITEC_AUTH_BASE");

        assert_eq!(
            authorize_url(DEFAULT_CALLBACK_PORT),
            "http://101.133.153.37:8080/?redirect_uri=http%3A%2F%2F127.0.0.1%3A1455%2Fauth%2Fcallback"
        );
    }

    #[test]
    fn session_from_callback_input_exchanges_real_token_and_api_key() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let server = spawn_saitec_flow_server();
        let _core_api = EnvVarGuard::set_value("CORE_API_BASE", &server);

        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
        let session = runtime
            .block_on(session_from_callback_input(
                "http://127.0.0.1:1455/auth/callback?auth_token=jwt-token",
            ))
            .expect("session from callback");

        assert_eq!(session.token_type, "Bearer");
        assert_eq!(session.user_id.as_deref(), Some("user-123"));
        assert_eq!(session.api_key, "sk_live_real_key");
        assert_eq!(session.display_name.as_deref(), Some("张三"));
    }

    #[tokio::test]
    async fn wait_for_auth_callback_extracts_auth_token_from_local_redirect() {
        let _lock = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
        let server = spawn_saitec_flow_server();
        let _core_api = EnvVarGuard::set_value("CORE_API_BASE", &server);
        let listener = bind_callback_listener(0).expect("bind callback listener");
        let port = listener.local_addr().expect("local addr").port();

        let task = tokio::spawn(async move { wait_for_auth_callback(listener).await });

        let response = reqwest::get(format!(
            "http://127.0.0.1:{port}{CALLBACK_PATH}?auth_token=jwt-token"
        ))
        .await
        .expect("http callback");
        assert!(response.status().is_success());

        let session = task.await.expect("join").expect("session");
        assert_eq!(session.user_id.as_deref(), Some("user-123"));
        assert_eq!(session.api_key, "sk_live_real_key");
    }

    fn spawn_saitec_flow_server() -> String {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        std::thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().expect("accept connection");
                let mut buf = [0u8; 4096];
                let size = stream.read(&mut buf).expect("read request");
                let request = String::from_utf8_lossy(&buf[..size]);
                let first_line = request.lines().next().unwrap_or_default().to_string();
                let authorization = request
                    .lines()
                    .find(|line| line.to_ascii_lowercase().starts_with("authorization:"))
                    .unwrap_or_default()
                    .to_string();

                let (status, body) = if first_line.starts_with("GET /api/v1/users/me ")
                    && authorization.contains("Bearer jwt-token")
                {
                    (
                        200,
                        r#"{"success":true,"message":"success","data":{"user_id":"user-123","email":"user@example.com","phone":null,"display_name":"张三","avatar_url":null}}"#.to_string(),
                    )
                } else if first_line.starts_with("POST /api/v1/api-keys ")
                    && authorization.contains("Bearer jwt-token")
                {
                    (
                        200,
                        r#"{"success":true,"message":"success","data":{"id":"key-123","name":"SAITEC-TUI","is_active":true,"expires_at":null,"revoked_at":null,"created_at":"2026-05-11T10:00:00Z","raw_key":"sk_live_real_key"}}"#.to_string(),
                    )
                } else {
                    (
                        401,
                        r#"{"success":false,"message":"unauthorized","data":{}}"#.to_string(),
                    )
                };
                let status_text = if status == 200 { "OK" } else { "Unauthorized" };
                let response = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status,
                    status_text,
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write response");
            }
        });
        format!("http://127.0.0.1:{}", addr.port())
    }

    fn spawn_refresh_session_server() -> String {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().expect("accept connection");
                let mut buf = [0u8; 4096];
                let size = stream.read(&mut buf).expect("read request");
                let request = String::from_utf8_lossy(&buf[..size]);
                let first_line = request.lines().next().unwrap_or_default().to_string();
                let authorization = request
                    .lines()
                    .find(|line| line.to_ascii_lowercase().starts_with("authorization:"))
                    .unwrap_or_default()
                    .to_string();

                let (status, status_text, body) = if first_line.starts_with("GET /api/v1/users/me ")
                    && authorization.contains("Bearer jwt-refresh-token")
                {
                    (
                        200,
                        "OK",
                        r#"{"success":true,"message":"success","data":{"user_id":"user-456","email":"fresh@example.com","phone":"13800138000","display_name":"Fresh User","avatar_url":null}}"#.to_string(),
                    )
                } else {
                    (
                        401,
                        "Unauthorized",
                        r#"{"success":false,"message":"unauthorized","data":{}}"#.to_string(),
                    )
                };

                let response = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status,
                    status_text,
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write response");
            }
        });
        format!("http://127.0.0.1:{}", addr.port())
    }

    fn spawn_business_login_flow_server() -> String {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        std::thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().expect("accept connection");
                let mut buf = [0u8; 4096];
                let size = stream.read(&mut buf).expect("read request");
                let request = String::from_utf8_lossy(&buf[..size]);
                let first_line = request.lines().next().unwrap_or_default().to_string();
                let authorization = request
                    .lines()
                    .find(|line| line.to_ascii_lowercase().starts_with("authorization:"))
                    .unwrap_or_default()
                    .to_string();
                let body = request
                    .split("\r\n\r\n")
                    .nth(1)
                    .unwrap_or_default()
                    .trim_end_matches('\0')
                    .to_string();

                let (status, status_text, response_body) = if first_line
                    .starts_with("POST /api/v1/auth/login ")
                {
                    let payload: serde_json::Value =
                        serde_json::from_str(&body).expect("login body should be valid json");
                    assert_eq!(
                        payload.get("email"),
                        Some(&serde_json::json!("user@example.com"))
                    );
                    assert_eq!(payload.get("phone"), Some(&serde_json::Value::Null));
                    assert_eq!(
                        payload.get("password"),
                        Some(&serde_json::json!("secret-password"))
                    );
                    (
                            200,
                            "OK",
                            r#"{"success":true,"message":"success","data":{"token":"jwt-session-token","user_id":"login-user","email":"user@example.com","phone":null}}"#.to_string(),
                        )
                } else if first_line.starts_with("POST /api/v1/api-keys ")
                    && authorization.contains("Bearer jwt-session-token")
                {
                    (
                            200,
                            "OK",
                            r#"{"success":true,"message":"success","data":{"id":"key-789","name":"SAITEC-TUI-20260514-153000","is_active":true,"expires_at":"2026-06-14T15:30:00Z","revoked_at":null,"created_at":"2026-05-14T15:30:00Z","raw_key":"sk_business_live_key"}}"#.to_string(),
                        )
                } else if first_line.starts_with("GET /api/v1/users/me ")
                    && authorization.contains("Bearer jwt-session-token")
                {
                    (
                            200,
                            "OK",
                            r#"{"success":true,"message":"success","data":{"user_id":"user-789","email":"user@example.com","phone":"13900139000","display_name":"Business User","avatar_url":null}}"#.to_string(),
                        )
                } else {
                    (
                        401,
                        "Unauthorized",
                        r#"{"success":false,"message":"unauthorized","data":{}}"#.to_string(),
                    )
                };

                let response = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status,
                    status_text,
                    response_body.len(),
                    response_body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write response");
            }
        });
        format!("http://127.0.0.1:{}", addr.port())
    }

    fn spawn_single_json_response_server(
        status: u16,
        status_text: &'static str,
        body: &'static str,
    ) -> String {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let body = body.to_string();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept connection");
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf).expect("read request");
            let response = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                status_text,
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });
        format!("http://127.0.0.1:{}", addr.port())
    }

    fn spawn_api_key_creation_server() -> String {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept connection");
            let mut buf = [0u8; 4096];
            let size = stream.read(&mut buf).expect("read request");
            let request = String::from_utf8_lossy(&buf[..size]);
            let body = request
                .split("\r\n\r\n")
                .nth(1)
                .unwrap_or_default()
                .trim_end_matches('\0');
            let payload: serde_json::Value =
                serde_json::from_str(body).expect("request body should be valid json");
            let name = payload
                .get("name")
                .and_then(serde_json::Value::as_str)
                .expect("request should include name");
            assert!(
                name.starts_with("SAITEC-TUI-"),
                "expected generated API key name, got {name}"
            );

            let response_body = format!(
                r#"{{"success":true,"message":"success","data":{{"id":"key-123","name":"{name}","is_active":true,"expires_at":null,"revoked_at":null,"created_at":"2026-05-11T10:00:00Z","raw_key":"sk_live_real_key"}}}}"#
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });
        format!("http://127.0.0.1:{}", addr.port())
    }
}
