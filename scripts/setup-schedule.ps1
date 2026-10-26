param([Parameter(Mandatory)][string]$AppDir)
$ErrorActionPreference = 'Stop'
$updater = Join-Path $AppDir 'si_opencode_updater.exe'
$action = New-ScheduledTaskAction -Execute $updater -Argument '--check-update' -WorkingDirectory $AppDir
$trigger = New-ScheduledTaskTrigger -Daily -At 10:00AM
$trigger.Repetition.Interval = 'PT3H'
$trigger.Repetition.Duration = 'PT12H'
Register-ScheduledTask -TaskName 'SI OpenCode Update Check' -Action $action -Trigger $trigger -Description 'Checks SI OpenCode releases at 10:00, 13:00, 16:00, 19:00, and 22:00.' -Force | Out-Null
