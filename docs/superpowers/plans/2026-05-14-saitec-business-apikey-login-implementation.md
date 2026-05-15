# SAITEC Business APIKey Login Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current SAITEC browser-callback business login flow with a blocking TUI form that validates and persists only a business API key plus metadata, reuses valid stored keys at startup, and returns logged-out users to a required-login state until the business API key is valid.

**Architecture:** Keep SAITEC business login logic centralized in `src/saitec/auth.rs`, migrate the TUI from callback-style `PendingLogin::Saitec` handling to a structured SAITEC form state, and preserve the existing command/auth plumbing around `/login`, `/logout`, startup gating, and completion messages. Use local form validation plus a two-hop API flow (`/auth/login` then `/api-keys`) and refresh stored identity fields with `/users/me` whenever the business API key validates successfully.

**Tech Stack:** Rust, Tokio, reqwest, ratatui, existing JCode TUI state/render/input modules, cargo test.

---

### Task 1: Refactor SAITEC session storage from auth-token-first to APIKey-first

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\saitec\auth.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\saitec\auth.rs`

- [ ] **Step 1: Write the failing session-shape tests**

```rust
#[test]
fn save_and_reload_session_round_trips_business_api_key_without_jwt() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());

    let session = SaitecSession {
        api_key: "sk-live".to_string(),
        token_type: "Bearer".to_string(),
        user_id: Some("mock-user".to_string()),
        email: Some("mock@example.com".to_string()),
        phone: Some("13800000000".to_string()),
        display_name: Some("Mock User".to_string()),
        api_key_id: Some("key-1".to_string()),
        api_key_name: Some("SAITEC-TUI-20260514-153000".to_string()),
        api_key_created_at: Some("2026-05-14T15:30:00Z".to_string()),
        api_key_expires_at: None,
        last_validated_at: Some("2026-05-14T15:31:02Z".to_string()),
    };

    save_session(&session).expect("save session");
    let loaded = load_session()
        .expect("load session")
        .expect("stored session");

    assert_eq!(loaded.api_key, "sk-live");
    assert_eq!(loaded.user_id.as_deref(), Some("mock-user"));
    assert_eq!(loaded.api_key_name.as_deref(), Some("SAITEC-TUI-20260514-153000"));
}

#[test]
fn ensure_logged_in_fails_when_api_key_is_missing() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());

    crate::storage::write_json_secret(
        &crate::saitec::paths::auth_file().expect("auth file"),
        &serde_json::json!({
            "token_type": "Bearer"
        }),
    )
    .expect("write malformed auth");

    let error = ensure_logged_in().expect_err("missing api key should fail");
    assert!(error.to_string().contains("API key"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test save_and_reload_session_round_trips_business_api_key_without_jwt -- --exact`
Expected: FAIL because `SaitecSession` still requires `auth_token` and keeps `api_key` as `Option<String>`.

- [ ] **Step 3: Replace the session model with business-API-key-first fields**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaitecSession {
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

pub fn ensure_logged_in() -> Result<()> {
    let Some(session) = load_session()? else {
        anyhow::bail!("Saitec login required. Run `/login` in the TUI.");
    };
    if session.api_key.trim().is_empty() {
        anyhow::bail!("Saitec login required. Stored session is missing API key.");
    }
    Ok(())
}
```

- [ ] **Step 4: Keep env/config syncing aligned with the new required `api_key`**

```rust
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
```

- [ ] **Step 5: Run the focused session tests**

Run: `cargo test save_and_reload_session_round_trips_business_api_key_without_jwt ensure_logged_in_fails_when_api_key_is_missing -- --exact`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/saitec/auth.rs
git commit -m "refactor: store saitec sessions by business apikey"
```

### Task 2: Add SAITEC login form DTOs, validators, and API request helpers

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\saitec\auth.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\saitec\auth.rs`

- [ ] **Step 1: Write the failing login-form and name-generation tests**

```rust
#[test]
fn login_form_validation_requires_password_and_one_account_identifier() {
    let error = SaitecLoginForm::new("".to_string(), "".to_string(), "".to_string())
        .validate()
        .expect_err("empty form should fail");
    assert!(error.contains("password"));

    let error = SaitecLoginForm::new("".to_string(), "".to_string(), "secret".to_string())
        .validate()
        .expect_err("missing email and phone should fail");
    assert!(error.contains("email"));
    assert!(error.contains("phone"));
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test login_form_validation_requires_password_and_one_account_identifier -- --exact`
Expected: FAIL because `SaitecLoginForm` and `generate_api_key_name_for_time` do not exist yet.

- [ ] **Step 3: Add the form DTO and local validation helpers**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaitecLoginForm {
    pub email: String,
    pub phone: String,
    pub password: String,
}

impl SaitecLoginForm {
    pub fn new(email: String, phone: String, password: String) -> Self {
        Self { email, phone, password }
    }

    pub fn validate(&self) -> Result<()> {
        if self.password.trim().is_empty() {
            anyhow::bail!("Password cannot be empty.");
        }
        if self.email.trim().is_empty() && self.phone.trim().is_empty() {
            anyhow::bail!("Email and phone cannot both be empty.");
        }
        Ok(())
    }
}

fn generate_api_key_name_for_time(now: chrono::DateTime<chrono::Utc>) -> String {
    format!("SAITEC-TUI-{}", now.format("%Y%m%d-%H%M%S"))
}

pub fn generate_api_key_name() -> String {
    generate_api_key_name_for_time(chrono::Utc::now())
}
```

- [ ] **Step 4: Replace token-shaped helpers with login and validation helpers**

```rust
async fn login_with_password(form: &SaitecLoginForm) -> Result<LoginData> {
    form.validate()?;
    let url = format!("{}/api/v1/auth/login", core_api_base().trim_end_matches('/'));
    let response = reqwest::Client::new()
        .post(url)
        .json(&serde_json::json!({
            "email": if form.email.trim().is_empty() { serde_json::Value::Null } else { serde_json::Value::String(form.email.trim().to_string()) },
            "phone": if form.phone.trim().is_empty() { serde_json::Value::Null } else { serde_json::Value::String(form.phone.trim().to_string()) },
            "password": form.password,
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("login failed with {}: {}", status, body);
    }

    let payload: ApiEnvelope<LoginData> = response.json().await?;
    Ok(payload.data)
}

pub async fn validate_api_key(api_key: &str) -> Result<SaitecValidationResult> {
    validate_token(api_key).await
}
```

- [ ] **Step 5: Run the focused validation tests**

Run: `cargo test login_form_validation_requires_password_and_one_account_identifier generated_api_key_name_uses_saitec_prefix_and_timestamp_shape -- --exact`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/saitec/auth.rs
git commit -m "feat: add saitec business login form helpers"
```

### Task 3: Add refresh and login-exchange entrypoints for business API key sessions

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\saitec\auth.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\saitec\auth.rs`

- [ ] **Step 1: Write the failing session-refresh and login-exchange tests**

```rust
#[tokio::test]
async fn refresh_session_updates_profile_fields_after_valid_me_response() {
    let _lock = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _guard = EnvVarGuard::set_path("JCODE_HOME", temp.path());
    let server = spawn_saitec_test_server(vec![
        TestRoute::users_me_ok("mock-user", Some("user@example.com"), Some("13800000000"), Some("Mock User")),
    ])
    .await;
    let _api_base = EnvVarGuard::set_value("CORE_API_BASE", &server.base_url);

    let refreshed = refresh_session_from_api_key(&SaitecSession {
        api_key: "sk-live".to_string(),
        token_type: "Bearer".to_string(),
        user_id: None,
        email: None,
        phone: None,
        display_name: None,
        api_key_id: Some("key-1".to_string()),
        api_key_name: Some("SAITEC-TUI-20260514-153000".to_string()),
        api_key_created_at: None,
        api_key_expires_at: None,
        last_validated_at: None,
    })
    .await
    .expect("refresh session");

    assert_eq!(refreshed.user_id.as_deref(), Some("mock-user"));
    assert_eq!(refreshed.email.as_deref(), Some("user@example.com"));
    assert_eq!(refreshed.display_name.as_deref(), Some("Mock User"));
    assert!(refreshed.last_validated_at.is_some());
}

#[tokio::test]
async fn submit_business_login_returns_session_without_persisting_jwt() {
    let server = spawn_saitec_test_server(vec![
        TestRoute::login_ok("jwt-123", "mock-user", Some("user@example.com"), Some("13800000000")),
        TestRoute::api_key_create_ok("key-1", "SAITEC-TUI-20260514-153000", "sk-live"),
        TestRoute::users_me_ok("mock-user", Some("user@example.com"), Some("13800000000"), Some("Mock User")),
    ])
    .await;
    let _api_base = EnvVarGuard::set_value("CORE_API_BASE", &server.base_url);

    let session = submit_business_login(&SaitecLoginForm::new(
        "user@example.com".to_string(),
        "".to_string(),
        "secret".to_string(),
    ))
    .await
    .expect("submit login");

    assert_eq!(session.api_key, "sk-live");
    assert_eq!(session.api_key_id.as_deref(), Some("key-1"));
    assert_eq!(session.user_id.as_deref(), Some("mock-user"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test refresh_session_updates_profile_fields_after_valid_me_response -- --exact`
Expected: FAIL because `refresh_session_from_api_key`, `submit_business_login`, and the test helpers do not exist yet.

- [ ] **Step 3: Add the business-login entrypoints**

```rust
pub async fn refresh_session_from_api_key(session: &SaitecSession) -> Result<SaitecSession> {
    let validation = validate_api_key(&session.api_key).await?;
    if !validation.is_valid {
        anyhow::bail!(
            "{}",
            validation
                .message
                .unwrap_or_else(|| "token validation failed".to_string())
        );
    }

    let profile = fetch_user_profile(&session.api_key).await?;
    Ok(SaitecSession {
        api_key: session.api_key.clone(),
        token_type: session.token_type.clone(),
        user_id: Some(profile.user_id),
        email: profile.email,
        phone: profile.phone,
        display_name: profile.display_name,
        api_key_id: session.api_key_id.clone(),
        api_key_name: session.api_key_name.clone(),
        api_key_created_at: session.api_key_created_at.clone(),
        api_key_expires_at: session.api_key_expires_at.clone(),
        last_validated_at: Some(chrono::Utc::now().to_rfc3339()),
    })
}

pub async fn submit_business_login(form: &SaitecLoginForm) -> Result<SaitecSession> {
    let login = login_with_password(form).await?;
    let requested_name = generate_api_key_name();
    let api_key = create_api_key_with_name(&login.token, &requested_name).await?;
    let profile = fetch_user_profile(&api_key.raw_key).await?;

    Ok(SaitecSession {
        api_key: api_key.raw_key,
        token_type: "Bearer".to_string(),
        user_id: Some(profile.user_id),
        email: profile.email,
        phone: profile.phone,
        display_name: profile.display_name,
        api_key_id: Some(api_key.id),
        api_key_name: Some(api_key.name),
        api_key_created_at: api_key.created_at,
        api_key_expires_at: api_key.expires_at,
        last_validated_at: Some(chrono::Utc::now().to_rfc3339()),
    })
}
```

- [ ] **Step 4: Reuse the repo’s TcpListener-style mock-server pattern for auth tests**

```rust
async fn spawn_saitec_test_server(routes: Vec<TestRoute>) -> TestServer {
    // Follow the lightweight TcpListener mock-server pattern already used in
    // src/auth/oauth_tests/flow.rs and src/provider/openai_tests.rs.
}
```

- [ ] **Step 5: Run the focused async auth tests**

Run: `cargo test refresh_session_updates_profile_fields_after_valid_me_response submit_business_login_returns_session_without_persisting_jwt -- --exact`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/saitec/auth.rs
git commit -m "feat: add saitec business login exchange flow"
```

### Task 4: Replace callback-style SAITEC pending login with a structured form state

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth_types.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tui_lifecycle_runtime.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`

- [ ] **Step 1: Write the failing TUI pending-state tests**

```rust
#[test]
fn test_login_command_opens_saitec_login_form_state() {
    let mut app = create_test_app();
    app.input = "/login".to_string();
    app.submit_input();

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form, focus, .. }) => {
            assert_eq!(form.email, "");
            assert_eq!(form.phone, "");
            assert_eq!(form.password, "");
            assert_eq!(focus, crate::tui::app::auth::SaitecLoginField::Email);
        }
        ref other => panic!("unexpected pending login state: {other:?}"),
    }
}

#[test]
fn test_set_pending_saitec_login_for_tests_uses_form_variant() {
    let mut app = create_test_app();
    app.set_pending_saitec_login_for_tests();

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { .. }) => {}
        ref other => panic!("unexpected pending login state: {other:?}"),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_login_command_opens_saitec_login_form_state -- --exact`
Expected: FAIL because the SAITEC form variant does not exist yet.

- [ ] **Step 3: Add the SAITEC form state types**

```rust
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SaitecLoginField {
    Email,
    Phone,
    Password,
    Submit,
}

#[derive(Debug, Clone)]
pub(crate) struct SaitecPendingForm {
    pub form: crate::saitec::auth::SaitecLoginForm,
    pub focus: SaitecLoginField,
    pub error: Option<String>,
    pub submitting: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum PendingLogin {
    SaitecForm {
        form: SaitecPendingForm,
    },
    // existing variants...
}
```

- [ ] **Step 4: Update `App` test helpers and startup constructors**

```rust
#[cfg(test)]
pub(crate) fn set_pending_saitec_login_for_tests(&mut self) {
    self.pending_login = Some(super::auth::PendingLogin::SaitecForm {
        form: super::auth::SaitecPendingForm {
            form: crate::saitec::auth::SaitecLoginForm::new(
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ),
            focus: super::auth::SaitecLoginField::Email,
            error: None,
            submitting: false,
        },
    });
}
```

- [ ] **Step 5: Run the focused pending-state tests**

Run: `cargo test test_login_command_opens_saitec_login_form_state test_set_pending_saitec_login_for_tests_uses_form_variant -- --exact`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/app/auth_types.rs src/tui/app.rs src/tui/app/tui_lifecycle_runtime.rs src/tui/app/tests/commands_accounts_02/part_01.rs
git commit -m "feat: add saitec login form state"
```

### Task 5: Switch `/login`, startup login gating, and async completion over to the form flow

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tui_lifecycle.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\state_model_poke_03.rs`

- [ ] **Step 1: Write the failing command and startup gating tests**

```rust
#[test]
fn test_start_jcode_login_initializes_empty_saitec_form() {
    let mut app = create_test_app();
    app.input = "/login jcode".to_string();
    app.submit_input();

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            assert_eq!(form.form.email, "");
            assert_eq!(form.form.phone, "");
            assert_eq!(form.form.password, "");
            assert_eq!(form.focus, crate::tui::app::auth::SaitecLoginField::Email);
        }
        ref other => panic!("unexpected pending login state: {other:?}"),
    }
}

#[test]
fn test_login_picker_preview_enter_starts_saitec_form_flow() {
    let mut app = create_test_app();

    for c in "/login jcode".chars() {
        app.handle_key(KeyCode::Char(c), KeyModifiers::empty()).unwrap();
    }
    app.handle_key(KeyCode::Enter, KeyModifiers::empty()).unwrap();

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { .. }) => {}
        ref other => panic!("unexpected pending login state: {other:?}"),
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_start_jcode_login_initializes_empty_saitec_form test_login_picker_preview_enter_starts_saitec_form_flow -- --exact`
Expected: FAIL because `start_jcode_login()` still initializes the callback flow.

- [ ] **Step 3: Replace `start_jcode_login()` with form initialization**

```rust
pub(super) fn start_jcode_login(&mut self) {
    self.push_display_message(DisplayMessage::system(
        "**Saitec Login**\n\nEnter your email or phone plus password to continue."
            .to_string(),
    ));
    self.set_status_notice("Login: credentials required");
    self.begin_pending_login(PendingLogin::SaitecForm {
        form: SaitecPendingForm {
            form: crate::saitec::auth::SaitecLoginForm::new(
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ),
            focus: SaitecLoginField::Email,
            error: None,
            submitting: false,
        },
    });
}
```

- [ ] **Step 4: Update startup auto-login gating to open the form, not callback wait mode**

```rust
if !self.is_remote
    && !self.is_replay
    && self.display_messages.is_empty()
    && self.pending_login.is_none()
    && crate::saitec::auth::ensure_logged_in().is_err()
{
    self.push_display_message(DisplayMessage::system(
        "Saitec login is required before using this TUI. Opening the login form now."
            .to_string(),
    ));
    self.start_jcode_login();
}
```

- [ ] **Step 5: Run the focused command/startup tests**

Run: `cargo test test_start_jcode_login_initializes_empty_saitec_form test_login_picker_preview_enter_starts_saitec_form_flow -- --exact`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/app/auth.rs src/tui/app/tui_lifecycle.rs src/tui/app/tests/commands_accounts_02/part_01.rs src/tui/app/tests/state_model_poke_03.rs
git commit -m "feat: route saitec login commands to form flow"
```

### Task 6: Handle SAITEC form editing, masking, submit validation, and async login submission

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\input.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`

- [ ] **Step 1: Write the failing form-interaction tests**

```rust
#[test]
fn test_saitec_form_local_validation_blocks_empty_password() {
    let mut app = create_test_app();
    app.start_jcode_login();
    app.pending_login = Some(crate::tui::app::auth::PendingLogin::SaitecForm {
        form: crate::tui::app::auth::SaitecPendingForm {
            form: crate::saitec::auth::SaitecLoginForm::new(
                "user@example.com".to_string(),
                "".to_string(),
                "".to_string(),
            ),
            focus: crate::tui::app::auth::SaitecLoginField::Submit,
            error: None,
            submitting: false,
        },
    });

    app.submit_input();

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { ref form }) => {
            assert!(form.error.as_deref().unwrap_or_default().contains("Password"));
        }
        ref other => panic!("unexpected pending login state: {other:?}"),
    }
}

#[test]
fn test_logged_out_plain_prompt_still_blocks_normal_message_submission() {
    let mut app = create_test_app();
    app.input = "hello".to_string();
    app.submit_input();

    let last = app.display_messages().last().expect("missing response");
    assert!(last.content.contains("Please log in first"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_saitec_form_local_validation_blocks_empty_password test_logged_out_plain_prompt_still_blocks_normal_message_submission -- --exact`
Expected: FAIL because form validation and field-state handling do not exist yet.

- [ ] **Step 3: Add SAITEC form submit handling with local validation**

```rust
fn handle_saitec_form_submit(&mut self, mut form: SaitecPendingForm) {
    if let Err(err) = form.form.validate() {
        form.error = Some(err.to_string());
        form.submitting = false;
        self.pending_login = Some(PendingLogin::SaitecForm { form });
        self.set_status_notice("Login: validation failed");
        return;
    }

    form.error = None;
    form.submitting = true;
    self.set_status_notice("Login [saitec]: submitting...");
    let login_form = form.form.clone();
    tokio::spawn(async move {
        match crate::saitec::auth::submit_business_login(&login_form).await {
            Ok(session) => match crate::saitec::auth::save_session(&session) {
                Ok(()) => {
                    crate::auth::AuthStatus::invalidate_cache();
                    Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                        provider: "jcode".to_string(),
                        success: true,
                        message: format!(
                            "**Saitec login successful.**\n\nAuthenticated as `{}` and stored credentials at `~/.saitec_tui/auth.json`.",
                            session.user_id.as_deref().unwrap_or("unknown-user")
                        ),
                    }));
                }
                Err(err) => {
                    Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                        provider: "jcode".to_string(),
                        success: false,
                        message: format!("Saitec login failed while saving auth: {}", err),
                    }));
                }
            },
            Err(err) => {
                Bus::global().publish(BusEvent::LoginCompleted(LoginCompleted {
                    provider: "jcode".to_string(),
                    success: false,
                    message: format!("Saitec login failed: {}", err),
                }));
            }
        }
    });
}
```

- [ ] **Step 4: Preserve the logged-out prompt guard and update the message to the new form flow**

```rust
if !trimmed.is_empty()
    && !trimmed.starts_with('/')
    && crate::saitec::auth::ensure_logged_in().is_err()
{
    self.push_display_message(DisplayMessage::error(
        "Please log in first. Use `/login` to open the Saitec login form."
            .to_string(),
    ));
    self.set_status_notice("Login: required");
    return;
}
```

- [ ] **Step 5: Run the focused form-submission tests**

Run: `cargo test test_saitec_form_local_validation_blocks_empty_password test_logged_out_plain_prompt_still_blocks_normal_message_submission -- --exact`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/app/auth.rs src/tui/app/input.rs src/tui/app/tests/commands_accounts_02/part_01.rs
git commit -m "feat: validate and submit saitec login form"
```

### Task 7: Render the SAITEC login form overlay with masked password and focus state

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui_overlays.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui_prepare\tests.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui_tests\mod.rs`

- [ ] **Step 1: Write the failing overlay-render tests**

```rust
#[test]
fn saitec_pending_login_keeps_startup_splash_and_form_copy() {
    let mut app = create_test_app();
    app.pending_login = Some(crate::tui::app::auth::PendingLogin::SaitecForm {
        form: crate::tui::app::auth::SaitecPendingForm {
            form: crate::saitec::auth::SaitecLoginForm::new(
                "user@example.com".to_string(),
                "".to_string(),
                "secret".to_string(),
            ),
            focus: crate::tui::app::auth::SaitecLoginField::Password,
            error: Some("Password cannot be empty.".to_string()),
            submitting: false,
        },
    });

    let frame = prepare_messages_inner(&app, 80, 24);
    let rendered = rendered_lines(&frame).join("\n");

    assert!(rendered.contains("SAITEC"));
    assert!(rendered.contains("Email or phone"));
    assert!(rendered.contains("Password"));
    assert!(rendered.contains("******"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test saitec_pending_login_keeps_startup_splash_and_form_copy -- --exact`
Expected: FAIL because no SAITEC form overlay is rendered.

- [ ] **Step 3: Add a dedicated SAITEC login form overlay renderer**

```rust
pub(super) fn draw_saitec_login_overlay(
    frame: &mut Frame,
    area: Rect,
    form: &crate::tui::app::auth::SaitecPendingForm,
) {
    clear_area(frame, area);
    // Render a centered bordered box with:
    // - title: Saitec Login
    // - helper text: email/phone requirement
    // - email row
    // - phone row
    // - password row with masked value
    // - submit row
    // - inline error or submitting text
}
```

- [ ] **Step 4: Draw the overlay before the normal shell when `PendingLogin::SaitecForm` is active**

```rust
if let Some(crate::tui::app::auth::PendingLogin::SaitecForm { form }) = app.pending_login_state() {
    overlays::draw_saitec_login_overlay(frame, area, form);
    finalize_frame_metrics(app, total_start, Duration::ZERO, total_start.elapsed(), None);
    return;
}
```

- [ ] **Step 5: Run the focused overlay tests**

Run: `cargo test saitec_pending_login_keeps_startup_splash_and_form_copy -- --exact`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/ui_overlays.rs src/tui/ui.rs src/tui/ui_prepare/tests.rs src/tui/ui_tests/mod.rs
git commit -m "feat: render saitec login form overlay"
```

### Task 8: Refresh valid sessions at startup and re-enter login-required state on invalid sessions

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tui_lifecycle.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\saitec\auth.rs`

- [ ] **Step 1: Write the failing refresh-at-startup tests**

```rust
#[test]
fn test_logout_command_returns_app_to_pending_saitec_form() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev_home = std::env::var_os("JCODE_HOME");
    crate::env::set_var("JCODE_HOME", temp.path());

    crate::saitec::auth::save_session(&crate::saitec::auth::SaitecSession {
        api_key: "sk-live-test".to_string(),
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
    .expect("save auth");

    let mut app = create_test_app();
    app.input = "/logout".to_string();
    app.submit_input();

    match app.pending_login {
        Some(crate::tui::app::auth::PendingLogin::SaitecForm { .. }) => {}
        ref other => panic!("unexpected pending login state: {other:?}"),
    }

    if let Some(prev_home) = prev_home {
        crate::env::set_var("JCODE_HOME", prev_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_logout_command_returns_app_to_pending_saitec_form -- --exact`
Expected: FAIL because logout currently clears local auth but does not immediately reopen the form.

- [ ] **Step 3: Refresh the stored session when valid and relaunch the form when invalid**

```rust
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
            anyhow::bail!(err);
        }
    }
}
```

- [ ] **Step 4: Use the refresh result in startup and logout handling**

```rust
if trimmed == "/logout" {
    match crate::saitec::auth::clear_session() {
        Ok(()) => {
            crate::auth::AuthStatus::invalidate_cache();
            self.push_display_message(DisplayMessage::system(
                "Logged out from Saitec. Reopening the login form.".to_string(),
            ));
            self.set_status_notice("Login: required");
            self.start_jcode_login();
        }
        Err(err) => { /* existing error path */ }
    }
    return true;
}
```

- [ ] **Step 5: Run the focused startup/logout tests**

Run: `cargo test test_logout_command_returns_app_to_pending_saitec_form -- --exact`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/saitec/auth.rs src/tui/app/auth.rs src/tui/app/tui_lifecycle.rs src/tui/app/tests/commands_accounts_02/part_01.rs
git commit -m "feat: refresh saitec sessions and relaunch login on logout"
```

### Task 9: Verify the end-to-end business-login slice and update the plan consumer docs

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\input_help.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui_overlays.rs` (only if help copy or overlay copy still needs cleanup)
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`

- [ ] **Step 1: Add the final red test for updated user-facing copy**

```rust
#[test]
fn test_help_overlay_mentions_saitec_login_form() {
    let mut app = create_test_app();
    super::auth::handle_auth_command(&mut app, "/help");
    let rendered = app
        .display_messages()
        .last()
        .map(|m| m.content.clone())
        .unwrap_or_default();

    assert!(rendered.contains("/login"));
}
```

- [ ] **Step 2: Run the targeted verification commands**

Run: `cargo test test_login_command_opens_saitec_login_form_state -- --exact`
Expected: PASS

Run: `cargo test test_saitec_form_local_validation_blocks_empty_password -- --exact`
Expected: PASS

Run: `cargo test test_logout_command_returns_app_to_pending_saitec_form -- --exact`
Expected: PASS

Run: `cargo test saitec_pending_login_keeps_startup_splash_and_form_copy -- --exact`
Expected: PASS

- [ ] **Step 3: Run the broader auth/TUI verification set**

Run: `cargo test --package jcode --lib commands_accounts_02`
Expected: PASS

Run: `cargo test --package jcode --lib state_model_poke_03`
Expected: PASS

Run: `cargo test --package jcode --lib saitec_pending_login_keeps_startup_splash_after_system_waiting_message`
Expected: update selector or rename as needed so the SAITEC pending-login startup rendering test stays green under the new form flow.

- [ ] **Step 4: Run the final compile check**

Run: `cargo test --package jcode --lib`
Expected: PASS

Run: `cargo check -p jcode --bin jcode`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tui/app/input_help.rs src/tui/app/tests/commands_accounts_02/part_01.rs
git commit -m "test: verify saitec business login flow"
```
