# Saitec Login Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the existing `jcode` subscription/provider login path into a Saitec-branded mandatory login flow backed by `~/.saitec_tui/`, with startup gating, callback token capture, `/logout`, config relocation, and a verified Windows release build.

**Architecture:** Reuse the existing `ProviderChoice::Jcode` and `LoginProviderTarget::Jcode` path as the Saitec single-provider entrypoint instead of creating an unrelated auth stack. Centralize the product-home switch in storage/path helpers, add a focused `saitec` module for token persistence and mock validation, then wire startup and TUI command gating around that module. Keep the mock backend contract isolated so real HTTP validation can replace it later without reworking the UI flow.

**Tech Stack:** Rust, Tokio, reqwest-ready mock interfaces, clap CLI, ratatui TUI, PowerShell packaging/install script, cargo test/build.

---

### Task 1: Redirect Product Storage To `~/.saitec_tui`

**Files:**
- Create: `G:\Workspace\Project2026\JCode\jcode\crates\jcode-storage\src\tests.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\crates\jcode-storage\src\lib.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\crates\jcode-storage\src\tests.rs`

- [ ] **Step 1: Write the failing storage-path tests**

```rust
use super::*;

#[test]
fn jcode_dir_defaults_to_saitec_home_directory() {
    let _guard = crate::storage::lock_test_env();
    crate::env::remove_var("JCODE_HOME");

    let home = dirs::home_dir().expect("home dir");
    let actual = jcode_dir().expect("jcode dir");

    assert_eq!(actual, home.join(".saitec_tui"));
}

#[test]
fn app_config_dir_is_sandboxed_under_saitec_home_when_jcode_home_is_set() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    crate::env::set_var("JCODE_HOME", temp.path());

    let actual = app_config_dir().expect("config dir");

    assert_eq!(actual, temp.path().join("config").join("jcode"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jcode-storage jcode_dir_defaults_to_saitec_home_directory -- --exact`
Expected: FAIL because `jcode_dir()` still resolves to `~/.jcode`.

- [ ] **Step 3: Implement the minimal storage root switch**

```rust
pub fn jcode_dir() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("JCODE_HOME") {
        return Ok(PathBuf::from(path));
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("No home directory"))?;
    Ok(home.join(".saitec_tui"))
}
```

- [ ] **Step 4: Add the storage tests module hook**

```rust
#[cfg(test)]
mod tests;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p jcode-storage --lib`
Expected: PASS with the new storage tests green.

- [ ] **Step 6: Commit**

```bash
git add crates/jcode-storage/src/lib.rs crates/jcode-storage/src/tests.rs
git commit -m "refactor: move default product storage to saitec home"
```

### Task 2: Add Saitec Auth Persistence And Mock Validation

**Files:**
- Create: `G:\Workspace\Project2026\JCode\jcode\src\saitec\mod.rs`
- Create: `G:\Workspace\Project2026\JCode\jcode\src\saitec\auth.rs`
- Create: `G:\Workspace\Project2026\JCode\jcode\src\saitec\paths.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\lib.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\saitec\auth.rs`

- [ ] **Step 1: Write the failing Saitec auth tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_session_returns_none_when_auth_file_missing() {
        let _guard = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        crate::env::set_var("JCODE_HOME", temp.path());

        assert!(load_session().expect("load session").is_none());
    }

    #[test]
    fn validate_session_marks_invalid_prefixed_tokens_as_expired() {
        let result = validate_token_mock("invalid-demo-token").expect("validate token");

        assert!(!result.is_valid);
        assert_eq!(result.message.as_deref(), Some("mock token rejected"));
    }

    #[test]
    fn save_and_reload_session_round_trips_auth_token() {
        let _guard = crate::storage::lock_test_env();
        let temp = tempfile::tempdir().expect("tempdir");
        crate::env::set_var("JCODE_HOME", temp.path());

        let session = SaitecSession {
            auth_token: "mock-token".to_string(),
            token_type: "Bearer".to_string(),
            issued_at: Some("2026-05-09T14:00:00Z".to_string()),
            expires_at: None,
            user_id: Some("mock-user".to_string()),
            last_validated_at: None,
        };

        save_session(&session).expect("save session");
        let loaded = load_session()
            .expect("load session")
            .expect("stored session");

        assert_eq!(loaded.auth_token, "mock-token");
        assert_eq!(loaded.user_id.as_deref(), Some("mock-user"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test save_and_reload_session_round_trips_auth_token -- --exact`
Expected: FAIL because the `saitec` module does not exist yet.

- [ ] **Step 3: Implement the minimal Saitec auth module**

```rust
// src/saitec/mod.rs
pub mod auth;
pub mod paths;

// src/saitec/paths.rs
use anyhow::Result;
use std::path::PathBuf;

pub fn home_dir() -> Result<PathBuf> {
    crate::storage::jcode_dir()
}

pub fn auth_file() -> Result<PathBuf> {
    Ok(home_dir()?.join("auth.json"))
}

// src/saitec/auth.rs
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaitecSession {
    pub auth_token: String,
    pub token_type: String,
    pub issued_at: Option<String>,
    pub expires_at: Option<String>,
    pub user_id: Option<String>,
    pub last_validated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaitecValidationResult {
    pub is_valid: bool,
    pub user_id: Option<String>,
    pub expires_at: Option<String>,
    pub message: Option<String>,
}

pub fn load_session() -> Result<Option<SaitecSession>> {
    let path = crate::saitec::paths::auth_file()?;
    if !path.exists() {
        return Ok(None);
    }
    let session = crate::storage::read_json(&path)?;
    Ok(Some(session))
}

pub fn save_session(session: &SaitecSession) -> Result<()> {
    let path = crate::saitec::paths::auth_file()?;
    crate::storage::write_json_secret(&path, session)
}

pub fn clear_session() -> Result<()> {
    let path = crate::saitec::paths::auth_file()?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn validate_token_mock(token: &str) -> Result<SaitecValidationResult> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Ok(SaitecValidationResult {
            is_valid: false,
            user_id: None,
            expires_at: None,
            message: Some("missing auth token".to_string()),
        });
    }

    if trimmed.starts_with("invalid-") {
        return Ok(SaitecValidationResult {
            is_valid: false,
            user_id: None,
            expires_at: None,
            message: Some("mock token rejected".to_string()),
        });
    }

    Ok(SaitecValidationResult {
        is_valid: true,
        user_id: Some("mock-user".to_string()),
        expires_at: None,
        message: None,
    })
}
```

- [ ] **Step 4: Export the Saitec module**

```rust
pub mod saitec;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test validate_session_marks_invalid_prefixed_tokens_as_expired -- --exact`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/saitec/mod.rs src/saitec/auth.rs src/saitec/paths.rs
git commit -m "feat: add saitec auth persistence and mock validation"
```

### Task 3: Replace Jcode Subscription Auth With Saitec Login Flow

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\subscription_catalog.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\provider\jcode.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\cli\login.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\auth\mod.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\auth\tests.rs`

- [ ] **Step 1: Write the failing login tests**

```rust
#[test]
fn saitec_credentials_exist_when_auth_json_contains_auth_token() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    crate::env::set_var("JCODE_HOME", temp.path());

    crate::saitec::auth::save_session(&crate::saitec::auth::SaitecSession {
        auth_token: "mock-token".to_string(),
        token_type: "Bearer".to_string(),
        issued_at: None,
        expires_at: None,
        user_id: Some("mock-user".to_string()),
        last_validated_at: None,
    })
    .expect("save auth");

    assert!(crate::subscription_catalog::has_credentials());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test saitec_credentials_exist_when_auth_json_contains_auth_token -- --exact`
Expected: FAIL because `subscription_catalog::has_credentials()` still only checks env/config files.

- [ ] **Step 3: Change the subscription catalog and provider branding to Saitec**

```rust
pub const JCODE_API_KEY_ENV: &str = "SAITEC_API_KEY";
pub const JCODE_API_BASE_ENV: &str = "SAITEC_API_BASE";
pub const JCODE_ENV_FILE: &str = "saitec.env";
pub const DEFAULT_JCODE_API_BASE: &str = "https://api.saitec.local/v1";

pub fn configured_api_key() -> Option<String> {
    crate::saitec::auth::load_session()
        .ok()
        .flatten()
        .map(|session| session.auth_token)
        .or_else(|| provider_catalog::load_env_value_from_env_or_config(JCODE_API_KEY_ENV, JCODE_ENV_FILE))
}
```

- [ ] **Step 4: Replace the CLI/TUI Jcode login stubs with browser URL + callback messaging**

```rust
fn login_jcode_flow() -> Result<()> {
    anyhow::bail!("Interactive Saitec login must be completed from the TUI/browser flow.")
}

fn start_jcode_login(&mut self) {
    let auth_url = crate::saitec::auth::mock_authorize_url(1455);
    let browser_opened = Self::open_auth_browser(&auth_url);

    self.push_display_message(DisplayMessage::system(format!(
        "**Saitec Login**\n\nOpen this URL to sign in:\n{}\n\nBrowser opened automatically: {}\n\nAfter the callback returns with `auth_token`, paste the callback URL here. Type `/cancel` to abort.",
        auth_url,
        if browser_opened { "yes" } else { "no" }
    )));
    self.set_status_notice("Login: Saitec callback pending");
    self.begin_pending_login(PendingLogin::OpenAiAccount {
        verifier: "saitec-mock".to_string(),
        label: "saitec".to_string(),
        redirect_uri: Some("http://127.0.0.1:1455/auth/callback".to_string()),
    });
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test saitec_credentials_exist_when_auth_json_contains_auth_token -- --exact`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/subscription_catalog.rs src/provider/jcode.rs src/cli/login.rs src/tui/app/auth.rs src/auth/mod.rs
git commit -m "feat: repurpose jcode provider login as saitec auth flow"
```

### Task 4: Add Startup Login Gating And Token Validation

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\cli\startup.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\cli\dispatch.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\saitec\auth.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\saitec\auth.rs`

- [ ] **Step 1: Write the failing gate tests**

```rust
#[test]
fn ensure_logged_in_fails_when_session_missing() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    crate::env::set_var("JCODE_HOME", temp.path());

    let error = ensure_logged_in().expect_err("missing session should fail");

    assert!(error.to_string().contains("Saitec login required"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test ensure_logged_in_fails_when_session_missing -- --exact`
Expected: FAIL because `ensure_logged_in()` does not exist yet.

- [ ] **Step 3: Implement the login gate**

```rust
pub fn ensure_logged_in() -> anyhow::Result<()> {
    let Some(session) = load_session()? else {
        anyhow::bail!("Saitec login required. Run `/login` in the TUI.");
    };

    let validation = validate_token_mock(&session.auth_token)?;
    if !validation.is_valid {
        anyhow::bail!(
            "Saitec login required. Stored token is invalid: {}",
            validation.message.unwrap_or_else(|| "validation failed".to_string())
        );
    }

    Ok(())
}
```

- [ ] **Step 4: Invoke the gate during startup before normal dispatch**

```rust
if let Err(err) = crate::saitec::auth::ensure_logged_in() {
    crate::logging::warn(&format!("startup auth gate blocked access: {}", err));
}
```

Use the same gate in the TUI/server bootstrap path so non-authenticated users cannot proceed into the normal provider session.

- [ ] **Step 5: Run tests to verify it passes**

Run: `cargo test ensure_logged_in_fails_when_session_missing -- --exact`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/cli/startup.rs src/cli/dispatch.rs src/saitec/auth.rs
git commit -m "feat: gate startup on saitec login validation"
```

### Task 5: Add `/logout` And Command Gating In The TUI

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\state_ui_input_helpers.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth_account_commands.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\input.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_01\part_02.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`

- [ ] **Step 1: Write the failing TUI command tests**

```rust
#[test]
fn test_logout_command_clears_saitec_auth_file() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    crate::env::set_var("JCODE_HOME", temp.path());

    crate::saitec::auth::save_session(&crate::saitec::auth::SaitecSession {
        auth_token: "mock-token".to_string(),
        token_type: "Bearer".to_string(),
        issued_at: None,
        expires_at: None,
        user_id: Some("mock-user".to_string()),
        last_validated_at: None,
    })
    .expect("save auth");

    let mut app = create_test_app();
    app.input = "/logout".to_string();
    app.submit_input();

    assert!(crate::saitec::auth::load_session().expect("load").is_none());
}

#[test]
fn test_regular_prompt_is_blocked_when_saitec_login_missing() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    crate::env::set_var("JCODE_HOME", temp.path());

    let mut app = create_test_app();
    app.input = "hello".to_string();
    app.submit_input();

    let last = app.display_messages().last().expect("missing response");
    assert_eq!(last.role, "error");
    assert!(last.content.contains("Please log in first"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_logout_command_clears_saitec_auth_file -- --exact`
Expected: FAIL because `/logout` is not registered.

- [ ] **Step 3: Implement `/logout` and prompt gating**

```rust
if trimmed == "/logout" {
    match crate::saitec::auth::clear_session() {
        Ok(()) => {
            crate::auth::AuthStatus::invalidate_cache();
            app.push_display_message(DisplayMessage::system(
                "Logged out from Saitec. Please run `/login` to continue.".to_string(),
            ));
            app.set_status_notice("Login: required");
        }
        Err(err) => app.push_display_message(DisplayMessage::error(format!(
            "Failed to log out: {}",
            err
        ))),
    }
    return true;
}
```

And in input submission:

```rust
if !trimmed.starts_with('/')
    && crate::saitec::auth::ensure_logged_in().is_err()
{
    self.push_display_message(DisplayMessage::error(
        "Please log in first. Use `/login` to start the Saitec login flow.".to_string(),
    ));
    self.set_status_notice("Login: required");
    return;
}
```

- [ ] **Step 4: Register `/logout` in the visible command list**

```rust
RegisteredCommand::public("/logout", "Logout from Saitec and clear local auth"),
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test test_logout_command_clears_saitec_auth_file -- --exact`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/app/state_ui_input_helpers.rs src/tui/app/auth_account_commands.rs src/tui/app/input.rs src/tui/app/tests/commands_accounts_02/part_01.rs src/tui/app/tests/commands_accounts_01/part_02.rs
git commit -m "feat: add saitec logout command and TUI auth gating"
```

### Task 6: Move Base API Config To Saitec Files And Brand The Installer

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\config\config_file.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\subscription_catalog.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\scripts\install.ps1`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`

- [ ] **Step 1: Write the failing config-path test**

```rust
#[test]
fn test_config_path_uses_saitec_home() {
    let _guard = crate::storage::lock_test_env();
    crate::env::remove_var("JCODE_HOME");

    let path = crate::config::Config::path().expect("config path");
    assert!(path.to_string_lossy().contains(".saitec_tui"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_config_path_uses_saitec_home -- --exact`
Expected: FAIL if the config path still assumes `.jcode`.

- [ ] **Step 3: Implement the config/installer branding updates**

```rust
pub fn path() -> Option<PathBuf> {
    jcode_dir().ok().map(|d| d.join("config.toml"))
}
```

```powershell
$InstallDir = Join-Path $env:LOCALAPPDATA "saitec-tui\bin"
$JcodeHome = if ($env:JCODE_HOME) { $env:JCODE_HOME } elseif ($env:USERPROFILE) { Join-Path $env:USERPROFILE ".saitec_tui" } else { Join-Path ([Environment]::GetFolderPath("UserProfile")) ".saitec_tui" }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test test_config_path_uses_saitec_home -- --exact`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/config/config_file.rs src/subscription_catalog.rs scripts/install.ps1
git commit -m "feat: brand config and installer for saitec home"
```

### Task 7: Full Verification And Windows Release Build

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\Cargo.toml` (only if a packaging/version tweak is strictly required)
- Verify: `G:\Workspace\Project2026\JCode\jcode\target\release\jcode.exe`

- [ ] **Step 1: Run targeted auth and TUI tests**

Run: `cargo test saitec_credentials_exist_when_auth_json_contains_auth_token ensure_logged_in_fails_when_session_missing test_logout_command_clears_saitec_auth_file test_regular_prompt_is_blocked_when_saitec_login_missing -- --nocapture`
Expected: PASS.

- [ ] **Step 2: Run a broader regression slice**

Run: `cargo test /login -- --nocapture`
Expected: PASS for login-related test names and no newly introduced failures in the touched areas.

- [ ] **Step 3: Build the release binary**

Run: `cargo build --release`
Expected: exit code 0 and `target\release\jcode.exe` produced.

- [ ] **Step 4: Smoke-check the release binary version/help**

Run: `.\target\release\jcode.exe --help`
Expected: exit code 0 and help text renders without startup crash.

- [ ] **Step 5: Record packaging outcome**

```text
Artifact to report:
- G:\Workspace\Project2026\JCode\jcode\target\release\jcode.exe
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "build: verify saitec login flow and release artifact"
```
