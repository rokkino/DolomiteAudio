<#
.SYNOPSIS
    Registers the ZeroLatency Rust Kernel ASIO driver with Windows COM and DAW registry paths.
.DESCRIPTION
    Creates the necessary HKEY_LOCAL_MACHINE\SOFTWARE\ASIO and CLSID entries so that
    Reaper, Ableton Live, Cubase, FL Studio, and Studio One recognize the driver.
#>

param(
    [string]$DllPath = "$PSScriptRoot\target\release\audio_asio.dll"
)

$ErrorActionPreference = "Stop"

$Clsid = "{3E8C1234-A789-4B56-9DEF-0123456789AB}"
$DriverName = "ZeroLatency Rust Kernel ASIO"
$Description = "Ultra-Low-Latency Zero-Buffer Rust Kernel Audio Driver"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "  Registering ZeroLatency Rust ASIO Driver into Windows   " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Register under HKLM\SOFTWARE\ASIO
$AsioRegPath = "HKLM:\SOFTWARE\ASIO\$DriverName"
if (-not (Test-Path $AsioRegPath)) {
    New-Item -Path $AsioRegPath -Force | Out-Null
}
Set-ItemProperty -Path $AsioRegPath -Name "CLSID" -Value $Clsid
Set-ItemProperty -Path $AsioRegPath -Name "Description" -Value $Description
Write-Host "[OK] Registered ASIO Host Entry: $AsioRegPath" -ForegroundColor Green

# 2. Register COM InprocServer32
$ComRegPath = "HKLM:\SOFTWARE\Classes\CLSID\$Clsid\InprocServer32"
if (-not (Test-Path $ComRegPath)) {
    New-Item -Path $ComRegPath -Force | Out-Null
}
Set-ItemProperty -Path $ComRegPath -Name "(Default)" -Value (Resolve-Path $DllPath).Path
Set-ItemProperty -Path $ComRegPath -Name "ThreadingModel" -Value "Both"
Write-Host "[OK] Registered COM InprocServer32: $ComRegPath" -ForegroundColor Green

Write-Host "`nDriver successfully registered! DAWs will now see '$DriverName' in Audio Preferences." -ForegroundColor Yellow
