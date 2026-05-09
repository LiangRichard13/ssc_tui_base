# SAITEC-TUI Frontend Remodel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the SAITEC-TUI Product Mode front-end remodel: SAITEC branding, public-command surface reduction, hidden-but-compatible old commands, disabled decorative animations by default, stable table alignment, and a SAITEC-branded Windows package output.

**Architecture:** Add a small `saitec::product_profile` policy layer that centralizes branding and visibility rules, then route the existing TUI header/help/autocomplete/default-config code through that layer instead of scattering SAITEC-specific conditionals. Keep old commands executable for compatibility, but filter them out of public UI. Reuse the current release build path and add a thin SAITEC packaging script or branded copy step instead of renaming the whole binary stack.

**Tech Stack:** Rust, ratatui TUI, existing JCode command registry/helpers, markdown table renderer, PowerShell install/package scripts, cargo test/build.

---

### Task 1: Add The SAITEC Product Profile Layer

**Files:**
- Create: `G:\Workspace\Project2026\JCode\jcode\src\saitec\product_profile.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\saitec\mod.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\lib.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\saitec\product_profile.rs`

- [ ] **Step 1: Write the failing product-profile tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_command_list_contains_saitec_surface_commands() {
        let public = public_commands();

        assert!(public.contains(&"/help"));
        assert!(public.contains(&"/login"));
        assert!(public.contains(&"/logout"));
        assert!(public.contains(&"/auth"));
        assert!(public.contains(&"/model"));
        assert!(public.contains(&"/clear"));
        assert!(public.contains(&"/resume"));
        assert!(public.contains(&"/usage"));
        assert!(public.contains(&"/version"));
        assert!(public.contains(&"/quit"));
    }

    #[test]
    fn hidden_compatible_commands_include_git_and_selfdev() {
        assert_eq!(command_visibility("/git"), CommandVisibility::HiddenCompatible);
        assert_eq!(command_visibility("/selfdev"), CommandVisibility::HiddenCompatible);
        assert_eq!(command_visibility("/improve"), CommandVisibility::HiddenCompatible);
    }

    #[test]
    fn saitec_brand_header_uses_grape_logo() {
        assert_eq!(brand_header_label(), "🍇 SAITEC-TUI");
    }

    #[test]
    fn product_mode_disables_skill_visibility() {
        assert!(!show_skills_in_ui());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test saitec_brand_header_uses_grape_logo -- --exact`
Expected: FAIL because `product_profile.rs` does not exist yet.

- [ ] **Step 3: Implement the minimal SAITEC product profile**

```rust
// src/saitec/product_profile.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandVisibility {
    Public,
    HiddenCompatible,
    InternalOnly,
}

const PUBLIC_COMMANDS: &[&str] = &[
    "/help",
    "/?",
    "/commands",
    "/login",
    "/logout",
    "/auth",
    "/model",
    "/models",
    "/clear",
    "/resume",
    "/sessions",
    "/usage",
    "/version",
    "/quit",
];

const HIDDEN_COMPATIBLE_COMMANDS: &[&str] = &[
    "/git",
    "/selfdev",
    "/feedback",
    "/subscription",
    "/review",
    "/judge",
    "/swarm",
    "/memory",
    "/refactor",
    "/improve",
    "/autoreview",
    "/autojudge",
    "/observe",
    "/subagent",
    "/workspace",
    "/catchup",
    "/back",
    "/splitview",
    "/split-view",
    "/split",
    "/transfer",
    "/rebuild",
    "/restart",
    "/reload",
];

pub fn brand_header_label() -> &'static str {
    "🍇 SAITEC-TUI"
}

pub fn show_skills_in_ui() -> bool {
    false
}

pub fn emphasize_mcp_status() -> bool {
    true
}

pub fn command_visibility(command: &str) -> CommandVisibility {
    if PUBLIC_COMMANDS.contains(&command) {
        CommandVisibility::Public
    } else if HIDDEN_COMPATIBLE_COMMANDS.contains(&command) {
        CommandVisibility::HiddenCompatible
    } else {
        CommandVisibility::InternalOnly
    }
}

pub fn public_commands() -> Vec<&'static str> {
    PUBLIC_COMMANDS.to_vec()
}
```

- [ ] **Step 4: Export the new profile module**

```rust
// src/saitec/mod.rs
pub mod auth;
pub mod paths;
pub mod product_profile;
```

```rust
// src/lib.rs
pub mod saitec;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test public_command_list_contains_saitec_surface_commands hidden_compatible_commands_include_git_and_selfdev saitec_brand_header_uses_grape_logo product_mode_disables_skill_visibility -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/saitec/mod.rs src/saitec/product_profile.rs src/lib.rs
git commit -m "feat: add saitec product profile for tui visibility"
```

### Task 2: Move Header Branding To SAITEC Product Mode

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui_header.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui_header.rs`

- [ ] **Step 1: Write the failing header tests**

```rust
#[test]
fn build_persistent_header_shows_saitec_brand_when_session_name_missing() {
    let app = create_test_app();
    let lines = build_persistent_header(&app, 80);
    let rendered = lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .map(|span| span.content.as_ref())
        .collect::<String>();

    assert!(rendered.contains("🍇 SAITEC-TUI"), "rendered: {rendered}");
    assert!(!rendered.contains("JCode"), "rendered: {rendered}");
}

#[test]
fn build_header_lines_hides_skills_line_in_saitec_product_mode() {
    let app = create_test_app();
    let lines = build_header_lines(&app, 120);
    let rendered = lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .map(|span| span.content.as_ref())
        .collect::<String>();

    assert!(!rendered.contains("skills:"), "rendered: {rendered}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test build_persistent_header_shows_saitec_brand_when_session_name_missing -- --exact`
Expected: FAIL because the header still renders `JCode`.

- [ ] **Step 3: Update the header to use the SAITEC product profile**

```rust
// inside build_persistent_header
} else if server_name.is_none() {
    lines.push(
        Line::from(Span::styled(
            crate::saitec::product_profile::brand_header_label().to_string(),
            Style::default().fg(header_name_color()),
        ))
        .alignment(align),
    );
}
```

```rust
// inside build_header_lines
let skills = app.available_skills();
if crate::saitec::product_profile::show_skills_in_ui() && !skills.is_empty() {
    let full = format!(
        "skills: {}",
        skills
            .iter()
            .map(|s| format!("/{}", s))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let skills_text = if full.chars().count() <= w {
        full
    } else {
        format!("skills: {} loaded", skills.len())
    };
    lines.push(
        Line::from(Span::styled(skills_text, Style::default().fg(dim_color())))
            .alignment(align),
    );
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test build_persistent_header_shows_saitec_brand_when_session_name_missing build_header_lines_hides_skills_line_in_saitec_product_mode -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/ui_header.rs
git commit -m "feat: brand tui header for saitec product mode"
```

### Task 3: Filter Public Slash Commands And Suggestions Through Product Mode

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\state_ui_input_helpers.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\state_model_poke_02\part_01.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\state_model_poke_02\part_01.rs`

- [ ] **Step 1: Write the failing command-suggestion tests**

```rust
#[test]
fn slash_suggestions_hide_git_and_selfdev_in_saitec_product_mode() {
    let app = create_test_app();
    let suggestions = app.get_suggestions_for("/");
    let commands: Vec<String> = suggestions.into_iter().map(|(cmd, _)| cmd).collect();

    assert!(!commands.iter().any(|cmd| cmd == "/git"));
    assert!(!commands.iter().any(|cmd| cmd == "/selfdev"));
}

#[test]
fn slash_suggestions_keep_public_saitec_commands() {
    let app = create_test_app();
    let suggestions = app.get_suggestions_for("/");
    let commands: Vec<String> = suggestions.into_iter().map(|(cmd, _)| cmd).collect();

    assert!(commands.iter().any(|cmd| cmd == "/help"));
    assert!(commands.iter().any(|cmd| cmd == "/login"));
    assert!(commands.iter().any(|cmd| cmd == "/logout"));
    assert!(commands.iter().any(|cmd| cmd == "/model"));
    assert!(commands.iter().any(|cmd| cmd == "/quit"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test slash_suggestions_hide_git_and_selfdev_in_saitec_product_mode -- --exact`
Expected: FAIL because `/git` and `/selfdev` are still public/autocomplete-enabled.

- [ ] **Step 3: Reclassify command registration through the product profile**

```rust
impl App {
    fn command_candidates(&self) -> Vec<(String, &'static str)> {
        let mut seen = std::collections::HashSet::new();
        let mut commands: Vec<(String, &'static str)> = REGISTERED_COMMANDS
            .iter()
            .filter(|command| command.autocomplete)
            .filter(|command| !command.remote_only || self.is_remote)
            .filter(|command| {
                matches!(
                    crate::saitec::product_profile::command_visibility(command.name),
                    crate::saitec::product_profile::CommandVisibility::Public
                )
            })
            .filter_map(|command| {
                let name = command.name.to_string();
                seen.insert(name.clone()).then_some((name, command.help))
            })
            .collect();

        if crate::saitec::product_profile::show_skills_in_ui() {
            let skills = self.current_skills_snapshot();
            push_skill_commands(&mut commands, &mut seen, &skills);
            let working_dir = self
                .session
                .working_dir
                .as_deref()
                .map(std::path::Path::new);
            if let Ok(reloaded) = crate::skill::SkillRegistry::load_for_working_dir(working_dir) {
                push_skill_commands(&mut commands, &mut seen, &reloaded);
            }
        }

        commands
    }
}
```

- [ ] **Step 4: Preserve manual compatibility for hidden commands**

```rust
pub(super) fn command_accepts_args(cmd: &str) -> bool {
    matches!(
        cmd.trim(),
        "/help"
            | "/?"
            | "/model"
            | "/login"
            | "/auth"
            | "/usage"
            | "/clear"
            | "/resume"
            | "/quit"
            | "/git"
            | "/selfdev"
            | "/subscription"
            | "/memory"
            | "/swarm"
            | "/improve"
            | "/refactor"
    )
}
```

This step keeps parsing compatibility while reducing public discoverability.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test slash_suggestions_hide_git_and_selfdev_in_saitec_product_mode slash_suggestions_keep_public_saitec_commands -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/app/state_ui_input_helpers.rs src/tui/app/tests/state_model_poke_02/part_01.rs
git commit -m "feat: filter slash suggestions for saitec product mode"
```

### Task 4: Replace The Help Overlay With A SAITEC Public Surface

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui_overlays.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\input_help.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_01\part_01.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\tests\commands_accounts_01\part_01.rs`

- [ ] **Step 1: Write the failing help-overlay tests**

```rust
#[test]
fn help_overlay_hides_git_and_skills_in_saitec_product_mode() {
    let mut app = create_test_app();
    app.input = "/help".to_string();
    app.submit_input();

    let msg = app.display_messages().last().expect("help message");
    assert!(!msg.content.contains("/git"), "content: {}", msg.content);
    assert!(!msg.content.contains("Skills"), "content: {}", msg.content);
}

#[test]
fn help_overlay_keeps_login_logout_and_model_commands() {
    let mut app = create_test_app();
    app.input = "/help".to_string();
    app.submit_input();

    let msg = app.display_messages().last().expect("help message");
    assert!(msg.content.contains("/login"), "content: {}", msg.content);
    assert!(msg.content.contains("/logout"), "content: {}", msg.content);
    assert!(msg.content.contains("/model"), "content: {}", msg.content);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test help_overlay_hides_git_and_skills_in_saitec_product_mode -- --exact`
Expected: FAIL because `/help` still shows `/git` and skills.

- [ ] **Step 3: Reduce the main help overlay to SAITEC public commands**

```rust
// ui_overlays.rs
lines.push(Line::from(Span::styled("  Commands", section_style)));
lines.push(Line::from(""));
lines.push(help_entry("/help", "Show this help overlay"));
lines.push(help_entry("/login", "Start the Saitec login flow"));
lines.push(help_entry("/logout", "Logout from Saitec and clear local auth"));
lines.push(help_entry("/auth", "Show current authentication status"));
lines.push(help_entry("/model", "List or switch models"));
lines.push(help_entry("/clear", "Clear conversation and start fresh"));
lines.push(help_entry("/resume", "Browse and resume previous sessions"));
lines.push(help_entry("/usage", "Show connected provider usage limits"));
lines.push(help_entry("/version", "Show version and build details"));
lines.push(help_entry("/quit", "Exit SAITEC-TUI"));
```

```rust
// do not render the Skills section in product mode
if crate::saitec::product_profile::show_skills_in_ui() && !skills.is_empty() {
    // existing skills rendering block
}
```

- [ ] **Step 4: Update detailed command help to product-facing wording**

```rust
// input_help.rs
"auth" | "login" => {
    "`/auth`\nShow authentication status.\n\n`/login`\nStart the Saitec login flow.\n\n`/login jcode`\nAlias for the same Saitec login flow."
}
"quit" => "`/quit`\nExit SAITEC-TUI.",
```

Keep `/help git` as a compatibility-only detailed help entry for this phase if existing tests rely on it.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test help_overlay_hides_git_and_skills_in_saitec_product_mode help_overlay_keeps_login_logout_and_model_commands -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/tui/ui_overlays.rs src/tui/app/input_help.rs src/tui/app/tests/commands_accounts_01/part_01.rs
git commit -m "feat: reduce help overlay to saitec public command surface"
```

### Task 5: Disable Decorative Animations By Default In Product Mode

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\config\default_file.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\config\display_summary.rs`
- Test: `G:\Workspace\Project2026\JCode\jcode\src\config_tests.rs`

- [ ] **Step 1: Write the failing default-config test**

```rust
#[test]
fn default_config_disables_decorative_animations_for_saitec_product_mode() {
    let _guard = crate::storage::lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let dir = tempfile::TempDir::new().expect("tempdir");
    crate::env::set_var("JCODE_HOME", dir.path());

    let path = crate::config::Config::create_default_config_file()
        .expect("create default config file");
    let default_file = std::fs::read_to_string(path).expect("read config file");

    assert!(default_file.contains("idle_animation = false"));
    assert!(default_file.contains("prompt_entry_animation = false"));

    if let Some(prev) = prev_home {
        crate::env::set_var("JCODE_HOME", prev);
    } else {
        crate::env::remove_var("JCODE_HOME");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test default_config_disables_decorative_animations_for_saitec_product_mode -- --exact`
Expected: FAIL because the defaults still say `true`.

- [ ] **Step 3: Flip the default animation values**

```toml
# src/config/default_file.rs generated content
idle_animation = false
prompt_entry_animation = false
```

If the summary output mentions these values, keep it accurate and product-facing.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test default_config_disables_decorative_animations_for_saitec_product_mode -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/config/default_file.rs src/config/display_summary.rs src/config_tests.rs
git commit -m "feat: disable decorative animation defaults for saitec tui"
```

### Task 6: Stabilize Terminal Table Alignment

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\crates\jcode-tui-markdown\src\markdown_render_support.rs`
- Modify: `G:\Workspace\Project2026\JCode\jcode\src\tui\markdown.rs` if wrapper changes are needed
- Test: `G:\Workspace\Project2026\JCode\jcode\crates\jcode-tui-markdown\src\markdown_tests\cases\rendering.rs`

- [ ] **Step 1: Write the failing table-alignment tests**

```rust
#[test]
fn render_table_with_width_keeps_columns_stable_for_mixed_rows() {
    let rows = vec![
        vec!["NAME".to_string(), "STATUS".to_string(), "COUNT".to_string()],
        vec!["alpha".to_string(), "running".to_string(), "12".to_string()],
        vec!["beta-longer".to_string(), "idle".to_string(), "3".to_string()],
    ];

    let lines = render_table_with_width(&rows, 40);
    let rendered: Vec<String> = lines.iter().map(line_plain_text).collect();

    assert!(rendered[0].contains("NAME"));
    assert!(rendered[2].contains("running"));
    assert!(rendered[3].contains("idle"));
}

#[test]
fn render_table_with_width_does_not_drop_separator_row() {
    let rows = vec![
        vec!["A".to_string(), "B".to_string()],
        vec!["1".to_string(), "2".to_string()],
    ];

    let lines = render_table_with_width(&rows, 20);

    assert!(lines.len() >= 3, "expected header, separator, and data row");
}
```

- [ ] **Step 2: Run test to verify it fails or is insufficient**

Run: `cargo test render_table_with_width_keeps_columns_stable_for_mixed_rows render_table_with_width_does_not_drop_separator_row -- --nocapture`
Expected: FAIL or expose unstable behavior that the implementation must tighten.

- [ ] **Step 3: Implement stable per-column table alignment**

```rust
fn render_table(rows: &[Vec<String>], max_width: Option<usize>) -> Vec<Line<'static>> {
    if rows.is_empty() {
        return vec![];
    }

    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut col_widths = vec![0usize; num_cols];

    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            col_widths[i] = col_widths[i].max(UnicodeWidthStr::width(cell.as_str()));
        }
    }

    // preserve stable widths under max width constraints
    // text remains left aligned in phase one
    // numeric right alignment can be added only if it does not destabilize width math

    // existing rendering logic with deterministic width truncation and padding
}
```

Keep phase-one alignment simple and stable rather than attempting risky multi-mode formatting.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test render_table_with_width_keeps_columns_stable_for_mixed_rows render_table_with_width_does_not_drop_separator_row -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jcode-tui-markdown/src/markdown_render_support.rs src/tui/markdown.rs
git commit -m "feat: stabilize table alignment for saitec tui"
```

### Task 7: Produce A SAITEC-Branded Windows Package

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\scripts\install.ps1`
- Create: `G:\Workspace\Project2026\JCode\jcode\scripts\package_saitec.ps1`
- Verify: `G:\Workspace\Project2026\JCode\jcode\target\release\jcode.exe`

- [ ] **Step 1: Write the failing packaging smoke test script expectations**

```powershell
$PackageDir = Join-Path $PWD "dist\saitec-tui"
if (-not (Test-Path $PackageDir)) {
    throw "Expected SAITEC package directory to be created"
}

$Exe = Join-Path $PackageDir "saitec-tui.exe"
if (-not (Test-Path $Exe)) {
    throw "Expected branded executable copy at $Exe"
}
```

- [ ] **Step 2: Run the package script expectation to verify it fails**

Run: `powershell -ExecutionPolicy Bypass -File .\scripts\package_saitec.ps1`
Expected: FAIL because the packaging script does not exist yet.

- [ ] **Step 3: Add a thin SAITEC packaging script**

```powershell
param(
    [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$TargetExe = Join-Path $RepoRoot "target\$Profile\jcode.exe"
$DistDir = Join-Path $RepoRoot "dist\saitec-tui"
$BrandedExe = Join-Path $DistDir "saitec-tui.exe"

if (-not (Test-Path $TargetExe)) {
    throw "Missing build artifact: $TargetExe"
}

New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
Copy-Item -Path $TargetExe -Destination $BrandedExe -Force
Copy-Item -Path (Join-Path $RepoRoot "scripts\install.ps1") -Destination (Join-Path $DistDir "install.ps1") -Force

Write-Host "SAITEC package ready at $DistDir"
```

- [ ] **Step 4: Update installer branding where low-risk**

```powershell
# scripts/install.ps1
if (-not $InstallDir) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "saitec-tui\bin"
}
```

Keep deeper installer wording changes minimal in this round unless they are required for consistency.

- [ ] **Step 5: Run packaging verification**

Run: `cargo build --release`
Expected: exit code 0 and `target\release\jcode.exe` exists.

Run: `powershell -ExecutionPolicy Bypass -File .\scripts\package_saitec.ps1`
Expected: exit code 0 and `dist\saitec-tui\saitec-tui.exe` exists.

Run: `.\dist\saitec-tui\saitec-tui.exe --help`
Expected: exit code 0 and startup help text renders without a crash.

- [ ] **Step 6: Commit**

```bash
git add scripts/install.ps1 scripts/package_saitec.ps1
git commit -m "build: add saitec branded windows package output"
```

### Task 8: Run End-To-End Verification For The Frontend Remodel

**Files:**
- Verify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui_header.rs`
- Verify: `G:\Workspace\Project2026\JCode\jcode\src\tui\ui_overlays.rs`
- Verify: `G:\Workspace\Project2026\JCode\jcode\src\tui\app\state_ui_input_helpers.rs`
- Verify: `G:\Workspace\Project2026\JCode\jcode\crates\jcode-tui-markdown\src\markdown_render_support.rs`
- Verify: `G:\Workspace\Project2026\JCode\jcode\dist\saitec-tui\saitec-tui.exe`

- [ ] **Step 1: Run the targeted UI test slice**

Run:
```bash
cargo test build_persistent_header_shows_saitec_brand_when_session_name_missing -- --nocapture
cargo test build_header_lines_hides_skills_line_in_saitec_product_mode -- --nocapture
cargo test slash_suggestions_hide_git_and_selfdev_in_saitec_product_mode -- --nocapture
cargo test slash_suggestions_keep_public_saitec_commands -- --nocapture
cargo test help_overlay_hides_git_and_skills_in_saitec_product_mode -- --nocapture
cargo test help_overlay_keeps_login_logout_and_model_commands -- --nocapture
cargo test render_table_with_width_keeps_columns_stable_for_mixed_rows -- --nocapture
cargo test render_table_with_width_does_not_drop_separator_row -- --nocapture
```
Expected: PASS.

- [ ] **Step 2: Run a broader touched-area regression slice**

Run:
```bash
cargo test /login -- --nocapture
cargo test /logout -- --nocapture
cargo test /help -- --nocapture
cargo test /model -- --nocapture
```
Expected: PASS for relevant touched-area tests with no newly introduced regressions.

- [ ] **Step 3: Run the release build**

Run: `cargo build --release`
Expected: exit code 0.

- [ ] **Step 4: Produce the branded package**

Run: `powershell -ExecutionPolicy Bypass -File .\scripts\package_saitec.ps1`
Expected: exit code 0 and branded output under `dist\saitec-tui`.

- [ ] **Step 5: Smoke-check the packaged binary**

Run: `.\dist\saitec-tui\saitec-tui.exe --help`
Expected: exit code 0 and no startup crash.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "build: verify saitec tui remodel and package artifacts"
```
