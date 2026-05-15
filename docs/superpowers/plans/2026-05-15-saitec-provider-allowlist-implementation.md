# SAITEC Provider Allowlist Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restrict SAITEC-TUI base-model login and account-management surfaces so they only show and support OpenAI, Anthropic, Z.AI, Kimi, and Alibaba Cloud Coding while preserving the SAITEC business login flow and the new top-level `/login` selector.

**Architecture:** Add one SAITEC product-layer provider allowlist in `src/saitec/product_profile.rs`, then thread that allowlist through provider list builders, `/account` and `/auth` execution paths, help text, and autocomplete. Reconnect the base-model branch of `/login` to the existing provider login picker so the “original login interface” still exists, but only for the five allowlisted providers, while keeping the separate SAITEC business-login path available.

**Tech Stack:** Rust, ratatui, existing JCode provider metadata/catalog helpers, TUI auth/account command handlers, focused cargo test targets.

---

### Task 1: Add a SAITEC product-level provider allowlist API

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\saitec\product_profile.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\provider_catalog_tests.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\provider_catalog_tests.rs`

- [ ] **Step 1: Write the failing catalog tests for the allowlist source of truth**

```rust
#[test]
fn saitec_allowlisted_provider_ids_match_product_requirements() {
    assert_eq!(
        crate::saitec::product_profile::allowed_base_model_provider_ids(),
        &["openai", "claude", "zai", "kimi", "alibaba-coding-plan"]
    );
}

#[test]
fn saitec_allowlist_accepts_aliases_through_catalog_resolution() {
    let provider = crate::provider_catalog::resolve_login_provider("bailian")
        .expect("alias should resolve");
    assert_eq!(provider.id, "alibaba-coding-plan");
    assert!(crate::saitec::product_profile::is_allowed_base_model_provider(provider.id));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test saitec_allowlisted_provider_ids_match_product_requirements -- --exact`
Expected: FAIL because `allowed_base_model_provider_ids()` and `is_allowed_base_model_provider()` do not exist yet.

- [ ] **Step 3: Add the allowlist helpers to the SAITEC product profile**

```rust
const ALLOWED_BASE_MODEL_PROVIDER_IDS: &[&str] = &[
    "openai",
    "claude",
    "zai",
    "kimi",
    "alibaba-coding-plan",
];

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
```

- [ ] **Step 4: Add a catalog-facing filter helper so later tasks do not duplicate filtering logic**

```rust
#[test]
fn saitec_allowlist_filters_provider_catalog_to_five_entries() {
    let filtered = crate::provider_catalog::saitec_visible_base_model_providers();
    let ids: Vec<&str> = filtered.iter().map(|provider| provider.id).collect();
    assert_eq!(
        ids,
        vec!["claude", "openai", "zai", "kimi", "alibaba-coding-plan"]
    );
}
```

- [ ] **Step 5: Run the focused provider-catalog tests**

Run: `cargo test saitec_allowlist -- --nocapture`
Expected: PASS for the new allowlist tests.

### Task 2: Reconnect the base-model `/login` branch to the original provider picker, filtered to five providers

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\crates\jcode-provider-metadata\src\lib.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\provider_catalog.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`

- [ ] **Step 1: Write the failing `/login` flow regression tests**

```rust
#[test]
fn test_login_mode_selector_routes_base_models_to_filtered_login_picker() {
    let mut app = create_test_app();
    app.input = "/login".to_string();
    app.submit_input();

    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("move to base-model branch");
    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("open filtered login picker");

    assert!(app.login_picker_overlay.is_some());
    assert!(app.account_picker_overlay.is_none());
}

#[test]
fn test_filtered_login_picker_contains_only_saitec_allowlisted_providers() {
    let mut app = create_test_app();
    app.input = "/login".to_string();
    app.submit_input();
    app.handle_key(KeyCode::Down, KeyModifiers::empty())
        .expect("move to base-model branch");
    app.handle_key(KeyCode::Enter, KeyModifiers::empty())
        .expect("open filtered login picker");

    let picker_cell = app.login_picker_overlay.as_ref().expect("login picker");
    let profile = picker_cell.borrow().debug_memory_profile();
    assert_eq!(profile["items_count"], 5);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_login_mode_selector_routes_base_models_to_filtered_login_picker -- --exact`
Expected: FAIL because the base-model selector still opens `account_picker_overlay`.

- [ ] **Step 3: Restore the generic TUI login-provider list in metadata and align the Alibaba display label**

```rust
pub fn tui_login_providers() -> Vec<LoginProviderDescriptor> {
    login_providers_for_surface(LoginProviderSurface::TuiLogin)
}
```

```rust
pub const ALIBABA_CODING_PLAN_LOGIN_PROVIDER: LoginProviderDescriptor = LoginProviderDescriptor {
    id: "alibaba-coding-plan",
    display_name: "Alibaba Cloud Coding",
    aliases: &[
        "bailian",
        "aliyun-bailian",
        "coding-plan",
        "alibaba-coding",
        "alibaba-cloud-coding",
    ],
    // unchanged fields omitted
};
```

- [ ] **Step 4: Add a product-filtered provider list in `src/provider_catalog.rs`**

```rust
pub fn saitec_visible_base_model_providers() -> Vec<LoginProviderDescriptor> {
    jcode_provider_metadata::tui_login_providers()
        .into_iter()
        .filter(|provider| crate::saitec::product_profile::is_allowed_base_model_provider(provider.id))
        .collect()
}
```

- [ ] **Step 5: Replace the base-model branch of `open_login_mode_selector()` with the original login picker**

```rust
AccountPickerItem::action(
    "model-config",
    "Base models",
    "Base-model login or configuration",
    "open the original provider login/configuration picker for supported base models",
    crate::tui::account_picker::AccountPickerCommand::SubmitInput(
        "/login base-models".to_string(),
    ),
)
```

```rust
if requested.eq_ignore_ascii_case("base-models") {
    app.open_base_model_login_picker();
    return true;
}
```

- [ ] **Step 6: Add an app helper that builds `login_picker_overlay` from the filtered provider list**

```rust
pub(crate) fn open_base_model_login_picker(&mut self) {
    use crate::tui::login_picker::{LoginPicker, LoginPickerItem, LoginPickerSummary};

    let status = crate::auth::AuthStatus::check_fast();
    let providers = crate::provider_catalog::saitec_visible_base_model_providers();

    let mut summary = LoginPickerSummary::default();
    let items = providers
        .into_iter()
        .enumerate()
        .map(|(index, provider)| {
            let auth_state = status.state_for_provider(provider);
            match auth_state {
                crate::auth::AuthState::Available => summary.ready_count += 1,
                crate::auth::AuthState::Expired => summary.attention_count += 1,
                crate::auth::AuthState::NotConfigured => summary.setup_count += 1,
            }
            if provider.recommended {
                summary.recommended_count += 1;
            }
            LoginPickerItem::new(
                index + 1,
                provider,
                auth_state,
                status.method_detail_for_provider(provider),
            )
        })
        .collect::<Vec<_>>();

    self.login_picker_overlay = Some(std::cell::RefCell::new(LoginPicker::with_summary(
        " Base-model login ",
        items,
        summary,
    )));
    self.account_picker_overlay = None;
    self.inline_interactive_state = None;
    self.input.clear();
    self.cursor_pos = 0;
    self.set_status_notice("Login: choose a supported base-model provider");
}
```

- [ ] **Step 7: Re-run the focused login-picker regression tests**

Run: `cargo test test_filtered_login_picker_contains_only_saitec_allowlisted_providers -- --exact`
Expected: PASS with only the five provider entries visible.

### Task 3: Filter `/account`, `/auth`, and provider resolution through the SAITEC allowlist while preserving SAITEC login access

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\provider_catalog.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth_account_commands.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth_account_picker.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`

- [ ] **Step 1: Write the failing command and surface regression tests**

```rust
#[test]
fn test_auth_status_lists_only_saitec_allowlisted_base_model_providers() {
    let mut app = create_test_app();
    app.input = "/auth".to_string();
    app.submit_input();

    let msg = app.display_messages().last().expect("missing auth status");
    assert!(msg.content.contains("Saitec Subscription"));
    assert!(msg.content.contains("OpenAI"));
    assert!(msg.content.contains("Anthropic/Claude"));
    assert!(msg.content.contains("Z.AI"));
    assert!(msg.content.contains("Kimi"));
    assert!(msg.content.contains("Alibaba Cloud Coding"));
    assert!(!msg.content.contains("GitHub Copilot"));
    assert!(!msg.content.contains("Google Gemini"));
}

#[test]
fn test_account_openrouter_settings_is_rejected_by_saitec_allowlist() {
    let mut app = create_test_app();
    app.input = "/account openrouter settings".to_string();
    app.submit_input();

    let last = app.display_messages().last().expect("missing response");
    assert_eq!(last.role, "error");
    assert!(last.content.contains("SAITEC-TUI only supports these base-model providers"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_account_openrouter_settings_is_rejected_by_saitec_allowlist -- --exact`
Expected: FAIL because `resolve_account_provider_descriptor("openrouter")` still resolves and `/auth` still renders the broad auth-status list.

- [ ] **Step 3: Add SAITEC-filtered provider accessors in `src/provider_catalog.rs`**

```rust
pub fn saitec_auth_status_login_providers() -> Vec<LoginProviderDescriptor> {
    let mut providers = Vec::from([JCODE_LOGIN_PROVIDER]);
    providers.extend(
        auth_status_login_providers()
            .into_iter()
            .filter(|provider| crate::saitec::product_profile::is_allowed_base_model_provider(provider.id)),
    );
    providers
}

pub fn saitec_account_providers() -> Vec<LoginProviderDescriptor> {
    let mut providers = Vec::from([JCODE_LOGIN_PROVIDER]);
    providers.extend(
        login_providers()
            .iter()
            .copied()
            .filter(|provider| crate::saitec::product_profile::is_allowed_base_model_provider(provider.id)),
    );
    providers
}
```

```rust
pub fn saitec_base_model_account_providers() -> Vec<LoginProviderDescriptor> {
    auth_status_login_providers()
        .into_iter()
        .filter(|provider| crate::saitec::product_profile::is_allowed_base_model_provider(provider.id))
        .collect()
}
```

- [ ] **Step 4: Restrict provider resolution in `/account` and explicit command execution**

```rust
pub(crate) fn resolve_account_provider_descriptor(
    input: &str,
) -> Option<crate::provider_catalog::LoginProviderDescriptor> {
    let provider = crate::provider_catalog::resolve_login_provider(input)?;
    (provider.id == crate::provider_catalog::JCODE_LOGIN_PROVIDER.id
        || crate::saitec::product_profile::is_allowed_base_model_provider(provider.id))
        .then_some(provider)
}
```

```rust
fn unsupported_provider_error() -> String {
    crate::saitec::product_profile::unsupported_base_model_provider_message()
}
```

- [ ] **Step 5: Change `/auth`, `/account`, and doctor/settings builders to use the filtered provider lists**

```rust
let providers = crate::provider_catalog::saitec_auth_status_login_providers();
```

```rust
None => crate::provider_catalog::saitec_account_providers(),
```

```rust
let configured = crate::provider_catalog::saitec_auth_status_login_providers()
    .into_iter()
    .filter(|provider| {
        status.state_for_provider(*provider) != crate::auth::AuthState::NotConfigured
    })
    .collect::<Vec<_>>();
```

- [ ] **Step 6: Tighten default-provider validation to the SAITEC-supported set**

```rust
Some(
    "auto"
        | "claude"
        | "openai"
        | "zai"
        | "kimi"
        | "alibaba-coding-plan"
)
```

```rust
"Unsupported default provider `{}`. Use claude, openai, zai, kimi, alibaba-coding-plan, or auto."
```

- [ ] **Step 7: Run the focused account/auth regression tests**

Run: `cargo test test_auth_status_lists_only_saitec_allowlisted_base_model_providers -- --exact`
Expected: PASS with unsupported providers omitted from `/auth`.

### Task 4: Regenerate help text and autocomplete from the SAITEC allowlist

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\input_help.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\state_ui_input_helpers.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\state_model_poke_02\part_01.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\state_model_poke_02\part_01.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`

- [ ] **Step 1: Write the failing help and autocomplete regression tests**

```rust
#[test]
fn test_account_command_suggestions_hide_unsupported_providers() {
    let app = create_test_app();
    let suggestions = app.get_suggestions_for("/account ");

    assert!(suggestions.iter().any(|(cmd, _)| cmd == "/account openai"));
    assert!(suggestions.iter().any(|(cmd, _)| cmd == "/account claude"));
    assert!(suggestions.iter().any(|(cmd, _)| cmd == "/account zai"));
    assert!(suggestions.iter().any(|(cmd, _)| cmd == "/account kimi"));
    assert!(suggestions.iter().any(|(cmd, _)| cmd == "/account alibaba-coding-plan"));
    assert!(!suggestions.iter().any(|(cmd, _)| cmd == "/account openrouter"));
    assert!(!suggestions.iter().any(|(cmd, _)| cmd == "/account copilot"));
}

#[test]
fn test_help_account_topic_mentions_only_supported_base_model_providers() {
    let mut app = create_test_app();
    app.input = "/help account".to_string();
    app.submit_input();

    let msg = app.display_messages().last().expect("missing help");
    assert!(msg.content.contains("/account jcode login"));
    assert!(msg.content.contains("openai"));
    assert!(msg.content.contains("claude"));
    assert!(msg.content.contains("zai"));
    assert!(msg.content.contains("kimi"));
    assert!(msg.content.contains("alibaba-coding-plan"));
    assert!(!msg.content.contains("openai-compatible"));
    assert!(!msg.content.contains("copilot"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_account_command_suggestions_hide_unsupported_providers -- --exact`
Expected: FAIL because `/account` suggestions are still built from `login_providers()` and hard-coded OpenAI-compatible entries.

- [ ] **Step 3: Replace hard-coded provider help text with the allowlisted provider set**

```rust
let supported = crate::saitec::product_profile::allowed_base_model_provider_ids().join("|");
```

```rust
"`/account default-provider <claude|openai|zai|kimi|alibaba-coding-plan|auto>`"
```

- [ ] **Step 4: Regenerate `/account` suggestions from the filtered provider list**

```rust
for provider in crate::provider_catalog::saitec_account_providers() {
    suggestions.push((
        format!("/account {}", provider.id),
        "Open this provider's account/settings actions",
    ));
    suggestions.push((
        format!("/account {} settings", provider.id),
        "Show provider-specific settings",
    ));
    suggestions.push((
        format!("/account {} login", provider.id),
        "Start or refresh login for this provider",
    ));
}
```

- [ ] **Step 5: Remove unsupported provider-specific suggestion stubs while keeping supported OpenAI settings**

```rust
// Delete:
// "/account openai-compatible settings"
// "/account openai-compatible api-base"
// Copilot-specific suggestion entries
```

- [ ] **Step 6: Keep `/auth doctor` and `/login jcode` suggestions while adding filtered base-model `/login <provider>` suggestions**

```rust
for provider in crate::provider_catalog::saitec_visible_base_model_providers() {
    suggestions.push((
        format!("{base} {}", provider.id),
        provider.menu_detail,
    ));
}
```

- [ ] **Step 7: Run the focused suggestion/help tests**

Run: `cargo test test_auth_doctor_command_suggestion_is_not_shadowed_by_provider_suggestions test_account_command_suggestions_hide_unsupported_providers -- --nocapture`
Expected: PASS with only the five provider ids suggested.

### Task 5: Enforce product-level rejection for unsupported provider commands and verify the end-to-end surface

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth_account_commands.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth_tests.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth_tests.rs`

- [ ] **Step 1: Write the failing command-rejection tests**

```rust
#[test]
fn test_login_openrouter_is_rejected_with_product_message() {
    let mut app = create_test_app();
    app.input = "/login openrouter".to_string();
    app.submit_input();

    let last = app.display_messages().last().expect("missing response");
    assert_eq!(last.role, "error");
    assert_eq!(
        last.content,
        "SAITEC-TUI only supports these base-model providers: openai, claude, zai, kimi, alibaba-cloud-coding."
    );
}

#[test]
fn test_account_copilot_login_is_rejected_with_product_message() {
    let mut app = create_test_app();
    app.input = "/account copilot login".to_string();
    app.submit_input();

    let last = app.display_messages().last().expect("missing response");
    assert_eq!(last.role, "error");
    assert!(last.content.contains("SAITEC-TUI only supports these base-model providers"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_login_openrouter_is_rejected_with_product_message -- --exact`
Expected: FAIL because `/login openrouter` currently falls into the generic Saitec-only error path instead of the new product-level allowlist message.

- [ ] **Step 3: Add explicit unsupported-provider rejection before generic fallback logic**

```rust
if let Some(requested_provider) = crate::provider_catalog::resolve_login_provider(requested) {
    if requested_provider.id != crate::provider_catalog::JCODE_LOGIN_PROVIDER.id
        && !crate::saitec::product_profile::is_allowed_base_model_provider(requested_provider.id)
    {
        app.push_display_message(DisplayMessage::error(
            crate::saitec::product_profile::unsupported_base_model_provider_message(),
        ));
        return true;
    }
}
```

- [ ] **Step 4: Route allowlisted base-model providers into `start_login_provider()` again**

```rust
match crate::provider_catalog::resolve_login_provider(requested) {
    Some(provider) if provider.id == crate::provider_catalog::JCODE_LOGIN_PROVIDER.id => {
        app.start_login_provider(provider);
    }
    Some(provider)
        if crate::saitec::product_profile::is_allowed_base_model_provider(provider.id) =>
    {
        app.start_login_provider(provider);
    }
    Some(_) => app.push_display_message(DisplayMessage::error(
        crate::saitec::product_profile::unsupported_base_model_provider_message(),
    )),
    None => app.push_display_message(DisplayMessage::error(
        "Unknown provider. Use `/login` to choose a supported login mode.".to_string(),
    )),
}
```

- [ ] **Step 5: Update `/account ... login` and provider-specific subcommand failures to use the same product error**

```rust
Some(_) => app.push_display_message(DisplayMessage::error(
    crate::saitec::product_profile::unsupported_base_model_provider_message(),
)),
```

- [ ] **Step 6: Run the focused end-to-end auth/account tests**

Run: `cargo test test_login_openrouter_is_rejected_with_product_message test_account_copilot_login_is_rejected_with_product_message -- --nocapture`
Expected: PASS with the exact product-facing error message.

### Task 6: Run the verification batch and summarize any remaining drift

**Files:**
- Test: `G:\Workspace\Project2026\JCode\jcode\src\provider_catalog_tests.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\auth_tests.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_02\part_01.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\state_model_poke_02\part_01.rs`

- [ ] **Step 1: Run the provider-catalog test batch**

Run: `cargo test saitec_allowlist -- --nocapture`
Expected: PASS for the new allowlist catalog checks.

- [ ] **Step 2: Run the focused auth/account command test batch**

Run: `cargo test commands_accounts_02 -- --nocapture`
Expected: PASS for `/login`, `/account`, and product-level rejection regressions.

- [ ] **Step 3: Run the focused autocomplete/help test batch**

Run: `cargo test state_model_poke_02 -- --nocapture`
Expected: PASS for `/auth` and `/account` suggestion coverage.

- [ ] **Step 4: Run the focused auth module tests**

Run: `cargo test auth_tests -- --nocapture`
Expected: PASS with no regression in SAITEC login-form behavior.

- [ ] **Step 5: Commit**

```bash
git add crates/jcode-provider-metadata/src/lib.rs src/provider_catalog.rs src/provider_catalog_tests.rs src/saitec/product_profile.rs src/tui/app/auth.rs src/tui/app/auth_account_commands.rs src/tui/app/auth_account_picker.rs src/tui/app/input_help.rs src/tui/app/state_ui_input_helpers.rs src/tui/app/auth_tests.rs src/tui/app/tests/commands_accounts_02/part_01.rs src/tui/app/tests/state_model_poke_02/part_01.rs docs/superpowers/plans/2026-05-15-saitec-provider-allowlist-implementation.md
git commit -m "feat: restrict saitec provider surfaces to supported allowlist"
```
