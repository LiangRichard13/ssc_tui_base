# SAITEC Startup Login Gate And Model Summary Design

## Goal

When SAITEC-TUI starts, users who are not fully ready to use the product should land on the existing first-level login selector instead of the normal chat surface. The first-level selector is the current `Login` picker with two choices:

- `SAITEC` business account login
- `Base models` login/configuration

The gate should open this selector when either SAITEC business authentication is unavailable or no allowed base-model provider is configured. The lower-left model widget should also show which base-model providers are configured.

## Current Context

The codebase already has:

- SAITEC auth persisted through `src/saitec/auth.rs` and `~/.saitec_tui/auth.json`
- first-level login selector in `App::open_login_mode_selector`
- base-model provider filtering through `saitec_visible_base_model_providers`
- status probing through `AuthStatus::check_fast`
- lower-left model widget data built in `App::info_widget_data`

The implementation should reuse these paths rather than adding a separate login surface.

## Startup Gate

Add a focused startup helper on `App`, called during local app construction after the struct is initialized and provider startup notices are drained.

The helper should open the first-level login selector when:

- SAITEC business auth is not available, or
- none of the allowed base-model providers is available/configured.

Allowed base-model providers are the existing SAITEC allowlist:

- OpenAI
- Claude
- Z.AI
- Kimi Code
- Alibaba Cloud Coding Plan

The helper must not override an already active interaction surface. It should return without changing state if any of these are already active:

- `pending_login`
- `login_picker_overlay`
- `account_picker_overlay`
- `inline_interactive_state`
- `session_picker_overlay`
- queued startup input or pending images

This keeps reload, restore, and explicit startup input flows from being hijacked.

## Base-Model Configuration Summary

Extend `InfoWidgetData` with a compact configured base-model summary, then render it in `render_model_widget`.

The summary should be derived from `saitec_visible_base_model_providers()` and `AuthStatus::check_fast()`, not from all JCode providers. Providers count as configured when `status.state_for_provider(provider) == AuthState::Available`.

Rendering should be compact and width-aware. Suggested text:

- `Base models: none` when no allowed base model is configured
- `Base models: OpenAI`
- `Base models: OpenAI, Kimi +2`

The widget should avoid secrets, API keys, endpoint credentials, or raw config values. It should show provider display names only.

## Error Handling

Startup gating is a UI decision and should not perform network validation. It should use local cached/fast probes only.

If the auth status probe cannot classify a provider as available, it should be treated as missing and the selector should open. Existing explicit login and validation flows continue to surface detailed errors.

## Tests

Use test-first changes around the existing TUI unit tests:

- no SAITEC auth and no base-model auth opens the first-level login selector
- SAITEC auth available but no base-model auth still opens the selector
- base-model auth available but no SAITEC auth still opens the selector
- both SAITEC auth and one allowed base model available do not open the selector
- active login/inline/session/startup input surfaces are not overridden
- model widget data lists configured allowed base-model providers and omits unconfigured ones
- rendered model widget includes the compact base-model summary

## Non-Goals

- Do not replace the first-level selector with the direct SAITEC credential form.
- Do not add network validation during startup.
- Do not expose API keys, auth tokens, env var values, or provider endpoint secrets in the widget.
- Do not change the underlying SAITEC login submission flow.
