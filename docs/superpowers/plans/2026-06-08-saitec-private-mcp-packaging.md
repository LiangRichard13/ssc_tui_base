# SAITEC Private MCP Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Package SAITEC-TUI with a private installed SAITEC MCP resource bundle without packaging API keys.

**Architecture:** Add a PowerShell packager helper that archives `_vendor/SAITEC-Skills` as `saitec-mcp.resources`, update the generated installer to extract the archive into a hidden current-user directory, and update Rust MCP bootstrap to resolve that private installed path first. Keep API key handling unchanged and runtime-only.

**Tech Stack:** PowerShell packaging/installer scripts, Rust MCP bootstrap, existing PowerShell and Rust tests.

---

### Task 1: Private MCP Resource Resolution

**Files:**
- Modify: `src/saitec/mcp.rs`
- Test: `src/mcp/protocol_tests.rs`

- [x] **Step 1: Write failing Rust tests**

Add tests that set `LOCALAPPDATA` to a temporary directory, create `saitec-tui/resources/.saitec-mcp/SAITEC-Skills/mcp_server/server.py`, and assert `ensure_bootstrap()` writes an MCP entry pointing there.

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test test_saitec_bootstrap_prefers_private_installed_mcp_resources -- --exact`
Expected: FAIL because the resolver does not search the private installed directory yet.

- [x] **Step 3: Implement resolver support**

Add a Windows-aware private resource candidate based on `LOCALAPPDATA`, then keep existing `SAITEC_SKILLS_ROOT`, executable-relative, and source-tree fallbacks.

- [x] **Step 4: Verify Rust tests pass**

Run: `cargo test test_saitec_bootstrap_prefers_private_installed_mcp_resources -- --exact`
Expected: PASS.

### Task 2: Packager Archive And Installer Extraction

**Files:**
- Modify: `scripts/package_saitec.ps1`
- Test: `tests/powershell/package_saitec_tui_support.tests.ps1`

- [x] **Step 1: Write failing PowerShell tests**

Add tests for creating a hidden MCP resource archive from a fake `SAITEC-Skills` tree and for installer text containing extraction, hiding, and ACL hardening logic.

- [x] **Step 2: Run tests to verify they fail**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -Command "Invoke-Pester tests/powershell/package_saitec_tui_support.tests.ps1"`
Expected: FAIL because package helper support does not exist yet.

- [x] **Step 3: Implement packaging support**

Create `saitec-mcp.resources` with `Compress-Archive`; update generated `install.ps1` to extract it into `%LOCALAPPDATA%\saitec-tui\resources\.saitec-mcp`, hide the directory, and apply current-user-only ACLs.

- [x] **Step 4: Verify PowerShell tests pass**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -Command "Invoke-Pester tests/powershell/package_saitec_tui_support.tests.ps1"`
Expected: PASS.

### Task 3: Smoke Package Verification

**Files:**
- Modify: `.jcode/skills/saitec-tui-packager/scripts/package_saitec_tui.ps1` if needed
- Verify: `dist/saitec-tui-*`

- [x] **Step 1: Run package smoke**

Run: `powershell -ExecutionPolicy Bypass -File .jcode\skills\saitec-tui-packager\scripts\package_saitec_tui.ps1 -SkipBuild -OutputDir dist\saitec-tui-private-mcp-smoke`
Expected: output contains `saitec-mcp.resources`, not `SAITEC-Skills/`.

- [x] **Step 2: Run final build**

Run: `cargo build -p jcode --bin jcode`
Expected: PASS, or use `scripts/remote_build.sh` if the local machine cannot build.

- [x] **Step 3: Run dev debug script**

Run: `powershell -ExecutionPolicy Bypass -File scripts/dev_saitec_tui.ps1`
Expected: TUI starts for inspection.

- [x] **Step 4: Commit and push**

Commit the spec, plan, tests, and implementation. Push the branch when verification completes.
