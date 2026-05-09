# SAITEC-TUI Frontend Remodel Design

## Goal

Turn the current JCode terminal UI into a SAITEC-branded product surface for the "TUI front-end remodel" scope only, while preserving underlying compatibility so later login and MCP work can build on top of the same runtime instead of forking the whole interface stack.

## Scope

This design covers:

- SAITEC product branding in the TUI
- Removal of decorative dynamic effects from the visible product mode
- Hiding existing skills from the visible interface
- Hiding git-related and self-dev-oriented commands from the visible interface
- Narrowing the visible slash-command surface to a product-safe set
- Adding stable table-aligned rendering behavior for terminal display
- Windows packaging for a SAITEC-branded release directory or executable wrapper

This design does not cover:

- Mandatory login gating implementation details
- Real MCP protocol integration details
- Real auth callback handling
- Full internal crate or binary renaming from `jcode` to `saitec-tui`
- Deleting old commands or runtime capabilities from the codebase

## Product Direction

The recommended direction is **Product Mode**, not a thin cosmetic reskin.

Product Mode means the existing TUI keeps its proven runtime, rendering, and command plumbing, but a new SAITEC-specific profile decides what the user can see by default. This allows the product to feel clean and intentionally constrained without forcing a risky rewrite of core behavior.

The key idea is:

- keep runtime compatibility
- reduce visible surface area
- centralize product branding and visibility rules
- make MCP visibility a first-class part of the header and help system
- leave room for later login and MCP changes without reworking the TUI twice

## Recommendation

Use a dedicated **SAITEC product profile** that drives:

- brand name and logo
- visible versus hidden commands
- whether skills are shown
- whether decorative animations are enabled
- help overlay content
- header copy and status emphasis
- MCP presentation behavior

This is preferable to scattering `if saitec` conditions across unrelated TUI files because:

- the behavior becomes easier to reason about
- future login and MCP work can reuse the same profile
- the visible product surface stays consistent
- compatibility commands can remain implemented without remaining publicly advertised

## User Experience

### Header behavior

The persistent header should present SAITEC as the product identity instead of JCode.

Visible behavior:

- primary brand line becomes `🍇 SAITEC-TUI`
- current model and provider status remain visible
- authentication status remains visible
- MCP server status remains visible and should be treated as more important than skills
- the current `skills: /...` line is removed from the visible header
- version/build information remains available, but the wording becomes product-facing rather than self-dev facing

The header should still communicate system state, but it should stop advertising internal framework concepts such as loaded skills or developer-focused release phrasing.

### Command visibility behavior

Product Mode should distinguish between:

- commands that remain visible and recommended
- commands that remain implemented for compatibility but are hidden from ordinary UI

Visible commands:

- `/help`
- `/login`
- `/logout`
- `/auth`
- `/model`
- `/clear`
- `/resume`
- `/usage`
- `/version`
- `/quit`

Commands hidden from public UI, but still available for compatibility in this phase:

- `/git`
- `/selfdev`
- `/feedback`
- `/subscription`
- `/review`
- `/judge`
- `/swarm`
- `/memory`
- `/refactor`
- `/improve`
- `/autoreview`
- `/autojudge`
- `/observe`
- `/subagent`
- `/workspace`
- `/catchup`
- `/back`
- `/splitview`
- `/split`
- `/transfer`
- `/rebuild`
- `/restart`
- `/reload`
- related aliases and older operator-facing commands

This keeps the product surface intentionally narrow while avoiding breakage for hidden workflows during the transition.

### Help overlay behavior

The help overlay should become a SAITEC product help panel rather than a runtime inventory dump.

Visible help sections should be limited to:

- core commands
- login and auth
- model selection
- session basics
- quit/exit
- essential keyboard navigation
- MCP status interpretation if already available in the running session

The help overlay should not list:

- skills
- git commands
- self-dev flows
- advanced review or swarm tooling
- internal system management commands

If a user manually requests help for a hidden command such as `/help git`, Product Mode should prefer one of the following behaviors consistently:

- either treat it as unknown in the public product surface
- or show a short compatibility-only note without re-advertising the command set broadly

The recommended behavior for this phase is the second one: a short compatibility note is lower risk for existing internals.

### Slash-command suggestion behavior

Input suggestions should follow the same Product Mode policy as help.

When the user types `/`, suggestions should only include public SAITEC commands.

When the user is not logged in, suggestions should be narrowed further to:

- `/login`
- `/logout`
- `/help`
- `/quit`

This keeps the input surface aligned with the intended product workflow and avoids leaking advanced system capabilities before the user is even authenticated.

### Animation behavior

SAITEC-TUI should remove decorative motion, not operational state feedback.

Disabled in Product Mode:

- idle animation
- prompt entry animation
- decorative pulsing or animated theme effects

Still preserved:

- connection progress visibility
- thinking or running-tool status updates
- error and recovery messages
- MCP status refresh
- ordinary redraws needed for streaming output and interaction correctness

This keeps the product visually calmer without sacrificing observability.

### Table display behavior

The TUI should render tables with stable alignment rather than best-effort wrapping that causes columns to drift.

Phase-one expected behavior:

- stable column widths per rendered table
- left alignment for text columns
- right alignment for numeric columns when the current table renderer can support it without large churn
- fallback to stable left alignment for all columns if mixed alignment adds too much risk in this round
- markdown-generated tables and status tables should use the same alignment strategy where practical

The most important acceptance criterion is that tables remain readable in common terminal widths and do not visually collapse into uneven fragments.

## Architecture

### New product-profile layer

Add a new SAITEC-focused product-profile module, for example:

- `src/saitec/product_profile.rs`

This module should centralize:

- brand strings
- logo/icon text
- command visibility classification
- header display policy
- help overlay filtering policy
- animation defaults for product mode
- whether skills are visible
- whether MCP status is emphasized

This file should be declarative where possible so product behavior can be adjusted without revisiting many unrelated rendering functions.

### Existing files likely to change

- `src/tui/ui_header.rs`
- `src/tui/ui_overlays.rs`
- `src/tui/app/state_ui_input_helpers.rs`
- `src/tui/app/input_help.rs`
- `src/tui/ui_input.rs`
- `src/config/default_file.rs`
- `src/config/display_summary.rs` if product-facing summaries need to reflect the new defaults
- `crates/jcode-tui-markdown/src/markdown_render_support.rs`
- `src/tui/markdown.rs` if the table path needs a narrow wrapper update
- `scripts/install.ps1`

Optional packaging helper if needed:

- `scripts/package_saitec.ps1`

### Responsibility split

Recommended file responsibilities:

- `product_profile.rs`
  Central source of truth for SAITEC product-mode visibility and branding rules.
- `ui_header.rs`
  Applies brand and header-line filtering.
- `ui_overlays.rs`
  Builds the public help overlay from product-approved commands only.
- `state_ui_input_helpers.rs`
  Filters command registration visibility and slash suggestions.
- `input_help.rs`
  Keeps detailed help text aligned with the visible public surface.
- `default_file.rs`
  Changes default animation behavior for the SAITEC product experience.
- markdown table renderer files
  Own stable table width and alignment behavior instead of scattering layout hacks into business UI code.
- install/package scripts
  Produce a SAITEC-branded deliverable without forcing a full internal rename.

## Command Model

Introduce an explicit classification model for commands in Product Mode:

- `Public`
  Shown in help, slash suggestions, and other product-facing UI.
- `HiddenCompatible`
  Not shown in public UI, but still executable if entered manually.
- `InternalOnly`
  Hidden and not part of the public product contract.

The current command registration code already distinguishes visible and hidden behavior in parts of the stack. Product Mode should extend that idea into a single policy rather than ad hoc per-command filtering.

## MCP Presentation

Even though deep MCP integration belongs to another workstream, Product Mode should prepare the UI now.

Required visible behavior:

- MCP status remains in the header
- MCP information is not visually buried under skills or internal framework details
- the wording should sound product-facing, for example server count, readiness, or tool count

Not in scope for this round:

- live MCP control panels
- per-tool browsing
- MCP execution tracing redesign

This round only needs the visible status layer to feel intentional and productized.

## Error Handling

The UI-remodel work should preserve graceful behavior if:

- hidden commands are typed manually
- no MCP servers are connected
- terminal width is too narrow for the full branded header
- table content is wider than the viewport

Expected behavior:

- narrow terminals should fall back to shorter header variants
- hidden commands should not panic or corrupt the UI
- table rendering should truncate or wrap predictably rather than misalign columns
- removing decorative animations must not interfere with streaming or redraw timing

## Testing Strategy

### Header tests

Add or update tests that verify:

- the header shows `🍇 SAITEC-TUI`
- the header no longer shows visible skills lines
- MCP status remains present
- shorter header fallbacks remain readable at narrow widths

### Help overlay tests

Add or update tests that verify:

- `/help` shows only the approved public SAITEC command set
- `/git` and `/selfdev` are absent from the main help overlay
- skills are absent from the help overlay

### Command suggestion tests

Add or update tests that verify:

- `/` suggestions only include Product Mode public commands
- `/git` is no longer suggested publicly
- unauthenticated suggestion mode only exposes login-safe commands

### Table rendering tests

Add or update tests that verify:

- rendered tables keep stable column widths
- text cells remain readable under common terminal widths
- numeric or status columns do not cause row collapse

### Compatibility tests

Add or update tests that verify:

- hidden compatibility commands can still execute when typed directly
- Product Mode changes do not break login, logout, or model-selection flows

### Packaging verification

Manual or scripted verification should confirm:

1. release build succeeds
2. packaged SAITEC-branded output directory is produced
3. executable starts and shows the SAITEC-branded header/help behavior
4. `--help` or equivalent smoke path does not fail

## Packaging Plan

This round should reuse the existing build pipeline where possible.

Recommended packaging approach:

- continue building the existing Rust binary through the normal release path
- produce a SAITEC-branded packaged output directory
- if low-risk, also produce a branded executable alias such as `saitec-tui.exe`
- keep the underlying `jcode.exe` build artifact available for compatibility and internal tooling

This avoids turning a UI remodel into a full repository renaming effort.

Expected deliverable shape:

- release binary built successfully
- SAITEC-branded distribution directory or wrapper output on Windows
- installer script updated to use SAITEC-facing naming and install paths where appropriate

## Implementation Order

Recommended implementation order:

1. add the SAITEC product-profile module
2. update header branding and visible status lines
3. filter help overlay and slash suggestions through the profile
4. switch decorative animation defaults off in product mode
5. improve table alignment behavior
6. update Windows packaging and install branding
7. run targeted UI tests and release build verification

This order reduces the chance of mixing product-surface changes with lower-level renderer work too early.

## Acceptance Criteria

The TUI front-end remodel is complete for this phase when all of the following are true:

- the visible product brand is `🍇 SAITEC-TUI`
- decorative dynamic effects are no longer shown by default in product mode
- visible skills listings are removed from the UI
- git-related and self-dev-oriented commands are not shown in public help or slash suggestions
- public slash suggestions are reduced to the SAITEC product-safe command set
- MCP status remains visible and is treated as a first-class product signal
- tables render with stable alignment suitable for terminal use
- a Windows release build succeeds
- a SAITEC-branded packaged output is produced without breaking the underlying runtime compatibility layer
