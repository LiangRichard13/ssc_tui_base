# SAITEC Business APIKey Login Design

## Goal

Replace the current SAITEC browser-callback business login path with a TUI-native credential form that authenticates against the SAITEC Core API, exchanges the returned JWT for a business API key, persists only the business API key plus metadata in `~/.saitec_tui/auth.json`, and blocks normal TUI use until a valid business API key is available.

## Scope

This design covers:

- startup validation of the SAITEC business API key
- an auto-opened blocking login form in the TUI
- the request chain `POST /api/v1/auth/login -> POST /api/v1/api-keys`
- persistence of the business API key and metadata in `~/.saitec_tui/auth.json`
- refresh of user profile fields from `GET /api/v1/users/me`
- `/login` and `/logout` behavior for the business login layer
- local gating that blocks normal use until the business login succeeds

This design does not cover:

- base model provider login or base model API key management
- server-side API key revocation on logout
- MCP integration changes
- broad account-center redesign across all other providers

## Recommendation

Use a dedicated SAITEC form-based login state inside the existing TUI auth framework, while keeping the SAITEC storage and HTTP logic centralized in `src/saitec/auth.rs`.

This is the best fit for the current product direction because:

- the user experience needs to feel like a required product login, not an optional provider callback flow
- the business login now depends on a terminal form and a two-step HTTP exchange, which does not match the old callback-first interaction
- the current repo already has the right storage root, command hooks, auth gating points, and async completion plumbing, so we can replace the SAITEC business path without redesigning unrelated provider flows
- the business login layer and the future base-model login layer should stay structurally separate from the start

## Alternatives Considered

### Option 1: Sequential prompt flow inside chat input

Ask for `email`, `phone`, and `password` one by one using the existing `pending_login` text exchange.

Pros:

- smallest code diff
- easy to wire into the current `pending_login` mechanism

Cons:

- does not feel like a real blocking login form
- awkward for field editing, focus movement, and password masking
- harder to evolve later into a dual-layer login surface

### Option 2: Dedicated SAITEC blocking form in the TUI

Open a SAITEC-specific modal form with `email`, `phone`, and `password` fields whenever the business API key is missing or invalid.

Pros:

- matches the desired startup behavior
- supports local validation, password masking, and field navigation cleanly
- creates a stable surface for later adding base-model login separately

Cons:

- requires coordinated updates across auth state, rendering, input handling, and tests

### Option 3: Full provider/account-center unification

Move the business login completely into the generic account-center abstraction and redesign the SAITEC flow as a specialized provider setup flow.

Pros:

- long-term structural consistency

Cons:

- too large for the current scope
- adds unrelated complexity before the business login path is stable

Chosen option: Option 2.

## User Experience

### Startup behavior

When the TUI starts:

1. Read `~/.saitec_tui/auth.json`.
2. If the file is missing, malformed, or does not contain a non-empty `api_key`, treat the user as logged out.
3. If `api_key` exists, validate it with `GET /api/v1/users/me` using `Authorization: Bearer <api_key>`.
4. If validation succeeds, enter the normal TUI flow and refresh profile fields in `auth.json`.
5. If validation fails, treat the stored key as invalid and immediately open the SAITEC login form.

The login form opens automatically on startup when login is required. It is blocking for normal business use, but the user can still access a small whitelist of commands:

- `/login`
- `/logout`
- `/help`
- `/quit`

All other commands and normal prompt submission are blocked with a clear message telling the user to complete SAITEC login first.

### Login form behavior

The form contains exactly three editable fields:

- `email`
- `phone`
- `password`

Interaction rules:

- default focus is `email`
- `Tab` and `Shift+Tab` move focus between fields and the submit action
- `password` is masked in the UI
- the form copy explicitly says that at least one of `email` or `phone` must be provided
- local validation prevents submission if `password` is empty
- local validation prevents submission if both `email` and `phone` are empty
- `Esc` clears the current form error message but does not dismiss the login requirement
- `/login` reopens or refocuses the same form if login is required

### Login success behavior

After a valid submission:

1. `POST /api/v1/auth/login` sends the JSON body exactly as entered in the form.
2. The returned JWT is held only in memory.
3. `POST /api/v1/api-keys` uses that JWT to create a new business API key.
4. The API key name is generated as `SAITEC-TUI-YYYYMMDD-HHMMSS`.
5. The returned business API key and metadata are written to `~/.saitec_tui/auth.json`.
6. The TUI closes the blocking login form and enters the normal working state.

### Login failure behavior

If either request fails:

- the form remains open
- field values remain populated
- the error is shown inline in the form
- the user can immediately edit and resubmit

### Re-login behavior

If the stored business API key is invalid at startup or becomes invalid on an explicit re-check:

- the TUI opens the blocking login form
- a successful re-login creates a new API key
- the new session overwrites the old `auth.json`

If the stored business API key is valid:

- the TUI does not create a new API key
- the existing `auth.json` is retained and only profile fields are refreshed

### Logout behavior

For this version, `/logout` is local-only:

- clear `~/.saitec_tui/auth.json`
- clear in-memory SAITEC auth state
- return the TUI to the blocking login-required state

The code structure should leave a clear insertion point for future server-side API key revocation, but this version does not call a revoke endpoint.

## API Contracts

### Login request

`POST /api/v1/auth/login`

Request body:

```json
{
  "email": "string or null",
  "phone": "string or null",
  "password": "string"
}
```

Local rules before submission:

- `password` must be non-empty
- `email` and `phone` cannot both be empty

The form still sends both `email` and `phone` fields in the JSON body so the server sees the same shape every time.

### API key creation request

`POST /api/v1/api-keys`

Headers:

- `Authorization: Bearer <jwt>`

Request body:

```json
{
  "name": "SAITEC-TUI-YYYYMMDD-HHMMSS"
}
```

This request is only made after a successful login. It is not made when the stored business API key is already valid.

### Current-user validation request

`GET /api/v1/users/me`

Headers:

- `Authorization: Bearer <api_key>`

This request is the source of truth for whether the stored business API key is currently valid.

### API key list request

`GET /api/v1/api-keys`

This version does not use the list endpoint in the startup hot path because it does not return plaintext keys and does not change the business rule we need:

- valid local API key: reuse it
- invalid local API key: make the user log in again and create a new API key

The list endpoint remains relevant for future revoke/sync work.

## Persistence Design

### Storage location

Business login state remains under:

- `~/.saitec_tui/auth.json`

This keeps the SAITEC product state separated from `~/.jcode`.

### `auth.json` format

Persist only the business API key and metadata, not the JWT:

```json
{
  "api_key": "sk-...",
  "token_type": "Bearer",
  "user_id": "user-id",
  "email": "user@example.com",
  "phone": "13800000000",
  "display_name": "Alice",
  "api_key_id": "api-key-id",
  "api_key_name": "SAITEC-TUI-20260514-153000",
  "api_key_created_at": "2026-05-14T15:30:00Z",
  "api_key_expires_at": null,
  "last_validated_at": "2026-05-14T15:31:02Z"
}
```

Field rules:

- `api_key` is required for a logged-in session
- `token_type` stays `Bearer`
- `api_key_id`, `api_key_name`, and `api_key_created_at` come from the API key creation response
- `api_key_expires_at` is nullable
- `user_id`, `email`, `phone`, and `display_name` are refreshed from `/api/v1/users/me`
- `last_validated_at` is refreshed whenever validation succeeds

JWT is intentionally not written to disk.

## Architecture

### Modules to update

- `src/saitec/auth.rs`
- `src/tui/app/auth_types.rs`
- `src/tui/app/auth.rs`
- `src/tui/app/input.rs`
- the relevant SAITEC auth rendering and gating helpers under `src/tui`

### `src/saitec/auth.rs` responsibilities

This module should own the business-login HTTP and persistence logic:

- load/save/clear `auth.json`
- validate the stored API key via `/api/v1/users/me`
- refresh stored profile fields after successful validation
- submit `/api/v1/auth/login`
- exchange the JWT for an API key via `/api/v1/api-keys`
- generate the timestamped API key name
- expose a focused async entrypoint for "submit login form and return a session"

The old SAITEC callback helpers may remain temporarily for compatibility during migration, but they should no longer be the primary business login path.

### TUI state design

Add a dedicated SAITEC form state instead of reusing the current callback-oriented `PendingLogin::Saitec` flow.

The new state should capture:

- current `email` field value
- current `phone` field value
- current `password` field value
- current focused field/action
- current validation or submission error
- whether an async submission is in progress

This state can live either as:

- a new `PendingLogin` variant that contains structured form state

or:

- a separate SAITEC form field on `App`

The preferred direction is a dedicated structured `PendingLogin` variant so the login requirement remains anchored to the existing auth flow model.

### Auth gating

The gating should happen at the TUI input layer, not only in backend request execution.

That means:

- blocked users should see the login form immediately
- normal prompt submission should not be allowed to proceed and fail later
- the command whitelist should be explicit

This preserves a product-login feel instead of making login look like a provider misconfiguration.

## Data Flow

### Fresh login

```text
startup or /login
-> open SAITEC form
-> local validation
-> POST /api/v1/auth/login
-> receive JWT in memory
-> POST /api/v1/api-keys with JWT
-> receive plaintext business API key and metadata
-> GET /api/v1/users/me using business API key if profile data needs normalization
-> save auth.json
-> close form
-> enter normal TUI
```

### Reuse existing key

```text
startup
-> load auth.json
-> GET /api/v1/users/me with stored business API key
-> if valid, refresh profile fields and last_validated_at
-> save refreshed auth.json
-> continue into normal TUI
```

### Recover from invalid key

```text
startup
-> load auth.json
-> GET /api/v1/users/me with stored business API key
-> if invalid, open SAITEC form
-> new successful login creates a new API key
-> overwrite auth.json
```

## Error Handling

The TUI should handle these cases explicitly:

- missing `auth.json`
- malformed `auth.json`
- empty stored `api_key`
- local validation failure for the form
- `/api/v1/auth/login` request failure
- `/api/v1/api-keys` request failure
- `/api/v1/users/me` validation failure
- filesystem write failure

Expected behavior:

- show a clear inline or system error message
- keep the TUI alive
- keep the user in a recoverable state
- never enter the normal business flow without a valid stored API key

If `auth.json` is malformed, treat it as logged out and ask the user to log in again instead of crashing or partially trusting the file.

## Security Notes

- store `auth.json` with the existing secret-file write path
- do not persist JWT
- only persist the business API key that the product actually uses after login
- keep the login form local to the TUI and do not echo the password back into message history
- keep logout structured so future server-side API key revocation can be added without reshaping the local state machine

## Testing Strategy

### Unit tests

At minimum, add tests for:

- local validation fails when `password` is empty
- local validation fails when both `email` and `phone` are empty
- successful login persistence stores business API key and metadata but not JWT
- startup with a valid stored API key refreshes `user_id`, `email`, `phone`, `display_name`, and `last_validated_at`
- startup with an invalid stored API key enters login-required state
- `/logout` clears the stored session and re-enters login-required state
- generated API key names follow `SAITEC-TUI-YYYYMMDD-HHMMSS`

### TUI tests

At minimum, add tests for:

- startup opens the SAITEC login form when login is required
- `/login` opens or refocuses the SAITEC login form
- non-whitelisted commands are blocked while logged out
- whitelisted commands remain usable while logged out
- password field rendering is masked

### Integration-style auth tests

Mock the SAITEC HTTP calls so the request chain is validated:

- `/auth/login` success followed by `/api-keys` success
- `/auth/login` success followed by `/api-keys` failure
- `/users/me` valid response refreshes local profile fields
- `/users/me` unauthorized response re-enters login-required flow

## Success Criteria

This work is complete when all of the following are true:

- first launch without a valid `auth.json` automatically opens the SAITEC login form
- the form enforces local validation for `password` and `email`/`phone`
- successful submission completes the `login -> api key creation` flow and writes `~/.saitec_tui/auth.json`
- the file stores only the business API key and metadata, not the JWT
- restart with a valid stored business API key does not create a new API key
- restart with an invalid stored business API key forces re-login and overwrites the old session after success
- `/logout` clears local business-login state and returns the user to the login-required UI
