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
