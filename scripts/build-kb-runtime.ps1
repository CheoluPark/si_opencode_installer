param(
    [string]$Source = (Join-Path $env:USERPROFILE 'Downloads\ross_kb'),
    [Parameter(Mandatory)][string]$Output
)
$ErrorActionPreference = 'Stop'

# Only the files proven necessary for the WSL Docker deployment are distributed:
# the AMD KB snapshot/images and the complete offline Docker DEB dependency set.
$required = @(
    'Docker\deb',
    'MCP\20260918.qwen3-embedding-0.6b.tar.gz'
)
foreach ($item in $required) {
    if (-not (Test-Path -LiteralPath (Join-Path $Source $item))) {
        throw "Missing required AMD KB asset: $item"
    }
}
if (Test-Path -LiteralPath $Output) { throw "Refusing to overwrite: $Output" }
$tar = Get-Command tar.exe -ErrorAction Stop
& $tar.Source -czf $Output -C $Source Docker/deb MCP/20260918.qwen3-embedding-0.6b.tar.gz
if ($LASTEXITCODE -ne 0) { throw 'Could not create the KB runtime archive' }
$contents = & $tar.Source -tzf $Output
if ($LASTEXITCODE -ne 0 -or $contents -notcontains 'MCP/20260918.qwen3-embedding-0.6b.tar.gz') {
    throw 'KB runtime archive verification failed'
}
Write-Host "KB runtime archive created: $Output"
