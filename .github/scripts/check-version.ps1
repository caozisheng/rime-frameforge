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

if ([string]::IsNullOrWhiteSpace($Tag)) {
    if (-not [string]::IsNullOrWhiteSpace($env:GITHUB_REF_NAME)) {
        $Tag = $env:GITHUB_REF_NAME
    } else {
        throw 'A release tag is required. Use -Tag vX.Y.Z or set GITHUB_REF_NAME.'
    }
}

if ($Tag -notmatch '^v(?<version>\d+\.\d+\.\d+)$') {
    throw "Invalid release tag '$Tag'. Expected vX.Y.Z."
}

$expected = $Matches.version
$cargoText = Get-Content -Raw -LiteralPath 'Cargo.toml'
if ($cargoText -notmatch '(?m)^version\s*=\s*"(?<version>[^"]+)"\s*$') {
    throw 'Cargo.toml workspace version was not found.'
}
$cargoVersion = $Matches.version

$versions = [ordered]@{
    'Cargo.toml' = $cargoVersion
    'package.json' = Read-JsonVersion 'package.json'
    'apps/desktop/package.json' = Read-JsonVersion 'apps/desktop/package.json'
    'web/package.json' = Read-JsonVersion 'web/package.json'
    'apps/desktop/src-tauri/tauri.conf.json' = Read-JsonVersion 'apps/desktop/src-tauri/tauri.conf.json'
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

Write-Host "Release tag $Tag matches version $expected in all project manifests."
