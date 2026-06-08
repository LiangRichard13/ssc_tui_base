# SAITEC Private MCP Packaging Design

## Goal

Package SAITEC-TUI with its bound `SAITEC-Skills` MCP assets while avoiding a plain readable `SAITEC-Skills` directory in the distributed package. Installed assets should live in a current-user private hidden location. API keys and login credentials must remain outside the package.

## Chosen Approach

The branded packaging script creates a single MCP resource archive from `_vendor/SAITEC-Skills` and places it in the packaged folder. The packaged installer extracts that archive under `%LOCALAPPDATA%\saitec-tui\resources\.saitec-mcp\SAITEC-Skills`, marks the directory hidden, and applies current-user-only ACLs on Windows. After install, the TUI bootstrap resolves this private resource root before falling back to existing source-tree and release-layout locations.

This is a practical hiding and packaging boundary, not a cryptographic DRM boundary. The current user must be able to execute the Python MCP server, so the current user can ultimately access the extracted files. The goal is to keep the MCP out of obvious package contents and block other local users from reading the installed assets.

## Runtime Flow

1. `scripts/package_saitec.ps1` verifies `_vendor/SAITEC-Skills` exists.
2. The script writes `saitec-mcp.resources` as an archive of that directory.
3. The generated `install.ps1` copies the executable and logo, extracts `saitec-mcp.resources` to `%LOCALAPPDATA%\saitec-tui\resources\.saitec-mcp`, hides that directory, and restricts ACLs to the current user.
4. `src/saitec/mcp.rs` checks `%LOCALAPPDATA%\saitec-tui\resources\.saitec-mcp\SAITEC-Skills` when resolving the MCP server script.
5. The existing MCP runtime env injection continues to add `SAITEC_API_KEY`, `CORE_API_BASE`, and `SAITEC_TUI_HOME` only at runtime.

## Non-Goals

- Do not package `auth.json`, `*.env`, Windows environment variables, logs, or sessions.
- Do not rewrite the Python MCP into Rust.
- Do not claim current-user-proof secrecy for Python source files that must execute locally.

## Verification

- PowerShell packaging tests cover archive creation and installer text.
- Rust MCP tests cover private installed resource root resolution.
- A packaging smoke test verifies the output contains `saitec-mcp.resources` and not a plain `SAITEC-Skills` directory.
