# Bundles the counter example for Windows and proves each artifact on a real
# runner: exe icon resource, NSIS silent install/uninstall, MSI install/uninstall.
# Usage: verify-windows.ps1 <path-to-alloy.exe>
param([Parameter(Mandatory)][string]$Alloy)
$ErrorActionPreference = 'Stop'
$out = Join-Path $PWD 'bundle-out'
Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
$summary = @()

function Step($t) { Write-Host "::group::$t" }
function EndStep { Write-Host '::endgroup::' }
function Fail($m) { Write-Host "::error::$m"; exit 1 }

Step 'alloy bundle'
& $Alloy bundle -p alloy-ui-examples --bin counter --format exe,nsis,msi --out $out
if ($LASTEXITCODE -ne 0) { Fail 'alloy bundle failed' }
Get-ChildItem -Recurse $out | Where-Object { -not $_.PSIsContainer } | ForEach-Object { Write-Host ("{0,12:N0}  {1}" -f $_.Length, $_.FullName.Substring($out.Length)) }
EndStep

$exe = Get-ChildItem $out -Recurse -Filter counter.exe | Select-Object -First 1
$nsis = Get-ChildItem $out -Filter *-setup.exe | Select-Object -First 1
$msi = Get-ChildItem $out -Filter *.msi | Select-Object -First 1
if (-not $exe -or -not $nsis -or -not $msi) { Fail 'missing an expected artifact' }
$summary += "| counter.exe (portable, icon embedded) | $($exe.Length) |"
$summary += "| NSIS setup | $($nsis.Length) |"
$summary += "| MSI | $($msi.Length) |"

# --- icon resource -------------------------------------------------------
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Shell {
  [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
  public static extern uint ExtractIconEx(string file, int index, IntPtr[] large, IntPtr[] small, uint n);
  [DllImport("user32.dll")] public static extern bool DestroyIcon(IntPtr h);
}
'@
function Test-ExeIcon($path) {
  # ExtractIconEx(-1) counts the RT_GROUP_ICON resources: 0 means no icon.
  $n = [Shell]::ExtractIconEx($path, -1, $null, $null, 0)
  if ($n -lt 1) { Fail "no icon resource in $path" }
  $large = New-Object IntPtr[] 1
  [void][Shell]::ExtractIconEx($path, 0, $large, $null, 1)
  $icon = [System.Drawing.Icon]::FromHandle($large[0])
  $bmp = $icon.ToBitmap()
  $px = $bmp.GetPixel($bmp.Width / 2, $bmp.Height / 2)
  Write-Host "icon groups: $n, first icon $($bmp.Width)x$($bmp.Height), centre pixel $px"
  [Shell]::DestroyIcon($large[0]) | Out-Null
  # Our icon is a blue-ish gradient (b > r); the generic Windows icon is not.
  if ($px.A -lt 200 -or $px.B -le $px.R) { Fail "extracted icon from $path is not the Alloy icon: $px" }
}
function Test-Runs($path) {
  $png = Join-Path $env:TEMP 'counter-check.png'
  Remove-Item $png -ErrorAction SilentlyContinue
  & $path --headless $png
  if ($LASTEXITCODE -ne 0 -or -not (Test-Path $png) -or (Get-Item $png).Length -lt 1000) { Fail "$path --headless did not render a frame" }
  Write-Host "$path rendered $((Get-Item $png).Length) bytes of PNG"
}

Step 'portable exe: embedded icon and run'
Test-ExeIcon $exe.FullName
Test-Runs $exe.FullName
EndStep

$arp = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall'

# --- NSIS ----------------------------------------------------------------
Step 'NSIS: silent install, run, silent uninstall'
$p = Start-Process $nsis.FullName -ArgumentList '/S' -Wait -PassThru
if ($p.ExitCode -ne 0) { Fail "NSIS installer exit code $($p.ExitCode)" }
$dir = Join-Path $env:LOCALAPPDATA 'Programs\Alloy Counter'
$inst = Join-Path $dir 'counter.exe'
if (-not (Test-Path $inst)) { Fail "NSIS did not install $inst" }
if (-not (Test-Path "$arp\dev.alloyui.examples")) { Fail 'NSIS wrote no uninstall registry key' }
$lnk = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Alloy Counter.lnk'
if (-not (Test-Path $lnk)) { Fail 'NSIS wrote no Start menu shortcut' }
Test-ExeIcon $inst
Test-Runs $inst
# `_?=` makes the uninstaller run in-place so -Wait really waits.
$p = Start-Process (Join-Path $dir 'Uninstall.exe') -ArgumentList '/S', "_?=$dir" -Wait -PassThru
if ($p.ExitCode -ne 0) { Fail "NSIS uninstaller exit code $($p.ExitCode)" }
if (Test-Path $inst) { Fail 'uninstall left counter.exe behind' }
if (Test-Path "$arp\dev.alloyui.examples") { Fail 'uninstall left the registry key' }
if (Test-Path $lnk) { Fail 'uninstall left the Start menu shortcut' }
Write-Host 'NSIS install and uninstall verified'
EndStep

# --- MSI -----------------------------------------------------------------
Step 'MSI: silent install, run, silent uninstall'
$log = Join-Path $PWD 'msi-install.log'
$p = Start-Process msiexec -ArgumentList '/i', "`"$($msi.FullName)`"", '/qn', '/l*v', "`"$log`"" -Wait -PassThru
if ($p.ExitCode -ne 0) { Get-Content $log -Tail 60; Fail "msiexec /i exit code $($p.ExitCode)" }
$found = Get-ChildItem (Join-Path $env:LOCALAPPDATA 'Programs') -Recurse -Filter counter.exe -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $found) { Get-Content $log -Tail 60; Fail 'MSI installed no counter.exe under %LOCALAPPDATA%\Programs' }
Write-Host "MSI installed $($found.FullName)"
Test-ExeIcon $found.FullName
Test-Runs $found.FullName
$p = Start-Process msiexec -ArgumentList '/x', "`"$($msi.FullName)`"", '/qn', '/l*v', "`"$PWD\msi-uninstall.log`"" -Wait -PassThru
if ($p.ExitCode -ne 0) { Fail "msiexec /x exit code $($p.ExitCode)" }
if (Test-Path $found.FullName) { Fail 'MSI uninstall left counter.exe behind' }
Write-Host 'MSI install and uninstall verified'
EndStep

$md = "### Windows bundle sizes (counter, real run)`n`n| Artifact | Bytes |`n|---|---|`n" + ($summary -join "`n") + "`n"
if ($env:GITHUB_STEP_SUMMARY) { Add-Content $env:GITHUB_STEP_SUMMARY $md }
Write-Host $md
