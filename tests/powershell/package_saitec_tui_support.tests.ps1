Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$SupportScript = Join-Path $PSScriptRoot "..\..\.jcode\skills\saitec-tui-packager\scripts\package_saitec_tui_support.ps1"
. $SupportScript

Describe "Test-ShouldUseIsolatedCargoTargetDir" {
    It "returns true for a locked target executable build failure" {
        $buildExe = "G:\Workspace\Project2026\JCode\jcode\target\release\jcode.exe"
        $cargoOutput = @'
error: failed to remove file `G:\Workspace\Project2026\JCode\jcode\target\release\jcode.exe`

Caused by:
  拒绝访问。 (os error 5)
'@

        Test-ShouldUseIsolatedCargoTargetDir -CargoOutput $cargoOutput -BuildExePath $buildExe | Should Be $true
    }

    It "returns false for unrelated cargo failures" {
        $buildExe = "G:\Workspace\Project2026\JCode\jcode\target\release\jcode.exe"
        $cargoOutput = @'
error[E0432]: unresolved import `missing`
'@

        Test-ShouldUseIsolatedCargoTargetDir -CargoOutput $cargoOutput -BuildExePath $buildExe | Should Be $false
    }
}
