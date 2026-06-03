<#
.SYNOPSIS
    Build a shippable Venore Windows installer (NSIS, single-click .exe).

.DESCRIPTION
    One reproducible command for producing a release installer. It:
      1. Refuses to build if the `devtools` feature is still enabled
         (DevTools must never ship to users).
      2. Closes any running venore-desktop.exe so the linker can replace it
         (avoids the "Acceso denegado (os error 5)" link failure).
      3. Removes the previous binary + bundle so a stale artifact can't be
         mistaken for a fresh one.
      4. Runs `cargo tauri build` — this also runs the frontend build
         (`pnpm run build`) and the NSIS bundler.
      5. Prints the final installer path + size.

.EXAMPLE
    pwsh scripts/release.ps1

.NOTES
    Run from anywhere; paths resolve relative to this script.
    The build is unsigned — Windows SmartScreen will warn on first run.
#>

$ErrorActionPreference = "Stop"

# ── Resolve paths relative to this script ───────────────────────────
$RepoRoot   = Split-Path -Parent $PSScriptRoot
$DesktopDir = Join-Path $RepoRoot "crates\venore-desktop"
$CargoToml  = Join-Path $DesktopDir "Cargo.toml"
$ReleaseDir = Join-Path $RepoRoot "target\release"
$ExePath    = Join-Path $ReleaseDir "venore-desktop.exe"
$BundleDir  = Join-Path $ReleaseDir "bundle\nsis"

Write-Host "=== Venore release build ===" -ForegroundColor Cyan
Write-Host "Repo: $RepoRoot"

# ── 1. Guard: devtools must NOT be enabled ──────────────────────────
$tauriLine = Select-String -Path $CargoToml -Pattern '^\s*tauri\s*=\s*\{.*\}' |
    Select-Object -First 1
if ($null -eq $tauriLine) {
    throw "Could not find the `tauri = { ... }` dependency line in $CargoToml"
}
if ($tauriLine.Line -match 'devtools') {
    Write-Host ""
    Write-Host "ABORT: the 'devtools' feature is still enabled:" -ForegroundColor Red
    Write-Host "  $($tauriLine.Line.Trim())" -ForegroundColor Red
    Write-Host "Remove 'devtools' from the tauri features before shipping." -ForegroundColor Red
    exit 1
}
Write-Host "[1/5] devtools guard OK (not enabled)" -ForegroundColor Green

# ── 2. Close any running instance (prevents linker lock) ────────────
$running = Get-Process -Name "venore-desktop" -ErrorAction SilentlyContinue
if ($running) {
    Write-Host "[2/5] Closing $($running.Count) running venore-desktop.exe ..." -ForegroundColor Yellow
    $running | Stop-Process -Force
    Start-Sleep -Milliseconds 700
} else {
    Write-Host "[2/5] No running instance" -ForegroundColor Green
}

# ── 3. Clean previous artifacts ─────────────────────────────────────
if (Test-Path $ExePath) { Remove-Item $ExePath -Force }
if (Test-Path $BundleDir) { Remove-Item $BundleDir -Recurse -Force }
Write-Host "[3/5] Cleaned old binary + NSIS bundle" -ForegroundColor Green

# ── 4. Build (frontend + release binary + NSIS installer) ───────────
Write-Host "[4/5] Running 'cargo tauri build' (this takes several minutes) ..." -ForegroundColor Cyan
Push-Location $DesktopDir
try {
    cargo tauri build
    if ($LASTEXITCODE -ne 0) { throw "cargo tauri build failed (exit $LASTEXITCODE)" }
}
finally {
    Pop-Location
}

# ── 5. Report the installer ─────────────────────────────────────────
$installer = Get-ChildItem -Path $BundleDir -Filter "*-setup.exe" -ErrorAction SilentlyContinue |
    Select-Object -First 1
if ($null -eq $installer) {
    throw "Build finished but no *-setup.exe was found in $BundleDir"
}
$sizeMb = [math]::Round($installer.Length / 1MB, 1)
Write-Host ""
Write-Host "[5/5] Installer ready:" -ForegroundColor Green
Write-Host "  $($installer.FullName)" -ForegroundColor White
Write-Host "  $sizeMb MB" -ForegroundColor White
Write-Host ""
Write-Host "Note: unsigned build — SmartScreen will warn on first run." -ForegroundColor DarkGray
