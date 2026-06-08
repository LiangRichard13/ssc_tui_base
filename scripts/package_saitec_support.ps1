Set-StrictMode -Version Latest

$script:SaitecMcpArchiveName = "saitec-mcp.resources"

function New-SaitecMcpResourceArchive {
    param(
        [Parameter(Mandatory = $true)][string]$SourceDir,
        [Parameter(Mandatory = $true)][string]$DestinationPath
    )

    if (-not (Test-Path -LiteralPath $SourceDir)) {
        throw "SAITEC-Skills source directory not found: $SourceDir"
    }

    $serverScript = Join-Path $SourceDir "mcp_server\server.py"
    if (-not (Test-Path -LiteralPath $serverScript)) {
        throw "SAITEC-Skills MCP server script not found: $serverScript"
    }

    $destinationDir = Split-Path -Parent $DestinationPath
    if (-not [string]::IsNullOrWhiteSpace($destinationDir)) {
        New-Item -ItemType Directory -Path $destinationDir -Force | Out-Null
    }

    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("saitec-mcp-archive-" + [guid]::NewGuid().ToString("N"))
    $stagedSkillsRoot = Join-Path $stagingRoot "SAITEC-Skills"
    $tempZip = Join-Path ([System.IO.Path]::GetTempPath()) ("saitec-mcp-archive-" + [guid]::NewGuid().ToString("N") + ".zip")

    try {
        New-Item -ItemType Directory -Path $stagingRoot -Force | Out-Null
        Copy-Item -LiteralPath $SourceDir -Destination $stagedSkillsRoot -Recurse -Force

        if (Test-Path -LiteralPath $tempZip) {
            Remove-Item -LiteralPath $tempZip -Force
        }
        Compress-Archive -LiteralPath $stagedSkillsRoot -DestinationPath $tempZip -Force

        if (Test-Path -LiteralPath $DestinationPath) {
            Remove-Item -LiteralPath $DestinationPath -Force
        }
        Move-Item -LiteralPath $tempZip -Destination $DestinationPath -Force
    } finally {
        if (Test-Path -LiteralPath $stagingRoot) {
            Remove-Item -LiteralPath $stagingRoot -Recurse -Force -ErrorAction SilentlyContinue
        }
        if (Test-Path -LiteralPath $tempZip) {
            Remove-Item -LiteralPath $tempZip -Force -ErrorAction SilentlyContinue
        }
    }
}

function Expand-SaitecMcpResourceArchive {
    param(
        [Parameter(Mandatory = $true)][string]$ArchivePath,
        [Parameter(Mandatory = $true)][string]$DestinationRoot
    )

    if (-not (Test-Path -LiteralPath $ArchivePath)) {
        throw "SAITEC MCP resource archive not found: $ArchivePath"
    }

    New-Item -ItemType Directory -Path $DestinationRoot -Force | Out-Null
    $existingSkillsRoot = Join-Path $DestinationRoot "SAITEC-Skills"
    if (Test-Path -LiteralPath $existingSkillsRoot) {
        Remove-Item -LiteralPath $existingSkillsRoot -Recurse -Force
    }

    $tempZip = Join-Path ([System.IO.Path]::GetTempPath()) ("saitec-mcp-expand-" + [guid]::NewGuid().ToString("N") + ".zip")
    try {
        Copy-Item -LiteralPath $ArchivePath -Destination $tempZip -Force
        Expand-Archive -LiteralPath $tempZip -DestinationPath $DestinationRoot -Force
    } finally {
        if (Test-Path -LiteralPath $tempZip) {
            Remove-Item -LiteralPath $tempZip -Force -ErrorAction SilentlyContinue
        }
    }

    $serverScript = Join-Path $existingSkillsRoot "mcp_server\server.py"
    if (-not (Test-Path -LiteralPath $serverScript)) {
        throw "SAITEC MCP resource archive did not contain SAITEC-Skills\mcp_server\server.py"
    }
}

function Get-SaitecMcpInstallerSupportScript {
    return @'
function Get-SaitecMcpPrivateResourceRoot([string]$InstallDir) {
    $installRoot = Split-Path -Parent $InstallDir
    if ([string]::IsNullOrWhiteSpace($installRoot)) {
        $installRoot = $InstallDir
    }
    return Join-Path (Join-Path $installRoot "resources") ".saitec-mcp"
}

function Expand-SaitecMcpResourceArchive {
    param(
        [Parameter(Mandatory = $true)][string]$ArchivePath,
        [Parameter(Mandatory = $true)][string]$DestinationRoot
    )

    if (-not (Test-Path -LiteralPath $ArchivePath)) {
        throw "SAITEC MCP resource archive not found: $ArchivePath"
    }

    New-Item -ItemType Directory -Path $DestinationRoot -Force | Out-Null
    $existingSkillsRoot = Join-Path $DestinationRoot "SAITEC-Skills"
    if (Test-Path -LiteralPath $existingSkillsRoot) {
        Remove-Item -LiteralPath $existingSkillsRoot -Recurse -Force
    }

    $tempZip = Join-Path ([System.IO.Path]::GetTempPath()) ("saitec-mcp-expand-" + [guid]::NewGuid().ToString("N") + ".zip")
    try {
        Copy-Item -LiteralPath $ArchivePath -Destination $tempZip -Force
        Expand-Archive -LiteralPath $tempZip -DestinationPath $DestinationRoot -Force
    } finally {
        if (Test-Path -LiteralPath $tempZip) {
            Remove-Item -LiteralPath $tempZip -Force -ErrorAction SilentlyContinue
        }
    }

    $serverScript = Join-Path $existingSkillsRoot "mcp_server\server.py"
    if (-not (Test-Path -LiteralPath $serverScript)) {
        throw "SAITEC MCP resource archive did not contain SAITEC-Skills\mcp_server\server.py"
    }
}

function Set-SaitecMcpPrivateAcl([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }

    $item = Get-Item -LiteralPath $Path -Force
    $item.Attributes = $item.Attributes -bor [System.IO.FileAttributes]::Hidden

    $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
    if (Get-Command icacls -ErrorAction SilentlyContinue) {
        & icacls $Path /inheritance:r /grant:r "$($identity):(OI)(CI)F" /grant:r "SYSTEM:(OI)(CI)F" /C | Out-Null
    }
}
'@
}
