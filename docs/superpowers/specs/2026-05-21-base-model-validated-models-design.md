# Base-Model Validated Models Design

**Problem**

After a user configures and validates SAITEC base-model providers, `/model` should surface the corresponding validated models, not just whatever static fallback or live catalog happens to be available at that moment.

**Decision**

Persist validated model ids in auth-validation state, then merge those model ids into the provider route aggregation layer used by `/model`.

**Why this shape**

- It keeps one source of truth for "this provider was validated against these models".
- It avoids teaching the TUI model picker a second custom data source.
- It lets `/model`, model suggestions, and other model-list consumers benefit from the same route enrichment.

**Scope**

- Extend `ProviderValidationRecord` with optional validated model ids.
- Record the selected/discovered validation model during auth-test persistence.
- Merge validated OpenAI-compatible model ids into `MultiProvider::model_routes()` for configured providers that do not already expose the same route.
- Preserve backward compatibility for old validation files.

**Non-goals**

- No UI-only badges in this pass.
- No redesign of the login picker or auth status views.
