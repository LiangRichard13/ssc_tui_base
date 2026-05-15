# SAITEC Packager Skill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a project-local `saitec-tui-packager` skill that packages SAITEC-TUI into a parameterized output directory, defaulting to `dist/saitec-tui-<timestamp>`.

**Architecture:** Keep the repository's existing `scripts/package_saitec.ps1` as the single source of truth for branded packaging, and add a thin skill-local wrapper script that resolves parameters, optionally builds, and copies the standard output into the final timestamped directory. Store the skill under `.jcode/skills` so it travels with the repo and can be invoked in project context without depending on global Codex state.

**Tech Stack:** Markdown skill metadata, PowerShell wrapper script, existing Python skill tooling (`generate_openai_yaml.py`, `quick_validate.py`), Cargo build command reuse.

---

### Task 1: Add The Project-Local Skill Skeleton

**Files:**
- Create: `G:\Workspace\Project2026\JCode\jcode\.jcode\skills\saitec-tui-packager\SKILL.md`
- Create: `G:\Workspace\Project2026\JCode\jcode\.jcode\skills\saitec-tui-packager\references\usage.md`
- Create: `G:\Workspace\Project2026\JCode\jcode\.jcode\skills\saitec-tui-packager\scripts\package_saitec_tui.ps1`

- [ ] **Step 1: Write the skill frontmatter and invocation rules**

```md
---
name: saitec-tui-packager
description: Use when packaging or exporting SAITEC-TUI from this JCode repo, especially when the user wants a packaged folder, a custom output directory, or the default dist/saitec-tui-<timestamp> output.
---
```

- [ ] **Step 2: Document the execution workflow in SKILL.md**

```md
## Workflow

1. Resolve the repo root and confirm `scripts/package_saitec.ps1` exists.
2. Resolve the final output directory:
   - default: `dist/saitec-tui-<yyyyMMdd-HHmmss>`
   - explicit `output_dir`: use exactly that directory
3. If `skip_build` is false and the release binary is missing, build `jcode.exe`.
4. Run the bundled wrapper script instead of recreating packaging commands manually.
5. Report the final packaged paths, including `saitec-tui.exe` and `install.ps1`.
```

- [ ] **Step 3: Document concrete examples in `references/usage.md`**

```md
- Default output: package into `dist/saitec-tui-<timestamp>`
- Custom folder: package into a specific `output_dir`
- Existing build only: package with `skip_build=true`
- Symbols: package with `include_debug_symbols=true`
```

- [ ] **Step 4: Commit**

```bash
git add .jcode/skills/saitec-tui-packager
git commit -m "feat: scaffold saitec packager skill"
```

### Task 2: Implement The Thin PowerShell Wrapper

**Files:**
- Modify: `G:\Workspace\Project2026\JCode\jcode\.jcode\skills\saitec-tui-packager\scripts\package_saitec_tui.ps1`
- Test: `G:\Workspace\Project2026\JCode\jcode\.jcode\skills\saitec-tui-packager\scripts\package_saitec_tui.ps1`

- [ ] **Step 1: Add parameter parsing and default timestamp logic**

```powershell
param(
    [string]$OutputDir = "",
    [string]$OutputParent = "",
    [string]$Timestamp = "",
    [string]$Profile = "release",
    [string]$TargetTriple = "",
    [switch]$IncludeDebugSymbols,
    [switch]$SkipBuild,
    [switch]$OpenOutput,
    [string]$RepoRoot = ""
)
```

- [ ] **Step 2: Resolve the repo root and validate required files**

```powershell
if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\..\..\.."))
}

$PackageScript = Join-Path $RepoRoot "scripts\package_saitec.ps1"
$CargoToml = Join-Path $RepoRoot "Cargo.toml"

if (-not (Test-Path -LiteralPath $CargoToml)) {
    throw "Repo root does not look like jcode: $RepoRoot"
}
if (-not (Test-Path -LiteralPath $PackageScript)) {
    throw "Missing packaging script: $PackageScript"
}
```

- [ ] **Step 3: Add optional build behavior and final directory sync**

```powershell
$buildExe = if ([string]::IsNullOrWhiteSpace($TargetTriple)) {
    Join-Path $RepoRoot "target\$Profile\jcode.exe"
} else {
    Join-Path $RepoRoot "target\$TargetTriple\$Profile\jcode.exe"
}

if (-not $SkipBuild -and -not (Test-Path -LiteralPath $buildExe)) {
    & cargo build --release -p jcode --bin jcode
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed"
    }
}
```

- [ ] **Step 4: Copy `dist/saitec-tui` into the resolved final output directory**

```powershell
$standardDist = Join-Path $RepoRoot "dist\saitec-tui"
if (Test-Path -LiteralPath $FinalOutputDir) {
    Remove-Item -LiteralPath $FinalOutputDir -Recurse -Force
}
New-Item -ItemType Directory -Path $FinalOutputDir -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $standardDist "*") -Destination $FinalOutputDir -Recurse -Force
```

- [ ] **Step 5: Commit**

```bash
git add .jcode/skills/saitec-tui-packager/scripts/package_saitec_tui.ps1
git commit -m "feat: add saitec packager wrapper script"
```

### Task 3: Generate Skill Metadata And Validate

**Files:**
- Create: `G:\Workspace\Project2026\JCode\jcode\.jcode\skills\saitec-tui-packager\agents\openai.yaml`
- Test: `G:\Workspace\Project2026\JCode\jcode\.jcode\skills\saitec-tui-packager`

- [ ] **Step 1: Generate `agents/openai.yaml` with deterministic interface values**

```bash
python C:\Users\H3C\.codex\skills\.system\skill-creator\scripts\generate_openai_yaml.py ^
  G:\Workspace\Project2026\JCode\jcode\.jcode\skills\saitec-tui-packager ^
  --interface display_name="SAITEC TUI Packager" ^
  --interface short_description="Package SAITEC-TUI builds with output controls" ^
  --interface default_prompt="Use $saitec-tui-packager to package SAITEC-TUI into a timestamped or custom output directory."
```

- [ ] **Step 2: Run the skill validator**

```bash
python C:\Users\H3C\.codex\skills\.system\skill-creator\scripts\quick_validate.py G:\Workspace\Project2026\JCode\jcode\.jcode\skills\saitec-tui-packager
```

- [ ] **Step 3: Smoke-check the wrapper**

```powershell
powershell -ExecutionPolicy Bypass -File .jcode\skills\saitec-tui-packager\scripts\package_saitec_tui.ps1 -SkipBuild -OutputDir dist\saitec-tui-smoke
```

- [ ] **Step 4: Commit**

```bash
git add .jcode/skills/saitec-tui-packager/agents/openai.yaml
git commit -m "chore: validate saitec packager skill"
```
