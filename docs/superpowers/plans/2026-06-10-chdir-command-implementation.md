# Chdir Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `/chdir` and `/cd` so the current TUI session can persistently change its working directory.

**Architecture:** Keep the feature in the existing session-command path. The command resolves and validates a directory, updates `App.session.working_dir`, saves the session, and lets existing footer/tool consumers pick up the changed session field.

**Tech Stack:** Rust, existing ratatui TUI app state, `crate::session::Session` persistence, existing `cargo test` and `cargo check` workflow.

---

## File Structure

- Modify: `src/tui/app/commands.rs`
  - Add a small `resolve_chdir_target` helper near `active_working_dir`.
  - Add `handle_chdir_command` and call it from `handle_session_command`.
  - Keep the command local to existing session command handling; no new module is needed.
- Modify: `src/tui/app/state_ui_input_helpers.rs`
  - Register `/chdir` and `/cd` for help/autocomplete and mark them as argument-taking commands.
- Modify: `src/tui/app/input_help.rs`
  - Add `/help chdir` and `/help cd` details.
- Modify: `src/saitec/product_profile.rs`
  - Add `/chdir` and `/cd` to the SAITEC public command surface so they are visible in suggestions and public help.
- Modify: `src/tui/app/tests/commands_accounts_01/part_01.rs`
  - Add focused tests for absolute, relative, alias, and invalid path behavior.
- Modify: `src/tui/app/tests/state_model_poke_02/part_01.rs`
  - Update command suggestion expectations for the new public command.

---

### Task 1: Add Failing `/chdir` Tests

**Files:**
- Modify: `src/tui/app/tests/commands_accounts_01/part_01.rs`

- [ ] **Step 1: Write the failing tests**

Add these tests near the existing `/git` working-directory tests:

```rust
#[test]
fn test_chdir_command_updates_and_persists_absolute_working_directory() {
    with_temp_jcode_home(|| {
        let target = tempfile::tempdir().expect("target dir");
        let mut app = create_test_app();
        let session_id = app.session.id.clone();

        app.input = format!("/chdir {}", target.path().display());
        app.submit_input();

        let expected = target.path().canonicalize().expect("canonical target");
        assert_eq!(
            app.session.working_dir.as_deref(),
            Some(expected.to_string_lossy().as_ref())
        );

        let persisted = crate::session::Session::load(&session_id).expect("load session");
        assert_eq!(
            persisted.working_dir.as_deref(),
            Some(expected.to_string_lossy().as_ref())
        );

        let msg = app.display_messages().last().expect("missing chdir response");
        assert_eq!(msg.role, "system");
        assert!(msg.content.contains("Changed working directory"));
    });
}

#[test]
fn test_chdir_command_resolves_relative_path_from_session_working_directory() {
    with_temp_jcode_home(|| {
        let base = tempfile::tempdir().expect("base dir");
        let child = base.path().join("child");
        std::fs::create_dir(&child).expect("create child dir");

        let mut app = create_test_app();
        app.session.working_dir = Some(base.path().display().to_string());
        app.input = "/chdir child".to_string();
        app.submit_input();

        let expected = child.canonicalize().expect("canonical child");
        assert_eq!(
            app.session.working_dir.as_deref(),
            Some(expected.to_string_lossy().as_ref())
        );
    });
}

#[test]
fn test_cd_alias_updates_working_directory() {
    with_temp_jcode_home(|| {
        let target = tempfile::tempdir().expect("target dir");
        let mut app = create_test_app();

        app.input = format!("/cd {}", target.path().display());
        app.submit_input();

        let expected = target.path().canonicalize().expect("canonical target");
        assert_eq!(
            app.session.working_dir.as_deref(),
            Some(expected.to_string_lossy().as_ref())
        );
    });
}

#[test]
fn test_chdir_invalid_path_leaves_working_directory_unchanged() {
    with_temp_jcode_home(|| {
        let base = tempfile::tempdir().expect("base dir");
        let missing = base.path().join("missing");
        let mut app = create_test_app();
        let original = base.path().display().to_string();
        app.session.working_dir = Some(original.clone());

        app.input = format!("/chdir {}", missing.display());
        app.submit_input();

        assert_eq!(app.session.working_dir.as_deref(), Some(original.as_str()));
        let msg = app.display_messages().last().expect("missing chdir error");
        assert_eq!(msg.role, "error");
        assert!(msg.content.contains("does not exist"));
    });
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
cargo test test_chdir_command -- --nocapture
```

Expected: FAIL because `/chdir` is currently treated as an unknown skill or normal slash miss, and `session.working_dir` is not updated.

---

### Task 2: Implement `/chdir` in Session Commands

**Files:**
- Modify: `src/tui/app/commands.rs`

- [ ] **Step 1: Add the path resolver helper**

Place this near `active_working_dir`:

```rust
fn resolve_chdir_target(app: &App, raw_path: &str) -> Result<std::path::PathBuf, String> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err("Usage: `/chdir <path>`".to_string());
    }

    let input_path = std::path::PathBuf::from(trimmed);
    let candidate = if input_path.is_absolute() {
        input_path
    } else if let Some(base) = active_working_dir(app) {
        base.join(input_path)
    } else {
        std::env::current_dir()
            .map_err(|error| format!("Unable to read current directory: {}", error))?
            .join(input_path)
    };

    if !candidate.exists() {
        return Err(format!("Directory `{}` does not exist.", candidate.display()));
    }
    if !candidate.is_dir() {
        return Err(format!("Path `{}` is not a directory.", candidate.display()));
    }

    candidate
        .canonicalize()
        .map_err(|error| format!("Unable to resolve `{}`: {}", candidate.display(), error))
}
```

- [ ] **Step 2: Add the command handler**

Place this near other session command helpers:

```rust
fn handle_chdir_command(app: &mut App, trimmed: &str) -> bool {
    let raw_path = if trimmed == "/chdir" || trimmed == "/cd" {
        ""
    } else if let Some(path) = trimmed.strip_prefix("/chdir ") {
        path
    } else if let Some(path) = trimmed.strip_prefix("/cd ") {
        path
    } else if trimmed.starts_with("/chdir") || trimmed.starts_with("/cd") {
        ""
    } else {
        return false;
    };

    let previous = app.session.working_dir.clone();
    let target = match resolve_chdir_target(app, raw_path) {
        Ok(target) => target,
        Err(error) => {
            app.push_display_message(DisplayMessage::error(error));
            return true;
        }
    };

    let target_display = target.display().to_string();
    app.session.working_dir = Some(target_display.clone());
    if let Err(error) = app.session.save() {
        app.session.working_dir = previous;
        app.push_display_message(DisplayMessage::error(format!(
            "Failed to save working directory: {}",
            error
        )));
        return true;
    }

    app.push_display_message(DisplayMessage::system(format!(
        "Changed working directory to `{}`.",
        target_display
    )));
    app.set_status_notice(format!(
        "CWD: {}",
        crate::util::truncate_str(&target_display, 48)
    ));
    true
}
```

- [ ] **Step 3: Wire the handler into `handle_session_command`**

In the first handler chain inside `handle_session_command`, insert the new handler immediately after `handle_btw_command(app, trimmed)`:

```rust
        || handle_btw_command(app, trimmed)
        || handle_chdir_command(app, trimmed)
        || handle_export_command(app, trimmed)
```

Do not move the later `/clear`, `/rewind`, `/poke`, `/transfer`, memory, swarm, improve, or refactor branches.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```powershell
cargo test test_chdir_command -- --nocapture
```

Expected: PASS for the new `/chdir` tests.

- [ ] **Step 5: Commit**

Run:

```powershell
git add -- src/tui/app/commands.rs src/tui/app/tests/commands_accounts_01/part_01.rs
git commit -m "feat: add chdir command"
```

---

### Task 3: Register Help, Public Visibility, and Autocomplete

**Files:**
- Modify: `src/tui/app/state_ui_input_helpers.rs`
- Modify: `src/tui/app/input_help.rs`
- Modify: `src/saitec/product_profile.rs`
- Modify: `src/tui/app/tests/state_model_poke_02/part_01.rs`

- [ ] **Step 1: Register public commands**

In `REGISTERED_COMMANDS`, add entries near `/git`:

```rust
RegisteredCommand::public("/chdir", "Change the current session working directory"),
RegisteredCommand::public("/cd", "Alias for /chdir"),
```

- [ ] **Step 2: Mark both commands as accepting path arguments**

In `App::command_accepts_args`, add both commands near `/git` and `/export`:

```rust
"/chdir"
    | "/cd"
    | "/help"
    | "/?"
    | "/btw"
    | "/export"
    | "/git"
```

- [ ] **Step 3: Add SAITEC public visibility**

In `src/saitec/product_profile.rs`, add both commands to `PUBLIC_COMMANDS` near `/export`:

```rust
"/chdir",
"/cd",
```

Also update `public_command_list_contains_saitec_surface_commands`:

```rust
assert!(public.contains(&"/chdir"));
assert!(public.contains(&"/cd"));
```

- [ ] **Step 4: Add command help**

In `App::command_help`, add this match arm near `"git"`:

```rust
"chdir" | "cd" => {
    "`/chdir <path>`\nChange the current session working directory and save it to this session.\n\n`/cd <path>`\nAlias for `/chdir <path>`.\n\nRelative paths are resolved from the current session working directory when available, otherwise from the current client directory."
}
```

- [ ] **Step 5: Add focused help and suggestion tests**

Add this test near other `/help` command tests in `src/tui/app/tests/commands_accounts_01/part_01.rs`:

```rust
#[test]
fn test_help_chdir_describes_session_working_directory() {
    let mut app = create_test_app();
    app.input = "/help chdir".to_string();
    app.submit_input();

    let msg = app.display_messages().last().expect("missing help response");
    assert_eq!(msg.role, "system");
    assert!(msg.content.contains("/chdir <path>"));
    assert!(msg.content.contains("session working directory"));
}
```

In `src/tui/app/tests/state_model_poke_02/part_01.rs`, update `test_registered_command_suggestions_match_saitec_public_surface` so the public command loop includes the new commands:

```rust
        "/chdir",
        "/cd",
```

Add a direct fuzzy suggestion test near `test_fuzzy_command_suggestions`:

```rust
#[test]
fn test_chdir_command_suggestions() {
    let app = create_test_app();
    let suggestions = app.get_suggestions_for("/chd");
    assert!(suggestions.iter().any(|(cmd, _)| cmd == "/chdir"));

    let alias = app.get_suggestions_for("/c");
    assert!(alias.iter().any(|(cmd, _)| cmd == "/cd"));
}
```

- [ ] **Step 6: Run focused help, suggestion, and product-profile tests**

Run:

```powershell
cargo test test_help_chdir_describes_session_working_directory -- --nocapture
cargo test test_chdir_command_suggestions -- --nocapture
cargo test test_registered_command_suggestions_match_saitec_public_surface -- --nocapture
cargo test public_command_list_contains_saitec_surface_commands -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```powershell
git add -- src/tui/app/state_ui_input_helpers.rs src/tui/app/input_help.rs src/saitec/product_profile.rs src/tui/app/tests/commands_accounts_01/part_01.rs src/tui/app/tests/state_model_poke_02/part_01.rs
git commit -m "docs: expose chdir command help"
```

---

### Task 4: Final Verification and Dev Launch

**Files:**
- No source edits expected.

- [ ] **Step 1: Run focused command tests**

Run:

```powershell
cargo test test_chdir_command -- --nocapture
cargo test test_cd_alias_updates_working_directory -- --nocapture
cargo test test_chdir_invalid_path_leaves_working_directory_unchanged -- --nocapture
cargo test test_help_chdir_describes_session_working_directory -- --nocapture
cargo test test_chdir_command_suggestions -- --nocapture
cargo test public_command_list_contains_saitec_surface_commands -- --nocapture
```

Expected: PASS.

- [ ] **Step 2: Run `cargo check`**

Run:

```powershell
cargo check
```

Expected: PASS. If this is killed for local resource pressure, run the repo remote build path:

```powershell
bash scripts/remote_build.sh
```

- [ ] **Step 3: Build the source**

Run:

```powershell
cargo build
```

Expected: PASS. If local build is terminated for resource pressure, use `bash scripts/remote_build.sh`.

- [ ] **Step 4: Run the dev debug script**

Run:

```powershell
scripts/dev_saitec_tui.ps1
```

Expected: the script builds/installs the selfdev runtime, writes `dist/dev-saitec-tui/dev-runtime-state.json`, and leaves a running TUI process for inspection.

- [ ] **Step 5: Inspect git status**

Run:

```powershell
git status --short --branch
```

Expected: only unrelated pre-existing files such as `a.md` remain untracked or modified.

- [ ] **Step 6: Push commits**

Run:

```powershell
git push
```

Expected: the branch pushes successfully to `origin/main`.
