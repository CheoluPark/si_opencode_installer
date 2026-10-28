param(
    [string]$PublicKey = (Join-Path $env:LOCALAPPDATA 'SI OpenCode\release-keys\publisher-ed25519.pub')
)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
$payload = Join-Path $PSScriptRoot 'payload'
New-Item -ItemType Directory -Path $payload -Force | Out-Null

if (-not (Test-Path -LiteralPath $PublicKey)) { throw 'Publisher public key is required to build the updater' }
$env:SI_RELEASE_PUBLIC_KEY_PATH = (Resolve-Path -LiteralPath $PublicKey).Path

function Invoke-Cargo([string]$Arguments) {
    $log = Join-Path $env:TEMP ('si-opencode-cargo-' + [guid]::NewGuid() + '.log')
    try {
        $process = Start-Process -FilePath 'cmd.exe' -ArgumentList "/c cargo $Arguments > `"$log`" 2>&1" -Wait -PassThru -NoNewWindow
        Get-Content -LiteralPath $log | Write-Host
        if ($process.ExitCode -ne 0) { throw "cargo $Arguments failed" }
    } finally {
        Remove-Item -LiteralPath $log -Force -ErrorAction SilentlyContinue
    }
}

Invoke-Cargo 'test --locked --offline'
Invoke-Cargo 'build --release --locked --offline --bin si_opencode_updater'
Copy-Item 'target\release\si_opencode_updater.exe' (Join-Path $payload 'si_opencode_updater.exe') -Force

New-Item -ItemType Directory -Path out -Force | Out-Null
$compiler = Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'
if (-not (Test-Path $compiler)) { throw 'Install Inno Setup 6.5.2+' }
& $compiler 'installer\SI_OpenCode.iss'
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed' }
Write-Host 'Build complete. Publish the signed release separately to FileStation before distributing the installer.'
