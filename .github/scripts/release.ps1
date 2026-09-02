[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Version,

    [Parameter(Mandatory = $false)]
    [switch]$Push,

    [Parameter(Mandatory = $false)]
    [switch]$ReplaceExistingTag
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$tag = if ($Version.StartsWith('v')) { $Version } else { "v$Version" }
if ($tag -notmatch '^v\d+\.\d+\.\d+$') {
    throw "Invalid version '$Version'. Expected X.Y.Z or vX.Y.Z."
}

$status = git status --porcelain
if ($status) {
    throw 'Working tree is not clean. Commit or stash unrelated changes before creating a release.'
}

$existingTag = git tag --list $tag
if ($existingTag -and -not $ReplaceExistingTag) {
    throw "Tag '$tag' already exists. Choose a new version or pass -ReplaceExistingTag to repair a failed release."
}

& "$PSScriptRoot/set-version.ps1" -Version ($tag.Substring(1))


& npm install --package-lock-only --ignore-scripts
if ($LASTEXITCODE -ne 0) {
    throw 'Failed to refresh package-lock.json.'
}

& cargo metadata --format-version 1 --no-deps | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw 'Failed to refresh Cargo.lock workspace versions.'
}
& cargo metadata --locked --format-version 1 --no-deps | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw 'Cargo.lock is stale or invalid.'
}

& "$PSScriptRoot/check-version.ps1" -Tag $tag

& npm ci
if ($LASTEXITCODE -ne 0) {
    throw 'JavaScript dependency installation failed.'
}
& npm run generate:wasm
if ($LASTEXITCODE -ne 0) {
    throw 'WASM generation failed.'
}
& npm run generate:manifest
if ($LASTEXITCODE -ne 0) {
    throw 'Manifest generation failed.'
}
& git diff --exit-code -- web/src/generated
if ($LASTEXITCODE -ne 0) {
    throw 'Generated web assets are stale.'
}
& npm test
if ($LASTEXITCODE -ne 0) {
    throw 'Frontend tests failed.'
}
& npx tsc -b --pretty false
if ($LASTEXITCODE -ne 0) {
    throw 'TypeScript checks failed.'
}
$fixtureTarget = Join-Path (Get-Location) 'pipeline/normal/P1020601.dng'
$expectedFixtureHash = 'ed8ebc01903b36bd6c86287d960c2244059105a6ae7b661a0efcfea1acdf2848'
$temporaryFixture = $false
try {
    if (-not (Test-Path -LiteralPath $fixtureTarget)) {
        $defaultFixture = Join-Path $HOME 'Documents/cao/99_data/isp/pana_gh5s/P1020601.dng'
        $fixtureSource = if ([string]::IsNullOrWhiteSpace($env:RIME_LOCAL_DNG_FIXTURE)) { $defaultFixture } else { $env:RIME_LOCAL_DNG_FIXTURE }
        if (-not (Test-Path -LiteralPath $fixtureSource)) {
            throw 'Local GH5S fixture not found. Set RIME_LOCAL_DNG_FIXTURE to the canonical P1020601.dng before releasing.'
        }
        Copy-Item -LiteralPath $fixtureSource -Destination $fixtureTarget
        $temporaryFixture = $true
    }

    $fixtureHash = (Get-FileHash -LiteralPath $fixtureTarget -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($fixtureHash -ne $expectedFixtureHash) {
        throw "Local GH5S fixture hash $fixtureHash does not match canonical hash $expectedFixtureHash."
    }

    & cargo test --workspace --all-targets --locked
    if ($LASTEXITCODE -ne 0) {
        throw 'Rust tests failed.'
    }
    & cargo clippy --workspace --all-targets --locked -- -D warnings
    if ($LASTEXITCODE -ne 0) {
        throw 'Clippy failed.'
    }
} finally {
    if ($temporaryFixture) {
        Remove-Item -LiteralPath $fixtureTarget -Force
    }
}

git add .github/scripts/set-version.ps1 Cargo.toml Cargo.lock package.json package-lock.json apps/desktop/package.json web/package.json apps/desktop/src-tauri/tauri.conf.json
if ($LASTEXITCODE -ne 0) {
    throw 'Failed to stage release manifests.'
}

git diff --cached --quiet
if ($LASTEXITCODE -eq 1) {
    git commit --message "chore: prepare $tag release"
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to commit release version $tag."
    }
} elseif ($LASTEXITCODE -ne 0) {
    throw 'Failed to inspect staged release changes.'
}

if ($ReplaceExistingTag) {
    git tag --force --annotate $tag --message "Release $tag"
} else {
    git tag --annotate $tag --message "Release $tag"
}
if ($LASTEXITCODE -ne 0) {
    throw "Failed to create tag '$tag'."
}
Write-Host "Prepared annotated tag $tag."

if ($Push) {
    git push origin main
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to push main.'
    }
    if ($ReplaceExistingTag) {
        git push --force origin $tag
    } else {
        git push origin $tag
    }
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to push $tag."
    }
    Write-Host "Pushed main and $tag. GitHub Actions will build the Windows release."
} else {
    $tagPush = if ($ReplaceExistingTag) { "git push --force origin $tag" } else { "git push origin $tag" }
    Write-Host "Push with: git push origin main; $tagPush"
}
