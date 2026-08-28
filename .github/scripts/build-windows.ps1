[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)]
    [string]$OutputDirectory = 'release-artifacts'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-Step([string]$Command, [string[]]$Arguments) {
    Write-Host "> $Command $($Arguments -join ' ')"
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $Command"
    }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo is required to build Rime FrameForge.'
}
if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    throw 'npm is required to build Rime FrameForge.'
}

if (-not (Get-Command wasm-pack -ErrorAction SilentlyContinue)) {
    Invoke-Step 'cargo' @('install', 'wasm-pack', '--locked')
}

Invoke-Step 'rustup' @('target', 'add', 'wasm32-unknown-unknown')
Invoke-Step 'npm' @('ci')
Invoke-Step 'npm' @('run', 'generate:manifest')
Invoke-Step 'npm' @('run', 'tauri:build')

$bundleDirectory = Join-Path (Get-Location) 'target/release/bundle'
if (-not (Test-Path -LiteralPath $bundleDirectory)) {
    throw "Tauri bundle directory was not created: $bundleDirectory"
}

$outputPath = [System.IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null

$artifacts = Get-ChildItem -LiteralPath $bundleDirectory -Recurse -File |
    Where-Object { $_.Extension -in @('.msi', '.exe', '.zip') }
if ($artifacts.Count -eq 0) {
    throw "No Windows release artifacts found under $bundleDirectory"
}

foreach ($artifact in $artifacts) {
    Copy-Item -LiteralPath $artifact.FullName -Destination $outputPath -Force
    Write-Host "Collected $($artifact.Name)"
}

Write-Host "Windows artifacts are available in $outputPath."
