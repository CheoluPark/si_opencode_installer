param(
    [Parameter(Mandatory)][string]$PrivateKey,
    [Parameter(Mandatory)][string]$PublicKey,
    [Parameter(Mandatory)][ValidateRange(1, [long]::MaxValue)][long]$Version
)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
$payload = Join-Path $PSScriptRoot 'payload'

function Build-ManagedRuntime {
    $archive = Join-Path $payload 'managed-runtime.tar.gz'
    Remove-Item -LiteralPath $archive -Force -ErrorAction SilentlyContinue
    $delivery = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'config\managed-delivery.json') -Raw | ConvertFrom-Json
    $tarArgs = @('-czf', $archive)
    foreach ($entry in $delivery.files.PSObject.Properties | Where-Object { $_.Value.installMethod -eq 'npm' }) {
        $tarArgs += ('--exclude=./' + $entry.Name)
    }
    $tarArgs += '.'
    Push-Location (Join-Path $payload 'managed')
    try { & "$env:SystemRoot\System32\tar.exe" @tarArgs } finally { Pop-Location }
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $archive)) { throw 'Managed runtime archive creation failed' }
}

function Build-BootstrapPackage {
    param([Parameter(Mandatory)][string]$OutputDirectory)
    $bootstrap = Join-Path $PSScriptRoot 'bootstrap'
    $packagePath = Join-Path $bootstrap 'package.json'
    $original = Get-Content -LiteralPath $packagePath -Raw -Encoding utf8
    $packageName = if ($env:SI_BOOTSTRAP_PACKAGE_NAME) { $env:SI_BOOTSTRAP_PACKAGE_NAME } else { '@satreci/PLACEHOLDER_BOOTSTRAP_PKG' }
    try {
        $package = $original | ConvertFrom-Json
        $package.name = $packageName
        [IO.File]::WriteAllText($packagePath, ($package | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
        Push-Location $bootstrap
        try { $packed = & npm pack --json --pack-destination $OutputDirectory } finally { Pop-Location }
        if ($LASTEXITCODE -ne 0) { throw 'Bootstrap npm package creation failed' }
        $tarball = ($packed | ConvertFrom-Json)[0].filename
        if (-not (Test-Path -LiteralPath (Join-Path $OutputDirectory $tarball))) { throw 'Bootstrap tarball was not created' }
    } finally {
        [IO.File]::WriteAllText($packagePath, $original, [Text.UTF8Encoding]::new($false))
    }
}

function Stage-OfflinePayload {
    param(
        [Parameter(Mandatory)][string]$OutputDirectory,
        [Parameter(Mandatory)][string]$PublicKey,
        [Parameter(Mandatory)][string]$Manifest,
        [Parameter(Mandatory)][string]$Signature
    )
    $offline = Join-Path $OutputDirectory 'offline-payload'
    Remove-Item -LiteralPath $offline -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Path $offline -Force | Out-Null
    Copy-Item -Path (Join-Path $payload 'managed\*') -Destination $offline -Recurse -Force
    Copy-Item -LiteralPath $Manifest -Destination (Join-Path $offline 'manifest.json') -Force
    Copy-Item -LiteralPath $Signature -Destination (Join-Path $offline 'manifest.sig') -Force
    Copy-Item -LiteralPath $PublicKey -Destination (Join-Path $offline 'publisher-ed25519.pub') -Force
}

Build-ManagedRuntime
foreach ($name in @('opencode_desktop_win64.exe','kb-runtime.tar.gz','ubuntu-resolute-amd64.wsl','wsl.msi','managed-runtime.tar.gz','updater.json','managed/bin/node.exe')) {
    if (-not (Test-Path -LiteralPath (Join-Path $payload $name))) { throw "Missing required offline asset: payload/$name" }
}
foreach ($name in @('plugins','skills','mcp','bin')) {
    if (-not (Test-Path -LiteralPath (Join-Path $payload "managed/$name"))) { throw "Missing reviewed extension inventory: payload/managed/$name" }
}
foreach ($name in @('managed/plugins/ponytail/LICENSE','managed/mcp/rea/node_modules/rea-agents/LICENSE','managed/mcp/drawio/node_modules/@next-ai-drawio/mcp-server/LICENSE','managed/context-mode/node_modules/context-mode/package.json','managed/context-mode/node_modules/context-mode/LICENSE','managed/planning-with-files/node_modules/opencode-planning-with-files/package.json','managed/ui-ux-pro-max/node_modules/ui-ux-pro-max-cli/package.json','managed/bin/LICENSE','managed/bin/lsp/rust/rust-analyzer.exe')) {
    if (-not (Test-Path -LiteralPath (Join-Path $payload $name))) { throw "Missing bundled license or LSP asset: $name" }
}
if ((Get-AuthenticodeSignature -LiteralPath (Join-Path $payload 'wsl.msi')).Status -ne 'Valid') { throw 'Offline WSL MSI is not signed with a valid certificate' }
if ((Get-AuthenticodeSignature -LiteralPath (Join-Path $payload 'opencode_desktop_win64.exe')).Status -ne 'Valid' -or
    (Get-AuthenticodeSignature -LiteralPath (Join-Path $payload 'opencode_desktop_win64.exe')).SignerCertificate.Subject -notmatch 'Anomaly Innovations') {
    throw 'Desktop installer is not Authenticode signed with a valid certificate'
}
$nas = Get-Content -LiteralPath (Join-Path $payload 'updater.json') -Raw -Encoding utf8 | ConvertFrom-Json
if ($nas.host -ne 'https://file.satreci.com' -or $nas.manifest_path -match 'REPLACE|SynoToken|\?|\.\.') {
    throw 'Verify payload/updater.json FileStation manifest path on the corporate network first'
}
$public = [IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $PublicKey))
if ($public.Length -ne 32) { throw 'Expected a 32-byte Ed25519 public key' }
$env:SI_PUBLISHER_PUBLIC_KEY = [Convert]::ToBase64String($public)
try {
    cargo test --locked --offline
    if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed' }
    cargo build --release --locked --offline --bin si_opencode_updater --bin release
    if ($LASTEXITCODE -ne 0) { throw 'Offline Rust compilation failed; vendor dependencies before entering the air gap' }
    Copy-Item 'target\release\si_opencode_updater.exe' (Join-Path $payload 'si_opencode_updater.exe') -Force
    $manifest = Join-Path $env:TEMP 'si-opencode-manifest.json'
    $signature = Join-Path $env:TEMP 'si-opencode-manifest.sig'
    $delivery = Join-Path $PSScriptRoot 'config\managed-delivery.json'
    if (-not (Test-Path -LiteralPath $delivery)) { throw 'Missing managed delivery configuration' }
    Remove-Item -LiteralPath $manifest,$signature -ErrorAction SilentlyContinue
    & 'target\release\release.exe' sign (Resolve-Path -LiteralPath $PrivateKey) (Join-Path $payload 'managed') $Version $manifest $signature $delivery
    if ($LASTEXITCODE -ne 0) { throw 'Manifest signing failed' }
    & 'target\release\release.exe' verify (Resolve-Path -LiteralPath $PublicKey) $manifest $signature
    if ($LASTEXITCODE -ne 0) { throw 'Publisher private/public key mismatch' }
    Copy-Item -LiteralPath $manifest -Destination (Join-Path $payload 'manifest.json') -Force
    Copy-Item -LiteralPath $signature -Destination (Join-Path $payload 'manifest.sig') -Force
    New-Item -ItemType Directory -Path (Join-Path $PSScriptRoot 'out') -Force | Out-Null
    $offlinePayload = Join-Path $PSScriptRoot 'out\offline-payload'
    if ($env:SI_INCLUDE_MANAGED_FALLBACK -eq '1') {
        Stage-OfflinePayload -OutputDirectory (Join-Path $PSScriptRoot 'out') -PublicKey (Resolve-Path -LiteralPath $PublicKey) -Manifest $manifest -Signature $signature
    } else {
        Remove-Item -LiteralPath $offlinePayload -Recurse -Force -ErrorAction SilentlyContinue
    }
    Build-BootstrapPackage -OutputDirectory (Join-Path $PSScriptRoot 'out')
    $compiler = Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'
    if (-not (Test-Path $compiler)) { throw 'Install Inno Setup 6.5.2+ or point this script to the approved ISCC.exe' }
    $kb = Join-Path $payload 'kb-runtime.tar.gz'
    $kbHash = (Get-FileHash -LiteralPath $kb -Algorithm SHA256).Hash.ToLowerInvariant()
    $kbSize = (Get-Item -LiteralPath $kb).Length
    $ubuntu = Join-Path $payload 'ubuntu-resolute-amd64.wsl'
    $ubuntuHash = (Get-FileHash -LiteralPath $ubuntu -Algorithm SHA256).Hash.ToLowerInvariant()
    $ubuntuSize = (Get-Item -LiteralPath $ubuntu).Length
    $desktop = Join-Path $payload 'opencode_desktop_win64.exe'
    $desktopHash = (Get-FileHash -LiteralPath $desktop -Algorithm SHA256).Hash.ToLowerInvariant()
    $desktopSize = (Get-Item -LiteralPath $desktop).Length
    $wsl = Join-Path $payload 'wsl.msi'
    $wslHash = (Get-FileHash -LiteralPath $wsl -Algorithm SHA256).Hash.ToLowerInvariant()
    $wslSize = (Get-Item -LiteralPath $wsl).Length
    $managedRuntime = Join-Path $payload 'managed-runtime.tar.gz'
    $managedHash = (Get-FileHash -LiteralPath $managedRuntime -Algorithm SHA256).Hash.ToLowerInvariant()
    $managedSize = (Get-Item -LiteralPath $managedRuntime).Length
    & $compiler "/DKB_HASH=$kbHash" "/DKB_SIZE=$kbSize" "/DUBUNTU_HASH=$ubuntuHash" "/DUBUNTU_SIZE=$ubuntuSize" "/DDESKTOP_HASH=$desktopHash" "/DDESKTOP_SIZE=$desktopSize" "/DWSL_HASH=$wslHash" "/DWSL_SIZE=$wslSize" "/DMANAGED_HASH=$managedHash" "/DMANAGED_SIZE=$managedSize" 'installer\SI_OpenCode.iss'
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed' }
    $installer = Join-Path $PSScriptRoot 'out\si_opencode_installer.exe'
    if ((Get-Item $installer).Length -ge 4GB) {
        Remove-Item -LiteralPath $installer
        throw 'Single installer EXE exceeds the Windows ~4GB executable limit: rebuild a smaller WSL image'
    }
    Copy-Item (Join-Path $payload 'si_opencode_updater.exe') 'out\si_opencode_updater.exe' -Force
    Copy-Item -LiteralPath $desktop -Destination 'out\opencode_desktop_win64.exe' -Force
    Copy-Item -LiteralPath $wsl -Destination 'out\wsl.msi' -Force
    Copy-Item -LiteralPath $managedRuntime -Destination 'out\managed-runtime.tar.gz' -Force
    Copy-Item -LiteralPath $kb -Destination 'out\kb-runtime.tar.gz' -Force
    Copy-Item -LiteralPath $ubuntu -Destination 'out\ubuntu-resolute-amd64.wsl' -Force
    Write-Host 'Installer/updater plus verified FPGA KB runtime sidecar generated in out/. Do not distribute before clean-PC offline validation.'
} finally {
    Remove-Item Env:SI_PUBLISHER_PUBLIC_KEY -ErrorAction SilentlyContinue
}
