Set-StrictMode -Version Latest

function Resolve-DevCargoCommand {
    $cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    if ($null -ne $cargoCommand -and -not [string]::IsNullOrWhiteSpace($cargoCommand.Source)) {
        return $cargoCommand.Source
    }

    $candidatePaths = @()
    foreach ($homeRoot in @($env:USERPROFILE, $env:HOME)) {
        if ([string]::IsNullOrWhiteSpace($homeRoot)) {
            continue
        }

        $candidatePaths += (Join-Path $homeRoot ".cargo\bin\cargo.exe")
        $candidatePaths += (Join-Path $homeRoot ".rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe")
    }

    foreach ($candidate in $candidatePaths) {
        if ([string]::IsNullOrWhiteSpace($candidate)) {
            continue
        }

        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }

    throw "cargo was not found on PATH and no fallback cargo.exe was found under the current user's .cargo\bin or .rustup\toolchains directories."
}

function Get-DevBuildArtifactPaths {
    param(
        [string]$RepoRootPath,
        [string]$ProfileName,
        [string]$TargetTripleName
    )

    $buildDir = if ([string]::IsNullOrWhiteSpace($TargetTripleName)) {
        Join-Path $RepoRootPath "target\$ProfileName"
    } else {
        Join-Path $RepoRootPath "target\$TargetTripleName\$ProfileName"
    }

    [pscustomobject]@{
        BuildDir = $buildDir
        ExePath = Join-Path $buildDir "jcode.exe"
        PdbPath = Join-Path $buildDir "jcode.pdb"
    }
}

function Get-DevSaitecMcpLayout {
    param([string]$RepoRootPath)

    $skillsRoot = Join-Path $RepoRootPath "_vendor\SAITEC-Skills"
    $requirementsPath = Join-Path $skillsRoot "requirements.txt"
    $serverPath = Join-Path $skillsRoot "mcp_server\server.py"
    $venvDir = Join-Path $RepoRootPath "target\saitec-mcp-venv"
    $windowsPython = Join-Path $venvDir "Scripts\python.exe"
    $posixPython = Join-Path $venvDir "bin\python"

    [pscustomobject]@{
        SkillsRoot = $skillsRoot
        RequirementsPath = $requirementsPath
        ServerPath = $serverPath
        VenvDir = $venvDir
        PythonPath = if (Test-Path -LiteralPath $windowsPython) { $windowsPython } else { $windowsPython }
        PosixPythonPath = $posixPython
        RequirementsStampPath = Join-Path $venvDir ".requirements.sha256"
    }
}

function Resolve-DevPythonCommand {
    $pythonCommand = Get-Command python -ErrorAction SilentlyContinue
    if ($null -ne $pythonCommand -and -not [string]::IsNullOrWhiteSpace($pythonCommand.Source)) {
        return $pythonCommand.Source
    }

    $pyCommand = Get-Command py -ErrorAction SilentlyContinue
    if ($null -ne $pyCommand -and -not [string]::IsNullOrWhiteSpace($pyCommand.Source)) {
        return $pyCommand.Source
    }

    throw "python was not found on PATH; install Python or set SAITEC_TUI_PYTHON to a Python executable with SAITEC-Skills dependencies."
}

function Ensure-DevSaitecMcpPython {
    param([string]$RepoRootPath)

    $layout = Get-DevSaitecMcpLayout -RepoRootPath $RepoRootPath
    if (Test-Path -LiteralPath $layout.SkillsRoot) {
        $env:SAITEC_SKILLS_ROOT = $layout.SkillsRoot
    }

    if (-not [string]::IsNullOrWhiteSpace($env:SAITEC_TUI_PYTHON)) {
        return $env:SAITEC_TUI_PYTHON
    }

    if (-not (Test-Path -LiteralPath $layout.ServerPath) -or -not (Test-Path -LiteralPath $layout.RequirementsPath)) {
        return $null
    }

    if (-not (Test-Path -LiteralPath $layout.PythonPath) -and -not (Test-Path -LiteralPath $layout.PosixPythonPath)) {
        $basePython = Resolve-DevPythonCommand
        Write-Info "Creating SAITEC MCP Python environment at $($layout.VenvDir)"
        & $basePython -m venv $layout.VenvDir 2>&1 | ForEach-Object { Write-Host $_ }
        if ($LASTEXITCODE -ne 0) {
            throw "python -m venv failed with exit code $LASTEXITCODE"
        }
    }

    $venvPython = if (Test-Path -LiteralPath $layout.PythonPath) {
        $layout.PythonPath
    } elseif (Test-Path -LiteralPath $layout.PosixPythonPath) {
        $layout.PosixPythonPath
    } else {
        throw "SAITEC MCP Python environment was created but no Python executable was found under $($layout.VenvDir)"
    }

    $requirementsHash = (Get-FileHash -LiteralPath $layout.RequirementsPath -Algorithm SHA256).Hash
    $installedHash = if (Test-Path -LiteralPath $layout.RequirementsStampPath) {
        (Get-Content -LiteralPath $layout.RequirementsStampPath -Raw).Trim()
    } else {
        ""
    }

    if ($installedHash -ne $requirementsHash) {
        Write-Info "Installing SAITEC MCP Python dependencies"
        & $venvPython -m pip install --disable-pip-version-check -r $layout.RequirementsPath 2>&1 | ForEach-Object { Write-Host $_ }
        if ($LASTEXITCODE -ne 0) {
            throw "pip install for SAITEC MCP dependencies failed with exit code $LASTEXITCODE"
        }
        $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText($layout.RequirementsStampPath, $requirementsHash, $utf8NoBom)
    }

    $env:SAITEC_TUI_PYTHON = $venvPython
    $env:SAITEC_SKILLS_ROOT = $layout.SkillsRoot
    return $venvPython
}

function New-DevRuntimeLayout {
    param(
        [string]$RepoRootPath,
        [string]$ProfileName,
        [string]$TargetTripleName,
        [string]$Timestamp
    )

    $runtimeRoot = Join-Path $RepoRootPath "dist\dev-saitec-tui"
    $nameParts = @("run", $ProfileName)
    if (-not [string]::IsNullOrWhiteSpace($TargetTripleName)) {
        $nameParts += $TargetTripleName
    }
    $nameParts += $Timestamp
    $runtimeDirName = ($nameParts -join "-")
    $runtimeDir = Join-Path $runtimeRoot $runtimeDirName

    [pscustomobject]@{
        RuntimeRoot = $runtimeRoot
        RuntimeDir = $runtimeDir
        RuntimeExe = Join-Path $runtimeDir "jcode.exe"
        RuntimePdb = Join-Path $runtimeDir "jcode.pdb"
        StatePath = Join-Path $runtimeRoot "dev-runtime-state.json"
    }
}

function Read-DevRuntimeState {
    param([string]$StatePath)

    if (-not (Test-Path -LiteralPath $StatePath)) {
        return $null
    }

    return Get-Content -LiteralPath $StatePath -Raw | ConvertFrom-Json
}

function Write-DevRuntimeState {
    param(
        [string]$StatePath,
        [int]$RuntimePid,
        [string]$RuntimeDir,
        [string]$RuntimeExe
    )

    $parent = Split-Path -Parent $StatePath
    if (-not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }

    $payload = [pscustomobject]@{
        pid = $RuntimePid
        runtime_dir = $RuntimeDir
        runtime_exe = $RuntimeExe
        updated_at = (Get-Date).ToString("o")
    }

    $json = $payload | ConvertTo-Json -Depth 4
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($StatePath, $json, $utf8NoBom)
}

function Remove-DevRuntimeState {
    param([string]$StatePath)

    if (Test-Path -LiteralPath $StatePath) {
        Remove-Item -LiteralPath $StatePath -Force
    }
}

function Stop-DevRuntimeProcess {
    param([string]$StatePath)

    $state = Read-DevRuntimeState -StatePath $StatePath
    if ($null -eq $state) {
        return $false
    }

    $stopped = $false
    try {
        $process = Get-Process -Id ([int]$state.pid) -ErrorAction Stop
        Stop-Process -Id $process.Id -Force
        $stopped = $true
    } catch {
        $stopped = $false
    }

    Remove-DevRuntimeState -StatePath $StatePath
    return $stopped
}

function Stop-ProcessesByExecutablePath {
    param([string]$ExecutablePath)

    if ([string]::IsNullOrWhiteSpace($ExecutablePath)) {
        return 0
    }

    $normalizedTarget = [System.IO.Path]::GetFullPath($ExecutablePath)
    $stoppedCount = 0

    foreach ($process in Get-Process -ErrorAction SilentlyContinue) {
        $processPath = $null
        try {
            $processPath = $process.Path
        } catch {
            $processPath = $null
        }

        if ([string]::IsNullOrWhiteSpace($processPath)) {
            continue
        }

        $normalizedProcessPath = $null
        try {
            $normalizedProcessPath = [System.IO.Path]::GetFullPath($processPath)
        } catch {
            $normalizedProcessPath = $null
        }

        if ($null -eq $normalizedProcessPath) {
            continue
        }

        if ([string]::Equals($normalizedProcessPath, $normalizedTarget, [System.StringComparison]::OrdinalIgnoreCase)) {
            try {
                Stop-Process -Id $process.Id -Force -ErrorAction Stop
                $stoppedCount += 1
            } catch {
                # Ignore races where the process exits between enumeration and stop.
            }
        }
    }

    return $stoppedCount
}

function Stop-ProcessesUnderDirectory {
    param([string]$DirectoryPath)

    if ([string]::IsNullOrWhiteSpace($DirectoryPath) -or -not (Test-Path -LiteralPath $DirectoryPath)) {
        return 0
    }

    $normalizedDirectory = [System.IO.Path]::GetFullPath($DirectoryPath)
    if (-not $normalizedDirectory.EndsWith([System.IO.Path]::DirectorySeparatorChar)) {
        $normalizedDirectory += [System.IO.Path]::DirectorySeparatorChar
    }

    $stoppedCount = 0

    foreach ($process in Get-Process -ErrorAction SilentlyContinue) {
        $processPath = $null
        try {
            $processPath = $process.Path
        } catch {
            $processPath = $null
        }

        if ([string]::IsNullOrWhiteSpace($processPath)) {
            continue
        }

        $normalizedProcessPath = $null
        try {
            $normalizedProcessPath = [System.IO.Path]::GetFullPath($processPath)
        } catch {
            $normalizedProcessPath = $null
        }

        if ($null -eq $normalizedProcessPath) {
            continue
        }

        if ($normalizedProcessPath.StartsWith($normalizedDirectory, [System.StringComparison]::OrdinalIgnoreCase)) {
            try {
                Stop-Process -Id $process.Id -Force -ErrorAction Stop
                $stoppedCount += 1
            } catch {
                # Ignore races where the process exits between enumeration and stop.
            }
        }
    }

    return $stoppedCount
}
