# Build Themisio for Windows 11 (x64 or ARM64).
# Usage:  powershell -ExecutionPolicy Bypass -File scripts\build.ps1 [-Profile dist|release|quick|dev] [-WithCraftFonts] [-Register]
#   -Profile         dist (default: what the GitHub releases ship, small and quick to start), release (LTO), quick (builds faster), dev
#   -WithCraftFonts  embed craft-fonts (Japanese/Chinese UI faces, PowerPoint-metric Latin fonts;
#                    the exe grows by ~80 MB)
#   -Register        register Themisio's file types for this user after building
# Needs: Rust 1.95+ (rustup, MSVC toolchain + "Desktop development with C++" build tools), git.
param([string]$Profile = "dist", [switch]$WithCraftFonts, [switch]$Register)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
if (-not (Test-Path "upstream\wordcraft\Cargo.toml")) { & "$PSScriptRoot\fetch-upstream.ps1" }
if ($WithCraftFonts) { $env:CRAFT_FONTS_DIR = (Resolve-Path "upstream\craft-fonts").Path } else { Remove-Item Env:CRAFT_FONTS_DIR -ErrorAction SilentlyContinue }
$env:CRAFT_I18N_DIR = (Resolve-Path "upstream\craft-i18n").Path
$env:CRAFT_I18N_REQUIRED = "1"
$cargoProfile = if ($Profile -eq "dev") { "dev" } else { $Profile }
cargo build -p themisio --profile $cargoProfile
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
$outDir = if ($Profile -eq "dev") { "debug" } else { $Profile }
$exe = Join-Path $root "target\$outDir\themisio.exe"
$dist = Join-Path $root "dist"
New-Item -ItemType Directory -Force -Path $dist | Out-Null
Copy-Item $exe (Join-Path $dist "themisio.exe") -Force
Write-Host "Built $dist\themisio.exe"
if ($Register) { & (Join-Path $dist "themisio.exe") --register-file-types; Write-Host "File types registered (Settings > Apps > Default apps > Themisio to make it the default)." }
