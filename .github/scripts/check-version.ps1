[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)]
    [string]$Tag
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Read-JsonVersion([string]$Path) {
    $json = Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json
    return [string]$json.version
}

function Read-Json([string]$Path) {
    return Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json
}

function Read-JsonHash([string]$Path) {
    return Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json -AsHashtable
}

$cargoText = Get-Content -Raw -LiteralPath 'Cargo.toml'
if ($cargoText -notmatch '(?m)^version\s*=\s*"(?<version>[^"]+)"\s*$') {
    throw 'Cargo.toml workspace version was not found.'
}
$cargoVersion = $Matches.version

if ([string]::IsNullOrWhiteSpace($Tag)) {
    $expected = $cargoVersion
} else {
    if ($Tag -notmatch '^v(?<version>\d+\.\d+\.\d+)$') {
        throw "Invalid release tag '$Tag'. Expected vX.Y.Z."
    }
    $expected = $Matches.version
}

$desktopPackage = Read-Json 'apps/desktop/package.json'
$packageLock = Read-JsonHash 'package-lock.json'
$lockRoot = $packageLock.packages['']
$lockDesktop = $packageLock.packages['apps/desktop']
$lockWeb = $packageLock.packages['web']

$versions = [ordered]@{
    'Cargo.toml' = $cargoVersion
    'package.json' = Read-JsonVersion 'package.json'
    'apps/desktop/package.json' = [string]$desktopPackage.version
    'apps/desktop/package.json @rime/web-runtime' = [string]$desktopPackage.dependencies.'@rime/web-runtime'
    'web/package.json' = Read-JsonVersion 'web/package.json'
    'apps/desktop/src-tauri/tauri.conf.json' = Read-JsonVersion 'apps/desktop/src-tauri/tauri.conf.json'
    'package-lock.json' = [string]$packageLock.version
    'package-lock.json root workspace' = [string]$lockRoot.version
    'package-lock.json desktop workspace' = [string]$lockDesktop.version
    'package-lock.json desktop @rime/web-runtime' = [string]$lockDesktop.dependencies.'@rime/web-runtime'
    'package-lock.json web workspace' = [string]$lockWeb.version
}

$errors = @()
foreach ($entry in $versions.GetEnumerator()) {
    if ($entry.Value -ne $expected) {
        $errors += "$($entry.Key) declares $($entry.Value), expected $expected"
    }
}

if ($errors.Count -gt 0) {
    throw ($errors -join [Environment]::NewLine)
}

if ([string]::IsNullOrWhiteSpace($Tag)) {
    Write-Host "Project manifests consistently declare version $expected."
} else {
    Write-Host "Release tag $Tag matches version $expected in all project manifests."
}
