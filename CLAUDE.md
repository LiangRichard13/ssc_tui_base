# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```powershell
# Quick check (fastest feedback)
cargo check

# Build debug
cargo build

# Build release
cargo build --release

# Run all tests
cargo test

# Run a single unit test
cargo test test_name -- --nocapture

# Run a single integration test
cargo test --test e2e test_name -- --exact --nocapture

# Run tests for a specific crate
cargo test -p jcode-core

# Format check
cargo fmt --all -- --check

# Clippy (as CI does)
cargo clippy --all-targets --all-features -- -D warnings

# Run with specific profile
.\scripts\dev_saitec_tui.ps1 -Profile selfdev

# Build dev version
.\scripts\dev_saitec_tui.ps1

# Package for distribution
cargo build --release
.\scripts\package_saitec.ps1

# Remote build (when local resources insufficient)
scripts/remote_build.sh

# Stop running dev instance
.\scripts\dev_saitec_tui.ps1 -StopRunning -NoBuild
```

E2E tests live in `tests/e2e/` and use a mock provider (`tests/e2e/mock_provider/`). They verify the full flow from user input to response without actual API calls. E2E modules: `ambient`, `binary_integration`, `burst_spawn`, `provider_behavior`, `safety`, `session_flow`, `transport`, `windows_lifecycle`.

Unit tests are inlined in each source file via `#[cfg(test)] mod tests` or `#[cfg(test)] mod tests { ... }`. There are ~64 test module locations across the crate.

Budget enforcement scripts in `scripts/` check code size, panic count, swallowed errors, test size, and warnings — these run as part of CI.

## Project Architecture

**jcode** is a Rust workspace monorepo (edition 2024). The main binary is defined in `src/main.rs`, the library in `src/lib.rs`. Execution flow: `main.rs` → `lib.rs::run()` → `cli::startup::run()`.

### Key Source Modules (`src/`)

| Module | Purpose |
|---|---|
| `cli/` | CLI argument parsing, command dispatch, TUI launch, login flows |
| `tui/` | Terminal UI — ratatui-based widgets, rendering, keybindings, sessions, pinned items |
| `session/` | Session lifecycle, persistence, crash recovery, memory profiles |
| `provider/` | LLM provider implementations (OpenAI, Claude, Gemini, Bedrock, OpenRouter, Copilot, Cursor) |
| `auth/` | Authentication — OAuth flows (PKCE), account store, provider-specific login (Claude, Codex, Gemini, Copilot, Cursor, etc.), AuthStatus cache with 30s TTL |
| `config/` | Configuration file management, env overrides, display |
| `server/` | Server/subprocess orchestration, multi-process architecture |
| `agent/` | Agent runtime — tool execution, conversation loop |
| `memory/` | Conversation memory, memory agent, memory graph, log |
| `tool/` | Tool definitions and execution |
| `mcp/` | MCP (Model Context Protocol) server integration |
| `saitec/` | SAITEC platform-specific integration |
| `replay/` | Session replay functionality |
| `telemetry/` | Telemetry collection and reporting |
| `update/` | Auto-update mechanism |
| `transport/` | Transport layer for IPC |
| `gateway.rs` | WebSocket gateway for remote clients (iOS app, web) |
| `sidecar.rs` | Lightweight sidecar client for fast/cheap model calls (memory relevance verification) |
| `bus.rs` | Event broadcast system — tool events, background tasks, subagent status, swarm events |
| `storage.rs` | JSON persistence with automatic backup/corruption recovery |
| `message.rs` | Message types, secret redaction, background task notification parsing |
| `protocol.rs` | Re-exports from `jcode-protocol` crate — `ServerEvent`, `HistoryMessage`, etc. |

### Workspace Crates (`crates/`)

50+ crates organized by domain: `jcode-core`, `jcode-provider-*`, `jcode-tui-*`, `jcode-auth-types`, `jcode-memory-types`, `jcode-protocol`, `jcode-storage`, `jcode-agent-runtime`, `jcode-swarm-core`, `jcode-mobile-core`, `jcode-desktop`, etc.

### Feature Flags

- `default = ["pdf"]` — PDF parsing support (heavy deps)
- `embeddings` — Local ONNX/tokenizer embedding inference (very heavy, 163+ crates)
- `jemalloc` / `jemalloc-prof` — Alternative allocator
- `dev-bins` — Development-only binaries (benchmarks, probes)

### Build Profiles

| Profile | Use | LTO | Codegen Units |
|---|---|---|---|
| `dev` | Default debug | none | default |
| `release` | Local release builds | none | 256 |
| `selfdev` | Self-development builds (inherits release, opt-level=0) | none | 256 |
| `release-lto` | Distribution builds | thin | 16 |

### Build System

`build.rs` auto-generates version strings from git (hash, date, tag, changelog). Counter-based patch bumping logic for dev builds.

### CI Pipeline (GitHub Actions)

- **quality**: `cargo fmt --check`, `cargo check --all-targets --all-features`, `clippy -D warnings`, budget enforcement scripts
- **build & test**: Ubuntu + macOS — release build, library/binary tests, provider matrix tests, e2e tests
- **windows-build-test**: Windows x64 — release build, targeted validation tests, e2e smoke tests, installer verification
- **mobile-simulator**: Mobile core + simulator tests on Linux
- **windows-cross-check**: Cross-compilation checks via cargo-xwin on Linux

## Agent Runtime Architecture

The Agent (`src/agent.rs`) is the core conversation driver. It owns a `Provider`, a `Registry` (tools + skills), and a `Session`. Key sub-modules:

| Sub-module | Purpose |
|---|---|
| `compaction` | Message compaction for long conversations |
| `environment` | Environment setup for agent runs |
| `interrupts` | Soft/hard interrupt handling (Ctrl+C, tool interrupts) |
| `messages` | Message construction and transformation |
| `prompting` | System prompt construction and injection |
| `provider` | Provider-specific adaptation layer |
| `response_recovery` | Recovery from failed/skipped responses |
| `status` | Agent status reporting over the bus |
| `streaming` | Streaming response handling with keepalive |
| `tools` | Tool call dispatch, tool output processing |
| `turn_execution` | Single-turn execution logic |
| `turn_loops` | Multi-turn conversation loops |
| `turn_streaming_broadcast` / `turn_streaming_mpsc` | Streaming variants for different client types |

The Agent processes messages in a loop: construct system prompt → call provider → handle streaming response → execute tool calls → continue. Uses `Registry` (cloned per subagent with fresh `CompactionManager`) for tool dispatch. Native tools (`selfdev`, `communicate`) get special handling.

## Server & Multi-Process Architecture

The `server` module (`src/server.rs`) is a multi-session, multi-client runtime with 30+ sub-modules:

| Sub-module | Purpose |
|---|---|
| `runtime` | Core `ServerRuntime` — manages sessions, headless mode, lifecycle |
| `headless` | Headless session creation (no TUI) |
| `client_*` (7 files) | Client lifecycle, communication, state management |
| `comm_*` (4 files) | Communication with sessions (control, plan, sync, session) |
| `swarm*` (4 files) | Swarm/multi-agent coordination |
| `debug*` (6 files) | Debug commands, events, state inspection, swarm read/write |
| `socket` | Unix socket transport for client-server IPC |
| `lifecycle` | Server startup/shutdown lifecycle |
| `durable_state` | Persistent server state across restarts |
| `background_tasks` | Background task completion/progress dispatch |
| `provider_control` | Provider-level control commands |
| `reload*` (3 files) | Server reload / hot-reload mechanisms |
| `await_members_state` | Wait for all swarm members to reach a state |

Clients connect via Unix sockets (or WebSocket through the `gateway` module). The server relays NDJSON protocol messages. Headless mode (`jcode --headless`) spawns a server without TUI.

## Event Bus System (`src/bus.rs`)

The `Bus` is a `tokio::sync::broadcast`-based event system that connects components:

| Event Type | Purpose |
|---|---|
| `ToolEvent` | Tool execution status (running/completed/error) per session |
| `ToolSummary` | Tool state summary (for TUI rendering) |
| `SubagentStatus` | Subagent API call status for progress display |
| `BackgroundTaskProgressEvent` | Background task progress updates |
| `BackgroundTaskCompleted` | Background task completion notification |
| `TodoEvent` | Todo list state per session |
| `SidePanelSnapshot` | Side panel state for TUI |
| `SwarmEvent` / `SwarmUpdate` | Swarm coordination events |
| `BusEvent` | High-level events (session status, agent status, plan status) |

The bus is used for TUI rendering, progress display, swarm coordination, and cross-component communication. Usage pattern: `bus.send(...)` to publish, `bus.subscribe()` to get a `broadcast::Receiver`.

## Memory System (`src/memory.rs`)

Persistent cross-session memory organized by scope:
- **Project** (per working directory via `memory_graph.rs`)
- **Global** (user-level preferences)

Key components:
- `MemoryGraph` — graph-based memory storage with versioned schema (`GRAPH_VERSION`)
- `MemoryAgent` — autonomous memory extraction/management agent
- `MemoryLog` / `RuntimeMemoryLog` — activity logs for memory operations
- `Sidecar` — lightweight model client for relevance verification and extraction (avoids full Agent SDK overhead). Automatically selects OpenAI (Codex/chatgpt) or Claude backend based on available credentials.
- `MemoryPrompt` — prompt construction for memory-aware interactions

Memory flows: extract from conversation → sidecar verifies relevance → store in MemoryGraph → inject into future sessions via relevance search.

## Storage & Crash Recovery (`src/storage.rs`, re-exports `jcode_storage`)

JSON persistence with automatic corruption recovery:
- `read_json<T>` — reads JSON with fallback to backup file on corruption
- `write_json` / `write_json_atomic` — atomic writes with backup file
- Backup files stored alongside primary files
- Recovery events logged via `StorageRecoveryEvent` callback

Session persistence (`src/session/persistence.rs`, `session/journal.rs`):
- Journal-based session storage (`SessionJournalMeta`, `SessionPersistState`, `PersistVectorMode`)
- Crash detection: `detect_crashed_sessions()`, `recover_crashed_sessions()`
- Active PID tracking: `register_active_pid()` / `unregister_active_pid()` in `src/session/active_pids.rs`

## WebSocket Gateway (`src/gateway.rs`)

TCP WebSocket gateway (port 7643 by default) connecting remote clients (iOS app, web) to the session server. Architecture: `TCP :7643 → WebSocket upgrade → UnixStream::pair() → handle_client()`. Each remote client gets a virtual Unix socket pair — one end to the server's existing client handler, the other bridged to WebSocket frames. Supports bearer token and query parameter auth.

## Configuration System (`src/config.rs`)

Config loaded from `~/.jcode/config.toml` (or `$JCODE_HOME/config.toml`), exposed via `config::config()` returning `&'static Config` through a `OnceLock` singleton. Environment variables override file settings. Key config types: `DisplayConfig`, `ProviderConfig`, `AuthConfig`, `FeatureConfig`, `SafetyConfig`, `CompactionConfig`, `KeybindingsConfig`, `NamedProviderConfig`.

## Login Flow Architecture

Two-layer login system in `src/auth/`, `src/cli/login.rs`, `src/tui/login_picker.rs`:

- **SAITEC platform login**: Email/phone + password → `POST /api/v1/auth/login` → get JWT → `POST /api/v1/api-keys` → creates a business API Key → stored as `SaitecSession` struct
- **Base model login**: Multiple auth methods dispatched by `LoginProviderTarget`:
  - OAuth + PKCE (Claude, OpenAI/Codex, Gemini, Antigravity) — generates verifier+SHA256 challenge, binds local callback server, exchanges code for tokens
  - API key input (OpenAI, OpenRouter, Bedrock, Cursor) — secret prompt, saved to `.env` files
  - Device code flow (GitHub Copilot) — `device_code` → poll → token
  - Form-based (SAITEC) — email/phone + password
  - OpenAI-compatible provider flow: API key → optionally model name → profile activation
- TUI uses `PendingLogin` state machine enum to track in-progress login steps, with these variants:
  - `StartupGuide { focused, is_reminder }` — Welcome guide overlay (see below)
  - `SaitecForm` — SAITEC email/phone + password form
  - `ClaudeAccount` / `OpenAiAccount` / `Antigravity` / `Gemini` — OAuth PKCE flows
  - `ApiKeyProfile` — API key text entry
  - `OpenAiCompatibleApiBase` — OpenAI-compatible base URL entry
  - `OpenAiCompatibleModelName` — Optional model name prompt after API key save (when provider lacks `default_model`)
  - `CursorApiKey` / `Copilot` — API key / device code flows
- `AuthStatus` cached with 30s TTL (`check()`) and 5s TTL (`check_fast()`), invalidated after auth changes
- `AuthStatus::has_any_base_model()` — checks if any real base-model provider (excluding SAITEC/jcode) is configured

## Startup Guide System

Introduced in commits b8c2701a / e147098d. A `PendingLogin::StartupGuide` overlay shown on the branded startup splash when credentials are missing:

- **Setup mode** (`is_reminder: false`): Shown when no base model is configured. Blocking — user must configure at least one base model.
- **Reminder mode** (`is_reminder: true`): Shown when base models are OK but SAITEC login is missing. Skippable via the "Skip SAITEC" button.

Two action buttons navigable via Tab/Enter: **Log in to SAITEC Platform** and either **Configure AI Base Model** (setup) or **Skip SAITEC, Continue** (reminder). Wired into both `App::run()` and `App::run_remote()`. The `restore_startup_guide_if_needed()` method reopens the guide when the login picker is cancelled.

## OpenAI-Compatible Provider Env System

OpenAI-compatible providers (generic endpoints, Kimi, etc.) use a set of `JCODE_OPENAI_COMPAT_*` env vars at `src/provider_catalog.rs`:

- `JCODE_OPENAI_COMPAT_API_BASE` — base URL
- `JCODE_OPENAI_COMPAT_API_KEY_NAME` / `JCODE_OPENAI_COMPAT_ENV_FILE` — credential references
- `JCODE_OPENAI_COMPAT_DEFAULT_MODEL` — default model name (set via the model name prompt or profile)

When switching between openai-compatible providers, all four vars are cleared from both the **process environment** and the **env file** (`~/.jcode/openai_compat.env`) to prevent stale values from leaking. This uses `force_apply_openai_compatible_profile_env(None)` which bypasses the `JCODE_PROVIDER_PROFILE_ACTIVE` named-profile lock guard. Clearance happens at three layers: profile apply, env file rewrite, and pre-login cleanup.

## SAITEC Credential Storage

- **`~/.saitec_tui/auth.json`** — main session file (`SaitecSession` struct with `api_key`, `auth_token`, `user_id`, `email`, `display_name`, `api_key_id`, etc.)
- **`~/.saitec_tui/saitec.env`** — env bridge file, stores `SAITEC_API_KEY=<key>` for MCP subprocess injection
- **`~/.saitec_tui/mcp.json`** — MCP server config (Python command + args, no API key persisted on disk)
- API key flows to MCP via `runtime_api_key()` (`src/saitec/mcp.rs:113`): reads `configured_api_key()` (from `saitec.env`) first, falls back to `load_session()` (from `auth.json`)
- `clear_session()` on logout: deletes `auth.json`, clears `SAITEC_API_KEY` from `saitec.env`, removes process env var

## SAITEC Platform Integration

- SAITEC-Skills MCP service handles detection/evaluation task dispatch
- Skills stored in `SAITEC-Skills/` (external resource directory, resolved at runtime from `_vendor/`, `resources/`, or `SAITEC_SKILLS_ROOT` env var)
- Task flow: upload file → get `storage_uri` → create task → poll → download results
- Default API endpoints: `http://101.133.153.37:8080` (overridable via `CORE_API_BASE`, `SAITEC_AUTH_BASE`, `SAITEC_API_BASE` env vars)
- **MCP Lifecycle Sync** (`src/saitec/mcp.rs`): SAITEC-Skills MCP server is automatically reconnected on SAITEC login (`reconnect_saitec_mcp()`) and disconnected on SAITEC logout (`disconnect_saitec_mcp()`). This ensures the MCP subprocess always has the current API key without manual restart.
  - `reconnect_saitec_mcp()` — disconnects existing server, loads fresh `McpConfig` (triggers `apply_runtime_env` with current credentials), reconnects via `SharedMcpPool`
  - `disconnect_saitec_mcp()` — disconnects server from shared pool, logs the action

## Target Platforms

- Windows x64 (primary), Windows ARM64
- Linux x86_64, macOS aarch64
- Mobile simulator (iOS via `jcode-mobile-core` + `jcode-mobile-sim`)

## Project Memory

### SAITEC-Skills vendor sync procedure

`_vendor/SAITEC-Skills/` is a manually maintained vendor copy (NOT git submodule, NOT git subtree). It is tracked by git (~22 files) and is NOT excluded by `.gitignore`. `package_saitec.ps1` and `release.yml` both bundle it directly.

#### Key facts (verified 2026-06-29)

- The SAITEC-TUI team adds **two helper modules** to vendor that **upstream never has**:
  - `mcp_server/api_tools/auth_headers.py` — `build_auth_headers()` / `resolve_api_key()`. Falls back to `~/.saitec_tui/auth.json` when `SAITEC_API_KEY` env is not set.
  - `mcp_server/api_tools/http_errors.py` — `raise_for_status_with_body()` raises with response body attached, used in place of httpx's default `raise_for_status()`.
- Every `*_tools.py` in vendor uses these helpers via `from api_tools.auth_headers import build_auth_headers` + `from api_tools.http_errors import raise_for_status_with_body`. **The selective sync MUST preserve these imports and call sites**, not blindly replace them with upstream's inlined `os.getenv("SAITEC_API_KEY")` and `resp.raise_for_status()` style.
- `tests/` is vendor-only (upstream has no `tests/`).
- Upstream has `test_data/` (data sets, not tests). Vendor does not.
- Credentials are injected by Rust: `src/saitec/mcp.rs apply_runtime_env` writes `SAITEC_API_KEY` (constant value `"SAITEC_API_KEY"`, defined at `src/subscription_catalog.rs:3`) into the MCP subprocess env. So Python `os.getenv("SAITEC_API_KEY")` always works even without auth_headers.py. The auth_headers.py fallback to auth.json is **defense in depth**, not strictly required.

#### Sync procedure (selective mode)

1. **Richard clones upstream** at `C:\Users\Administrator\Desktop\projects\SAITEC-Skills` (separate git repo). Check `git log --oneline` to see the upstream commit history.
2. **Per file**: Read source first, Read vendor current state second, then compare and decide:
   - For new tools (e.g., `read_file_content`, `get_tested_models`): copy from upstream, BUT translate `resp.raise_for_status()` → `raise_for_status_with_body(resp)` to keep vendor style.
   - For docstring hardening (IMPORTANT blocks, Must be a JSON rules): copy from upstream verbatim — these don't conflict with the helpers.
   - For `run_mcp.sh` CORE_API_BASE port: upstream is 8080, vendor was 8000. Always sync to 8080.
   - For `skills/*.md`: full copy from upstream (SOPs change frequently).
3. **Run `python -m py_compile <file>` after every change** to catch syntax errors immediately.
4. **Verify after all changes**:
   - `python -m py_compile` on all 10 .py files
   - `grep -h "async def " mcp_server/api_tools/*.py | grep -v register_` — count should match upstream (currently 30 tools)
   - `cargo check` — should pass with no new warnings
   - `diff -rq <src> <vendor>` — only expected diffs (helper imports, helper call sites, download_file stricter error body check)
5. **Commit**: `chore(vendor): selective sync of SAITEC-Skills to upstream <short-sha>`. Include: tools added, docstring changes, port fix, docs sync, preserved items, verification results.

#### Anti-patterns (mistakes made in past sessions)

- **DO NOT** blindly `cp -f` upstream files into vendor — that loses the helper imports and breaks SAITEC-TUI's auth.json fallback.
- **DO NOT** delete `auth_headers.py` / `http_errors.py` just because upstream doesn't have them. They are intentional SAITEC-TUI additions.
- **DO NOT** rely on `grep -E` for verifying docstring presence — it can be misleading when output is sliced. Use `sed -n` for explicit line ranges, or read the actual docstring content.
- **DO NOT** trust first read of file content. The `image_detect_tools.py` and `video_detect_tools.py` had vendor-leading content in some places (`upload_file` 6-type, `detect_image` method list) and vendor-trailing content in others (read_file_content missing). Always diff explicitly.
- **DO NOT** make a single big Edit that combines `"""..."""` boundaries with content changes — repeated docstring opens from `"""` to `"""` cause "unterminated triple-quoted string literal" errors when the edit pattern overlaps. If an Edit fails with that error, `git checkout HEAD -- <file>` to revert and redo with a smaller `old_string` that does NOT include the closing `"""`.

#### Diff direction (which side is "new")

Vendor is **historically lagging** upstream. Upstream `b178ab8` has more tools, more docstring detail, and the 8080 port fix. Vendor's only advances over upstream are the two helper modules and the stricter `download_file` error-body check.

Last sync: commit `9f350494` (2026-06-29), upstream at `b178ab8`. 14 files, +486 / -52.

### OpenAI-compatible provider: config vs credentials env hygiene

The generic `OPENAI_COMPAT_PROFILE` (id `"openai-compatible"`, default `api_base = "https://api.openai.com/v1"`, `default_model = None`) stores the user's **custom API base** and **model name** ONLY in the env file (on Windows: `AppData/Roaming/jcode/openai-compatible.env`, via `storage::app_config_dir()`) under `JCODE_OPENAI_COMPAT_API_BASE` / `JCODE_OPENAI_COMPAT_DEFAULT_MODEL`. They are read back via `resolve_openai_compatible_profile` (`src/provider_catalog.rs:20`) → `env_override` (`src/provider_catalog.rs:817`, process-env → env-file fallback). If those env-file rows are deleted, the resolved profile falls back to the OpenAI default endpoint and `None` model — the user must reconfigure both. (Named profiles like Kimi/ZAI/DeepSeek are different: their base/model are profile constants and `resolve` returns early for non-generic ids at `provider_catalog.rs:34`, so they never depend on these env-file rows. Note: `auth: OPENAI_COMPAT_API_KEY` in logs means generic profile was used, even if the endpoint is a named-provider URL like deepseek — the user typed that base into the generic openai-compatible login flow.)

**Config vs credentials distinction** (verified 2026-06-29, fix for P0 logout bug):
- **Credentials** (must be cleared on logout): the API key (`resolved.api_key_env`), ZAI/ZHIPU linked keys, `JCODE_OPENAI_COMPAT_LOCAL_ENABLED` flag.
- **Config** (must survive logout): `JCODE_OPENAI_COMPAT_API_BASE`, `JCODE_OPENAI_COMPAT_DEFAULT_MODEL`, and the credential-source metadata `JCODE_OPENAI_COMPAT_API_KEY_NAME` / `JCODE_OPENAI_COMPAT_ENV_FILE`.
- **Runtime** (clear from process env only, never touch env file on logout): all `JCODE_OPENROUTER_*` derived vars + the named-profile lock guards + the 4 `JCODE_OPENAI_COMPAT_*` process-env overrides.

Two production call sites used to wipe config via `force_apply_openai_compatible_profile_env(None)` (which deletes the 4 env-file rows in `apply_openai_compatible_profile_env_impl`'s `profile.is_none()` branch): the base-model logout path (`clear_openai_compatible_profile_credentials` in `src/tui/app/auth_account_commands.rs`) and the startup activation-failure rollback (`try_activate_configured_base_model` in `src/provider/jcode.rs`). A **third** site was the real root cause of the "restart after closing TUI loses base/model" P0: `init_provider_with_options` in `src/cli/provider_init.rs` calls `apply_openai_compatible_profile_env(None)` when `profile_for_choice(choice)` returns None — which happens on every plain `jcode`/SAITEC-TUI launch (`ProviderChoice::Jcode` has no profile, `profile_for_choice` line 202 `_ => None`). Since TUI `/login` for generic openai-compatible does NOT set `JCODE_PROVIDER_PROFILE_ACTIVE` / `JCODE_NAMED_PROVIDER_PROFILE`, the guard at `provider_init.rs:1115` is satisfied and `apply(None)` runs on every startup, wiping the env file's base/model. All three sites now call `clear_openai_compatible_runtime_env_keep_config()` instead — clears process-env runtime vars only, leaves env-file config intact. `force_apply_openai_compatible_profile_env` itself is unchanged (its "clear everything" semantics are correct for the other call sites: test guards, profile switching).

**The env file lives at `AppData/Roaming/jcode/openai-compatible.env` on Windows** (i.e. `storage::app_config_dir()`, NOT `~/.saitec_tui/` and NOT `~/.jcode/`). When debugging openai-compat config loss, inspect that file (and its `.bak`). A `.bak` that contains `JCODE_OPENAI_COMPAT_DEFAULT_MODEL` while the `.env` does not is direct evidence that a `apply(None)`/`force_apply(None)` wiped the env file.

**Anti-pattern**: do NOT call `force_apply_openai_compatible_profile_env(None)` from logout or activation-rollback paths — it deletes the user's configured base/model from the env file. Use `clear_openai_compatible_runtime_env_keep_config()` there. Reserve `force_apply(None)` for true "reset everything" contexts (test isolation, switching to a different provider).

**Note on re-login**: even after this fix, walking `/login openai-compatible` again clears `JCODE_OPENAI_COMPAT_DEFAULT_MODEL` from the env file (`src/tui/app/auth.rs` ~line 2310, pre-login stale-override cleanup) — so re-login preserves the base (Enter keeps current value) but re-prompts for model. That is intentional existing behavior (avoids stale model when switching providers); do not "fix" it as part of a logout-config-preservation task.

**Diagnostic method for "config lost after restart/logout" bugs** (proven 2026-06-29):
1. Inspect the real env file `AppData/Roaming/jcode/openai-compatible.env` AND its `.bak` (the `.bak` is the pre-write snapshot from `upsert_env_file_value`). If `.bak` has `JCODE_OPENAI_COMPAT_DEFAULT_MODEL=...` but `.env` does not, the env file was wiped between those two timestamps — compare the two mtimes.
2. Cross-reference the `.env` mtime against `~/.saitec_tui/logs/jcode-<date>.log`. A `jcode starting` line at that exact second means the wipe happened during **startup** (→ `init_provider_with_options` / `try_activate_configured_base_model`); a `Logged out from` line means **logout** (→ `clear_openai_compatible_profile_credentials`); a `Hot-initialized ... after auth change` line means **auth-change reinit**.
3. `jcode_version` in `ENV_SNAPSHOT` log lines tells you which commit the running binary was built from — if it does not include the fix commit, the user is running a stale binary (rebuild via `dev_saitec_tui.ps1`, which runs `cargo build --profile selfdev`).
4. The fix function is `clear_openai_compatible_runtime_env_keep_config()` in `src/provider_catalog.rs` — grep for any remaining `force_apply_openai_compatible_profile_env(None)` / `apply_openai_compatible_profile_env(None)` in NON-test code; each such call site in a logout/startup/auth-change path is a candidate config-wipe bug.


### AuthTest deadlocks via stale `auth-validation.json` record

Symptom: user configures an openai-compatible provider (e.g. deepseek) successfully and can chat, but pressing `R` in the login picker shows `validation failed (just now)` with NO error detail visible in the picker. The actual failure cause lives in `~/.saitec_tui/auth-validation.json`.

**Where the deadlock lives** (proven 2026-07-01, fixed in commit `b18a4c17`):
- `src/auth/mod.rs:245` `state_for_provider` for `OpenAiCompatible` returns `Expired` whenever there is a stale `success: false` row in `auth-validation.json` for that provider_id — regardless of whether the env-file key is still present.
- `src/cli/auth_test/probes.rs` (pre-fix) used `state_for_provider` for `credential_probe`, so an `Expired` result short-circuited the probe and made `report.success = false` before smoke even ran.
- `src/cli/auth_test/run.rs:9` (`maybe_run_auth_test_smoke`) gates the smoke on `report.success`, so smoke was skipped entirely.
- `populate_auth_test_target_report` then wrote the new failure back to `auth-validation.json` — locking the state in until the user manually deleted the file.

**Why the picker label is uninformative**: `auth/validation.rs:40` `format_record_label` always renders failure as `"validation failed ({age})"`. The actual `record.summary` (e.g. `credential_probe: OpenAI-compatible auth status is expired (not configured).`) is not shown in the picker label — read `auth-validation.json` directly to see why.

**Fix** (commit `b18a4c17`, file `src/cli/auth_test/probes.rs`): for `OpenAiCompatible` targets, `probe_generic_provider_auth` now bypasses the stale `Expired` status by checking `openai_compatible_profile_is_configured(profile)` directly (key still present on disk → credential is `Available`). The smoke then runs for real and either passes or surfaces a concrete HTTP error. Non-OpenAiCompatible targets still use `state_for_provider` unchanged.

**Diagnostic method for "validation failed (just now)" on picker `R`**:
1. **Read `~/.saitec_tui/auth-validation.json` first** — `summary` field tells you the precise failure (HTTP 401, model not supported, connection timeout, etc.). Do NOT trust the picker label alone.
2. If `summary` starts with `credential_probe: ... auth status is expired (not configured)` — this is the deadlock above. With the fix (`b18a4c17`) it cannot recur. Without the fix (older binary), the recovery is `rm ~/.saitec_tui/auth-validation.json` then re-press `R`.
3. If `summary` is `provider_smoke: ...`: the smoke actually ran and failed. Look at the trailing HTTP status / response body for the real cause (auth header, model name not accepted by the endpoint, TLS, etc.). Config + key are fine; the endpoint rejected something.
4. If the file is missing entirely, the picker shows `not validated yet`; if the row exists with `success: true`, the picker shows `runtime + tool validated` / `runtime validated` per `format_record_label` (`auth/validation.rs:40`).

### OpenAI-compatible context window + revalidate: 200K fallback and `anthropic/claude-sonnet-4` regression

Symptom (fixed 2026-07-06, commit `e05304a1`): user configures the generic `openai-compatible` profile (e.g. `api_base = https://api.deepseek.com`, `model = deepseek-v4-flash`) and can chat. After TUI restart or pressing `R` in the picker to revalidate, the right-side context bar drops to **200K** and requests fail with `400 Bad Request: The supported API model names are deepseek-v4-pro or deepseek-v4-flash, but you passed anthropic/claude-sonnet-4.`

This single symptom had **four independent root causes**, all of which needed fixing to make restart + revalidate + logout/login actually preserve deepseek as the active configuration.

#### 1. Hardcoded `"anthropic/claude-sonnet-4"` fallback in `MultiProvider.model()` / `context_window()`

**Files**: `src/provider/mod.rs:732-735` (model) and `src/provider/mod.rs:1926-1929` (context_window).

```rust
ActiveProvider::OpenRouter => self
    .openrouter_provider()
    .map(|o| o.model())
    .unwrap_or_else(|| "anthropic/claude-sonnet-4".to_string()),
```

When `MultiProvider.openrouter` is `None` (no sub-provider constructed) **or** the sub-provider's model is wrong, the hardcoded `"anthropic/claude-sonnet-4"` is returned. Same `unwrap_or(DEFAULT_CONTEXT_LIMIT)` returns **200K** for context_window. This string is also the value of `DEFAULT_MODEL` in `src/provider/openrouter.rs:63` — the same fallback fires inside `OpenRouterProvider::new()` when `JCODE_OPENROUTER_MODEL` env var is unset and `autodetected_openai_compatible_profile()` returns `None`.

**Fix**: added an exact-match model→context table in `jcode-provider-core` (`OPENAI_COMPAT_MODEL_CONTEXT_LIMITS` in `crates/jcode-provider-core/src/models.rs:39-101`) covering DeepSeek, Kimi, GLM, Qwen. The function `openai_compatible_model_context_limit()` is now called from `context_limit_for_model_with_provider_and_cache` (`models.rs:247-249`) so **every provider** (including the inert client provider in self-dev mode) hits the table for common openai-compatible models. Without this, even the `InertRuntimeProvider.model()` = `"unknown"` path (used at startup before the first server event) has no chance of resolving to the right context window.

#### 2. Server bootstrap env vars not set → `MultiProvider.openrouter = None`

`new_with_auth_status` (`src/provider/startup.rs:50-247`) computes `has_openrouter_creds` from `active_compatible_profile`, which itself comes from `active_openai_compatible_profile_id()` — that function reads `JCODE_OPENROUTER_CACHE_NAMESPACE` / `JCODE_NAMED_PROVIDER_PROFILE` env vars. At server bootstrap **none** of these are set, so `active_compatible_profile = None` → `has_openrouter_creds = false` → `openrouter = None` in the resulting `MultiProvider`. Even if I forced `active = OpenRouter` (fix #3 below), the sub-provider doesn't exist → `model()` falls back to the hardcoded string.

**Fix** (commit `e05304a1`, `src/provider/startup.rs:87-99`): at the start of `new_with_auth_status`, scan disk env files for any `openai_compatible_profile_is_configured` profile, and call `apply_openai_compatible_profile_env(Some(profile))` **before** the `active_compatible_profile` lookup. This sets `JCODE_OPENROUTER_CACHE_NAMESPACE` etc. on the process env so the rest of the function (and `OpenRouterProvider::new()` shortly after) all agree.

#### 3. `auto_default_provider` priority ignored user's actual config

`auto_default_provider` (`crates/jcode-provider-core/src/selection.rs:45`) returns `OpenRouter` only as a last-resort fallback (after OpenAI/Claude/Copilot/Antigravity/Gemini/Cursor/Bedrock). When the user has Claude credentials on disk (from a previous Anthropic login) **and** an openai-compatible profile configured (e.g. deepseek), Claude wins → `MultiProvider.active = Claude` → `MultiProvider.model()` returns the Claude sub-provider's default (`claude-opus-4-5-20251101`) — but actual API requests still go to deepseek because `complete_with_failover` falls through to the openai-compatible sub-provider. **The static label and the real routing model are decoupled**, which is the architectural cause of the 200K / wrong-model symptoms.

**Fix** (commit `e05304a1`): added `prefer_openai_compatible: bool` parameter to `auto_default_provider`. When `true` (detected from the env-file scan in fix #2), it returns `OpenRouter` **before** OpenAI/Claude. Plumbed through `src/provider/selection.rs` and the two test call sites in `src/provider/tests/fallback_failover.rs`. **Trade-off**: if a user has both Claude and openai-compatible configured, this prefers openai-compatible. Acceptable in the common case; future-proof via step 2 below.

#### 4. `JCODE_OPENROUTER_MODEL` cleared but never re-set by `apply_openai_compatible_profile_env_impl`

`src/provider_catalog.rs:347-372` lists `JCODE_OPENROUTER_MODEL` in `RUNTIME_OPENAI_COMPAT_ENV_VARS`. `apply_openai_compatible_profile_env_impl` (`src/provider_catalog.rs:403-446`) clears that list, then re-sets `JCODE_OPENROUTER_API_BASE` / `API_KEY_NAME` / `ENV_FILE` / `CACHE_NAMESPACE` / `STATIC_MODELS` / `ALLOW_NO_AUTH` — but **never re-sets `JCODE_OPENROUTER_MODEL`**. Every call site that triggers this function (server bootstrap, revalidate via `apply_login_provider_profile_env` at `src/cli/auth_test/run.rs:125`, `MultiProvider::on_auth_changed` rebuild, logout/login) therefore wipes the user's chosen model from process env. The next `OpenRouterProvider::new()` then reads a missing `JCODE_OPENROUTER_MODEL`, finds `autodetected_profile = None` (because explicit runtime vars are now set), and falls back to `DEFAULT_MODEL = "anthropic/claude-sonnet-4"`. The newly-constructed OpenRouterProvider's `self.model` is the wrong string, and **subsequent server events carry it to the client**, triggering `update_context_limit_for_model("anthropic/claude-sonnet-4")` → 200K + 400 errors on every request.

**Fix** (commit `e05304a1`, `src/provider_catalog.rs:444-450`): after the runtime env var assignments, also re-apply the persistent default model so `OpenRouterProvider::new()` always sees the correct model regardless of which caller invoked this function:

```rust
if let Some(model) = resolved.default_model {
    crate::env::set_var("JCODE_OPENROUTER_MODEL", &model);
}
```

`resolved.default_model` already reflects `JCODE_OPENAI_COMPAT_DEFAULT_MODEL` (read from env or env file at `provider_catalog.rs:75-77`), so it stays in sync with whatever the user actually configured.

#### 5. Startup session has no model in self-dev mode

In self-dev mode the TUI client uses `InertRuntimeProvider`, whose `model()` always returns `"unknown"` (`src/tui/app.rs:1022-1024`). Without `--resume`, `dev_saitec_tui.ps1` calls `new_minimal_with_session` with a fresh `Session::create(None, None)` → `session.model = None`. The startup-time `context_limit = provider.context_window()` therefore looks up `"unknown"` and gets 200K. Fix #1's mapping table catches this for known openai-compatible model names, but a fresh session has no model to pass at all.

**Fix** (commit `e05304a1`, `src/tui/app/tui_lifecycle.rs:166-180`): after computing `context_limit`, if `provider.model() == "unknown"` and `session.model` is `Some(real_model)`, recompute with `context_limit_for_model_with_provider(session_model, Some(provider.name()))` so fix #1's table is hit on the freshly-resumed session.

#### 6. Display formatting: `1000K` → `1M`

Bonus fix (commit `e05304a1`, `src/tui/info_widget_usage.rs:321-329`): `format_token_k` for ≥ 1,000,000 tokens now renders `1M` (matches the convention used by the existing `format_tokens`). Test fixture updated in `src/tui/info_widget_tests.rs`.

#### Architectural note: the static-label / real-routing decoupling is the root cause

Even after all the above fixes, the `MultiProvider::model()` static-label and `complete_with_failover()` dynamic-routing remain architecturally decoupled. Step #4 (re-apply default_model as `JCODE_OPENROUTER_MODEL`) is the **practical resolution** because it makes the static label and the routing result converge at the env-var source. A future more architecturally clean fix would be to add a `MultiProvider::effective_provider_and_model()` dry-run that replays `fallback_sequence_for` without making a request, and use that for `ServerEvent::History.provider_model` / `context_window`. **This is not required for the current bug** but is the long-term fix for the same class of "label says X, routing does Y" issues. Track separately.

#### Diagnostic method for "context stuck at 200K" or "model is anthropic/claude-sonnet-4"

1. **Read `~/.saitec_tui/logs/jcode-<date>.log`** for the most recent `remote bootstrap` line — it prints the `model` field that `MultiProvider.model()` returned to the client at startup. If it's `claude-opus-4-5-20251101` instead of `deepseek-v4-flash`, you're seeing the static-label / real-routing decoupling.
2. **`jcode_version` field in the `ENV_SNAPSHOT` log line** tells you which commit the running binary was built from. If it does not include `e05304a1` (or later), the user is running a stale binary.
3. Check `JCODE_OPENROUTER_MODEL` in the process env (`Get-ChildItem Env:JCODE_OPENROUTER_MODEL` in pwsh). If it's missing while a `JCODE_OPENAI_COMPAT_DEFAULT_MODEL=...` is set in the env file, fix #4 was not applied or not active.
4. `MultiProvider.openrouter` is `None` ⇒ fall back to the hardcoded `"anthropic/claude-sonnet-4"`. The `openrouter.is_some()` decision is logged at startup; cross-reference with the timing of fix #2's `apply_openai_compatible_profile_env` call.
5. For follow-up failures: if the user re-logs in via `/login base-model` and the model **still** says claude, walk through fixes #4 → #3 → #2 → #1 in order. Each one is independently insufficient.

#### Anti-patterns to avoid when touching this area

- **Do NOT** add `JCODE_OPENROUTER_MODEL` to `RUNTIME_OPENAI_COMPAT_ENV_VARS` excludes in `provider_catalog.rs:347-372`. The "clear-then-set" pattern is correct; the bug was the missing **re-set** (fix #4), not the **clear**.
- **Do NOT** call `MultiProvider::set_active_provider(Claude)` in revalidate / `handle_notify_auth_changed` paths. The auto-detection is wrong; forcing Claude makes it worse. Fix #3 (preference flag) is the right place to fix this.
- **Do NOT** silently swallow `OpenRouterProvider::new()` errors in `MultiProvider::on_auth_changed` (`src/provider/mod.rs:1397-1424`). If it fails, log loudly. Currently `MultiProvider.openrouter` is left as `Some(stale_provider)` with the wrong model; if the error is propagated, the caller can either retry or fall back to a known-good state.
- **Do NOT** call `force_apply_openai_compatible_profile_env(None)` (full env file wipe) from any revalidate / restart / logout code path. Use `clear_openai_compatible_runtime_env_keep_config` for those (preserves the env file's `JCODE_OPENAI_COMPAT_DEFAULT_MODEL` so fix #4 has something to read).
