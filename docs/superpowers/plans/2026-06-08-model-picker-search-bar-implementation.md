# Model Picker Search Bar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a visible search/help row to the `/model` picker only.

**Architecture:** Keep existing model filtering state and keyboard handling unchanged. Add model-only rendering in `src/tui/ui_inline_interactive.rs`, and verify through TUI buffer-render tests in `src/tui/ui_tests/mod.rs`.

**Tech Stack:** Rust 2024, ratatui `TestBackend`, existing `TuiState` test harness, Cargo.

---

## File Structure

- Modify: `src/tui/ui_tests/mod.rs`
  - Adds renderer tests proving the model picker shows typed search text and `Esc` help while account and agent-target pickers do not receive the model search row.
- Modify: `src/tui/ui_inline_interactive.rs`
  - Adds small helper functions for building/truncating the model picker search/help row.
  - Reserves one extra row only for real model pickers, excluding agent-target pickers.
- Verify: `cargo test -p jcode model_picker_search_bar --lib`
- Verify: `cargo check -p jcode`
- Verify: `cargo build -p jcode`
- Verify: `powershell -ExecutionPolicy Bypass -File scripts/dev_saitec_tui.ps1`

### Task 1: Renderer Tests

**Files:**
- Modify: `src/tui/ui_tests/mod.rs`

- [ ] **Step 1: Write failing tests**

Add tests near the existing inline picker tests:

```rust
fn sample_account_picker_state() -> crate::tui::InlineInteractiveState {
    crate::tui::InlineInteractiveState {
        kind: crate::tui::PickerKind::Account,
        entries: vec![crate::tui::PickerEntry {
            name: "work".to_string(),
            options: vec![crate::tui::PickerOption {
                provider: "OpenAI".to_string(),
                api_method: "saved".to_string(),
                available: true,
                detail: String::new(),
                estimated_reference_cost_micros: None,
            }],
            action: crate::tui::PickerAction::Account(
                crate::tui::AccountPickerAction::Switch {
                    provider_id: "openai".to_string(),
                    label: "work".to_string(),
                },
            ),
            selected_option: 0,
            is_current: true,
            is_default: false,
            recommended: false,
            recommendation_rank: usize::MAX,
            old: false,
            created_date: None,
            effort: None,
        }],
        filtered: vec![0],
        selected: 0,
        column: 0,
        filter: String::new(),
        preview: false,
    }
}

fn sample_agent_target_picker() -> crate::tui::InlineInteractiveState {
    crate::tui::InlineInteractiveState {
        kind: crate::tui::PickerKind::Model,
        entries: vec![crate::tui::PickerEntry {
            name: "Swarm / subagent".to_string(),
            options: vec![crate::tui::PickerOption {
                provider: "gpt-5 default".to_string(),
                api_method: "agents.swarm_model".to_string(),
                available: true,
                detail: "/agents swarm".to_string(),
                estimated_reference_cost_micros: None,
            }],
            action: crate::tui::PickerAction::AgentTarget(
                crate::tui::AgentModelTarget::Swarm,
            ),
            selected_option: 0,
            is_current: false,
            is_default: false,
            recommended: false,
            recommendation_rank: usize::MAX,
            old: false,
            created_date: None,
            effort: None,
        }],
        filtered: vec![0],
        selected: 0,
        column: 0,
        filter: String::new(),
        preview: false,
    }
}

#[test]
fn model_picker_search_bar_shows_filter_and_escape_hint() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(100, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let mut picker = sample_model_picker();
    picker.filter = "kimi".to_string();
    let state = TestState {
        inline_interactive_state: Some(picker),
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("inline picker draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(rendered.contains("Search: kimi"), "rendered: {rendered}");
    assert!(rendered.contains("Esc: back/close"), "rendered: {rendered}");
}

#[test]
fn model_picker_search_bar_prompts_when_filter_is_empty() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(100, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        inline_interactive_state: Some(sample_model_picker()),
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("inline picker draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(
        rendered.contains("Search: type to filter models"),
        "rendered: {rendered}"
    );
}

#[test]
fn account_picker_does_not_render_model_picker_search_bar() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(100, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        inline_interactive_state: Some(sample_account_picker_state()),
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("inline picker draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(!rendered.contains("Search:"), "rendered: {rendered}");
}

#[test]
fn agent_target_picker_does_not_render_model_picker_search_bar() {
    let _guard = viewport_snapshot_test_lock();
    let backend = ratatui::backend::TestBackend::new(100, 24);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");
    let state = TestState {
        inline_interactive_state: Some(sample_agent_target_picker()),
        ..Default::default()
    };

    clear_test_render_state_for_tests();
    terminal
        .draw(|frame| crate::tui::ui::draw(frame, &state))
        .expect("inline picker draw should succeed");

    let rendered = buffer_to_text(&terminal).join("\n");
    assert!(!rendered.contains("Search:"), "rendered: {rendered}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jcode model_picker_search_bar --lib`
Expected: FAIL because the search row is not rendered yet.

### Task 2: Model-Only Search Row

**Files:**
- Modify: `src/tui/ui_inline_interactive.rs`

- [ ] **Step 1: Implement the minimal renderer change**

Add a model-only search row helper and reserve one extra list row only when `picker.kind == PickerKind::Model && !picker.is_agent_target_picker()`.

```rust
fn model_picker_search_bar(picker: &crate::tui::InlineInteractiveState, width: usize) -> Line<'static> {
    let query = if picker.filter.is_empty() {
        "type to filter models".to_string()
    } else {
        picker.filter.clone()
    };
    let text = format!(
        " Search: {}  Esc: back/close  Enter: select  Up/Down: move",
        query
    );
    Line::from(Span::styled(
        truncate_display(&text, width),
        Style::default().fg(dim_color()),
    ))
}
```

Render it before the existing header when the picker kind is `Model`, and change list height from `height.saturating_sub(1)` to subtract both the search row and the header.

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p jcode model_picker_search_bar --lib`
Expected: PASS.

- [ ] **Step 3: Run broader checks**

Run: `cargo check -p jcode`
Expected: exit 0.

Run: `cargo build -p jcode`
Expected: exit 0.

Run: `powershell -ExecutionPolicy Bypass -File scripts/dev_saitec_tui.ps1`
Expected: starts the TUI debug script without a startup error.

- [ ] **Step 4: Commit**

```bash
git add src/tui/ui_tests/mod.rs src/tui/ui_inline_interactive.rs docs/superpowers/plans/2026-06-08-model-picker-search-bar-implementation.md
git commit -m "feat: show model picker search input"
```
