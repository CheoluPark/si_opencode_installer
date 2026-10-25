param([Parameter(Mandatory)][string]$AppDir, [int]$Fpga)
$ErrorActionPreference = 'Stop'
$updater = Join-Path $AppDir 'si_opencode_updater.exe'
Write-Host '[SI OpenCode] Creating the daily update check task...'
if (-not (Test-Path -LiteralPath $updater)) { throw 'Updater executable is missing' }
$action = New-ScheduledTaskAction -Execute $updater -Argument '--update' -WorkingDirectory $AppDir
$trigger = New-ScheduledTaskTrigger -Daily -At '13:00'
$principal = New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -ExecutionTimeLimit (New-TimeSpan -Hours 1)
Register-ScheduledTask -TaskName 'SI OpenCode Update Check' -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Force | Out-Null
if ($Fpga -eq 1) {
    Write-Host '[SI OpenCode] Creating the FPGA knowledge base startup task...'
    $start = Join-Path $AppDir 'start-kb.ps1'
    if (-not (Test-Path -LiteralPath $start)) { throw 'KB startup script missing' }
    $kbAction = New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -ExecutionPolicy Bypass -File "' + $start + '"')
    $kbTrigger = New-ScheduledTaskTrigger -AtLogOn -User ([Security.Principal.WindowsIdentity]::GetCurrent().Name)
    Register-ScheduledTask -TaskName 'SI OpenCode KB Startup' -Action $kbAction -Trigger $kbTrigger -Principal $principal -Settings $settings -Force | Out-Null
}
