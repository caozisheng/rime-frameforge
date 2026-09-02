[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Version
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($Version.StartsWith('v')) {
    $Version = $Version.Substring(1)
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Invalid version '$Version'. Expected X.Y.Z or vX.Y.Z."
}

function Replace-Text([string]$Path, [string]$Pattern, [string]$Description) {
    $text = Get-Content -Raw -LiteralPath $Path
    $match = [regex]::Match($text, $Pattern)
    if (-not $match.Success) {
        throw "Could not find version in $Description ($Path)."
    }
    if ($match.Groups[2].Value -eq $Version) {
        return
    }
    $updated = [regex]::Replace($text, $Pattern, {
        param($current)
        $current.Groups[1].Value + $Version + $current.Groups[3].Value
    }, 1)
    Set-Content -LiteralPath $Path -Value $updated -NoNewline
}

Replace-Text 'Cargo.toml' '(?m)^(\s*version\s*=\s*")([^"]+)("\s*$)' 'Cargo workspace manifest'
Replace-Text 'package.json' '(?m)^(\s*"version"\s*:\s*")([^"]+)(",?\s*$)' 'root npm manifest'
Replace-Text 'apps/desktop/package.json' '(?m)^(\s*"version"\s*:\s*")([^"]+)(",?\s*$)' 'desktop npm manifest'
Replace-Text 'apps/desktop/package.json' '(?m)^(\s*"@rime/web-runtime"\s*:\s*")([^"]+)(",?\s*$)' 'desktop workspace dependency'
Replace-Text 'web/package.json' '(?m)^(\s*"version"\s*:\s*")([^"]+)(",?\s*$)' 'web npm manifest'
Replace-Text 'apps/desktop/src-tauri/tauri.conf.json' '(?m)^(\s*"version"\s*:\s*")([^"]+)(",?\s*$)' 'Tauri manifest'

Write-Host "Synchronized project manifests to $Version."
