Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Script = Join-Path $PSScriptRoot "..\..\scripts\tui_io.ps1"

Describe "tui_io.ps1 dry-run command mapping" {
    It "maps send text to a client debug message command" {
        $result = & $Script -DryRun -JcodePath "jcode-test" send "hello" "world"

        $result.Executable | Should Be "jcode-test"
        ($result.Arguments -join "|") | Should Be "debug|client:message:hello world"
        $result.ReadsStdin | Should Be $false
    }

    It "maps append text to the client append input command" {
        $result = & $Script -DryRun append " more"

        ($result.Arguments -join "|") | Should Be "debug|client:append_input: more"
    }

    It "maps read aliases with session and socket targeting" {
        $result = & $Script `
            -DryRun `
            -JcodePath "jcode-test" `
            -Socket "C:\tmp\jcode.sock" `
            -Session "session-123" `
            read last

        ($result.Arguments -join "|") | Should Be "debug|-s|C:\tmp\jcode.sock|-S|session-123|client:last_response"
    }

    It "describes pipe mode as stdin-driven without invoking jcode" {
        $result = & $Script -DryRun -Mode send pipe

        $result.Action | Should Be "pipe"
        $result.ReadsStdin | Should Be $true
        $result.PipeMode | Should Be "send"
    }
}
