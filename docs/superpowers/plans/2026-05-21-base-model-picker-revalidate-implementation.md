# Base-Model Picker Revalidate Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an in-picker action that revalidates the currently selected base-model provider and refreshes the red/yellow/green runtime-validation status without forcing the user back through login.

**Architecture:** Extend the login picker overlay with a dedicated revalidate action keyed off the selected provider, then let the app layer run the existing `auth-test`-backed runtime validation asynchronously. When validation completes, publish a focused bus event so local and remote TUI paths can refresh the open picker in place and surface a success or failure notice.

**Tech Stack:** Rust, ratatui TUI overlays, existing auth validation persistence, existing bus event plumbing, focused cargo tests.

---

### Task 1: Add failing tests for the new picker action

**Files:**
- Modify: `src/tui/login_picker.rs`
- Modify: `src/tui/app/auth_tests.rs`

- [ ] **Step 1: Add a login-picker unit test for the new keybinding**

Write a test that selects a provider and asserts pressing `r` returns a dedicated revalidate overlay action instead of falling through to filtering.

- [ ] **Step 2: Run the focused login-picker test to verify it fails**

Run: `cargo test --profile selfdev test_login_picker_revalidate_key_targets_selected_provider --lib`
Expected: FAIL because the overlay action and key handling do not exist yet.

- [ ] **Step 3: Add an app-level failing test for picker refresh after validation**

Write a test that opens the base-model picker, stores a fresh validation record, injects a validation-completed bus event, and asserts the picker refreshes the current provider status/notice without closing.

- [ ] **Step 4: Run the focused auth test to verify it fails**

Run: `cargo test --profile selfdev provider_validation_completion_refreshes_open_login_picker_status --lib`
Expected: FAIL because no provider-validation completion event is handled yet.

### Task 2: Implement the picker revalidate action

**Files:**
- Modify: `src/tui/login_picker.rs`
- Modify: `src/tui/app/auth_account_picker_saved_accounts.rs`
- Modify: `src/tui/app/navigation.rs`

- [ ] **Step 1: Add a new overlay action variant for revalidation**

Extend `OverlayAction` so the picker can return a provider-targeted revalidate action distinct from `Execute`.

- [ ] **Step 2: Wire `r` to the selected provider**

Update picker key handling and footer/detail hints so users can discover and invoke revalidation from the overlay.

- [ ] **Step 3: Run the focused login-picker test to verify it passes**

Run: `cargo test --profile selfdev test_login_picker_revalidate_key_targets_selected_provider --lib`
Expected: PASS.

### Task 3: Implement async validation completion refresh

**Files:**
- Modify: `src/bus.rs`
- Modify: `src/tui/app/auth.rs`
- Modify: `src/tui/app/local.rs`
- Modify: `src/tui/app/remote.rs`

- [ ] **Step 1: Add a focused bus event for provider validation completion**

Define a payload that carries provider identity plus success/failure message so the TUI can refresh state without reusing `LoginCompleted`.

- [ ] **Step 2: Add an app helper to preserve and rebuild the open base-model picker**

Refresh the overlay items from persisted validation data while keeping the current selection/filter when possible.

- [ ] **Step 3: Start async revalidation from the picker action**

Reuse `run_post_login_validation_quiet(...)`, then publish the new bus event with a user-facing message.

- [ ] **Step 4: Handle the new bus event in both local and remote TUI loops**

Refresh the open picker in place and surface a status notice or display message based on success/failure.

- [ ] **Step 5: Run the focused auth test to verify it passes**

Run: `cargo test --profile selfdev provider_validation_completion_refreshes_open_login_picker_status --lib`
Expected: PASS.

### Task 4: Verify and ship

**Files:**
- Modify: `src/tui/login_picker.rs`
- Modify: `src/tui/app/auth.rs`
- Modify: `src/tui/app/auth_tests.rs`
- Modify: `src/bus.rs`
- Modify: `src/tui/app/local.rs`
- Modify: `src/tui/app/remote.rs`

- [ ] **Step 1: Run focused regression tests**

Run:
- `cargo test --profile selfdev test_login_picker_revalidate_key_targets_selected_provider --lib`
- `cargo test --profile selfdev provider_validation_completion_refreshes_open_login_picker_status --lib`
- `cargo test --profile selfdev test_filtered_login_picker_uses_validation_results_for_provider_status_text --lib`

Expected: PASS.

- [ ] **Step 2: Run broader verification**

Run:
- `cargo check --profile selfdev`
- `cargo build --profile selfdev`

Expected: PASS with only pre-existing warnings.

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/plans/2026-05-21-base-model-picker-revalidate-implementation.md src/bus.rs src/tui/login_picker.rs src/tui/app/auth.rs src/tui/app/auth_account_picker_saved_accounts.rs src/tui/app/navigation.rs src/tui/app/local.rs src/tui/app/remote.rs src/tui/app/auth_tests.rs
git commit -m "Add picker-triggered provider revalidation"
```
