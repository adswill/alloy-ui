# Builds the counter as an MSIX with a throwaway self-signed certificate,
# trusts that certificate, installs the package with Add-AppxPackage, checks it
# is registered and its files are there, then removes it.
# Usage: verify-msix.ps1 <path-to-alloy.exe>
param([Parameter(Mandatory)][string]$Alloy)
$ErrorActionPreference = 'Stop'
function Fail($m) { Write-Host "::error::$m"; exit 1 }
$out = Join-Path $PWD 'msix-out'
New-Item -ItemType Directory -Force $out | Out-Null

# The manifest Publisher defaults to CN=<publisher> from Cargo.toml: CN=Alloy.
$subject = 'CN=Alloy'
$cert = New-SelfSignedCertificate -Type Custom -Subject $subject -KeyUsage DigitalSignature `
  -FriendlyName 'Alloy CI MSIX' -CertStoreLocation 'Cert:\CurrentUser\My' `
  -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
$pw = 'ci-only-password'
$pfx = Join-Path $PWD 'ci-cert.pfx'
$cer = Join-Path $PWD 'ci-cert.cer'
Export-PfxCertificate -Cert $cert -FilePath $pfx -Password (ConvertTo-SecureString $pw -Force -AsPlainText) | Out-Null
Export-Certificate -Cert $cert -FilePath $cer | Out-Null
# Windows only installs a signed package whose certificate it trusts.
Import-Certificate -FilePath $cer -CertStoreLocation 'Cert:\LocalMachine\TrustedPeople' | Out-Null

$st = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -like '*\x64\*' } | Sort-Object FullName | Select-Object -Last 1
if (-not $st) { Fail 'signtool.exe not found on this runner' }
Write-Host "signtool: $($st.FullName)"
$env:PATH = "$($st.DirectoryName);$env:PATH"
$env:ALLOY_WIN_CERT = $pfx
$env:ALLOY_WIN_CERT_PASSWORD = $pw

& $Alloy bundle -p alloy-ui-examples --bin counter --format msix --out $out
if ($LASTEXITCODE -ne 0) { Fail 'alloy bundle --format msix failed' }
$msix = Get-ChildItem $out -Recurse -Filter *.msix | Select-Object -First 1
if (-not $msix) { Fail 'no .msix produced' }
Write-Host "built $($msix.FullName) ($($msix.Length) bytes)"
$sig = Get-AuthenticodeSignature $msix.FullName
Write-Host "signature status: $($sig.Status), signer: $($sig.SignerCertificate.Subject)"
if ($sig.SignerCertificate.Subject -ne $subject) { Fail "package not signed by $subject" }

try {
  Add-AppxPackage -Path $msix.FullName -ErrorAction Stop
} catch {
  Write-Host "Add-AppxPackage failed: $_"
  Get-AppxLastError | Out-String | Write-Host
  Fail 'MSIX did not install'
}
$pkg = Get-AppxPackage | Where-Object { $_.Name -eq 'dev.alloyui.examples' }
if (-not $pkg) { Fail 'Add-AppxPackage succeeded but the package is not registered' }
Write-Host "installed: $($pkg.PackageFullName) at $($pkg.InstallLocation)"
$exe = Join-Path $pkg.InstallLocation 'counter.exe'
if (-not (Test-Path $exe)) { Fail "no counter.exe under $($pkg.InstallLocation)" }

# Run the packaged copy (soft: WindowsApps paths can refuse direct launches).
$ran = 'not attempted'
try {
  $png = Join-Path $env:TEMP 'msix-counter.png'
  Remove-Item $png -ErrorAction SilentlyContinue
  & $exe --headless $png
  if ((Test-Path $png) -and (Get-Item $png).Length -gt 1000) { $ran = "rendered $((Get-Item $png).Length) bytes" } else { $ran = "exit $LASTEXITCODE, no frame" }
} catch { $ran = "launch refused: $_" }
Write-Host "packaged exe: $ran"

Remove-AppxPackage -Package $pkg.PackageFullName
if (Get-AppxPackage | Where-Object { $_.Name -eq 'dev.alloyui.examples' }) { Fail 'package still registered after removal' }

$md = "### MSIX on a real Windows runner`n`n- built and signed with a throwaway self-signed certificate ($($msix.Length) bytes)`n- ``Add-AppxPackage`` installed it, ``Get-AppxPackage`` listed it, ``counter.exe`` was in the install location`n- packaged exe: $ran`n- ``Remove-AppxPackage`` removed it`n"
if ($env:GITHUB_STEP_SUMMARY) { Add-Content $env:GITHUB_STEP_SUMMARY $md }
Write-Host $md
New-Item -ItemType Directory -Force bundle-out | Out-Null
Copy-Item $msix.FullName bundle-out\
