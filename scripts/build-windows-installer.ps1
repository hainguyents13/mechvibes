# Mechvibes Windows Installer Build Script
# This script builds the release binary and creates a Windows installer using Inno Setup

param(
    [switch]$SkipBuild = $false
)

$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Mechvibes Windows Installer Builder" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Get project root directory (where this script is located)
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

Write-Host "Script directory: $ScriptDir" -ForegroundColor Gray
Write-Host "Project root: $ProjectRoot" -ForegroundColor Gray
Write-Host ""

# Change to project root
Set-Location $ProjectRoot
Write-Host "Working directory: $(Get-Location)" -ForegroundColor Gray
Write-Host ""

# Check if Cargo is installed
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: Cargo is not installed or not in PATH" -ForegroundColor Red
    exit 1
}

# Fail first, and with a clear message, if the Rust in use is older than the
# `rust-version` declared in Cargo.toml - the crate's minimum supported Rust.
# It is a floor, not an exact match: any newer compiler is fine. What this
# catches is a compiler too old to build the crate, which otherwise surfaces late
# as an unrelated-looking error. This is the PowerShell twin of
# verify_rust_toolchain in scripts/lib/common.sh.
function ConvertTo-RustVersion([string]$Text) {
    # "1.88", "1.88.0" or "1.89.0-nightly" -> [version], a missing part as 0.
    # Normalised to three parts on purpose: a bare [version]"1.88" has Build = -1
    # and so compares as OLDER than "1.88.0".
    if ($Text -notmatch '^\s*(\d+)\.(\d+)(?:\.(\d+))?') { return $null }
    $Patch = 0
    if ($Matches[3]) { $Patch = [int]$Matches[3] }
    return [version]::new([int]$Matches[1], [int]$Matches[2], $Patch)
}

function Get-ActiveRustVersion {
    try { return ((& rustc --version) -split ' ')[1] } catch { return "" }
}

$CargoTomlFile = Join-Path $ProjectRoot "Cargo.toml"
$FloorMatch = Select-String -Path $CargoTomlFile -Pattern '^\s*rust-version\s*=\s*"([^"]*)"' | Select-Object -First 1
if (-not $FloorMatch) {
    Write-Host "ERROR: Cargo.toml has no 'rust-version', so there is no minimum Rust to check against" -ForegroundColor Red
    exit 1
}
$FloorRust = $FloorMatch.Matches[0].Groups[1].Value
$FloorVersion = ConvertTo-RustVersion $FloorRust
if (-not $FloorVersion) {
    Write-Host "ERROR: could not read rust-version '$FloorRust' from Cargo.toml" -ForegroundColor Red
    exit 1
}

$GotRust = Get-ActiveRustVersion
$GotVersion = ConvertTo-RustVersion $GotRust
if ($null -eq $GotVersion -or $GotVersion -lt $FloorVersion) {
    Write-Host "ERROR: Rust '$GotRust' is active but Cargo.toml requires rust-version $FloorRust or newer." -ForegroundColor Red
    Write-Host "Update it ('rustup update stable'), or check for a RUSTUP_TOOLCHAIN override." -ForegroundColor Red
    exit 1
}
Write-Host "Rust toolchain OK: $GotRust (at least rust-version $FloorRust from Cargo.toml)" -ForegroundColor Green
Write-Host ""

# Read version from Cargo.toml (single source of truth - no more manual sync
# between Cargo.toml and the installer script).
$CargoTomlPath = Join-Path $ProjectRoot "Cargo.toml"
$CargoTomlContent = Get-Content $CargoTomlPath -Raw
if ($CargoTomlContent -notmatch '(?m)^\s*version\s*=\s*"([^"]+)"') {
    Write-Host "ERROR: Could not find version in Cargo.toml" -ForegroundColor Red
    exit 1
}
$AppVersion = $Matches[1]
Write-Host "App version (from Cargo.toml): $AppVersion" -ForegroundColor Cyan
Write-Host ""

# Step 1: Build release binary
if ($SkipBuild) {
    Write-Host "[1/4] Skipping build (using existing binary)" -ForegroundColor Yellow
    # Nothing is compiled here, so still validate the toolchain pin
    # (`rust-version` in Cargo.toml) and Cargo.lock.
    Write-Host "Running: cargo check --locked" -ForegroundColor Gray
    cargo check --locked
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: 'cargo check --locked' failed - check the Rust version against rust-version in Cargo.toml, or Cargo.lock" -ForegroundColor Red
        exit 1
    }
    Write-Host ""
}
if (-not $SkipBuild) {
    Write-Host "[1/4] Building release binary..." -ForegroundColor Yellow
    Write-Host "Running: cargo build --release --locked" -ForegroundColor Gray

    cargo build --release --locked

    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: Build failed" -ForegroundColor Red
        exit 1
    }

    Write-Host "Build completed successfully" -ForegroundColor Green
    Write-Host ""
}

# Check if executable exists
$ExePath = Join-Path $ProjectRoot "target\release\mechvibes.exe"
if (-not (Test-Path $ExePath)) {
    Write-Host "ERROR: Executable not found at $ExePath" -ForegroundColor Red
    Write-Host "Please run without -SkipBuild flag" -ForegroundColor Red
    exit 1
}

# -SkipBuild packages whatever binary is already there, which could be left over
# from an older checkout. Refuse a binary older than anything it is built from,
# rather than ship stale code under a new version number.
if ($SkipBuild) {
    $ExeTime = (Get-Item $ExePath).LastWriteTime
    $Sources = @("src", "assets", "patches", "Cargo.toml", "Cargo.lock", "build.rs") |
        ForEach-Object { Join-Path $ProjectRoot $_ } |
        Where-Object { Test-Path $_ }
    $Stale = @(Get-ChildItem -Path $Sources -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object { $_.LastWriteTime -gt $ExeTime } |
        Select-Object -First 5)
    if ($Stale.Count -gt 0) {
        Write-Host "ERROR: $ExePath is older than these files - rebuild it, or run without -SkipBuild:" -ForegroundColor Red
        $Stale | ForEach-Object { Write-Host "  $($_.FullName)" -ForegroundColor Red }
        exit 1
    }
}

# Get file version
$FileVersion = (Get-Item $ExePath).VersionInfo.FileVersion
Write-Host "Executable version: $FileVersion" -ForegroundColor Cyan
Write-Host ""

# Step 2: Create dist directory
Write-Host "[2/4] Preparing output directory..." -ForegroundColor Yellow
$DistDir = Join-Path $ProjectRoot "dist"
if (-not (Test-Path $DistDir)) {
    New-Item -ItemType Directory -Path $DistDir | Out-Null
}
Write-Host "Output directory ready: $DistDir" -ForegroundColor Green
Write-Host ""

# Step 3: Build installer with Inno Setup
Write-Host "[3/4] Building Inno Setup installer..." -ForegroundColor Yellow

# Check if Inno Setup is installed. Checks common install locations across
# the installer types users end up with (traditional installer under
# Program Files, or a per-user install e.g. via winget under
# LocalAppData\Programs).
$InnoSetupPaths = @(
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles}\Inno Setup 6\ISCC.exe",
    "${env:LOCALAPPDATA}\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 5\ISCC.exe",
    "${env:ProgramFiles}\Inno Setup 5\ISCC.exe"
)

$ISCC = $null
foreach ($path in $InnoSetupPaths) {
    if (Test-Path $path) {
        $ISCC = $path
        break
    }
}

if (-not $ISCC) {
    Write-Host "ERROR: Inno Setup not found" -ForegroundColor Red
    Write-Host "Please install Inno Setup from: https://jrsoftware.org/isinfo.php" -ForegroundColor Yellow
    exit 1
}

Write-Host "Found Inno Setup: $ISCC" -ForegroundColor Gray

$InnoScript = Join-Path $ProjectRoot "installer\windows\mechvibes-setup.iss"
if (-not (Test-Path $InnoScript)) {
    Write-Host "ERROR: Inno Setup script not found at $InnoScript" -ForegroundColor Red
    exit 1
}

Write-Host "Running Inno Setup compiler..." -ForegroundColor Gray
& $ISCC "/DAppVersion=$AppVersion" $InnoScript

if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Inno Setup compilation failed" -ForegroundColor Red
    exit 1
}

Write-Host "Inno Setup installer created successfully" -ForegroundColor Green
Write-Host ""

# Step 4: Summary
Write-Host "[4/4] Build Summary" -ForegroundColor Yellow
Write-Host "==================" -ForegroundColor Yellow
Write-Host "Executable: $ExePath" -ForegroundColor Gray
Write-Host "Output directory: $DistDir" -ForegroundColor Gray

$InstallerPath = Get-ChildItem -Path $DistDir -Filter "Mechvibes-*-Setup-x64.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($InstallerPath) {
    Write-Host "Installer: $($InstallerPath.FullName)" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Installer ready!" -ForegroundColor Green
}

# Verify the installer asset. The in-app auto-updater picks the first release
# asset whose name contains "x64" and ends in ".exe", so the name is part of the
# contract: asserted here rather than trusted.
$ExpectedInstaller = "Mechvibes-$AppVersion-Setup-x64.exe"
$ExpectedInstallerPath = Join-Path $DistDir $ExpectedInstaller
if (-not (Test-Path $ExpectedInstallerPath)) {
    Write-Host "ERROR: expected installer $ExpectedInstaller not found in $DistDir" -ForegroundColor Red
    exit 1
}
if ($ExpectedInstaller -notmatch "x64" -or $ExpectedInstaller -notmatch "\.exe$") {
    Write-Host "ERROR: installer name '$ExpectedInstaller' does not match the auto-updater's filter (must contain 'x64' and end in '.exe')" -ForegroundColor Red
    exit 1
}
Write-Host "Verified installer asset: $ExpectedInstallerPath" -ForegroundColor Green

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Build completed successfully!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
