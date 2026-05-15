# SAITEC Provider Allowlist Design

## Goal

Restrict the original base-model login and account-management surfaces in SAITEC-TUI so they only show and support these five providers:

- `OpenAI`
- `Anthropic`
- `Z.AI`
- `Kimi`
- `Alibaba Cloud Coding`

All other provider login/configuration paths should stop appearing in the UI and should be rejected consistently if invoked through commands.

## Scope

This design covers:

- the original login provider list used by the base-model login/configuration flow
- `/account` provider lists and provider-specific account/configuration entry points
- `/auth` provider status output
- help text and command autocomplete that currently expose provider-specific login/configuration routes
- command-path rejection for providers outside the allowlist

This design does not cover:

- the SAITEC business login form itself
- the top-level `/login` two-step selector that was just added
- removing provider implementations from the shared codebase
- provider runtime dispatch for sessions that already use a non-allowlisted provider outside SAITEC-TUI product mode

## Recommendation

Use a shared SAITEC-TUI product-level provider allowlist and apply it consistently across login surfaces, account surfaces, auth-status surfaces, help, and command validation.

This is the best fit because:

- it keeps the product experience internally consistent instead of hiding providers in one screen while leaving them visible elsewhere
- it minimizes risk by leaving shared provider definitions intact and only filtering them at the SAITEC product layer
- it creates a single future maintenance point when the allowed provider set changes again

## Alternatives Considered

### Option 1: Hide providers only in the original login screen

Pros:

- smallest immediate diff

Cons:

- `/account`, `/auth`, help, and autocomplete still expose unsupported providers
- users can still reach hidden providers through commands
- product behavior becomes confusing and inconsistent

### Option 2: Shared product-level allowlist across all visible auth/account surfaces

Pros:

- one source of truth
- consistent UI and command behavior
- lower long-term maintenance cost

Cons:

- touches more than one surface
- requires a few targeted regression tests

### Option 3: Delete unsupported providers from shared provider metadata

Pros:

- the cleanest visible end state

Cons:

- too invasive for this branch
- risks breaking non-SAITEC flows and broad test coverage
- makes future re-enablement expensive

Chosen option: Option 2.

## User Experience

### Visible provider set

In SAITEC-TUI product mode, the original provider-based login/configuration surfaces only expose:

- `OpenAI`
- `Anthropic`
- `Z.AI`
- `Kimi`
- `Alibaba Cloud Coding`

No other provider names should appear in:

- the original login UI
- `/account`
- `/auth`
- login/account help text
- login/account autocomplete suggestions

### Unsupported-provider behavior

If the user explicitly enters a provider command outside the allowlist, SAITEC-TUI should reject it with a clear product-level error.

Examples:

- `/login gemini`
- `/account openrouter settings`
- `/account copilot login`

Recommended error shape:

`SAITEC-TUI only supports these base-model providers: openai, claude, zai, kimi, alibaba-cloud-coding.`

The message should be product-facing and should not imply that the provider is broken; it is intentionally unsupported in this product mode.

### Top-level `/login` behavior

The new top-level `/login` selector remains unchanged:

- `SAITEC login` still opens the SAITEC business login form
- `Base-model login or configuration` still opens the original provider/account flow

The only difference is that the original provider/account flow is now filtered to the five-provider allowlist above.

## Architecture

## Shared allowlist

Introduce one product-level source of truth for the allowed provider ids in SAITEC-TUI.

Recommended ids:

- `openai`
- `claude`
- `zai`
- `kimi`
- `alibaba-coding-plan`

This allowlist should live in SAITEC product code, not inside generic provider metadata, so the shared provider catalog remains intact for other product modes.

## Filtering strategy

### Login-provider list

Wherever the original login UI builds its provider entries, filter the source provider list through the SAITEC allowlist before rendering.

### Account center

Wherever `/account` or the account picker builds provider entries, only include providers in the allowlist.

If the account center currently injects generic global actions before provider rows, those global actions may remain, but all provider-specific actions must be limited to the five allowed providers.

### Auth status

Wherever `/auth` builds the provider status table, only include the allowlisted providers plus any SAITEC-specific row that is already part of the product surface.

### Command validation

Parsing can stay broad if needed, but execution must reject non-allowlisted providers in SAITEC-TUI mode.

That keeps shared parser logic reusable while enforcing the product boundary at execution time.

### Help and autocomplete

Provider-specific examples and suggestions should be regenerated from the filtered allowlist, not from the full provider catalog.

That prevents drift between command help and actual supported behavior.

## Files Likely to Change

- `src/saitec/product_profile.rs`
- `crates/jcode-provider-metadata/src/lib.rs`
- `src/tui/app/auth_account_commands.rs`
- `src/tui/app/auth_account_picker.rs`
- `src/tui/app/input_help.rs`
- `src/tui/app/state_ui_input_helpers.rs`
- `src/provider_catalog_tests.rs`
- targeted TUI auth/account tests under `src/tui/app/tests/`

## Testing

Add or update regression coverage for:

- the filtered original login provider list contains only the five allowlisted providers
- `/account` surfaces only the five allowlisted providers
- `/auth` output does not list unsupported providers
- `/login <unsupported>` returns the new product-level unsupported message
- `/account <unsupported> ...` returns the new product-level unsupported message
- autocomplete/help no longer suggest unsupported providers

## Risks

### Risk 1: Partial filtering creates inconsistent product behavior

If only one surface is filtered, unsupported providers can still leak through help, auth status, or command paths.

Mitigation:

- use one shared allowlist
- add coverage for visible lists and explicit command rejection

### Risk 2: Filtering too deep breaks non-SAITEC flows

If the shared provider catalog is globally narrowed instead of product-filtered, unrelated workflows may regress.

Mitigation:

- keep the full provider metadata intact
- apply allowlist logic only when SAITEC-TUI product mode is active

### Risk 3: Provider ids and display names drift

If the allowlist uses unstable display strings instead of canonical ids, future metadata edits can silently break filtering.

Mitigation:

- filter by canonical provider ids
- keep display names derived from provider metadata

## Success Criteria

This work is successful when:

1. The original provider login/configuration surfaces in SAITEC-TUI only show `OpenAI`, `Anthropic`, `Z.AI`, `Kimi`, and `Alibaba Cloud Coding`.
2. Unsupported providers are not suggested in help or autocomplete.
3. Unsupported provider commands fail with a clear SAITEC-TUI product-level message.
4. The existing SAITEC business login flow and the top-level `/login` selector continue to work.
