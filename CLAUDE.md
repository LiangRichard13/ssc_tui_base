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

### Login Flow Architecture

Two-layer login system in `src/auth/`, `src/cli/login.rs`, `src/tui/login_picker.rs`:

- **SAITEC platform login**: Email/phone + password → `POST /api/v1/auth/login` → get JWT → `POST /api/v1/api-keys` → creates a business API Key → stored as `SaitecSession` struct
- **Base model login**: Multiple auth methods dispatched by `LoginProviderTarget`:
  - OAuth + PKCE (Claude, OpenAI/Codex, Gemini, Antigravity) — generates verifier+SHA256 challenge, binds local callback server, exchanges code for tokens
  - API key input (OpenAI, OpenRouter, Bedrock, Cursor) — secret prompt, saved to `.env` files
  - Device code flow (GitHub Copilot) — `device_code` → poll → token
  - Form-based (SAITEC) — email/phone + password
- TUI uses `PendingLogin` state machine enum to track in-progress login steps
- `AuthStatus` cached with 30s TTL (`check()`) and 5s TTL (`check_fast()`), invalidated after auth changes

### SAITEC Credential Storage

- **`~/.saitec_tui/auth.json`** — main session file (`SaitecSession` struct with `api_key`, `auth_token`, `user_id`, `email`, `display_name`, `api_key_id`, etc.)
- **`~/.saitec_tui/saitec.env`** — env bridge file, stores `SAITEC_API_KEY=<key>` for MCP subprocess injection
- **`~/.saitec_tui/mcp.json`** — MCP server config (Python command + args, no API key persisted on disk)
- API key flows to MCP via `runtime_api_key()` (`src/saitec/mcp.rs:113`): reads `configured_api_key()` (from `saitec.env`) first, falls back to `load_session()` (from `auth.json`)
- `clear_session()` on logout: deletes `auth.json`, clears `SAITEC_API_KEY` from `saitec.env`, removes process env var

### SAITEC Platform Integration

- SAITEC-Skills MCP service handles detection/evaluation task dispatch
- Skills stored in `SAITEC-Skills/` (external resource directory, resolved at runtime from `_vendor/`, `resources/`, or `SAITEC_SKILLS_ROOT` env var)
- Task flow: upload file → get `storage_uri` → create task → poll → download results
- Default API endpoints: `http://101.133.153.37:8080` (overridable via `CORE_API_BASE`, `SAITEC_AUTH_BASE`, `SAITEC_API_BASE` env vars)

### Target Platforms

- Windows x64 (primary), Windows ARM64
- Linux x86_64, macOS aarch64
- Mobile simulator (iOS via `jcode-mobile-core` + `jcode-mobile-sim`)
