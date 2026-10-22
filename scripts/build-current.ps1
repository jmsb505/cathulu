[CmdletBinding()]
param(
    [switch]$BuildOnly
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$stagingTarget = Join-Path $repoRoot 'target\m27-build'
$stagingRelease = Join-Path $stagingTarget 'release'
$versionRoot = Join-Path $repoRoot 'target\mcp-versions'
$currentPointer = Join-Path $repoRoot 'target\mcp-current'
$buildId = [guid]::NewGuid().ToString('N')
$candidateVersion = Join-Path $versionRoot ".candidate-$buildId"
$nextPointer = Join-Path $repoRoot ".mcp-current-next-$buildId"
$previousPointer = Join-Path $repoRoot ".mcp-current-previous-$buildId"

function Invoke-Native {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $Command $($Arguments -join ' ')"
    }
}

function Get-SourceFingerprint {
    $inputs = @(
        'Cargo.toml',
        'Cargo.lock',
        'src',
        'apps/desktop/package.json',
        'apps/desktop/index.html',
        'apps/desktop/public',
        'apps/desktop/src',
        'apps/desktop/src-tauri',
        'apps/desktop/vite.config.ts',
        'apps/desktop/tsconfig.json'
    )
    $files = foreach ($relative in $inputs) {
        $path = Join-Path $repoRoot $relative
        if (-not (Test-Path -LiteralPath $path)) { continue }
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            Get-Item -LiteralPath $path
        }
        else {
            Get-ChildItem -LiteralPath $path -File -Recurse -Force
        }
    }
    $fingerprintParts = foreach ($file in ($files | Sort-Object FullName -Unique)) {
        $relativePath = $file.FullName.Substring($repoRoot.Length + 1).Replace('\', '/')
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $file.FullName).Hash
        "$relativePath=$hash"
    }
    $hasher = [System.Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [System.Text.Encoding]::UTF8.GetBytes(($fingerprintParts -join "`n"))
        return [System.BitConverter]::ToString($hasher.ComputeHash($bytes)).Replace('-', '')
    }
    finally {
        $hasher.Dispose()
    }
}

$sourceFingerprint = Get-SourceFingerprint
$currentBuildPath = Join-Path $versionRoot 'current-build.json'
if (-not $BuildOnly -and (Test-Path -LiteralPath $currentBuildPath -PathType Leaf)) {
    $activeBuild = Get-Content -LiteralPath $currentBuildPath -Raw | ConvertFrom-Json
    if ($activeBuild.sourceFingerprint -eq $sourceFingerprint -and
        (Test-Path -LiteralPath (Join-Path $currentPointer 'cathulu-mcp.exe') -PathType Leaf) -and
        (Test-Path -LiteralPath (Join-Path $currentPointer 'cathulu-desktop.exe') -PathType Leaf) -and
        (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $currentPointer 'cathulu-mcp.exe')).Hash -eq $activeBuild.mcpSha256 -and
        (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $currentPointer 'cathulu-desktop.exe')).Hash -eq $activeBuild.desktopSha256) {
        Write-Host "Current source is already published at $currentPointer; no duplicate build created."
        return
    }
}

$previousCargoTarget = $env:CARGO_TARGET_DIR
try {
    New-Item -ItemType Directory -Force -Path $stagingTarget, $versionRoot | Out-Null
    $env:CARGO_TARGET_DIR = $stagingTarget

    Push-Location $repoRoot
    try {
        Invoke-Native 'cargo' @('build', '--locked', '--offline', '--release', '--bin', 'cathulu-mcp')
        Push-Location (Join-Path $repoRoot 'apps\desktop')
        try {
            Invoke-Native 'cargo' @('tauri', 'build', '--no-bundle', '--features', 'custom-protocol')
        }
        finally {
            Pop-Location
        }
    }
    finally {
        Pop-Location
    }

    $stagedMcp = Join-Path $stagingRelease 'cathulu-mcp.exe'
    $stagedDesktop = Join-Path $stagingRelease 'cathulu-desktop.exe'
    foreach ($binary in @($stagedMcp, $stagedDesktop)) {
        if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
            throw "Expected release binary was not produced: $binary"
        }
    }

    $revision = (& git -C $repoRoot rev-parse HEAD 2>$null | Select-Object -First 1)
    if ($LASTEXITCODE -ne 0) { $revision = 'unavailable' }
    $dirtyLines = @(& git -C $repoRoot status --porcelain 2>$null)
    $manifest = [ordered]@{
        builtAtUtc = [DateTime]::UtcNow.ToString('o')
        sourceFingerprint = $sourceFingerprint
        sourceRevision = $revision
        workingTreeDirty = ($dirtyLines.Count -gt 0)
        mcpSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $stagedMcp).Hash
        desktopSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $stagedDesktop).Hash
        desktopFeature = 'custom-protocol'
    }

    if ($BuildOnly) {
        $manifest | ConvertTo-Json | Write-Output
        Write-Host "Build-only mode. Matched binaries are in $stagingRelease"
        return
    }

    $versionId = $sourceFingerprint.Substring(0, 24)
    $versionDirectory = Join-Path $versionRoot $versionId
    if (Test-Path -LiteralPath $versionDirectory -PathType Container) {
        $versionManifestPath = Join-Path $versionDirectory 'build-manifest.json'
        if (-not (Test-Path -LiteralPath $versionManifestPath -PathType Leaf)) {
            throw "Version directory is missing its build manifest: $versionDirectory"
        }
        $manifest = Get-Content -LiteralPath $versionManifestPath -Raw | ConvertFrom-Json
        if ($manifest.sourceFingerprint -ne $sourceFingerprint) {
            throw "Version directory has a different source fingerprint: $versionDirectory"
        }
        $existingMcp = Join-Path $versionDirectory 'cathulu-mcp.exe'
        $existingDesktop = Join-Path $versionDirectory 'cathulu-desktop.exe'
        if (-not (Test-Path -LiteralPath $existingMcp -PathType Leaf) -or
            -not (Test-Path -LiteralPath $existingDesktop -PathType Leaf) -or
            (Get-FileHash -Algorithm SHA256 -LiteralPath $existingMcp).Hash -ne $manifest.mcpSha256 -or
            (Get-FileHash -Algorithm SHA256 -LiteralPath $existingDesktop).Hash -ne $manifest.desktopSha256) {
            throw "Version directory binaries do not match its manifest: $versionDirectory"
        }
    }
    else {
        New-Item -ItemType Directory -Path $candidateVersion | Out-Null
        Copy-Item -LiteralPath $stagedMcp, $stagedDesktop -Destination $candidateVersion
        $manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $candidateVersion 'build-manifest.json') -Encoding utf8

        $candidateMcp = Join-Path $candidateVersion 'cathulu-mcp.exe'
        $candidateDesktop = Join-Path $candidateVersion 'cathulu-desktop.exe'
        if ((Get-FileHash -Algorithm SHA256 -LiteralPath $candidateMcp).Hash -ne $manifest.mcpSha256 -or
            (Get-FileHash -Algorithm SHA256 -LiteralPath $candidateDesktop).Hash -ne $manifest.desktopSha256) {
            throw 'Staged binary hashes changed before publishing; the current pointer was left untouched.'
        }
        Move-Item -LiteralPath $candidateVersion -Destination $versionDirectory
    }

    New-Item -ItemType Junction -Path $nextPointer -Target $versionDirectory | Out-Null
    $hadPreviousPointer = $false
    try {
        if (Test-Path -LiteralPath $currentPointer) {
            $currentItem = Get-Item -LiteralPath $currentPointer -Force
            if (($currentItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) {
                throw "Refusing to replace a non-junction current path: $currentPointer"
            }
            Move-Item -LiteralPath $currentPointer -Destination $previousPointer
            $hadPreviousPointer = $true
        }

        Move-Item -LiteralPath $nextPointer -Destination $currentPointer
        $currentMcp = Join-Path $currentPointer 'cathulu-mcp.exe'
        $currentDesktop = Join-Path $currentPointer 'cathulu-desktop.exe'
        if ((Get-FileHash -Algorithm SHA256 -LiteralPath $currentMcp).Hash -ne $manifest.mcpSha256 -or
            (Get-FileHash -Algorithm SHA256 -LiteralPath $currentDesktop).Hash -ne $manifest.desktopSha256) {
            throw 'Current junction does not resolve to the validated MCP/desktop pair.'
        }
    }
    catch {
        if (Test-Path -LiteralPath $currentPointer) {
            Remove-Item -LiteralPath $currentPointer -ErrorAction SilentlyContinue
        }
        if ($hadPreviousPointer -and (Test-Path -LiteralPath $previousPointer)) {
            Move-Item -LiteralPath $previousPointer -Destination $currentPointer
        }
        throw
    }

    if ($hadPreviousPointer -and (Test-Path -LiteralPath $previousPointer)) {
        Remove-Item -LiteralPath $previousPointer
    }
    $manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $versionRoot 'current-build.json') -Encoding utf8
    Write-Host "Published matched build: $versionDirectory"
    Write-Host "Stable Codex path: $currentPointer\cathulu-mcp.exe"
    Write-Host "SHA-256 MCP:     $($manifest.mcpSha256)"
    Write-Host "SHA-256 desktop: $($manifest.desktopSha256)"
    Write-Host 'A running MCP keeps its loaded version until Codex restarts that server. This script never stops or restarts Codex.'
}
finally {
    if ($null -eq $previousCargoTarget) {
        Remove-Item Env:\CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    }
    else {
        $env:CARGO_TARGET_DIR = $previousCargoTarget
    }
    if (Test-Path -LiteralPath $candidateVersion -PathType Container) {
        Remove-Item -LiteralPath $candidateVersion -Recurse -Force
    }
    if (Test-Path -LiteralPath $nextPointer) {
        Remove-Item -LiteralPath $nextPointer
    }
}
