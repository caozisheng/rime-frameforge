[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Version,

    [Parameter(Mandatory = $false)]
    [switch]$Push
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$tag = if ($Version.StartsWith('v')) { $Version } else { "v$Version" }
if ($tag -notmatch '^v\d+\.\d+\.\d+$') {
    throw "Invalid version '$Version'. Expected X.Y.Z or vX.Y.Z."
}

& "$PSScriptRoot/check-version.ps1" -Tag $tag
if ($LASTEXITCODE -ne 0) {
    throw "Version validation failed for $tag."
}

$status = git status --porcelain
if ($status) {
    throw 'Working tree is not clean. Commit the version update before creating a release tag.'
}

$existingTag = git tag --list $tag
if ($existingTag) {
    throw "Tag '$tag' already exists."
}

git tag --annotate $tag --message "Release $tag"
Write-Host "Created annotated tag $tag."

if ($Push) {
    git push origin main
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to push main.'
    }
    git push origin $tag
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to push $tag."
    }
    Write-Host "Pushed main and $tag. GitHub Actions will build the Windows release."
} else {
    Write-Host "Push with: git push origin main; git push origin $tag"
}
