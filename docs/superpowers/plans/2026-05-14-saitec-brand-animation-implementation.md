# SAITEC Brand Animation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a restrained animated SAITEC text-logo treatment for startup and header rendering without changing layout or command behavior.

**Architecture:** Keep animation logic inside `src/tui/ui_header.rs` and feed it from existing `TuiState::animation_elapsed()` plus the decorative-animation policy. Startup text-logo rendering and the persistent brand line will share compact helper functions so the visible effect stays consistent while the layout remains unchanged.

**Tech Stack:** Rust, ratatui, unicode-width, existing TUI redraw policy and test harness

---

### Task 1: Define animation behavior with tests

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui_header.rs`

- [ ] **Step 1: Write failing tests for animated startup logo and brand pulse**

Add unit tests that compare startup logo frames across elapsed times, verify line lengths stay fixed, verify the persistent `🍇 SAITEC-TUI` text stays unchanged, and verify static fallback when animations are disabled.

- [ ] **Step 2: Run targeted tests to verify they fail for the new behavior**

Run: `cargo test --package jcode --lib startup_logo_ persistent_header_brand_`
Expected: FAIL because the animation helpers and assertions do not exist yet.

- [ ] **Step 3: Implement the minimal helper surface required by the tests**

Add helper entry points for animated startup logo lines and animated brand line spans so the tests can drive rendering without needing a full terminal frame.

- [ ] **Step 4: Re-run targeted tests to verify the helpers now satisfy the new behavior**

Run: `cargo test --package jcode --lib startup_logo_ persistent_header_brand_`
Expected: PASS

### Task 2: Wire animation into visible rendering paths

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui_header.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui.rs`

- [ ] **Step 1: Replace startup text-logo line generation with animated line rendering**

Use the new helpers in both `build_startup_header()` and `draw_startup_text_logo()` so startup surfaces and fallback text-logo rendering share the same animation behavior.

- [ ] **Step 2: Pulse the persistent brand header line**

When the session name is absent and the SAITEC product brand is shown, render the brand as animated spans instead of a single static span.

- [ ] **Step 3: Keep disabled-animation mode static**

Guard animation styling and glyph cycling behind `crate::perf::tui_policy().enable_decorative_animations`.

- [ ] **Step 4: Run the focused ui_header test set**

Run: `cargo test --package jcode --lib ui_header`
Expected: PASS

### Task 3: Verify no startup-layout regressions

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui_header.rs` (tests only if needed)

- [ ] **Step 1: Run the previously verified startup footer regression tests**

Run: `cargo test --package jcode --lib startup_header_footer_uses_separate_working_dir_line`
Expected: PASS

- [ ] **Step 2: Run the startup header noise/layout regression test**

Run: `cargo test --package jcode --lib build_startup_header_hides_runtime_noise_and_keeps_footer`
Expected: PASS

- [ ] **Step 3: Summarize verified behavior**

Record that the startup footer remains unchanged, the logo animates only through text styling/glyph density, and the persistent header preserves exact branding text.
