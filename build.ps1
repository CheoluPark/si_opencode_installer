param(
    [Parameter(Mandatory)][string]$PrivateKey,
    [Parameter(Mandatory)][string]$PublicKey,
    [Parameter(Mandatory)][ValidateRange(1, [long]::MaxValue)][long]$Version
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
Set-Location $PSScriptRoot
$payload = Join-Path $PSScriptRoot 'payload'
$direct = Join-Path $payload 'managed'
$archive = Join-Path $payload 'managed-runtime.tar.gz'

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

foreach ($path in @(
    'opencode_desktop_win64.exe',
    'managed',
    'managed\mcp',
    'managed\plugin',
    'managed\skills',
    'managed\lsp',
    'managed\lsp\node\node.exe'
)) {
    if (-not (Test-Path -LiteralPath (Join-Path $payload $path))) { throw "Missing required asset: payload/$path" }
}
if ((Get-AuthenticodeSignature -LiteralPath (Join-Path $payload 'opencode_desktop_win64.exe')).Status -ne 'Valid') {
    throw 'Desktop installer signature is not valid'
}

Remove-Item -LiteralPath $archive -Force -ErrorAction SilentlyContinue
Push-Location $direct
try { & "$env:SystemRoot\System32\tar.exe" -czf $archive . } finally { Pop-Location }
if ($LASTEXITCODE -ne 0) { throw 'Managed runtime archive creation failed' }

$public = [IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $PublicKey))
if ($public.Length -ne 32) { throw 'Expected a 32-byte Ed25519 public key' }
$env:SI_PUBLISHER_PUBLIC_KEY = [Convert]::ToBase64String($public)
try {
    Invoke-Cargo 'test --locked --offline'
    if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed' }
    Invoke-Cargo 'build --release --locked --offline --bin si_opencode_updater --bin release'
    if ($LASTEXITCODE -ne 0) { throw 'Rust compilation failed' }
    Copy-Item 'target\release\si_opencode_updater.exe' (Join-Path $payload 'si_opencode_updater.exe') -Force
    $manifest = Join-Path $env:TEMP 'si-opencode-manifest.json'
    $signature = Join-Path $env:TEMP 'si-opencode-manifest.sig'
    & 'target\release\release.exe' sign (Resolve-Path -LiteralPath $PrivateKey) $direct $Version $manifest $signature
    if ($LASTEXITCODE -ne 0) { throw 'Manifest signing failed' }
    & 'target\release\release.exe' verify (Resolve-Path -LiteralPath $PublicKey) $manifest $signature
    if ($LASTEXITCODE -ne 0) { throw 'Manifest signature verification failed' }
    Copy-Item $manifest (Join-Path $payload 'manifest.json') -Force
    Copy-Item $signature (Join-Path $payload 'manifest.sig') -Force
    New-Item -ItemType Directory -Path out -Force | Out-Null
    $compiler = Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'
    if (-not (Test-Path $compiler)) { throw 'Install Inno Setup 6.5.2+' }
    & $compiler 'installer\SI_OpenCode.iss'
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed' }
    Copy-Item (Join-Path $payload 'si_opencode_updater.exe') out\si_opencode_updater.exe -Force
    Copy-Item (Join-Path $payload 'opencode_desktop_win64.exe') out\opencode_desktop_win64.exe -Force
    Copy-Item $archive out\managed-runtime.tar.gz -Force
    Write-Host 'Build complete. Distribute installer, Desktop installer, and managed runtime together.'
} finally {
    Remove-Item Env:SI_PUBLISHER_PUBLIC_KEY -ErrorAction SilentlyContinue
}
