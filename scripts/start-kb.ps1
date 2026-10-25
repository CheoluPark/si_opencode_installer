$ErrorActionPreference = 'Stop'
$state = Join-Path $env:LOCALAPPDATA 'SI OpenCode\kb.json'
if (-not (Test-Path -LiteralPath $state)) { throw 'KB installation state is missing' }
$distro = (Get-Content -LiteralPath $state -Raw | ConvertFrom-Json).distro
if ([string]::IsNullOrWhiteSpace($distro)) { throw 'KB distribution name is missing' }
# The KB stack must come back after a WSL restart; the MCP server is useless until the
# vector index for the collection exists.
& wsl.exe -d $distro --exec /bin/bash -c @"
set -eu
config=\$(ls -d /home/*/.config/amd-embedded-doc-search 2>/dev/null | head -1)
test -n "\$config" || { echo 'KB compose configuration missing'; exit 1; }
cd "\$config"
systemctl is-active --quiet docker || systemctl start docker
docker compose up -d
docker compose ps
"@
if ($LASTEXITCODE -ne 0) { throw 'Could not start SI OpenCode KB' }
