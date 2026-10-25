param(
  [Parameter(Mandatory)][string]$AppDir,
  [Parameter(Mandatory)][string]$ManagedRuntime,
  [Parameter(Mandatory)][string]$NpmPackages
)
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

if (-not (Test-Path -LiteralPath $ManagedRuntime)) { throw 'Managed runtime archive is missing' }
$staged = Join-Path $AppDir 'managed'
New-Item -ItemType Directory -Path $staged -Force | Out-Null
& "$env:SystemRoot\System32\tar.exe" -xzf $ManagedRuntime -C $staged
if ($LASTEXITCODE -ne 0) { throw 'Managed runtime archive extraction failed' }

$npm = Get-Command npm -ErrorAction SilentlyContinue
if (-not $npm) { throw 'npm is required to install SI OpenCode extensions' }
$packages = Get-Content -LiteralPath $NpmPackages -Raw | ConvertFrom-Json
if ($packages.npmRegistry -match 'PLACEHOLDER') { throw 'Set the Nexus npm registry in config/managed-delivery.json before building' }
foreach ($package in $packages.packages) {
    $target = Join-Path $staged $package.target
    New-Item -ItemType Directory -Path $target -Force | Out-Null
    Write-Host "[SI OpenCode] Installing $($package.package)..."
    & $npm.Source install --prefix $target --registry $packages.npmRegistry --ignore-scripts --no-audit --no-fund --package-lock=false "$($package.package)@$($package.version)"
    if ($LASTEXITCODE -ne 0) { throw "npm install failed: $($package.package)" }
}

& (Join-Path $AppDir 'si_opencode_updater.exe') --install-managed $AppDir
if ($LASTEXITCODE -ne 0) { throw 'Managed extension installation failed' }
