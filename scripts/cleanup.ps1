$ErrorActionPreference = 'Stop'
foreach ($name in @('SI OpenCode Update Check', 'SI OpenCode KB Startup')) {
    if (Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue) {
        Unregister-ScheduledTask -TaskName $name -Confirm:$false
    }
}
