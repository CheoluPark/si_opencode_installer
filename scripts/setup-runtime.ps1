param(
  [Parameter(Mandatory)][string]$AppDir,
  [Parameter(Mandatory)][string]$ManagedRuntime,
  [Parameter(Mandatory)][int]$Fpga
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$desktopInstaller = Join-Path $AppDir 'opencode_desktop_win64.exe'
$desktop = Join-Path $env:LOCALAPPDATA 'Programs\@opencodedesktop\OpenCode.exe'
$cli = Join-Path $env:LOCALAPPDATA 'Programs\@opencodedesktop\resources\opencode-cli.exe'
function Assert-ManagedArtifact([string]$Path, $Artifact) {
    if (-not (Test-Path -LiteralPath $Path)) { throw "Managed npm artifact is missing: $Path" }
    if ((Get-Item -LiteralPath $Path).Length -ne [int64]$Artifact.bytes) { throw "Managed npm artifact size mismatch: $Path" }
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Artifact.sha256.ToLowerInvariant()) { throw "Managed npm artifact hash mismatch: $Path" }
}
function Install-NpmManaged {
    $manifest = Get-Content -LiteralPath (Join-Path $AppDir 'manifest.json') -Raw | ConvertFrom-Json
    $npmEntries = @($manifest.files.PSObject.Properties | Where-Object { $_.Value.installMethod -eq 'npm' })
    if ($npmEntries.Count -eq 0) { return }
    $npm = Get-Command npm -ErrorAction SilentlyContinue
    if (-not $npm) { throw 'npm is required for this SI OpenCode release but was not found' }
    Write-Host "[SI OpenCode] Downloading $($npmEntries.Count) managed artifact(s) from Nexus..."
    $work = Join-Path $env:TEMP ('si-opencode-npm-' + [guid]::NewGuid())
    $installs = @{}
    try {
        foreach ($entry in $npmEntries) {
            $artifact = $entry.Value
            $registry = if ($artifact.npmRegistry) { $artifact.npmRegistry } else { $manifest.delivery.npmRegistry }
            if (-not $artifact.npmPackage -or -not $artifact.npmVersion -or -not $artifact.npmPath -or -not $registry) { throw "Incomplete npm delivery metadata: $($entry.Name)" }
            $key = "$registry|$($artifact.npmPackage)|$($artifact.npmVersion)"
            if (-not $installs.ContainsKey($key)) {
                $prefix = Join-Path $work ([guid]::NewGuid().ToString())
                New-Item -ItemType Directory -Path $prefix -Force | Out-Null
                $npmrc = Join-Path $prefix '.npmrc'
                Set-Content -LiteralPath $npmrc -Value "registry=$registry`nalways-auth=true" -Encoding ascii
                $previousNpmrc = $env:NPM_CONFIG_USERCONFIG
                try {
                    $env:NPM_CONFIG_USERCONFIG = $npmrc
                    & $npm.Source install --prefix $prefix --ignore-scripts --no-audit --no-fund --package-lock=false "$($artifact.npmPackage)@$($artifact.npmVersion)" --registry $registry
                    if ($LASTEXITCODE -ne 0) { throw "npm install failed for $($artifact.npmPackage)@$($artifact.npmVersion)" }
                } finally {
                    $env:NPM_CONFIG_USERCONFIG = $previousNpmrc
                }
                $installs[$key] = $prefix
            }
            $source = Join-Path (Join-Path $installs[$key] ('node_modules\' + ($artifact.npmPackage -replace '/', '\'))) $artifact.npmPath
            Assert-ManagedArtifact $source $artifact
            $destination = Join-Path (Join-Path $AppDir 'managed') $entry.Name
            New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
            Copy-Item -LiteralPath $source -Destination $destination -Force
            Assert-ManagedArtifact $destination $artifact
        }
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }
}
Write-Host '[SI OpenCode] Extracting the managed runtime...'
if (-not (Test-Path -LiteralPath $ManagedRuntime)) { throw 'Managed runtime archive is missing' }
New-Item -ItemType Directory -Path (Join-Path $AppDir 'managed') -Force | Out-Null
& "$env:SystemRoot\System32\tar.exe" -xzf $ManagedRuntime -C (Join-Path $AppDir 'managed')
if ($LASTEXITCODE -ne 0) { throw 'Managed runtime archive extraction failed' }
Install-NpmManaged
Write-Host '[SI OpenCode] Verifying the Desktop installer signature...'
if ((Get-AuthenticodeSignature -LiteralPath $desktopInstaller).Status -ne 'Valid') { throw 'Desktop installer signature is not valid' }
if (-not (Test-Path -LiteralPath $desktop) -or
    (Get-Item -LiteralPath $desktop).VersionInfo.FileVersion -ne (Get-Item -LiteralPath $desktopInstaller).VersionInfo.FileVersion) {
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
Write-Host '[SI OpenCode] Installing signed plugins, MCPs, skills, and LSP tools...'
& (Join-Path $AppDir 'si_opencode_updater.exe') --install-managed $AppDir
if ($LASTEXITCODE -ne 0) { throw 'Signed offline extension installation failed' }
$nodeExe = Join-Path $env:USERPROFILE '.config\opencode\bin\node.exe'
if (-not (Test-Path -LiteralPath $nodeExe)) { throw 'Bundled Node.js executable missing' }
& $nodeExe --version
if ($LASTEXITCODE -ne 0) { throw 'Bundled Node.js executable cannot start' }
Write-Host '[SI OpenCode] Validating bundled MCP entry points...'
foreach ($entry in @('mcp\rea\node_modules\rea-agents\scripts\rea.mjs', 'mcp\drawio\node_modules\@next-ai-drawio\mcp-server\dist\index.js')) {
    $source = Join-Path (Join-Path $env:USERPROFILE '.config\opencode') $entry
    if (-not (Test-Path -LiteralPath $source)) { throw "Offline MCP entrypoint missing: $entry" }
    & $nodeExe --check $source
    if ($LASTEXITCODE -ne 0) { throw "Bundled MCP entrypoint invalid: $entry" }
}
Write-Host '[SI OpenCode] Validating bundled plugins and skills...'
$contextMode = Join-Path $env:USERPROFILE '.config\opencode\context-mode\node_modules\context-mode\build\adapters\opencode\plugin.js'
if (-not (Test-Path -LiteralPath $contextMode)) { throw 'Bundled context-mode OpenCode plugin is missing' }
& $nodeExe --input-type=module --eval "import { pathToFileURL } from 'node:url'; import(pathToFileURL(process.argv[1]).href).then(() => console.log('context-mode plugin loaded'))" $contextMode
if ($LASTEXITCODE -ne 0) { throw 'Bundled context-mode OpenCode plugin cannot load' }
$uiPro = Join-Path $env:USERPROFILE '.config\opencode\ui-ux-pro-max\node_modules\ui-ux-pro-max-cli\dist\index.js'
if (-not (Test-Path -LiteralPath $uiPro)) { throw 'Bundled UI/UX Pro Max installer is missing' }
Push-Location $env:USERPROFILE
try { & $nodeExe $uiPro init --ai universal --global --offline } finally { Pop-Location }
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath (Join-Path $env:USERPROFILE '.agents\skills\ui-ux-pro-max\SKILL.md'))) { throw 'UI/UX Pro Max skill installation failed' }
$contextModeCli = Join-Path $env:USERPROFILE '.config\opencode\context-mode\node_modules\context-mode\cli.bundle.mjs'
$contextModeCommand = Join-Path $AppDir 'context-mode.cmd'
Set-Content -LiteralPath $contextModeCommand -Encoding ascii -NoNewline -Value "@echo off`r`n`"$nodeExe`" `"$contextModeCli`" %*"

if ($Fpga -eq 1) {
    Write-Host '[SI OpenCode] Configuring the FPGA knowledge base...'
    $kb = Join-Path $AppDir 'kb-runtime.tar.gz'
    $ubuntu = Join-Path $AppDir 'ubuntu-resolute-amd64.wsl'
    if (-not (Test-Path -LiteralPath $kb)) { throw 'Bundled FPGA KB runtime archive missing' }
    if (-not (Test-Path -LiteralPath $ubuntu)) { throw 'Bundled Ubuntu WSL image missing' }
    & (Join-Path $PSScriptRoot 'setup-kb.ps1') -AppDir $AppDir -KbBundle $kb -UbuntuImage $ubuntu
    if ($LASTEXITCODE -ne 0) { throw 'The Vivado knowledge base could not be started and verified' }
}
