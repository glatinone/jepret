# Removes jepret: the logon task, the program folder and its settings. Saved screenshots are kept.
Unregister-ScheduledTask -TaskName 'jepret' -Confirm:$false -ErrorAction SilentlyContinue
Stop-Process -Name jepret -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 300
Remove-Item -LiteralPath (Join-Path $env:LOCALAPPDATA 'Programs\jepret') -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath 'HKCU:\Software\jepret' -Recurse -ErrorAction SilentlyContinue
'jepret removed (screenshots were kept).'
