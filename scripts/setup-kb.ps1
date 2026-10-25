param(
    [Parameter(Mandatory)][string]$AppDir,
    [Parameter(Mandatory)][string]$KbBundle,
    [Parameter(Mandatory)][string]$UbuntuImage,
    [int]$TimeoutMinutes = 90
)
$ErrorActionPreference = 'Stop'

# Use the user's existing WSL distro; never import, unregister, or delete one.
$distros = @(& wsl.exe --list --quiet 2>&1 | ForEach-Object { ([string]$_).Replace([char]0, '').Trim() } | Where-Object { $_ -and $_ -notmatch '^docker-desktop' })
if ($distros.Count -eq 0) {
    & wsl.exe --install --from-file $UbuntuImage --no-launch
    if ($LASTEXITCODE -ne 0) { throw "Bundled Ubuntu installation failed ($LASTEXITCODE)" }
    Start-Sleep -Seconds 5
    $distros = @(& wsl.exe --list --quiet 2>&1 | ForEach-Object { ([string]$_).Replace([char]0, '').Trim() } | Where-Object { $_ -and $_ -notmatch '^docker-desktop' })
}
if ($distros.Count -eq 0) { throw 'Bundled Ubuntu installation completed without registering a WSL distro. Restart Windows, then rerun the installer.' }
$distro = $distros[0]

$wslConfig = Join-Path $env:USERPROFILE '.wslconfig'
$existing = if (Test-Path -LiteralPath $wslConfig) { Get-Content -LiteralPath $wslConfig -Raw } else { '' }
if ($existing -match '(?im)^\s*memory\s*=\s*(\d+)\s*(MB|GB)?') {
    $amount = [int64]$Matches[1]
    $bytes = if ($Matches[2].ToUpperInvariant() -eq 'GB') { $amount * 1GB } else { $amount * 1MB }
    if ($bytes -lt 12GB) { throw 'WSL memory is configured below 12GB. Set memory=12GB in ~/.wslconfig, restart WSL, then rerun.' }
} elseif (-not $existing) {
    Set-Content -LiteralPath $wslConfig -NoNewline -Value "[wsl2]`nlocalhostForwarding=true`nmemory=12GB`nswap=8GB`n`n[general]`ninstanceIdleTimeout=-1`n"
    & wsl.exe --shutdown
    Start-Sleep -Seconds 8
}

$bundleLinux = (& wsl.exe -d $distro --exec wslpath -a $KbBundle).Trim()
if ($LASTEXITCODE -ne 0 -or -not $bundleLinux) { throw 'The KB runtime archive is not accessible from WSL' }

# Install the offline Docker DEBs and retain only this KB's runtime under /opt.
& wsl.exe -d $distro -u root --exec /bin/bash -c @"
set -eu
bundle='$bundleLinux'
runtime=/opt/si-opencode-kb
test -f "`$bundle" || { echo 'KB runtime archive missing inside WSL'; exit 1; }
mkdir -p "`$runtime"
if [ ! -f "`$runtime/MCP/20260918.qwen3-embedding-0.6b.tar.gz" ]; then tar -xzf "`$bundle" -C "`$runtime"; fi
if ! command -v dockerd >/dev/null 2>&1; then dpkg -i "`$runtime/Docker/deb"/*.deb; fi
mkdir -p /etc/systemd/system/docker.service.d
cat > /etc/systemd/system/docker.service.d/override.conf <<'EOF'
[Service]
ExecStart=
ExecStart=/usr/bin/dockerd -H unix:///var/run/docker.sock -H tcp://127.0.0.1:2375
EOF
systemctl daemon-reload
systemctl enable --now docker
for home in /home/*; do [ -d "`$home" ] && usermod -aG docker "`$(basename "`$home")" || true; done
docker version --format 'server={{.Server.Version}}'
docker compose version
"@
if ($LASTEXITCODE -ne 0) { throw 'Offline Docker setup failed inside the existing WSL distro' }

# The AMD CLI loads its bundled images and imports the snapshot only on first run.
& wsl.exe -d $distro --exec /bin/bash -c @"
set -eu
runtime=/opt/si-opencode-kb
if [ ! -x "`$runtime/amd-embedded-doc-search" ]; then tar -xzf "`$runtime/MCP/20260918.qwen3-embedding-0.6b.tar.gz" -C "`$runtime"; fi
config="`$HOME/.config/amd-embedded-doc-search"
if [ ! -f "`$config/docker-compose.yaml" ]; then
  cd "`$runtime"
  ./amd-embedded-doc-search --accessible configure --start
else
  cd "`$config"
  docker compose up -d
fi
grep -q 'command: \["--port", "8080", "--host", "0.0.0.0"\]' "`$config/docker-compose.yaml" || { echo 'AMD MCP HTTP command is missing'; exit 1; }
"@
if ($LASTEXITCODE -ne 0) { throw 'The AMD KB runtime could not be configured' }

& wsl.exe -d $distro -u root --exec /bin/bash -c @'
set -eu
cat > /etc/systemd/system/si-opencode-kb.service <<'EOF'
[Unit]
Description=SI OpenCode AMD Vivado knowledge base
After=docker.service
Requires=docker.service
[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/bin/bash -lc 'config=$(ls -d /home/*/.config/amd-embedded-doc-search 2>/dev/null | head -1); test -n "$config"; cd "$config"; docker compose up -d'
ExecStop=/bin/bash -lc 'config=$(ls -d /home/*/.config/amd-embedded-doc-search 2>/dev/null | head -1); test -n "$config" && cd "$config" && docker compose stop || true'
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload
systemctl enable si-opencode-kb.service
'@
if ($LASTEXITCODE -ne 0) { throw 'Could not configure KB auto-start' }

# Do not mistake a running container for success: validate a real MCP search.
$deadline = (Get-Date).AddMinutes($TimeoutMinutes)
while ((Get-Date) -lt $deadline) {
    $state = (& wsl.exe -d $distro --exec /bin/bash -c "docker inspect amd-embedded-doc-search-mcp-1 --format '{{.State.Status}}' 2>/dev/null").Trim()
    $failing = (& wsl.exe -d $distro --exec /bin/bash -c "docker logs --tail 3 amd-embedded-doc-search-mcp-1 2>&1 | grep -c 'non-existing index' || true").Trim()
    Write-Host "KB state: $state (index errors: $failing)"
    if ($state -eq 'running' -and [int]$failing -eq 0) {
        & (Join-Path $AppDir 'si_opencode_updater.exe') --enable-kb
        if ($LASTEXITCODE -ne 0) { throw 'The KB MCP search did not return documentation' }
        @{ distro = $distro } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $env:LOCALAPPDATA 'SI OpenCode\kb.json') -NoNewline
        Write-Host 'Vivado knowledge base search verified.'
        exit 0
    }
    Start-Sleep -Seconds 30
}
throw "The KB vector index was still unavailable after $TimeoutMinutes minutes. Do not interrupt its first import; rerun the installer after it completes."
