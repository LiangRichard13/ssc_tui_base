Set-StrictMode -Version Latest

function Test-ShouldUseIsolatedCargoTargetDir {
    param(
        [string]$CargoOutput,
        [string]$BuildExePath
    )

    if ([string]::IsNullOrWhiteSpace($CargoOutput) -or [string]::IsNullOrWhiteSpace($BuildExePath)) {
        return $false
    }

    $normalizedOutput = $CargoOutput.Replace("/", "\")
    $normalizedBuildExe = $BuildExePath.Replace("/", "\")

    if ($normalizedOutput -notmatch [regex]::Escape("failed to remove file")) {
        return $false
    }

    if ($normalizedOutput -notmatch [regex]::Escape($normalizedBuildExe)) {
        return $false
    }

    return $normalizedOutput -match "os error 5"
}

function Get-IsolatedCargoTargetDir {
    param(
        [string]$RepoRootPath,
        [string]$ProfileName,
        [string]$TargetTripleName
    )

    $suffix = if ([string]::IsNullOrWhiteSpace($TargetTripleName)) {
        $ProfileName
    } else {
        "$TargetTripleName-$ProfileName"
    }

    return Join-Path $RepoRootPath (Join-Path "target" ("saitec-packager-" + $suffix))
}
