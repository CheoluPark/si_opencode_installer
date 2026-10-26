param([Parameter(Mandatory)][string]$AppDir, [Parameter(Mandatory)][string]$ReleaseDir)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$toolsManifest = Get-Content -LiteralPath (Join-Path $ReleaseDir 'tools\tools-manifest.json') -Raw | ConvertFrom-Json
$desktopSpec = $toolsManifest.desktop
if (-not $desktopSpec -or -not $desktopSpec.installer -or -not $desktopSpec.sha256 -or -not $desktopSpec.minimum_version) {
    throw 'Release tools manifest has no complete Desktop installation specification'
}
$installerName = [string]$desktopSpec.installer
if ([IO.Path]::IsPathRooted($installerName) -or $installerName -match '(^|[\\/])\.\.([\\/]|$)') {
    throw 'Desktop installer path is invalid'
}
$desktopInstaller = Join-Path $ReleaseDir $installerName
$expectedHash = ([string]$desktopSpec.sha256).ToLowerInvariant()
if ($expectedHash -notmatch '^[0-9a-f]{64}$' -or (Get-FileHash -LiteralPath $desktopInstaller -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expectedHash) {
    throw 'Desktop installer does not match the release manifest hash'
}
$signature = Get-AuthenticodeSignature -LiteralPath $desktopInstaller
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'Anomaly Innovations') {
    throw 'Desktop installer signature is not valid for the expected publisher'
}
$desktop = Join-Path $env:LOCALAPPDATA 'Programs\@opencodedesktop\OpenCode.exe'
$cli = Join-Path $env:LOCALAPPDATA 'Programs\@opencodedesktop\resources\opencode-cli.exe'

function Get-DesktopVersion {
    if (-not (Test-Path -LiteralPath $desktop)) { return $null }
    $version = (Get-Item -LiteralPath $desktop).VersionInfo.ProductVersion
    try { return [version]($version -replace '[^0-9.].*$', '') } catch { return $null }
}

$minimumVersion = [version]$desktopSpec.minimum_version
$installedVersion = Get-DesktopVersion
if ($null -eq $installedVersion -or $installedVersion -lt $minimumVersion) {
    Write-Host '[SI OpenCode] Installing OpenCode Desktop...'
    $process = Start-Process -FilePath $desktopInstaller -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Desktop installer failed with exit code $($process.ExitCode)" }
}
$installedVersion = Get-DesktopVersion
if ($null -eq $installedVersion -or $installedVersion -lt $minimumVersion) { throw "Installed OpenCode Desktop is below required version $minimumVersion" }
if (-not (Test-Path -LiteralPath $cli)) { throw 'Desktop package did not install its bundled CLI' }
Copy-Item -LiteralPath $cli -Destination (Join-Path $AppDir 'opencode.exe') -Force

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (@($userPath -split ';') -notcontains $AppDir) {
    [Environment]::SetEnvironmentVariable('Path', (([string]$userPath).TrimEnd(';') + ';' + $AppDir), 'User')
}
