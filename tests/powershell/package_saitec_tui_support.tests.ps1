Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$SupportScript = Join-Path $PSScriptRoot "..\..\.jcode\skills\saitec-tui-packager\scripts\package_saitec_tui_support.ps1"
. $SupportScript
$PackageSupportScript = Join-Path $PSScriptRoot "..\..\scripts\package_saitec_support.ps1"
if (Test-Path -LiteralPath $PackageSupportScript) {
    . $PackageSupportScript
}

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

Describe "SAITEC MCP private resource packaging" {
    It "creates a single resource archive that expands to SAITEC-Skills" {
        Test-Path -LiteralPath $PackageSupportScript | Should Be $true

        $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("saitec-mcp-package-test-" + [guid]::NewGuid().ToString("N"))
        $sourceRoot = Join-Path $tempRoot "source\SAITEC-Skills"
        $serverDir = Join-Path $sourceRoot "mcp_server"
        $archivePath = Join-Path $tempRoot "dist\saitec-mcp.resources"
        $extractRoot = Join-Path $tempRoot "extract"

        try {
            New-Item -ItemType Directory -Path $serverDir -Force | Out-Null
            Set-Content -LiteralPath (Join-Path $serverDir "server.py") -Value "print('saitec')" -Encoding UTF8

            New-SaitecMcpResourceArchive -SourceDir $sourceRoot -DestinationPath $archivePath
            Test-Path -LiteralPath $archivePath | Should Be $true

            Expand-SaitecMcpResourceArchive -ArchivePath $archivePath -DestinationRoot $extractRoot
            Test-Path -LiteralPath (Join-Path $extractRoot "SAITEC-Skills\mcp_server\server.py") | Should Be $true
            Test-Path -LiteralPath (Join-Path (Split-Path -Parent $archivePath) "SAITEC-Skills") | Should Be $false
        } finally {
            if (Test-Path -LiteralPath $tempRoot) {
                Remove-Item -LiteralPath $tempRoot -Recurse -Force
            }
        }
    }

    It "generates installer support for private hidden MCP extraction" {
        Test-Path -LiteralPath $PackageSupportScript | Should Be $true

        $installerSupport = Get-SaitecMcpInstallerSupportScript
        $installerSupport | Should Match "Get-SaitecMcpPrivateResourceRoot"
        $installerSupport | Should Match "\.saitec-mcp"
        $installerSupport | Should Match "Expand-SaitecMcpResourceArchive"
        $installerSupport | Should Match "Set-SaitecMcpPrivateAcl"
        $installerSupport | Should Match "Hidden"
    }

    It "keeps installed private MCP files manageable by the current user" {
        Test-Path -LiteralPath $PackageSupportScript | Should Be $true

        $installerSupport = Get-SaitecMcpInstallerSupportScript
        Invoke-Expression $installerSupport

        $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("saitec-mcp-acl-test-" + [guid]::NewGuid().ToString("N"))
        $privateRoot = Join-Path $tempRoot "resources\.saitec-mcp"
        $serverDir = Join-Path $privateRoot "SAITEC-Skills\mcp_server"
        $serverFile = Join-Path $serverDir "server.py"

        try {
            New-Item -ItemType Directory -Path $serverDir -Force | Out-Null
            Set-Content -LiteralPath $serverFile -Value "print('saitec')" -Encoding UTF8

            Set-SaitecMcpPrivateAcl -Path $privateRoot

            $attributes = (Get-Item -LiteralPath $privateRoot -Force).Attributes
            ($attributes -band [System.IO.FileAttributes]::Hidden) | Should Not Be 0

            Remove-Item -LiteralPath $serverFile -Force
            Test-Path -LiteralPath $serverFile | Should Be $false
        } finally {
            if (Test-Path -LiteralPath $tempRoot) {
                $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
                if (Get-Command icacls -ErrorAction SilentlyContinue) {
                    & icacls $tempRoot /grant "$($identity):F" /T /C | Out-Null
                }
                Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
            }
        }
    }
}

Describe "SAITEC packager isolated staging" {
    It "stages the repo package support script next to the isolated package script" {
        $wrapperScript = Join-Path $PSScriptRoot "..\..\.jcode\skills\saitec-tui-packager\scripts\package_saitec_tui.ps1"
        $content = Get-Content -LiteralPath $wrapperScript -Raw

        $content | Should Match "package_saitec_support\.ps1"
        $content | Should Match "Copy-Item"
    }
}
