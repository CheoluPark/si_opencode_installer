param([Parameter(Mandatory)][string]$AppDir)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$desktopInstaller = Join-Path $AppDir 'opencode_desktop_win64.exe'
$desktop = Join-Path $env:LOCALAPPDATA 'Programs\@opencodedesktop\OpenCode.exe'
$cli = Join-Path $env:LOCALAPPDATA 'Programs\@opencodedesktop\resources\opencode-cli.exe'

if ((Get-AuthenticodeSignature -LiteralPath $desktopInstaller).Status -ne 'Valid') { throw 'Desktop installer signature is not valid' }
if (-not (Test-Path -LiteralPath $desktop)) {
    Write-Host '[SI OpenCode] Installing OpenCode Desktop...'
    $process = Start-Process -FilePath $desktopInstaller -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Desktop installer failed with exit code $($process.ExitCode)" }
}
if (-not (Test-Path -LiteralPath $cli)) { throw 'Desktop package did not install its bundled CLI' }
Copy-Item -LiteralPath $cli -Destination (Join-Path $AppDir 'opencode.exe') -Force

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (@($userPath -split ';') -notcontains $AppDir) {
    [Environment]::SetEnvironmentVariable('Path', (([string]$userPath).TrimEnd(';') + ';' + $AppDir), 'User')
}
