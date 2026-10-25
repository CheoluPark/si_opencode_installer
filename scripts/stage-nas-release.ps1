param(
    [Parameter(Mandatory)][string]$PrivateKey,
    [Parameter(Mandatory)][string]$PublicKey,
    [string]$Settings = '',
    [Parameter(Mandatory)][ValidateRange(1, [long]::MaxValue)][long]$Version,
    [Parameter(Mandatory)][string]$Destination
)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot)
if (Test-Path -LiteralPath $Destination) { throw 'Refusing to replace an existing NAS release staging directory' }
$source = Join-Path $PWD 'payload'
if (-not (Test-Path (Join-Path $source 'opencode_desktop_win64.exe'))) { throw 'Desktop installer missing' }
if (-not (Test-Path (Join-Path $source 'managed'))) { throw 'Managed extensions missing' }
New-Item -ItemType Directory -Path $Destination | Out-Null
Copy-Item (Join-Path $source 'managed\*') $Destination -Recurse
New-Item -ItemType Directory -Path (Join-Path $Destination 'desktop') | Out-Null
Copy-Item (Join-Path $source 'opencode_desktop_win64.exe') (Join-Path $Destination 'desktop\opencode_desktop_win64.exe')
if ($Settings) {
    & 'target\release\release.exe' sign (Resolve-Path -LiteralPath $PrivateKey) $Destination $Version ($Destination + '.json') ($Destination + '.sig') (Resolve-Path -LiteralPath $Settings)
} else {
    & 'target\release\release.exe' sign (Resolve-Path -LiteralPath $PrivateKey) $Destination $Version ($Destination + '.json') ($Destination + '.sig')
}
if ($LASTEXITCODE -ne 0) { throw 'NAS release signing failed' }
& 'target\release\release.exe' verify (Resolve-Path -LiteralPath $PublicKey) ($Destination + '.json') ($Destination + '.sig')
if ($LASTEXITCODE -ne 0) { throw 'NAS manifest signature does not match the updater public key' }
Move-Item -LiteralPath ($Destination + '.json') -Destination (Join-Path $Destination 'manifest.json')
Move-Item -LiteralPath ($Destination + '.sig') -Destination (Join-Path $Destination 'manifest.sig')
Write-Host 'Upload release folder CONTENTS to the configured FileStation directory after verifying access.'
