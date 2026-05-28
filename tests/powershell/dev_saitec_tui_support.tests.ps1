Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$SupportScript = Join-Path $PSScriptRoot "..\..\scripts\dev_saitec_tui_support.ps1"
. $SupportScript

Describe "New-DevRuntimeLayout" {
    It "places runtime copies under dist\\dev-saitec-tui with a timestamped run directory" {
        $repoRoot = "G:\Workspace\Project2026\JCode\jcode"
        $layout = New-DevRuntimeLayout `
            -RepoRootPath $repoRoot `
            -ProfileName "selfdev" `
            -TargetTripleName "" `
            -Timestamp "20260513-221500"

        $layout.RuntimeRoot | Should Be (Join-Path $repoRoot "dist\dev-saitec-tui")
        $layout.RuntimeDir | Should Be (Join-Path $repoRoot "dist\dev-saitec-tui\run-selfdev-20260513-221500")
        $layout.RuntimeExe | Should Be (Join-Path $repoRoot "dist\dev-saitec-tui\run-selfdev-20260513-221500\jcode.exe")
        $layout.StatePath | Should Be (Join-Path $repoRoot "dist\dev-saitec-tui\dev-runtime-state.json")
    }

    It "includes the target triple in the runtime directory name when provided" {
        $repoRoot = "G:\Workspace\Project2026\JCode\jcode"
        $layout = New-DevRuntimeLayout `
            -RepoRootPath $repoRoot `
            -ProfileName "selfdev" `
            -TargetTripleName "x86_64-pc-windows-msvc" `
            -Timestamp "20260513-221500"

        $layout.RuntimeDir | Should Be (Join-Path $repoRoot "dist\dev-saitec-tui\run-selfdev-x86_64-pc-windows-msvc-20260513-221500")
    }
}

Describe "Get-DevBuildArtifactPaths" {
    It "resolves the source exe and pdb inside the profile target directory" {
        $repoRoot = "G:\Workspace\Project2026\JCode\jcode"
        $paths = Get-DevBuildArtifactPaths `
            -RepoRootPath $repoRoot `
            -ProfileName "selfdev" `
            -TargetTripleName ""

        $paths.BuildDir | Should Be (Join-Path $repoRoot "target\selfdev")
        $paths.ExePath | Should Be (Join-Path $repoRoot "target\selfdev\jcode.exe")
        $paths.PdbPath | Should Be (Join-Path $repoRoot "target\selfdev\jcode.pdb")
    }
}

Describe "Get-DevSaitecMcpLayout" {
    It "places the managed MCP Python environment under target" {
        $repoRoot = "G:\Workspace\Project2026\JCode\jcode"
        $layout = Get-DevSaitecMcpLayout -RepoRootPath $repoRoot

        $layout.SkillsRoot | Should Be (Join-Path $repoRoot "_vendor\SAITEC-Skills")
        $layout.RequirementsPath | Should Be (Join-Path $repoRoot "_vendor\SAITEC-Skills\requirements.txt")
        $layout.ServerPath | Should Be (Join-Path $repoRoot "_vendor\SAITEC-Skills\mcp_server\server.py")
        $layout.VenvDir | Should Be (Join-Path $repoRoot "target\saitec-mcp-venv")
        $layout.PythonPath | Should Be (Join-Path $repoRoot "target\saitec-mcp-venv\Scripts\python.exe")
        $layout.RequirementsStampPath | Should Be (Join-Path $repoRoot "target\saitec-mcp-venv\.requirements.sha256")
    }
}

Describe "Resolve-DevCargoCommand" {
    It "falls back to the user's cargo.exe when cargo is not on PATH" {
        $originalPath = $env:Path
        $fallbackRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("saitec-cargo-fallback-test-" + [guid]::NewGuid().ToString("N"))
        $fallbackHome = Join-Path $fallbackRoot "home"
        $fallbackCargoDir = Join-Path $fallbackHome ".cargo\bin"
        $fallbackCargoExe = Join-Path $fallbackCargoDir "cargo.exe"
        $originalUserProfile = $env:USERPROFILE
        $originalHome = $env:HOME

        try {
            New-Item -ItemType Directory -Path $fallbackCargoDir -Force | Out-Null
            New-Item -ItemType File -Path $fallbackCargoExe -Force | Out-Null

            $env:Path = "C:\definitely-missing-from-path"
            $env:USERPROFILE = $fallbackHome
            $env:HOME = $fallbackHome

            Resolve-DevCargoCommand | Should Be $fallbackCargoExe
        } finally {
            $env:Path = $originalPath
            $env:USERPROFILE = $originalUserProfile
            $env:HOME = $originalHome

            if (Test-Path -LiteralPath $fallbackRoot) {
                Remove-Item -LiteralPath $fallbackRoot -Recurse -Force
            }
        }
    }
}

Describe "Write-DevRuntimeState" {
    It "persists runtime metadata without colliding with PowerShell built-in variables" {
        $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("saitec-dev-runtime-test-" + [guid]::NewGuid().ToString("N"))
        $statePath = Join-Path $tempRoot "dev-runtime-state.json"

        try {
            Write-DevRuntimeState `
                -StatePath $statePath `
                -RuntimePid 12345 `
                -RuntimeDir "G:\Workspace\Project2026\JCode\jcode\dist\dev-saitec-tui\run-selfdev-test" `
                -RuntimeExe "G:\Workspace\Project2026\JCode\jcode\dist\dev-saitec-tui\run-selfdev-test\jcode.exe"

            $state = Read-DevRuntimeState -StatePath $statePath
            $state.pid | Should Be 12345
            $state.runtime_exe | Should Be "G:\Workspace\Project2026\JCode\jcode\dist\dev-saitec-tui\run-selfdev-test\jcode.exe"
        } finally {
            if (Test-Path -LiteralPath $tempRoot) {
                Remove-Item -LiteralPath $tempRoot -Recurse -Force
            }
        }
    }
}
