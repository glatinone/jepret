# Installs jepret for the current user: copies the release build and starts it at every logon.
# pwsh -File scripts\install.ps1 [-SaveDir <folder>]
param([string]$SaveDir)
$ErrorActionPreference = 'Stop'

$src = Join-Path $PSScriptRoot '..\target\release\jepret.exe'
if (-not (Test-Path $src)) { throw 'Build first: cargo build --release' }
$dir = Join-Path $env:LOCALAPPDATA 'Programs\jepret'
New-Item -ItemType Directory -Force $dir | Out-Null
Stop-Process -Name jepret -Force -ErrorAction SilentlyContinue
Copy-Item $src $dir -Force
$exe = Join-Path $dir 'jepret.exe'

if ($SaveDir) {
    New-Item -Path 'HKCU:\Software\jepret' -Force | Out-Null
    Set-ItemProperty 'HKCU:\Software\jepret' SaveDir $SaveDir
}

$user = "$env:USERDOMAIN\$env:USERNAME"
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $user
$trigger.Delay = 'PT5S'
$settings = New-ScheduledTaskSettingsSet -Priority 4 -ExecutionTimeLimit ([TimeSpan]::Zero) -RestartCount 3 `
    -RestartInterval (New-TimeSpan -Minutes 1) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -MultipleInstances IgnoreNew
Register-ScheduledTask -TaskName 'jepret' -Action (New-ScheduledTaskAction -Execute $exe) -Trigger $trigger -Settings $settings `
    -Principal (New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited) `
    -Description 'jepret screenshot tool' -Force | Out-Null
Start-ScheduledTask -TaskName 'jepret'
"jepret installed: $exe (starts at logon). PrtSc / F8 = region, Shift+PrtSc = full screen."
