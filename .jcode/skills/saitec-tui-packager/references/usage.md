# Usage

## Default Output

Use the wrapper with no output arguments to produce:

`dist/saitec-tui-<yyyyMMdd-HHmmss>`

Example:

```powershell
powershell -ExecutionPolicy Bypass -File .jcode\skills\saitec-tui-packager\scripts\package_saitec_tui.ps1
```

## Exact Output Directory

Use `-OutputDir` when the final directory should be exact.

```powershell
powershell -ExecutionPolicy Bypass -File .jcode\skills\saitec-tui-packager\scripts\package_saitec_tui.ps1 -OutputDir G:\Builds\saitec-demo
```

## Parent Directory Plus Timestamp

Use `-OutputParent` when the user wants a parent folder but still wants the wrapper to create the timestamped child directory.

```powershell
powershell -ExecutionPolicy Bypass -File .jcode\skills\saitec-tui-packager\scripts\package_saitec_tui.ps1 -OutputParent G:\Builds
```

## Existing Build Only

Use `-SkipBuild` when `target/<profile>/jcode.exe` already exists and you do not want the wrapper to trigger Cargo.

```powershell
powershell -ExecutionPolicy Bypass -File .jcode\skills\saitec-tui-packager\scripts\package_saitec_tui.ps1 -SkipBuild
```

## Include Symbols

Use `-IncludeDebugSymbols` to copy `saitec-tui.pdb` when the packaged build generated it.

```powershell
powershell -ExecutionPolicy Bypass -File .jcode\skills\saitec-tui-packager\scripts\package_saitec_tui.ps1 -IncludeDebugSymbols
```
