param(
    [string]$Profile = "release",
    [string]$TargetTriple = "",
    [switch]$IncludeDebugSymbols
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Write-Info([string]$Message) {
    Write-Host $Message -ForegroundColor Blue
}

$RepoRoot = Split-Path -Parent $PSScriptRoot
$BuildDir = if ([string]::IsNullOrWhiteSpace($TargetTriple)) {
    Join-Path $RepoRoot "target\$Profile"
} else {
    Join-Path $RepoRoot "target\$TargetTriple\$Profile"
}

$SourceExe = Join-Path $BuildDir "jcode.exe"
$SourcePdb = Join-Path $BuildDir "jcode.pdb"
$DistDir = Join-Path $RepoRoot "dist\saitec-tui"
$BrandedExe = Join-Path $DistDir "saitec-tui.exe"
$BrandedPdb = Join-Path $DistDir "saitec-tui.pdb"
$InstallScriptPath = Join-Path $DistDir "install.ps1"
$LogoAsset = Join-Path $RepoRoot "SAITEC_logo.png"
$PackagedLogoAsset = Join-Path $DistDir "SAITEC_logo.png"

if (-not (Test-Path -LiteralPath $SourceExe)) {
    throw "Missing build artifact: $SourceExe"
}

if (Test-Path -LiteralPath $DistDir) {
    Remove-Item -LiteralPath $DistDir -Recurse -Force
}

New-Item -ItemType Directory -Path $DistDir -Force | Out-Null
Copy-Item -LiteralPath $SourceExe -Destination $BrandedExe -Force

if (Test-Path -LiteralPath $LogoAsset) {
    Copy-Item -LiteralPath $LogoAsset -Destination $PackagedLogoAsset -Force
}

if ($IncludeDebugSymbols -and (Test-Path -LiteralPath $SourcePdb)) {
    Copy-Item -LiteralPath $SourcePdb -Destination $BrandedPdb -Force
}

# Keep the packaged installer self-contained so the SAITEC bundle can be copied
# around without depending on the repo-level JCode installer behavior.
$installScript = @'
param(
    [string]$InstallDir,
    [switch]$SkipPathUpdate
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Write-Info([string]$Message) {
    Write-Host $Message -ForegroundColor Blue
}

if (-not $InstallDir) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "saitec-tui\bin"
}

$SourceExe = Join-Path $PSScriptRoot "saitec-tui.exe"
$TargetExe = Join-Path $InstallDir "saitec-tui.exe"
$SourceLogo = Join-Path $PSScriptRoot "SAITEC_logo.png"
$TargetLogo = Join-Path $InstallDir "SAITEC_logo.png"

if (-not (Test-Path -LiteralPath $SourceExe)) {
    throw "Missing packaged executable: $SourceExe"
}

New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
Copy-Item -LiteralPath $SourceExe -Destination $TargetExe -Force
if (Test-Path -LiteralPath $SourceLogo) {
    Copy-Item -LiteralPath $SourceLogo -Destination $TargetLogo -Force
}

if (-not $SkipPathUpdate) {
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ([string]::IsNullOrEmpty($UserPath)) {
        [Environment]::SetEnvironmentVariable("Path", $InstallDir, "User")
        Write-Info "Added $InstallDir to user PATH"
    } elseif ($UserPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$InstallDir;$UserPath", "User")
        Write-Info "Added $InstallDir to user PATH"
    }
}

$env:Path = "$InstallDir;$env:Path"

Write-Host ""
Write-Info "SAITEC-TUI installed successfully."
Write-Info "  launcher: $TargetExe"
Write-Host ""

if (Get-Command saitec-tui -ErrorAction SilentlyContinue) {
    Write-Info "Run 'saitec-tui' to get started."
} else {
    Write-Host "  Open a new terminal window, then run:"
    Write-Host ""
    Write-Host "    saitec-tui" -ForegroundColor Green
}
'@

$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($InstallScriptPath, $installScript, $Utf8NoBom)

Write-Host ""
Write-Info "SAITEC package ready at $DistDir"
Write-Info "  exe: $BrandedExe"
Write-Info "  installer: $InstallScriptPath"
if ($IncludeDebugSymbols -and (Test-Path -LiteralPath $BrandedPdb)) {
    Write-Info "  symbols: $BrandedPdb"
}
