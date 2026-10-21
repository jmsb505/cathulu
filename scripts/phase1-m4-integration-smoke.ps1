$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$tempBase = [System.IO.Path]::GetFullPath($env:TEMP).TrimEnd('\')
$smokeRoot = Join-Path $tempBase ("cathulu-phase1-m4-" + [Guid]::NewGuid().ToString('N'))
$dataDir = Join-Path $smokeRoot 'data'
$mcpPath = Join-Path $repoRoot 'target\debug\cathulu-mcp.exe'
$desktopPath = Join-Path $repoRoot 'target\release\cathulu-desktop.exe'
$desktopProcess = $null
$oldSmokeRoot = $env:CATHULU_SMOKE_ROOT
$oldMcpBin = $env:CATHULU_MCP_BIN
$oldDataDir = $env:CATHULU_DATA_DIR
$pushedLocation = $false

function Assert-LastExitCode([string] $commandName) {
    if ($LASTEXITCODE -ne 0) {
        throw "$commandName failed with exit code $LASTEXITCODE"
    }
}

function Invoke-McpDashboardRead([string] $executable, [string] $databaseDirectory) {
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $executable
    $startInfo.WorkingDirectory = $repoRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.EnvironmentVariables['CATHULU_DATA_DIR'] = $databaseDirectory

    $child = [System.Diagnostics.Process]::Start($startInfo)
    try {
        $messages = @(
            [ordered]@{
                jsonrpc = '2.0'
                id = 1
                method = 'initialize'
                params = @{
                    protocolVersion = '2025-11-25'
                    capabilities = @{}
                    clientInfo = @{ name = 'phase1-m4-integration-smoke'; version = '0.1.0' }
                }
            },
            [ordered]@{ jsonrpc = '2.0'; method = 'notifications/initialized' },
            [ordered]@{
                jsonrpc = '2.0'
                id = 2
                method = 'resources/read'
                params = @{ uri = 'cathulu://dashboard' }
            }
        )
        foreach ($message in $messages) {
            $child.StandardInput.WriteLine(($message | ConvertTo-Json -Depth 20 -Compress))
        }
        $child.StandardInput.Close()
        $stdout = $child.StandardOutput.ReadToEnd()
        $stderr = $child.StandardError.ReadToEnd()
        $child.WaitForExit()
        if ($child.ExitCode -ne 0) {
            throw "MCP restart/read exited $($child.ExitCode): $stderr"
        }
        $response = $stdout -split "`r?`n" |
            Where-Object { $_.Trim().Length -gt 0 } |
            ForEach-Object { $_ | ConvertFrom-Json } |
            Where-Object { $_.id -eq 2 } |
            Select-Object -First 1
        if ($null -eq $response -or $null -eq $response.result.contents[0].text) {
            throw "MCP restart/read returned no dashboard resource: $stdout`n$stderr"
        }
        return $response.result.contents[0].text | ConvertFrom-Json
    }
    finally {
        if (-not $child.HasExited) {
            $child.Kill()
            $child.WaitForExit()
        }
        $child.Dispose()
    }
}

try {
    New-Item -ItemType Directory -Path $smokeRoot | Out-Null
    Push-Location $repoRoot
    $pushedLocation = $true

    cargo build --offline --bin cathulu-mcp
    Assert-LastExitCode 'cargo build --offline --bin cathulu-mcp'
    cargo tauri build --no-bundle -- --offline
    Assert-LastExitCode 'cargo tauri build --no-bundle -- --offline'

    if (-not (Test-Path -LiteralPath $mcpPath -PathType Leaf)) {
        throw "MCP executable was not produced: $mcpPath"
    }
    if (-not (Test-Path -LiteralPath $desktopPath -PathType Leaf)) {
        throw "Desktop executable was not produced: $desktopPath"
    }

    $env:CATHULU_SMOKE_ROOT = $smokeRoot
    $env:CATHULU_MCP_BIN = $mcpPath
    cargo test --offline -p cathulu-desktop --lib phase1_m4_mcp_state_is_visible_through_tauri_and_survives_restarts -- --ignored --nocapture
    Assert-LastExitCode 'combined MCP/Tauri integration test'

    $env:CATHULU_DATA_DIR = $dataDir
    $desktopProcess = Start-Process -FilePath $desktopPath -WindowStyle Hidden -PassThru
    Start-Sleep -Seconds 5
    if ($desktopProcess.HasExited) {
        throw "Desktop smoke process exited with code $($desktopProcess.ExitCode)"
    }
    Stop-Process -Id $desktopProcess.Id -Force
    $desktopProcess.WaitForExit()
    $desktopProcess.Dispose()
    $desktopProcess = $null

    $dashboard = Invoke-McpDashboardRead $mcpPath $dataDir
    if ($dashboard.projects.Count -ne 1) {
        throw "Expected one persisted project after desktop restart; found $($dashboard.projects.Count)"
    }
    if ($dashboard.recentRuns.Count -lt 1 -or $dashboard.recentRuns[0].status -ne 'verified_success') {
        throw 'Verified run state did not survive the desktop and MCP restarts.'
    }
    if (-not $dashboard.projects[0].dirty -or $dashboard.projects[0].changedPaths -notcontains 'changed-after-registration.txt') {
        throw 'Git freshness reconciliation did not survive the restarts.'
    }

    Write-Output 'PASS: MCP workflow and memory state persisted; the read-only Tauri command saw the same database; the Windows desktop started; a later MCP restart returned verified run and fresh Git state.'
}
finally {
    if ($null -ne $desktopProcess -and -not $desktopProcess.HasExited) {
        Stop-Process -Id $desktopProcess.Id -Force -ErrorAction SilentlyContinue
        $desktopProcess.WaitForExit()
        $desktopProcess.Dispose()
    }
    if ($pushedLocation) { Pop-Location }

    if ($null -eq $oldSmokeRoot) { Remove-Item Env:CATHULU_SMOKE_ROOT -ErrorAction SilentlyContinue } else { $env:CATHULU_SMOKE_ROOT = $oldSmokeRoot }
    if ($null -eq $oldMcpBin) { Remove-Item Env:CATHULU_MCP_BIN -ErrorAction SilentlyContinue } else { $env:CATHULU_MCP_BIN = $oldMcpBin }
    if ($null -eq $oldDataDir) { Remove-Item Env:CATHULU_DATA_DIR -ErrorAction SilentlyContinue } else { $env:CATHULU_DATA_DIR = $oldDataDir }

    $resolvedSmokeRoot = [System.IO.Path]::GetFullPath($smokeRoot).TrimEnd('\')
    if ($resolvedSmokeRoot.StartsWith($tempBase + '\', [System.StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $resolvedSmokeRoot -Recurse -Force -ErrorAction SilentlyContinue
    } else {
        throw "Refusing to remove smoke directory outside TEMP: $resolvedSmokeRoot"
    }
}
