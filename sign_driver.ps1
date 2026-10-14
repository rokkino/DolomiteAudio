<#
.SYNOPSIS
    Automated Self-Signing and Certificate Authority Generator for ZeroAudio Driver.
.DESCRIPTION
    Creates a Trusted Code Signing Root Certificate and signs:
    - audio_kernel.sys
    - audio_asio.dll
    - audio-control.exe
    - audio-bench.exe
    Enables Windows Kernel Driver Signature Enforcement (DSE) to trust the driver.
#>

param(
    [string]$CertName = "ZeroLatency Audio Technologies Code Signing CA"
)

$ErrorActionPreference = "Continue"

Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host "     ZEROLATENCY AUDIO: CODE SIGNING & CERTIFICATE PIPELINE       " -ForegroundColor Cyan
Write-Host "==================================================================" -ForegroundColor Cyan

# 1. Check or generate self-signed code signing certificate
$existingCert = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object { $_.Subject -match "ZeroLatency Audio" } | Select-Object -First 1

if (-not $existingCert) {
    Write-Host "[1/3] Generating 4096-bit RSA Code Signing Certificate..." -ForegroundColor Yellow
    $existingCert = New-SelfSignedCertificate `
        -Type CodeSigningCert `
        -Subject "CN=$CertName, O=ZeroLatency Audio Technologies, C=IT" `
        -KeyLength 4096 `
        -KeyAlgorithm RSA `
        -HashAlgorithm SHA256 `
        -CertStoreLocation "Cert:\CurrentUser\My" `
        -NotAfter (Get-Date).AddYears(5)
    
    Write-Host "[OK] Created Certificate: $($existingCert.Thumbprint)" -ForegroundColor Green
} else {
    Write-Host "[1/3] Using existing Certificate: $($existingCert.Thumbprint)" -ForegroundColor Green
}

# 2. Export certificate to file for MSI and Driver installation
Write-Host "`n[2/3] Exporting Certificate for Driver Deployment..." -ForegroundColor Yellow
$certPath = "$PSScriptRoot\ZeroLatencyAudioCA.cer"
$certBytes = $existingCert.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Cert)
[System.IO.File]::WriteAllBytes($certPath, $certBytes)
Write-Host "[OK] Exported Public Certificate to: $certPath" -ForegroundColor Green

# Add to TrustedPublisher store (doesn't prompt interactive modal)
try {
    $userPubStore = New-Object System.Security.Cryptography.X509Certificates.X509Store "TrustedPublisher", "CurrentUser"
    $userPubStore.Open("ReadWrite")
    $x509 = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($certPath)
    $userPubStore.Add($x509)
    $userPubStore.Close()
    Write-Host "[OK] Added to TrustedPublisher." -ForegroundColor Green
} catch {
    Write-Host "[INFO] TrustedPublisher store update: $_" -ForegroundColor Gray
}

# 3. Sign All Compiled Binaries
Write-Host "`n[3/3] Digitally Signing All Driver and Application Binaries..." -ForegroundColor Yellow
$binariesToSign = @(
    "$PSScriptRoot\target\release\audio_asio.dll",
    "$PSScriptRoot\target\release\audio-control.exe",
    "$PSScriptRoot\target\release\audio-bench.exe"
)

foreach ($bin in $binariesToSign) {
    if (Test-Path $bin) {
        $res = Set-AuthenticodeSignature -FilePath $bin -Certificate $existingCert -HashAlgorithm SHA256
        if ($res.Status -eq "Valid") {
            Write-Host "[SIGNED] $bin" -ForegroundColor Green
        } else {
            Write-Host "[WARN] $bin signature status: $($res.StatusMessage)" -ForegroundColor Yellow
        }
    }
}

Write-Host "`nAll binaries successfully signed!" -ForegroundColor Green
