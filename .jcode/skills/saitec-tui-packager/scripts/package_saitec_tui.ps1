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

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Write-Info([string]$Message) {
    Write-Host $Message -ForegroundColor Blue
}

$SupportScriptPath = Join-Path $PSScriptRoot "package_saitec_tui_support.ps1"
. $SupportScriptPath

function Invoke-NativeCommandWithCapturedOutput {
    param(
        [string]$Executable,
        [string[]]$Arguments,
        [string]$WorkingDirectory
    )

    $stdoutFile = Join-Path ([System.IO.Path]::GetTempPath()) ("saitec-packager-stdout-" + [System.Guid]::NewGuid().ToString("N") + ".log")
    $stderrFile = Join-Path ([System.IO.Path]::GetTempPath()) ("saitec-packager-stderr-" + [System.Guid]::NewGuid().ToString("N") + ".log")
    try {
        $process = Start-Process `
            -FilePath $Executable `
            -ArgumentList $Arguments `
            -WorkingDirectory $WorkingDirectory `
            -RedirectStandardOutput $stdoutFile `
            -RedirectStandardError $stderrFile `
            -NoNewWindow `
            -Wait `
            -PassThru

        $stdoutLines = if (Test-Path -LiteralPath $stdoutFile) {
            Get-Content -LiteralPath $stdoutFile
        } else {
            @()
        }
        $stderrLines = if (Test-Path -LiteralPath $stderrFile) {
            Get-Content -LiteralPath $stderrFile
        } else {
            @()
        }
        $outputLines = @($stdoutLines) + @($stderrLines)

        return [pscustomobject]@{
            ExitCode = $process.ExitCode
            OutputLines = @($outputLines)
            OutputText = (@($outputLines) | ForEach-Object { [string]$_ }) -join [Environment]::NewLine
        }
    } finally {
        Remove-Item -LiteralPath $stdoutFile -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $stderrFile -Force -ErrorAction SilentlyContinue
    }
}

function Resolve-RepoRoot {
    param([string]$ExplicitRepoRoot)

    if (-not [string]::IsNullOrWhiteSpace($ExplicitRepoRoot)) {
        return [System.IO.Path]::GetFullPath($ExplicitRepoRoot)
    }

    return [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\..\..\.."))
}

function Resolve-FinalOutputDir {
    param(
        [string]$RepoRootPath,
        [string]$ExplicitOutputDir,
        [string]$ExplicitOutputParent,
        [string]$ResolvedTimestamp
    )

    if (-not [string]::IsNullOrWhiteSpace($ExplicitOutputDir)) {
        return [System.IO.Path]::GetFullPath($ExplicitOutputDir)
    }

    if (-not [string]::IsNullOrWhiteSpace($ExplicitOutputParent)) {
        $parent = [System.IO.Path]::GetFullPath($ExplicitOutputParent)
        return Join-Path $parent ("saitec-tui-" + $ResolvedTimestamp)
    }

    return Join-Path $RepoRootPath "dist\saitec-tui-$ResolvedTimestamp"
}

function Invoke-RepoPackagerIsolated {
    param(
        [string]$PackageScriptPath,
        [string]$RepoRootPath,
        [string]$ProfileName,
        [string]$TargetTripleName,
        [bool]$WantDebugSymbols
    )

    $stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("saitec-tui-packager-" + [System.Guid]::NewGuid().ToString("N"))
    $stagingDist = Join-Path $stagingRoot "dist"
    $stagingOutput = Join-Path $stagingDist "saitec-tui"
    $patchedScript = Join-Path $stagingRoot "package_saitec_isolated.ps1"

    New-Item -ItemType Directory -Path $stagingRoot -Force | Out-Null

    $rawScript = Get-Content -LiteralPath $PackageScriptPath -Raw
    $defaultRepoRootLine = '$RepoRoot = Split-Path -Parent $PSScriptRoot'
    $defaultDistLine = '$DistDir = Join-Path $RepoRoot "dist\saitec-tui"'
    $patchedRepoRootLine = '$RepoRoot = "' + ($RepoRootPath -replace '\\', '\\') + '"'
    $patchedDistLine = '$DistDir = "' + ($stagingOutput -replace '\\', '\\') + '"'

    if (-not $rawScript.Contains($defaultRepoRootLine)) {
        throw "Could not patch packaging script repo root: $PackageScriptPath"
    }
    if (-not $rawScript.Contains($defaultDistLine)) {
        throw "Could not patch packaging script output directory: $PackageScriptPath"
    }

    $rawScript = $rawScript.Replace($defaultRepoRootLine, $patchedRepoRootLine)
    $rawScript = $rawScript.Replace($defaultDistLine, $patchedDistLine)
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($patchedScript, $rawScript, $utf8NoBom)

    $packageArgs = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", $patchedScript,
        "-Profile", $ProfileName
    )
    if (-not [string]::IsNullOrWhiteSpace($TargetTripleName)) {
        $packageArgs += @("-TargetTriple", $TargetTripleName)
    }
    if ($WantDebugSymbols) {
        $packageArgs += "-IncludeDebugSymbols"
    }

    try {
        $packageOutput = & powershell @packageArgs
        if ($null -ne $packageOutput) {
            foreach ($line in @($packageOutput)) {
                if (-not [string]::IsNullOrWhiteSpace([string]$line)) {
                    Write-Host $line
                }
            }
        }
        if ($LASTEXITCODE -ne 0) {
            throw "Repo packaging script failed with exit code $LASTEXITCODE"
        }
        if (-not (Test-Path -LiteralPath $stagingOutput)) {
            throw "Expected staged packaged output was not created: $stagingOutput"
        }
        return $stagingOutput
    } catch {
        if (Test-Path -LiteralPath $stagingRoot) {
            Remove-Item -LiteralPath $stagingRoot -Recurse -Force -ErrorAction SilentlyContinue
        }
        throw
    }
}

function Invoke-CargoBuildWithFallback {
    param(
        [string]$RepoRootPath,
        [string]$ProfileName,
        [string]$TargetTripleName
    )

    $cargoArgs = @("build")
    if ($ProfileName -eq "release") {
        $cargoArgs += "--release"
    } else {
        $cargoArgs += @("--profile", $ProfileName)
    }
    if (-not [string]::IsNullOrWhiteSpace($TargetTripleName)) {
        $cargoArgs += @("--target", $TargetTripleName)
    }
    $cargoArgs += @("-p", "jcode", "--bin", "jcode")

    $primaryBuildExe = if ([string]::IsNullOrWhiteSpace($TargetTripleName)) {
        Join-Path $RepoRootPath "target\$ProfileName\jcode.exe"
    } else {
        Join-Path $RepoRootPath "target\$TargetTripleName\$ProfileName\jcode.exe"
    }

    Push-Location $RepoRootPath
    try {
        $buildResult = Invoke-NativeCommandWithCapturedOutput `
            -Executable "cargo" `
            -Arguments $cargoArgs `
            -WorkingDirectory $RepoRootPath
        if ($null -ne $buildResult.OutputLines) {
            foreach ($line in @($buildResult.OutputLines)) {
                if (-not [string]::IsNullOrWhiteSpace([string]$line)) {
                    Write-Host $line
                }
            }
        }

        if ($buildResult.ExitCode -eq 0) {
            return $primaryBuildExe
        }

        if (-not (Test-ShouldUseIsolatedCargoTargetDir -CargoOutput $buildResult.OutputText -BuildExePath $primaryBuildExe)) {
            throw "cargo build failed with exit code $($buildResult.ExitCode)"
        }

        $isolatedTargetDir = Get-IsolatedCargoTargetDir `
            -RepoRootPath $RepoRootPath `
            -ProfileName $ProfileName `
            -TargetTripleName $TargetTripleName

        Write-Info "Detected locked build artifact at $primaryBuildExe"
        Write-Info "Retrying cargo build with isolated target dir $isolatedTargetDir"

        $oldCargoTargetDir = $env:CARGO_TARGET_DIR
        $env:CARGO_TARGET_DIR = $isolatedTargetDir
        try {
            $fallbackResult = Invoke-NativeCommandWithCapturedOutput `
                -Executable "cargo" `
                -Arguments $cargoArgs `
                -WorkingDirectory $RepoRootPath
            if ($null -ne $fallbackResult.OutputLines) {
                foreach ($line in @($fallbackResult.OutputLines)) {
                    if (-not [string]::IsNullOrWhiteSpace([string]$line)) {
                        Write-Host $line
                    }
                }
            }
            if ($fallbackResult.ExitCode -ne 0) {
                throw "cargo build failed with exit code $($fallbackResult.ExitCode)"
            }
        } finally {
            if ($null -eq $oldCargoTargetDir) {
                Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
            } else {
                $env:CARGO_TARGET_DIR = $oldCargoTargetDir
            }
        }

        if ([string]::IsNullOrWhiteSpace($TargetTripleName)) {
            return Join-Path $isolatedTargetDir "$ProfileName\jcode.exe"
        }

        return Join-Path $isolatedTargetDir "$TargetTripleName\$ProfileName\jcode.exe"
    } finally {
        Pop-Location
    }
}

$ResolvedRepoRoot = Resolve-RepoRoot -ExplicitRepoRoot $RepoRoot
$CargoToml = Join-Path $ResolvedRepoRoot "Cargo.toml"
$PackageScript = Join-Path $ResolvedRepoRoot "scripts\package_saitec.ps1"

if (-not (Test-Path -LiteralPath $CargoToml)) {
    throw "Repo root does not look like jcode: $ResolvedRepoRoot"
}

if (-not (Test-Path -LiteralPath $PackageScript)) {
    throw "Missing packaging script: $PackageScript"
}

if ([string]::IsNullOrWhiteSpace($Timestamp)) {
    $Timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
}

$FinalOutputDir = Resolve-FinalOutputDir `
    -RepoRootPath $ResolvedRepoRoot `
    -ExplicitOutputDir $OutputDir `
    -ExplicitOutputParent $OutputParent `
    -ResolvedTimestamp $Timestamp

$BuildExe = if ([string]::IsNullOrWhiteSpace($TargetTriple)) {
    Join-Path $ResolvedRepoRoot "target\$Profile\jcode.exe"
} else {
    Join-Path $ResolvedRepoRoot "target\$TargetTriple\$Profile\jcode.exe"
}

if (-not $SkipBuild) {
    Write-Info "Running cargo build for profile $Profile"
    $BuildExe = Invoke-CargoBuildWithFallback `
        -RepoRootPath $ResolvedRepoRoot `
        -ProfileName $Profile `
        -TargetTripleName $TargetTriple
}

if (-not (Test-Path -LiteralPath $BuildExe)) {
    throw "Missing build artifact: $BuildExe"
}

Write-Info "Running repo packager"
$StandardDist = Invoke-RepoPackagerIsolated `
    -PackageScriptPath $PackageScript `
    -RepoRootPath $ResolvedRepoRoot `
    -ProfileName $Profile `
    -TargetTripleName $TargetTriple `
    -WantDebugSymbols $IncludeDebugSymbols.IsPresent

if (Test-Path -LiteralPath $FinalOutputDir) {
    Remove-Item -LiteralPath $FinalOutputDir -Recurse -Force
}

New-Item -ItemType Directory -Path $FinalOutputDir -Force | Out-Null
Get-ChildItem -LiteralPath $StandardDist -Force | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination $FinalOutputDir -Recurse -Force
}

$PackagedExe = Join-Path $FinalOutputDir "saitec-tui.exe"
$PackagedInstaller = Join-Path $FinalOutputDir "install.ps1"
$PackagedLogo = Join-Path $FinalOutputDir "SAITEC_logo.png"
$PackagedPdb = Join-Path $FinalOutputDir "saitec-tui.pdb"

if (-not (Test-Path -LiteralPath $PackagedExe)) {
    throw "Expected packaged executable missing: $PackagedExe"
}

if (-not (Test-Path -LiteralPath $PackagedInstaller)) {
    throw "Expected packaged installer missing: $PackagedInstaller"
}

Write-Host ""
Write-Info "SAITEC-TUI package copied to $FinalOutputDir"
Write-Info "  exe: $PackagedExe"
Write-Info "  installer: $PackagedInstaller"
Write-Info ("  logo: " + $(if (Test-Path -LiteralPath $PackagedLogo) { "included" } else { "missing" }))
Write-Info ("  symbols: " + $(if (Test-Path -LiteralPath $PackagedPdb) { "included" } else { "not included" }))

if ($OpenOutput) {
    Invoke-Item -LiteralPath $FinalOutputDir
}
