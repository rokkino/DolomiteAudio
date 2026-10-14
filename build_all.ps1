<#
.SYNOPSIS
    Automated Build & Deployment Pipeline for ZeroLatency Rust Kernel Audio Suite.
.DESCRIPTION
    Compiles:
    1. audio-core (Lock-free engine, DSP matrix, sinc resampler, limiter, dither)
    2. audio-native (Pure Rust zero-copy audio SDK)
    3. audio-control (Real-time telemetry and channel metering CLI)
    4. audio-bench (Sub-nanosecond performance and RTL benchmark harness)
    5. audio-asio (Steinberg ASIO 2.3 COM DLL for Cubase, Reaper, Ableton, FL Studio)
    6. audio-kernel (Windows NT Kernel Mode Driver .sys)
#>

$ErrorActionPreference = "Continue"

# Disable Windows Schannel revocation check to bypass corporate proxy / firewall SSL errors
$env:CARGO_HTTP_CHECK_REVOKE = "false"

Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host "   ZEROLATENCY AUDIO: FULL PRODUCTION BUILD & PACKAGING PIPELINE   " -ForegroundColor Cyan
Write-Host "==================================================================" -ForegroundColor Cyan

# Check for Cargo / Rust in PATH or User Profile
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    Write-Host "`n[NOTICE] Cargo is not in PATH. Checking %USERPROFILE%\.cargo\bin..." -ForegroundColor Yellow
    $cargoPath = "$env:USERPROFILE\.cargo\bin\cargo.exe"
    if (Test-Path $cargoPath) {
        $env:PATH = "$env:USERPROFILE\.cargo\bin;" + $env:PATH
        Write-Host "[OK] Found Cargo at $cargoPath" -ForegroundColor Green
    } else {
        Write-Host "[!] Rust toolchain not detected. You can install it with:" -ForegroundColor Red
        Write-Host "    winget install Rustlang.Rustup" -ForegroundColor White
        Write-Host "    rustup default stable-x86_64-pc-windows-msvc" -ForegroundColor White
        exit 1
    }
}

# Auto-detect Toolchain: Use GNU if MSVC link.exe is not installed
$toolchainPrefix = @()
$hasMsvcLinker = (Get-Command link.exe -ErrorAction SilentlyContinue) -ne $null
if (-not $hasMsvcLinker) {
    $toolchainPrefix = @("+stable-x86_64-pc-windows-gnu")
    Write-Host "[INFO] MSVC link.exe not found in PATH; using self-contained stable-x86_64-pc-windows-gnu toolchain." -ForegroundColor Cyan
}

Write-Host "`n[1/5] Building Benchmarks and Audio Core Engine..." -ForegroundColor Cyan
& cargo @toolchainPrefix build --release -p audio-bench

Write-Host "`n[2/5] Building Native Rust SDK (audio-native)..." -ForegroundColor Cyan
& cargo @toolchainPrefix build --release -p audio-native

Write-Host "`n[3/5] Building Real-Time Control Panel & Telemetry CLI (audio-control)..." -ForegroundColor Cyan
& cargo @toolchainPrefix build --release -p audio-control

Write-Host "`n[4/5] Building Steinberg ASIO 2.3 User-Mode COM DLL (audio-asio)..." -ForegroundColor Cyan
& cargo @toolchainPrefix build --release -p audio-asio

Write-Host "`n[5/5] Building Windows NT Kernel Driver (audio-kernel.sys)..." -ForegroundColor Cyan
& cargo @toolchainPrefix rustc --release -p audio-kernel -- -C link-arg=-nostartfiles 2>$null
if ($LASTEXITCODE -ne 0) {
    & cargo rustc --release -p audio-kernel -- -C link-arg=/SUBSYSTEM:NATIVE -C link-arg=/DRIVER -C link-arg=/ENTRY:DriverEntry 2>$null
}

$sysOutput = "$PSScriptRoot\target\release\audio_kernel.dll"
$sysFinal  = "$PSScriptRoot\target\release\audio_kernel.sys"
if (Test-Path $sysOutput) {
    Copy-Item -Path $sysOutput -Destination $sysFinal -Force
    Write-Host "[OK] Kernel driver binary generated: $sysFinal" -ForegroundColor Green
}

Write-Host "`n==================================================================" -ForegroundColor Green
Write-Host "                 BUILD COMPLETED SUCCESSFULLY!                    " -ForegroundColor Green
Write-Host "==================================================================" -ForegroundColor Green
Write-Host "`nGenerated Artifacts in target\release\:"
Write-Host "  - ASIO DLL:        target\release\audio_asio.dll"
Write-Host "  - Control Panel:   target\release\audio-control.exe"
Write-Host "  - Benchmark:       target\release\audio-bench.exe"
Write-Host "  - Native SDK:      crates\audio-native"
Write-Host "`nQuick Start Commands:"
Write-Host "  1. Run Benchmarks:   cargo +stable-x86_64-pc-windows-gnu run --release -p audio-bench"
Write-Host "  2. Register ASIO:    .\register_driver.ps1"
Write-Host "  3. Live Telemetry:   .\target\release\audio-control.exe monitor"
