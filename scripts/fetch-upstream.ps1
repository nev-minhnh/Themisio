# Fetch the four Craft apps (and craft-i18n / craft-fonts) at the commits in upstream.lock.
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
    $name, $url, $rev = -split $_
    $dir = Join-Path $up $name
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
