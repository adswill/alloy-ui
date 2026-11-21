# Dump the Windows UI Automation tree of a running process and require the counter's buttons.
param([int]$ProcessId, [string]$Out)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$AE = [System.Windows.Automation.AutomationElement]
$root = $AE::RootElement
$cond = New-Object System.Windows.Automation.PropertyCondition($AE::ProcessIdProperty, $ProcessId)
$win = $null
for ($i = 0; $i -lt 20 -and -not $win; $i++) { $win = $root.FindFirst('Children', $cond); if (-not $win) { Start-Sleep 1 } }
if (-not $win) { "UIA: no top-level window for pid $ProcessId" | Tee-Object $Out; exit 2 }
$all = $win.FindAll('Descendants', [System.Windows.Automation.Condition]::TrueCondition)
$lines = @("window: $($win.Current.Name) [$($win.Current.ControlType.ProgrammaticName)]")
foreach ($e in $all) { $lines += "  $($e.Current.ControlType.ProgrammaticName) name='$($e.Current.Name)'" }
$lines | Tee-Object $Out
$text = $lines -join "`n"
foreach ($want in 'Add', 'Subtract', 'Reset') {
  if ($text -notmatch "name='$want'") { "UIA: missing '$want'"; exit 3 }
}
"UIA OK"
