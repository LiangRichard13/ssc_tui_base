param(
    [Parameter(Position = 0)]
    [string]$Action = "help",

    [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
    [string[]]$Value = @(),

    [ValidateSet("send", "insert", "append", "replace")]
    [string]$Mode = "send",

    [string]$Read = "last",
    [string]$Session = "",
    [string]$Socket = "",
    [string]$JcodePath = "jcode",
    [int]$IntervalMs = 750,
    [switch]$Raw,
    [switch]$Once,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Show-Usage {
    Write-Host @"
Usage:
  scripts\tui_io.ps1 send "message"
  scripts\tui_io.ps1 set "draft text"
  scripts\tui_io.ps1 append " more text"
  scripts\tui_io.ps1 insert "text at cursor"
  scripts\tui_io.ps1 submit
  scripts\tui_io.ps1 key "ctrl+a,backspace"
  scripts\tui_io.ps1 read last|history|state|input|input-json|frame
  scripts\tui_io.ps1 watch last
  scripts\tui_io.ps1 -Mode send pipe

Options:
  -Session <id>      Target a specific live TUI session.
  -Socket <path>     Target a specific jcode server socket.
  -JcodePath <path>  jcode executable to invoke. Defaults to PATH lookup.
  -IntervalMs <ms>   Poll interval for watch. Defaults to 750.
  -Raw              Omit watch timestamps.
  -DryRun           Print the resolved invocation object without running jcode.
"@
}

function Read-TextArgument {
    param([string[]]$Parts)

    if ($null -ne $Parts -and $Parts.Count -gt 0) {
        return ($Parts -join " ")
    }

    if ([Console]::IsInputRedirected) {
        $stdin = [Console]::In.ReadToEnd()
        return $stdin.TrimEnd("`r", "`n")
    }

    throw "Provide text as an argument or pipe it via stdin."
}

function Resolve-ReadCommand {
    param([string]$Name)

    switch ($Name.Trim().ToLowerInvariant()) {
        "" { "last_response"; break }
        "last" { "last_response"; break }
        "last-response" { "last_response"; break }
        "last_response" { "last_response"; break }
        "output" { "last_response"; break }
        "messages" { "history"; break }
        "history" { "history"; break }
        "state" { "state"; break }
        "input" { "input"; break }
        "draft" { "input-json"; break }
        "input-json" { "input-json"; break }
        "frame" { "frame"; break }
        "frame-normalized" { "frame-normalized"; break }
        "screen" { "screen"; break }
        "layout" { "layout"; break }
        "picker" { "picker"; break }
        default { $Name.Trim() }
    }
}

function New-DebugInvocation {
    param([string]$ClientCommand)

    $args = @("debug")
    if (-not [string]::IsNullOrWhiteSpace($Socket)) {
        $args += @("-s", $Socket)
    }
    if (-not [string]::IsNullOrWhiteSpace($Session)) {
        $args += @("-S", $Session)
    }
    $args += "client:$ClientCommand"

    [pscustomobject]@{
        Executable = $JcodePath
        Arguments = $args
        ReadsStdin = $false
        Action = $Action.Trim().ToLowerInvariant()
    }
}

function Invoke-JcodeInvocation {
    param(
        [pscustomobject]$Invocation,
        [switch]$Capture
    )

    if ($DryRun) {
        return $Invocation
    }

    if ($Capture) {
        $output = & $Invocation.Executable @($Invocation.Arguments) 2>&1
        $exitCode = $LASTEXITCODE
        if ($exitCode -ne 0) {
            throw "jcode exited with code $exitCode while running: $($Invocation.Arguments -join ' ')`n$($output -join [Environment]::NewLine)"
        }
        return ($output | Out-String).TrimEnd("`r", "`n")
    }

    & $Invocation.Executable @($Invocation.Arguments)
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        exit $exitCode
    }
}

function New-ActionInvocation {
    param(
        [string]$ResolvedAction,
        [string[]]$Parts
    )

    switch ($ResolvedAction) {
        "send" {
            $text = Read-TextArgument -Parts $Parts
            return New-DebugInvocation -ClientCommand "message:$text"
        }
        "set" {
            $text = Read-TextArgument -Parts $Parts
            return New-DebugInvocation -ClientCommand "replace_input:$text"
        }
        "replace" {
            $text = Read-TextArgument -Parts $Parts
            return New-DebugInvocation -ClientCommand "replace_input:$text"
        }
        "append" {
            $text = Read-TextArgument -Parts $Parts
            return New-DebugInvocation -ClientCommand "append_input:$text"
        }
        "insert" {
            $text = Read-TextArgument -Parts $Parts
            return New-DebugInvocation -ClientCommand "insert_input:$text"
        }
        "submit" {
            if ($null -ne $Parts -and $Parts.Count -gt 0) {
                $text = Read-TextArgument -Parts $Parts
                return New-DebugInvocation -ClientCommand "message:$text"
            }
            return New-DebugInvocation -ClientCommand "submit"
        }
        "key" {
            $keys = Read-TextArgument -Parts $Parts
            return New-DebugInvocation -ClientCommand "keys:$keys"
        }
        "keys" {
            $keys = Read-TextArgument -Parts $Parts
            return New-DebugInvocation -ClientCommand "keys:$keys"
        }
        "read" {
            $target = if ($null -ne $Parts -and $Parts.Count -gt 0) { $Parts[0] } else { $Read }
            return New-DebugInvocation -ClientCommand (Resolve-ReadCommand -Name $target)
        }
        default {
            throw "Unknown action '$ResolvedAction'. Run scripts\tui_io.ps1 help."
        }
    }
}

function Invoke-PipeMode {
    if ($DryRun) {
        return [pscustomobject]@{
            Executable = $JcodePath
            Arguments = @()
            ReadsStdin = $true
            Action = "pipe"
            PipeMode = $Mode
        }
    }

    while ($null -ne ($line = [Console]::In.ReadLine())) {
        if ($line.Trim().Length -eq 0) {
            continue
        }
        $invocation = New-ActionInvocation -ResolvedAction $Mode -Parts @($line)
        Invoke-JcodeInvocation -Invocation $invocation | Out-Null
    }
}

function Invoke-WatchMode {
    param([string[]]$Parts)

    $target = if ($null -ne $Parts -and $Parts.Count -gt 0) { $Parts[0] } else { $Read }
    $invocation = New-DebugInvocation -ClientCommand (Resolve-ReadCommand -Name $target)

    if ($DryRun) {
        $invocation | Add-Member -NotePropertyName Repeats -NotePropertyValue $true
        $invocation | Add-Member -NotePropertyName IntervalMs -NotePropertyValue $IntervalMs
        return $invocation
    }

    $last = $null
    do {
        $current = Invoke-JcodeInvocation -Invocation $invocation -Capture
        if ($current -ne $last) {
            if ($Raw) {
                Write-Output $current
            } else {
                $stamp = Get-Date -Format "HH:mm:ss.fff"
                Write-Output "[$stamp] $target"
                Write-Output $current
                Write-Output ""
            }
            $last = $current
        }

        if ($Once) {
            break
        }

        Start-Sleep -Milliseconds $IntervalMs
    } while ($true)
}

$resolvedAction = $Action.Trim().ToLowerInvariant()

switch ($resolvedAction) {
    "help" {
        Show-Usage
    }
    "watch" {
        Invoke-WatchMode -Parts $Value
    }
    "pipe" {
        Invoke-PipeMode
    }
    default {
        $invocation = New-ActionInvocation -ResolvedAction $resolvedAction -Parts $Value
        Invoke-JcodeInvocation -Invocation $invocation
    }
}
