# Custom OpenAI-Compatible BaseModel Login Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a visible SAITEC BaseModel login option that lets users configure a custom OpenAI-compatible endpoint and API key from the TUI.

**Architecture:** Reuse the existing `openai-compatible` provider profile and TUI two-step login flow. The implementation only broadens the SAITEC product allowlist and route filter for the generic custom profile; it does not add a new provider or mix SAITEC platform credentials with BaseModel credentials.

**Tech Stack:** Rust, ratatui TUI tests, existing `provider_catalog`, `saitec::product_profile`, and OpenAI-compatible provider runtime.

---

## File Structure

- Modify: `src/saitec/product_profile.rs`
  - Add `openai-compatible` to the SAITEC BaseModel provider allowlist.
  - Allow the generic OpenAI-compatible profile id in the route filter.
  - Update SAITEC unsupported-provider copy so the custom option is named.
  - Add focused product-profile unit tests for provider and route allowlist behavior.
- Modify: `src/tui/app/tests/commands_accounts_02/part_01.rs`
  - Update the filtered Base models picker expectations from 5 providers to 6 providers.
  - Add a TUI interaction test for selecting the custom provider and entering the endpoint.
  - Add a credential-separation test for saving the custom key without altering SAITEC credentials.
- Validate only: `src/tui/app/auth.rs`
  - Existing `start_openai_compatible_profile_login`, `PendingLogin::OpenAiCompatibleApiBase`, and `PendingLogin::ApiKeyProfile` should already satisfy the login flow. Do not edit unless a failing test proves a wiring gap.
- Validate only: `src/provider_catalog.rs`
  - Existing `saitec_visible_base_model_providers()` should include the custom profile automatically once the product allowlist changes.

---

### Task 1: Product Allowlist And Route Tests

**Files:**
- Modify: `src/saitec/product_profile.rs`

- [ ] **Step 1: Write the failing product-profile tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `src/saitec/product_profile.rs`, after `allowed_kimi_provider_allows_kimi_base_model_route`:

```rust
#[test]
fn custom_openai_compatible_provider_is_saitec_basemodel_allowed() {
    assert!(is_allowed_base_model_provider("openai-compatible"));
    assert!(is_allowed_openai_compatible_profile("openai-compatible"));
}

#[test]
fn generic_openai_compatible_route_allows_validated_custom_model() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let previous_home = std::env::var_os("JCODE_HOME");
    crate::env::set_var("JCODE_HOME", temp.path());

    crate::auth::validation::save(
        "openai-compatible",
        crate::auth::validation::ProviderValidationRecord {
            checked_at_ms: chrono::Utc::now().timestamp_millis(),
            success: true,
            provider_smoke_ok: Some(true),
            tool_smoke_ok: Some(true),
            validated_models: vec!["custom-coder".to_string()],
            summary: "tool_smoke: AUTH_TEST_OK".to_string(),
        },
    )
    .expect("save validation");

    assert!(is_allowed_base_model_route(
        "",
        "custom-coder",
        "OpenAI-compatible",
        "openai-compatible",
    ));
    assert!(is_allowed_base_model_route(
        "",
        "custom-coder",
        "OpenAI-compatible",
        "openai-compatible:openai-compatible",
    ));
    assert!(!is_allowed_base_model_route(
        "",
        "unvalidated-model",
        "OpenAI-compatible",
        "openai-compatible",
    ));

    if let Some(previous_home) = previous_home {
        crate::env::set_var("JCODE_HOME", previous_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
}
```

- [ ] **Step 2: Run the product tests to verify they fail**

Run:

```powershell
cargo test -p jcode custom_openai_compatible_provider_is_saitec_basemodel_allowed
cargo test -p jcode generic_openai_compatible_route_allows_validated_custom_model
```

Expected: FAIL. The provider/profile assertions fail because `openai-compatible` is not yet in the SAITEC allowlists.

- [ ] **Step 3: Implement the minimal product allowlist change**

Change the allowlist and messages in `src/saitec/product_profile.rs`:

```rust
const ALLOWED_BASE_MODEL_PROVIDER_IDS: &[&str] = &[
    "openai",
    "claude",
    "zai",
    "kimi",
    "alibaba-coding-plan",
    "openai-compatible",
];
```

Change `is_allowed_openai_compatible_profile` to include the generic custom profile:

```rust
pub fn is_allowed_openai_compatible_profile(profile_id: &str) -> bool {
    matches!(
        profile_id.trim().to_ascii_lowercase().as_str(),
        "zai" | "kimi" | "alibaba-coding-plan" | "openai-compatible"
    )
}
```

Update the two user-facing messages in the same file so they mention the custom option:

```rust
pub fn unsupported_base_model_provider_message() -> String {
    "SAITEC-TUI only supports these base-model providers: openai, claude, zai, kimi, alibaba-coding-plan, openai-compatible.".to_string()
}
```

```rust
pub fn unsupported_base_model_route_message(model: &str) -> String {
    format!(
        "SAITEC-TUI cannot use `{}` because it is not routed through an allowed base-model provider. Use `/login base-models` to configure OpenAI, Anthropic/Claude, Z.AI, Kimi, Alibaba Cloud Coding, or a custom OpenAI-compatible endpoint.",
        model.trim()
    )
}
```

- [ ] **Step 4: Run the product tests to verify they pass**

Run:

```powershell
cargo test -p jcode custom_openai_compatible_provider_is_saitec_basemodel_allowed
cargo test -p jcode generic_openai_compatible_route_allows_validated_custom_model
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

Run:

```powershell
git add src/saitec/product_profile.rs
git commit -m "feat: allow custom basemodel provider"
```

---

### Task 2: Base Models Picker Visibility And Endpoint Prompt Tests

**Files:**
- Modify: `src/tui/app/tests/commands_accounts_02/part_01.rs`

- [ ] **Step 1: Update the failing picker visibility test**

In `test_filtered_login_picker_contains_only_saitec_allowlisted_providers`, update the provider counts and add the OpenAI-compatible assertion:

```rust
assert_eq!(profile["items_count"], 6);
assert_eq!(profile["filtered_count"], 6);
```

Add this assertion next to the existing provider-name assertions:

```rust
assert!(
    text.contains("OpenAI-compatible"),
    "rendered picker:\n{text}"
);
```

- [ ] **Step 2: Add the failing endpoint prompt interaction test**

Add this test after `test_filtered_login_picker_contains_only_saitec_allowlisted_providers`:

```rust
#[test]
fn test_base_models_picker_opens_custom_endpoint_prompt() {
    let mut app = create_test_app();
    app.input = "/login base-models".to_string();
    app.submit_input();

    for ch in "custom".chars() {
        app.handle_key(KeyCode::Char(ch), KeyModifiers::empty())
            .expect("filter login picker");
    }
    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("select custom provider");

    match app.pending_login.as_ref() {
        Some(crate::tui::app::auth::PendingLogin::OpenAiCompatibleApiBase { profile }) => {
            assert_eq!(profile.id, crate::provider_catalog::OPENAI_COMPAT_PROFILE.id);
        }
        other => panic!("expected OpenAI-compatible endpoint prompt, got: {other:?}"),
    }
    assert!(app.login_picker_overlay.is_none());
}
```

- [ ] **Step 3: Run the picker tests to verify they fail before implementation**

Run:

```powershell
cargo test -p jcode test_filtered_login_picker_contains_only_saitec_allowlisted_providers
cargo test -p jcode test_base_models_picker_opens_custom_endpoint_prompt
```

Expected before Task 1 implementation: FAIL because the custom provider is not visible. Expected after Task 1 implementation: PASS. If this task is executed after Task 1 is already green, treat the result as a regression check and avoid production edits.

- [ ] **Step 4: Keep implementation minimal**

If Task 1 already made this pass, do not edit `src/tui/app/auth.rs`. If the new test fails after Task 1, inspect only the provider filtering path and keep the target behavior:

```rust
crate::provider_catalog::saitec_visible_base_model_providers()
```

must include the descriptor whose id is:

```rust
"openai-compatible"
```

- [ ] **Step 5: Commit Task 2**

Run:

```powershell
git add src/tui/app/tests/commands_accounts_02/part_01.rs
git commit -m "test: cover custom basemodel picker flow"
```

---

### Task 3: Endpoint And API Key Persistence Tests

**Files:**
- Modify: `src/tui/app/tests/commands_accounts_02/part_01.rs`

- [ ] **Step 1: Add the failing endpoint persistence test**

Add this test after `test_base_models_picker_opens_custom_endpoint_prompt`:

```rust
#[test]
fn test_custom_openai_compatible_endpoint_advances_to_key_prompt() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let previous_home = std::env::var_os("JCODE_HOME");
    crate::env::set_var("JCODE_HOME", temp.path());

    let mut app = create_test_app();
    app.input = "/login openai-compatible".to_string();
    app.submit_input();

    app.input = "https://llm.example.com/v1".to_string();
    app.submit_input();

    let resolved = crate::provider_catalog::resolve_openai_compatible_profile(
        crate::provider_catalog::OPENAI_COMPAT_PROFILE,
    );
    assert_eq!(resolved.api_base, "https://llm.example.com/v1");
    match app.pending_login.as_ref() {
        Some(crate::tui::app::auth::PendingLogin::ApiKeyProfile {
            provider_id,
            key_name,
            endpoint,
            openai_compatible_profile,
            ..
        }) => {
            assert_eq!(provider_id, "openai-compatible");
            assert_eq!(key_name, &resolved.api_key_env);
            assert_eq!(endpoint.as_deref(), Some("https://llm.example.com/v1"));
            assert_eq!(
                openai_compatible_profile.map(|profile| profile.id),
                Some(crate::provider_catalog::OPENAI_COMPAT_PROFILE.id)
            );
        }
        other => panic!("expected OpenAI-compatible API-key prompt, got: {other:?}"),
    }

    if let Some(previous_home) = previous_home {
        crate::env::set_var("JCODE_HOME", previous_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
}
```

- [ ] **Step 2: Add the failing credential separation test**

Add this test after the endpoint persistence test:

```rust
#[test]
fn test_custom_openai_compatible_key_does_not_overwrite_saitec_credentials() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let previous_home = std::env::var_os("JCODE_HOME");
    let previous_saitec_key = std::env::var_os(crate::subscription_catalog::JCODE_API_KEY_ENV);
    crate::env::set_var("JCODE_HOME", temp.path());

    save_test_saitec_session();

    let mut app = create_test_app();
    app.input = "/login openai-compatible".to_string();
    app.submit_input();
    app.input = "https://llm.example.com/v1".to_string();
    app.submit_input();
    app.input = "custom-model-key".to_string();
    app.submit_input();

    let resolved = crate::provider_catalog::resolve_openai_compatible_profile(
        crate::provider_catalog::OPENAI_COMPAT_PROFILE,
    );
    let env_file = crate::storage::app_config_dir()
        .expect("config dir")
        .join(&resolved.env_file);
    let env_contents = std::fs::read_to_string(env_file).expect("read env file");
    assert!(
        env_contents.contains("JCODE_OPENAI_COMPAT_API_BASE=https://llm.example.com/v1"),
        "custom endpoint should be stored in openai-compatible env file:\n{env_contents}"
    );
    assert!(
        env_contents.contains(&format!("{}=custom-model-key", resolved.api_key_env)),
        "custom key should be stored under the resolved key name:\n{env_contents}"
    );
    assert_eq!(
        crate::subscription_catalog::configured_api_key().as_deref(),
        Some("sk-live-test")
    );
    assert!(
        crate::saitec::auth::load_session()
            .expect("load SAITEC session")
            .is_some(),
        "custom BaseModel login must not clear SAITEC session auth"
    );

    if let Some(previous_home) = previous_home {
        crate::env::set_var("JCODE_HOME", previous_home);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
    if let Some(previous_saitec_key) = previous_saitec_key {
        crate::env::set_var(
            crate::subscription_catalog::JCODE_API_KEY_ENV,
            previous_saitec_key,
        );
    } else {
        crate::env::remove_var(crate::subscription_catalog::JCODE_API_KEY_ENV);
    }
}
```

- [ ] **Step 3: Run the persistence tests to verify the red state**

Run:

```powershell
cargo test -p jcode test_custom_openai_compatible_endpoint_advances_to_key_prompt
cargo test -p jcode test_custom_openai_compatible_key_does_not_overwrite_saitec_credentials
```

Expected after Task 1: PASS if the existing TUI flow is fully wired. If one fails, use the failure to identify the smallest missing connection in `src/tui/app/auth.rs`.

- [ ] **Step 4: Implement only proven missing TUI wiring**

If the endpoint test fails because `/login openai-compatible` does not enter `PendingLogin::OpenAiCompatibleApiBase`, keep `start_openai_compatible_profile_login` shaped like this:

```rust
if profile.id == crate::provider_catalog::OPENAI_COMPAT_PROFILE.id {
    let resolved = crate::provider_catalog::resolve_openai_compatible_profile(profile);
    self.push_display_message(DisplayMessage::system(format!(
        "**{} Endpoint**\n\n\
         Setup docs: {}\n\
         Current API base: `{}`\n\n\
         **Paste the API base below**. Press Enter to keep the current value, or use Up/Down to select Validate or Cancel.",
        resolved.display_name, resolved.setup_url, resolved.api_base
    )));
    self.set_status_notice("Login: API base...");
    self.pending_login = Some(PendingLogin::OpenAiCompatibleApiBase { profile });
    return;
}
```

If the key persistence test fails because the key saves to the wrong place, keep the OpenAI-compatible branch in `PendingLogin::ApiKeyProfile` shaped like this:

```rust
if let Some(resolved) = resolved_openai_compatible.as_ref() {
    if resolved.requires_api_key {
        crate::provider_catalog::save_env_value_to_env_file(
            crate::provider_catalog::OPENAI_COMPAT_LOCAL_ENABLED_ENV,
            &resolved.env_file,
            None,
        )?;
        crate::provider_catalog::save_env_value_to_env_file(
            &resolved.api_key_env,
            &resolved.env_file,
            Some(key.trim()),
        )
    } else {
        crate::provider_catalog::save_env_value_to_env_file(
            crate::provider_catalog::OPENAI_COMPAT_LOCAL_ENABLED_ENV,
            &resolved.env_file,
            Some("1"),
        )?;
        crate::provider_catalog::save_env_value_to_env_file(
            &resolved.api_key_env,
            &resolved.env_file,
            if key.trim().is_empty() {
                None
            } else {
                Some(key.trim())
            },
        )
    }
}
```

- [ ] **Step 5: Run the persistence tests to verify green**

Run:

```powershell
cargo test -p jcode test_custom_openai_compatible_endpoint_advances_to_key_prompt
cargo test -p jcode test_custom_openai_compatible_key_does_not_overwrite_saitec_credentials
```

Expected: PASS.

- [ ] **Step 6: Commit Task 3**

Run:

```powershell
git add src/tui/app/tests/commands_accounts_02/part_01.rs src/tui/app/auth.rs
git commit -m "test: cover custom basemodel credential flow"
```

If `src/tui/app/auth.rs` did not change, stage only the test file.

---

### Task 4: Focused Regression, Build, Dev Launch, And Push

**Files:**
- Validate current tree only.

- [ ] **Step 1: Run focused Rust tests**

Run:

```powershell
cargo test -p jcode custom_openai_compatible_provider_is_saitec_basemodel_allowed
cargo test -p jcode generic_openai_compatible_route_allows_validated_custom_model
cargo test -p jcode test_filtered_login_picker_contains_only_saitec_allowlisted_providers
cargo test -p jcode test_base_models_picker_opens_custom_endpoint_prompt
cargo test -p jcode test_custom_openai_compatible_endpoint_advances_to_key_prompt
cargo test -p jcode test_custom_openai_compatible_key_does_not_overwrite_saitec_credentials
```

Expected: all PASS.

- [ ] **Step 2: Run fast compile verification**

Run:

```powershell
cargo check -p jcode
```

Expected: PASS.

- [ ] **Step 3: Build source**

Run:

```powershell
cargo build -p jcode
```

Expected: PASS. If the local machine terminates the build due to resources, use the repo-provided remote build path:

```powershell
bash scripts/remote_build.sh
```

- [ ] **Step 4: Start the TUI with the repo debug script**

Run:

```powershell
scripts/dev_saitec_tui.ps1
```

Expected: the script leaves a running TUI runtime and writes `dist/dev-saitec-tui/dev-runtime-state.json`.

- [ ] **Step 5: Inspect git state**

Run:

```powershell
git status --short --branch
```

Expected: only the user's pre-existing untracked `a.md` may remain outside the committed work.

- [ ] **Step 6: Push completed commits**

Run:

```powershell
git push
```

Expected: current branch pushes successfully.

---

## Self-Review Notes

- Spec coverage: Task 1 covers SAITEC provider and route allowlists. Task 2 covers Base models visibility and custom provider selection. Task 3 covers endpoint/key persistence and SAITEC credential separation. Task 4 covers final tests, build, dev launch, and push.
- Placeholder scan: the plan contains concrete file paths, test names, code snippets, commands, and expected outcomes.
- Type consistency: the snippets use existing names from the codebase: `PendingLogin::OpenAiCompatibleApiBase`, `PendingLogin::ApiKeyProfile`, `OPENAI_COMPAT_PROFILE`, `save_env_value_to_env_file`, and `ProviderValidationRecord`.
