# Login Mode Selector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `/login` open a two-level selector so users can choose between SAITEC login and base-model login/configuration.

**Architecture:** Reuse existing TUI overlay handling instead of inventing a new modal system. Route the first selector option into the current SAITEC pending-login form and the second option into the existing account/configuration center because the trimmed TUI login-provider list no longer exposes the broader provider set.

**Tech Stack:** Rust, ratatui, crossterm, existing JCode TUI overlay/state helpers, focused auth regression tests.

---

### Task 1: Add regression coverage for the new `/login` entry flow

**Files:**
- Modify: `src/tui/app/tests/commands_accounts_02/part_01.rs`
- Test: `src/tui/app/tests/commands_accounts_02/part_01.rs`

- [ ] Assert that `/login` opens a top-level selector state instead of immediately entering the SAITEC form.
- [ ] Assert that pressing `Enter` on the default selector item enters the SAITEC login form.
- [ ] Assert that moving selection and pressing `Enter` opens the existing base-model account/configuration surface.

### Task 2: Add selector state and routing

**Files:**
- Modify: `src/tui/app/auth_types.rs`
- Modify: `src/tui/app/auth.rs`
- Modify: `src/tui/app/auth_account_commands.rs`
- Modify: `src/tui/app/input_help.rs`
- Modify: `src/tui/app/state_ui_input_helpers.rs`

- [ ] Add a `PendingLogin` variant for the top-level `/login` mode selector.
- [ ] Add helper methods to open the selector, move selection, and execute the chosen branch.
- [ ] Change `/login` to open the selector while preserving `/login jcode` as a direct SAITEC shortcut.
- [ ] Update help text and command descriptions to describe the new selector behavior clearly.

### Task 3: Hook keyboard handling into the existing modal path

**Files:**
- Modify: `src/tui/app/input.rs`

- [ ] Reuse the modal key path so `Up`, `Down`, `Enter`, and `Esc` work inside the new selector.
- [ ] Keep the SAITEC form behavior unchanged once the user chooses that branch.

### Task 4: Verify with focused tests

**Files:**
- Test: `src/tui/app/tests/commands_accounts_02/part_01.rs`
- Test: `src/tui/app/auth_tests.rs`

- [ ] Run the focused auth/account test target that covers `/login` command behavior.
- [ ] Confirm the new tests pass and no existing login-specific tests regress.
