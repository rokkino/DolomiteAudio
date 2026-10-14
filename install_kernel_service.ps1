<#
.SYNOPSIS
    Installs and launches the ZeroLatencyAudio Kernel Driver (.sys) on Windows.
.NOTES
    Requires Administrator privileges and Windows Test-Signing mode enabled:
    bcdedit /set testsigning on (reboot required)
#>

param(
    [string]$SysPath = "$PSScriptRoot\target\release\audio_kernel.sys"
)

$ServiceName = "ZeroLatencyAudio"
$DisplayName = "ZeroLatency Rust Kernel Audio Driver"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   Installing ZeroLatency Kernel Service ($ServiceName)    " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# Stop and delete existing service if present
sc.exe stop $ServiceName 2>$null | Out-Null
sc.exe delete $ServiceName 2>$null | Out-Null

# Create new kernel-type service
$fullSysPath = (Resolve-Path $SysPath).Path
sc.exe create $ServiceName binPath= "$fullSysPath" type= kernel start= demand DisplayName= "$DisplayName"

if ($LASTEXITCODE -eq 0) {
    Write-Host "[OK] Kernel service '$ServiceName' created successfully." -ForegroundColor Green
    Write-Host "Attempting to start driver..." -ForegroundColor Cyan
    sc.exe start $ServiceName
    if ($LASTEXITCODE -eq 0) {
        Write-Host "[OK] Driver started! Device \\Device\\ZeroLatencyAudio is active." -ForegroundColor Green
    } else {
        Write-Warning "Could not start service directly. Ensure Driver Signature Enforcement (DSE) is in Test Mode or certificate is trusted."
    }
} else {
    Write-Error "Failed to create service. Make sure PowerShell is running as Administrator."
}
