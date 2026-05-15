---
name: saitec-tui-packager
description: Use when packaging or exporting SAITEC-TUI from this JCode repo, especially when the user wants a packaged folder, a custom output directory, or the default timestamped output under dist.
---

# SAITEC TUI Packager

Use this skill when the task is to package the SAITEC-branded TUI from this repository.

## When To Use

Trigger this skill when the user wants any of the following:

- package SAITEC-TUI
- export a release folder
- choose where the packaged output goes
- create the default timestamped bundle under `dist/`
- include debug symbols in the packaged folder

## Default Behavior

If the user does not specify an output directory, package to:

`dist/saitec-tui-<yyyyMMdd-HHmmss>`

The wrapper script keeps the repository's existing `scripts/package_saitec.ps1` as the source of truth, then copies the standard packaged output into the final timestamped directory.

## Parameters

Use these parameter names when calling the wrapper script:

- `output_dir`: exact final output directory
- `output_parent`: parent directory where the wrapper should create `saitec-tui-<timestamp>`
- `timestamp`: override the default generated timestamp
- `profile`: build profile, default `release`
- `target_triple`: optional cargo target triple
- `include_debug_symbols`: include `.pdb` when available
- `skip_build`: do not build first; require an existing `jcode.exe`
- `open_output`: open the final output folder after packaging

## Workflow

1. Confirm the task is happening in this JCode repo and prefer the bundled wrapper script over rebuilding commands manually.
2. Resolve the final output directory:
   - explicit `output_dir`: use it directly
   - explicit `output_parent`: create `saitec-tui-<timestamp>` inside it
   - otherwise: use `dist/saitec-tui-<timestamp>`
3. If `skip_build` is false and the expected `jcode.exe` is missing, let the wrapper build it.
4. Run:

```powershell
powershell -ExecutionPolicy Bypass -File .jcode\skills\saitec-tui-packager\scripts\package_saitec_tui.ps1
```

5. Report the final packaged output paths, especially:
   - final directory
   - `saitec-tui.exe`
   - `install.ps1`
   - whether `.pdb` is included

## Guardrails

- Do not reimplement the repository's packaging logic if the wrapper script can handle the request.
- Prefer `output_dir` when the user gave an exact destination.
- Prefer `output_parent` when the user only cares about the parent folder and still wants a timestamped child directory.
- If packaging fails because the build artifact is missing, either rerun without `skip_build` or explain that `target/<profile>/jcode.exe` was not present.

## References

For concrete invocation examples, read [references/usage.md](references/usage.md).
