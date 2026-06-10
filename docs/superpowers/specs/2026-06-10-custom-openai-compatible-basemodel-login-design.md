# Custom OpenAI-Compatible BaseModel Login Design

## Goal

SAITEC-TUI should let users configure an additional BaseModel provider by entering their own OpenAI-compatible endpoint and API key from the TUI login flow.

This is a BaseModel login feature, not a SAITEC platform-login feature. SAITEC account credentials continue to represent platform and MCP access only. The custom model endpoint and key are stored and used through the existing OpenAI-compatible provider path.

## Current Context

The repo already has most of the custom-provider machinery:

- `OPENAI_COMPAT_PROFILE` defines the generic `openai-compatible` profile.
- `/login openai-compatible` already starts a two-step TUI flow: API base first, then API key.
- The CLI login flow can save `JCODE_OPENAI_COMPAT_API_BASE`, API key env name, and default model hints.
- OpenAI-compatible credentials are stored in `openai-compatible.env`.
- OpenAI-compatible login triggers auth cache invalidation, model catalog refresh, and post-login activation.

The missing product behavior is that SAITEC's Base models surface does not expose `openai-compatible`, and the SAITEC route allowlist rejects generic custom routes.

## Product Behavior

Add `openai-compatible` to SAITEC Base models as a visible provider option. The first-level login selector remains unchanged:

- `SAITEC`
- `Base models`

Choosing `Base models` opens the filtered provider picker. That picker should include:

- OpenAI
- Anthropic/Claude
- Z.AI
- Kimi Code
- Alibaba Cloud Coding Plan
- OpenAI-compatible

Selecting `OpenAI-compatible` starts the existing custom endpoint flow:

1. Ask for API base.
2. Normalize and save the API base.
3. Ask for API key.
4. Save the API key when required, or save optional/no-key local endpoint setup when the endpoint is local.
5. Refresh model discovery and activate a usable model when possible.

## Endpoint And Credential Rules

Endpoint validation should reuse `normalize_api_base`:

- Accept `https://...`.
- Accept local/private `http://...` endpoints supported by existing normalization.
- Reject unsafe public `http://...` endpoints.

Credential handling should stay separate from SAITEC auth:

- Custom API key goes to the resolved OpenAI-compatible env file and key name.
- SAITEC `auth.json` and `SAITEC_API_KEY` are never treated as custom BaseModel credentials.
- Clearing or changing the custom provider must not clear SAITEC platform credentials.

For public hosted endpoints, API key remains required. For local endpoints where the resolved profile does not require a key, an empty key is valid and marks the local endpoint configured.

## Runtime Model Routing

Custom OpenAI-compatible routes should be allowed by SAITEC route filtering when they come from the generic `openai-compatible` profile and match models that the custom provider exposes or validates.

The preferred route identity is existing OpenAI-compatible metadata:

- provider id: `openai-compatible`
- API method: `openai-compatible` or `openai-compatible:openai-compatible`
- display name: `OpenAI-compatible`

The implementation should avoid opening unrelated third-party OpenAI-compatible profiles in SAITEC unless they are already explicitly allowed.

## Error Handling

Invalid API base input should keep the endpoint prompt open and show the existing clear error message.

Failure to save the endpoint or key should keep the relevant prompt open and show a save failure message.

Post-save model refresh failures should not roll back saved credentials. They should show the existing guidance to rerun `/refresh-model-list`, `auth status`, or `auth doctor`.

## Tests

Add focused tests before implementation:

- `/login base-models` includes `openai-compatible`.
- Selecting `openai-compatible` from the Base models picker opens the endpoint prompt.
- Entering a valid custom endpoint advances to the API key prompt and persists `JCODE_OPENAI_COMPAT_API_BASE`.
- Entering a key persists it to the resolved OpenAI-compatible env file/key.
- The SAITEC BaseModel provider allowlist accepts `openai-compatible` as a provider.
- The SAITEC model route allowlist accepts validated generic OpenAI-compatible routes.
- SAITEC credentials remain separate from custom BaseModel credentials.

## Non-Goals

- Do not create a new provider implementation.
- Do not store custom model credentials in SAITEC auth/session files.
- Do not expose raw API keys in the UI.
- Do not broaden SAITEC to every third-party OpenAI-compatible profile by default.
- Do not redesign the account picker or model picker.
