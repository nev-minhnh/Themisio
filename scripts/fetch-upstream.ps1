# Fetch the four Craft apps (and craft-i18n / craft-fonts, and draw.io's web app) at the commits in upstream.lock.
# Usage:  powershell -ExecutionPolicy Bypass -File scripts\fetch-upstream.ps1 [-Latest]
#   -Latest   check out each repository's newest main instead of the pinned commit
#             (may need small fixes in crates/themisio/src/hosts if an app's API changed).
param([switch]$Latest)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$up = Join-Path $root "upstream"
New-Item -ItemType Directory -Force -Path $up | Out-Null
if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw "git is not installed (winget install Git.Git)" }
Get-Content (Join-Path $root "upstream.lock") | Where-Object { $_ -and -not $_.StartsWith("#") } | ForEach-Object {
    $name, $url, $rev, $mode = -split $_
    $dir = Join-Path $up $name
    if ($mode -eq "shallow") {
        # Large repositories (draw.io): just the tree at the pinned commit.
        if (-not (Test-Path (Join-Path $dir ".git"))) {
            git init --quiet $dir
            git -C $dir remote add origin $url
        }
        $head = (git -C $dir rev-parse HEAD 2>$null)
        if ($head -ne $rev) {
            Write-Host "Fetching $name (shallow)…"
            git -C $dir -c core.autocrlf=false fetch --quiet --depth 1 origin $rev
            if ($LASTEXITCODE -ne 0) { throw "couldn't fetch $name" }
            git -C $dir -c core.autocrlf=false checkout --quiet FETCH_HEAD
        }
        Write-Host ("  {0} at {1}" -f $name, (git -C $dir rev-parse --short HEAD))
        return
    }
    if (-not (Test-Path (Join-Path $dir ".git"))) {
        Write-Host "Cloning $name…"
        git clone --quiet -c core.autocrlf=false $url $dir
    } else {
        Write-Host "Updating $name…"
        git -C $dir fetch --quiet origin
    }
    if ($Latest) { git -C $dir checkout --quiet origin/HEAD } else { git -C $dir checkout --quiet $rev }
    Write-Host ("  {0} at {1}" -f $name, (git -C $dir rev-parse --short HEAD))
    $patch = Join-Path $root "patches\$name.patch"
    if (Test-Path $patch) {
        git -C $dir checkout --quiet -- .
        git -C $dir apply --whitespace=nowarn --ignore-whitespace $patch
        if ($LASTEXITCODE -ne 0) { throw "patches\$name.patch doesn't apply to $name; see scripts/patch-upstream.py" }
        Write-Host "  applied Themisio's patch to $name"
    }
}
Write-Host "Done. Next: scripts\build.ps1"
