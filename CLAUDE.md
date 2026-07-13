# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```powershell
cargo check                           # quick check (fastest)
cargo build                           # debug build
cargo build --release                 # release build
cargo test                            # all tests
cargo test <name> -- --nocapture      # single unit test
cargo test --test e2e <name> -- --exact --nocapture  # single e2e test
cargo test -p jcode-core              # specific crate
cargo fmt --all -- --check            # format check
cargo clippy --all-targets --all-features -- -D warnings
.\scripts\dev_saitec_tui.ps1 [-Profile selfdev]  # build dev/selfdev
.\scripts\dev_saitec_tui.ps1 -StopRunning -NoBuild  # stop dev instance
cargo build --release; .\scripts\package_saitec.ps1  # package dist
scripts/remote_build.sh               # remote build (low resources)
```

E2E tests: `tests/e2e/` (mock provider, no API calls). Modules: `ambient`, `binary_integration`, `burst_spawn`, `provider_behavior`, `safety`, `session_flow`, `transport`, `windows_lifecycle`.
Unit tests: inline `#[cfg(test)]` in each source (~64 locations).
Budget scripts in `scripts/` (code size, panics, test size, warnings) — run in CI.

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

Two-layer login system (`src/auth/`, `src/cli/login.rs`, `src/tui/login_picker.rs`):
- **SAITEC**: email/phone + password → JWT → API key → `SaitecSession`
- **Base model**: OAuth+PKCE (Claude, OpenAI/Codex, Gemini, Antigravity), API key input (OpenAI, OpenRouter, Bedrock, Cursor), device code (Copilot), form (SAITEC), openai-compatible (key + model)
- `PendingLogin` variants: `StartupGuide`, `SaitecForm`, `ClaudeAccount`/`OpenAiAccount`/`Antigravity`/`Gemini` (OAuth), `ApiKeyProfile`, `OpenAiCompatibleApiBase`/`OpenAiCompatibleModelName`, `CursorApiKey`/`Copilot`
- `AuthStatus`: 30s TTL (`check()`), 5s TTL (`check_fast()`)

## Startup Guide System

`PendingLogin::StartupGuide` overlay on branded splash when credentials missing:
- **Setup mode** (no base model): blocking, user must configure one
- **Reminder mode** (SAITEC missing only): skippable via "Skip SAITEC" button
- Restored via `restore_startup_guide_if_needed()` when picker cancelled

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

### SAITEC-Skills vendor sync

`_vendor/SAITEC-Skills/` is a manually maintained git-tracked vendor copy (bundled by `package_saitec.ps1` & `release.yml`).

**Key facts**: SAITEC-TUI adds two helper modules upstream lacks: `auth_headers.py` (auth.json fallback) and `http_errors.py` (richer error body). Every `*_tools.py` imports both. `tests/` is vendor-only; upstream has `test_data/`.

**Sync**: clone upstream at `C:\Users\Administrator\Desktop\projects\SAITEC-Skills`. Per file: read upstream → read vendor → selectively copy. Translate `resp.raise_for_status()` → `raise_for_status_with_body(resp)`. Port: 8080. Verify: `python -m py_compile` on all .py files, `cargo check`, `diff -rq` for expected diffs only.

**Anti-patterns**: DO NOT `cp -f` (loses helper imports). DO NOT delete `auth_headers.py`/`http_errors.py`. DO NOT trust first read — diff explicitly. DO NOT make `"""..."""` spanning edits.

### OpenAI-compatible: config vs credentials env hygiene

**Config** (survive logout): `JCODE_OPENAI_COMPAT_API_BASE`, `JCODE_OPENAI_COMPAT_DEFAULT_MODEL`, `JCODE_OPENAI_COMPAT_API_KEY_NAME`, `JCODE_OPENAI_COMPAT_ENV_FILE`.
**Credentials** (cleared on logout): the API key, ZAI/ZHIPU linked keys, `JCODE_OPENAI_COMPAT_LOCAL_ENABLED`.
**Runtime** (process-env only): all `JCODE_OPENROUTER_*` vars, named-profile guards, 4 `JCODE_OPENAI_COMPAT_*` overrides.

Env file: `AppData/Roaming/jcode/openai-compatible.env` (NOT `~/.jcode/` or `~/.saitec_tui/`). Its `.bak` is pre-write snapshot — compare mtimes when debugging config loss.

**Anti-pattern**: never call `force_apply_openai_compatible_profile_env(None)` from logout/activation-rollback paths — it wipes the env file's config. Use `clear_openai_compatible_runtime_env_keep_config()` instead.


### AuthTest deadlock from stale `auth-validation.json`

**Symptom** (fixed `b18a4c17`): pressing `R` in login picker shows `validation failed (just now)` with no detail. Read `~/.saitec_tui/auth-validation.json` for actual error.

**Root cause**: stale `success: false` row → `state_for_provider` returns `Expired` → probe short-circuits before smoke runs → new failure written, locking state in.

**Fix** (`src/cli/auth_test/probes.rs`): for `OpenAiCompatible` targets, bypass stale `Expired` by checking `openai_compatible_profile_is_configured()` directly.

### MCP `notifications/initialized`: JSON-RPC notification must not have an `id` field

**Issue**: `McpClient::initialize()` sent `{"jsonrpc":"2.0","id":0,"method":"notifications/initialized"}` — the `id:0` field violates JSON-RPC 2.0 (notifications must have no `id`). This was rejected by strict Pydantic validation in the SAITEC-Skills Python MCP server (FastMCP), causing `PingRequest.method: Input should be 'ping', input_value='notifications/initialized'` warnings.

**Fix** (`src/mcp/client.rs:296-301`): build the notification payload with `serde_json::json!` and omit the `id` field entirely.

**Root cause chain** (3 bugs, fixed in one PR):
1. notifications/initialized with id:0 → Python MCP rejects → tools list may be incomplete
2. `RemoteConnection::next_event` (src/tui/backend.rs:808-819) immediately disconnects on ANY JSON parse failure → reconnect storm
3. Wire NDJSON corruption from large MCP tool results on Windows named pipes → triggers bug 2

**Fixes applied** (`fix/mcp-notification-id`):
- Fix 1: notifications/initialized without `id` field (src/mcp/client.rs)
- Fix 2: skip bad NDJSON lines up to 10 consecutive errors instead of immediate disconnect (src/tui/backend.rs)
- Fix 3: debug_assert! in encode_event to catch internal newlines (crates/jcode-protocol/src/lib.rs:1968)
- Fix 4: `#[serde(other)] Unknown` variant on ServerEvent for forward compat (crates/jcode-protocol/src/lib.rs:1195)

**Key takeaway**: the "Unknown tool" errors after reconnect were NOT from MCP — `register_mcp_tools` in `handle_subscribe` reacquires pool handles every time. They were from the AI model calling tools without the `mcp__` prefix after the disconnect interrupted its execution context.

### OpenAI-compatible: 200K / `anthropic/claude-sonnet-4` regression

**Symptom** (fixed `e05304a1`): context bar shows 200K, requests fail with `400: ... passed anthropic/claude-sonnet-4` after restart/revalidate.

**4 root causes** (all in `e05304a1`):

1. **Hardcoded fallback** (`src/provider/mod.rs:732-735`): `unwrap_or("anthropic/claude-sonnet-4")` when `openrouter` is `None`. Fix: `OPENAI_COMPAT_MODEL_CONTEXT_LIMITS` table in `jcode-provider-core` covering DeepSeek, Kimi, GLM, Qwen.

2. **Server bootstrap missing env vars** (`src/provider/startup.rs:87-99`): `has_openrouter_creds` was `false` at bootstrap. Fix: scan env files and `apply_openai_compatible_profile_env` before the lookup.

3. **`auto_default_provider` priority** (`crates/jcode-provider-core/src/selection.rs`): `OpenRouter` was after Claude/OpenAI. Fix: added `prefer_openai_compatible` flag.

4. **`JCODE_OPENROUTER_MODEL` cleared but not re-set** (`src/provider_catalog.rs:444-450`): `apply_openai_compatible_profile_env_impl` cleared it but never re-applied. Fix: re-apply from `resolved.default_model` after the clear.

**Anti-patterns**: Do NOT add `JCODE_OPENROUTER_MODEL` to excluded vars (clear-then-set is correct). Do NOT call `MultiProvider::set_active_provider(Claude)` in revalidate paths. Do NOT call `force_apply_openai_compatible_profile_env(None)` from revalidate/restart/logout.

### Config.toml named provider: `JCODE_OPENROUTER_MODEL` symmetric cleanup

**Issue** (fixed after `e05304a1`): config-toml provider falls back to `anthropic/claude-sonnet-4` when `default_model` absent in `[providers.xxx]`.

**Fix** (`src/provider_catalog.rs:531-540`): explicit `remove_var("JCODE_OPENROUTER_MODEL")` in `else` branch when no `default_model`. Tests: `named_provider_profile_env_*_model_*` in `provider_catalog_tests.rs`.

**Note**: other native providers (Claude, OpenAI, Bedrock, Copilot, Cursor, Antigravity, Gemini) don't use the `apply_*_env` pattern and are unaffected.
