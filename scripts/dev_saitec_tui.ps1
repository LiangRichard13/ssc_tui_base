param(
    [string]$Profile = "selfdev",
    [string]$TargetTriple = "",
    [switch]$NoBuild,
    [switch]$StopRunning,
    [switch]$PassThru
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Write-Info([string]$Message) {
    Write-Host $Message -ForegroundColor Blue
}

$RepoRoot = Split-Path -Parent $PSScriptRoot
$SupportScriptPath = Join-Path $PSScriptRoot "dev_saitec_tui_support.ps1"
. $SupportScriptPath

$buildArtifacts = Get-DevBuildArtifactPaths `
    -RepoRootPath $RepoRoot `
    -ProfileName $Profile `
    -TargetTripleName $TargetTriple

$layout = New-DevRuntimeLayout `
    -RepoRootPath $RepoRoot `
    -ProfileName $Profile `
    -TargetTripleName $TargetTriple `
    -Timestamp (Get-Date -Format "yyyyMMdd-HHmmss")

$stoppedRecordedRuntime = Stop-DevRuntimeProcess -StatePath $layout.StatePath
$stoppedCopiedRuntimeProcesses = Stop-ProcessesUnderDirectory -DirectoryPath $layout.RuntimeRoot
$stoppedBuildArtifactLocks = Stop-ProcessesByExecutablePath -ExecutablePath $buildArtifacts.ExePath

if ($stoppedRecordedRuntime) {
    Write-Info "Stopped existing recorded SAITEC dev runtime before build."
}

if ($stoppedCopiedRuntimeProcesses -gt 0) {
    Write-Info "Stopped $stoppedCopiedRuntimeProcesses copied runtime process(es) before build."
}

if ($stoppedBuildArtifactLocks -gt 0) {
    Write-Info "Stopped $stoppedBuildArtifactLocks process(es) still running from build output."
}

if ($StopRunning) {
    if ($stoppedRecordedRuntime -or $stoppedCopiedRuntimeProcesses -gt 0 -or $stoppedBuildArtifactLocks -gt 0) {
        Write-Info "Stopped existing SAITEC dev runtime."
    } else {
        Write-Info "No running SAITEC dev runtime was recorded."
    }

    if ($NoBuild) {
        return
    }
}

if (-not $NoBuild) {
    $cargoArgs = @("build")
    if ($Profile -eq "release") {
        $cargoArgs += "--release"
    } else {
        $cargoArgs += @("--profile", $Profile)
    }
    if (-not [string]::IsNullOrWhiteSpace($TargetTriple)) {
        $cargoArgs += @("--target", $TargetTriple)
    }
    $cargoArgs += @("-p", "jcode", "--bin", "jcode")

    Write-Info ("Building SAITEC dev runtime with: cargo " + ($cargoArgs -join " "))
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }
}

if (-not (Test-Path -LiteralPath $buildArtifacts.ExePath)) {
    throw "Missing build artifact: $($buildArtifacts.ExePath)"
}

if (Test-Path -LiteralPath $layout.RuntimeDir) {
    Remove-Item -LiteralPath $layout.RuntimeDir -Recurse -Force
}

New-Item -ItemType Directory -Path $layout.RuntimeDir -Force | Out-Null
Copy-Item -LiteralPath $buildArtifacts.ExePath -Destination $layout.RuntimeExe -Force
if (Test-Path -LiteralPath $buildArtifacts.PdbPath) {
    Copy-Item -LiteralPath $buildArtifacts.PdbPath -Destination $layout.RuntimePdb -Force
}

Write-Info "Launching SAITEC dev runtime from copied executable"
$process = Start-Process `
    -FilePath $layout.RuntimeExe `
    -WorkingDirectory $RepoRoot `
    -PassThru

Write-DevRuntimeState `
    -StatePath $layout.StatePath `
    -RuntimePid $process.Id `
    -RuntimeDir $layout.RuntimeDir `
    -RuntimeExe $layout.RuntimeExe

Write-Host ""
Write-Info "SAITEC dev runtime ready"
Write-Info "  source exe: $($buildArtifacts.ExePath)"
Write-Info "  runtime exe: $($layout.RuntimeExe)"
Write-Info "  pid: $($process.Id)"

if ($PassThru) {
    [pscustomobject]@{
        SourceExe = $buildArtifacts.ExePath
        RuntimeExe = $layout.RuntimeExe
        RuntimeDir = $layout.RuntimeDir
        StatePath = $layout.StatePath
        Pid = $process.Id
    }
}
