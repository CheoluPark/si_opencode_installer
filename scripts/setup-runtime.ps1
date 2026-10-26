param([Parameter(Mandatory)][string]$AppDir, [Parameter(Mandatory)][string]$ReleaseDir, [string]$WarningFile = (Join-Path $env:TEMP 'si-opencode-optional-tool-warnings.txt'))
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

# Optional tools are declared in order in tools-manifest.json; no tool is required for setup.
$warningFile = $WarningFile
$rebootFile = Join-Path (Split-Path -Parent $warningFile) 'si-opencode-reboot-required'
Remove-Item -LiteralPath $rebootFile -Force -ErrorAction SilentlyContinue
$warnings = [Collections.Generic.List[string]]::new()
$rebootRequired = $false
$orderedTools = @('msvc', 'rust', 'python', 'node', 'wsl')
$availableTools = @($orderedTools | Where-Object { $toolsManifest.tools.PSObject.Properties.Name -contains $_ } | ForEach-Object { $toolsManifest.tools.$_ })
if ($availableTools.Count) {
    Add-Type -AssemblyName System.Windows.Forms
    $form = [Windows.Forms.Form]::new()
    $form.Text = 'Optional development tools'
    $form.Width = 560; $form.Height = 330; $form.StartPosition = 'CenterScreen'
    $form.MinimizeBox = $false; $form.MaximizeBox = $false
    $label = [Windows.Forms.Label]::new()
    $label.Text = 'Select tools to install or upgrade. Existing tools meeting the minimum version are unchecked.'
    $label.SetBounds(12, 12, 520, 38); $form.Controls.Add($label)
    $list = [Windows.Forms.CheckedListBox]::new()
    $list.SetBounds(12, 55, 520, 190); $list.CheckOnClick = $true
    foreach ($tool in $availableTools) {
        if (-not $tool.name -or -not $tool.minimum_version -or -not $tool.installer -or -not $tool.sha256 -or -not $tool.detect_path) {
            throw 'Optional tool manifest entry is incomplete'
        }
        $path = [string]$tool.detect_path
        if (-not [IO.Path]::IsPathRooted($path)) { $path = Join-Path $env:LOCALAPPDATA $path }
        $version = $null
        if (Test-Path -LiteralPath $path) {
            try { $version = [version]((Get-Item -LiteralPath $path).VersionInfo.ProductVersion -replace '[^0-9.].*$', '') } catch {}
        }
        $tool | Add-Member -NotePropertyName _installedVersion -NotePropertyValue $version -Force
        $tool | Add-Member -NotePropertyName _path -NotePropertyValue $path -Force
        [void]$list.Items.Add("$($tool.name) (minimum $($tool.minimum_version); installed: $(if ($version) { $version } else { 'not detected' }))", ($null -eq $version -or $version -lt [version]$tool.minimum_version))
    }
    $form.Controls.Add($list)
    $ok = [Windows.Forms.Button]::new(); $ok.Text = 'Continue'; $ok.SetBounds(432, 255, 100, 28); $ok.DialogResult = 'OK'
    $form.Controls.Add($ok); $form.AcceptButton = $ok
    $dialogResult = $form.ShowDialog()
    foreach ($index in 0..($list.Items.Count - 1)) {
        $tool = $availableTools[$index]
        if ($dialogResult -ne 'OK' -or -not $list.GetItemChecked($index)) {
            $warnings.Add("$($tool.name): skipped by user")
            continue
        }
        try {
                $installerName = [string]$tool.installer
                if ([IO.Path]::IsPathRooted($installerName) -or $installerName -match '(^|[\\/])\.\.([\\/]|$)') { throw 'Invalid installer path' }
                $installer = Join-Path $ReleaseDir $installerName
                if (([string]$tool.sha256).ToLowerInvariant() -notmatch '^[0-9a-f]{64}$' -or (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant() -ne ([string]$tool.sha256).ToLowerInvariant()) { throw 'Installer hash check failed' }
                $arguments = [string]$tool.install_args
                if ($tool.scope -eq 'machine') {
                    try { $process = Start-Process -FilePath $installer -ArgumentList $arguments -Verb RunAs -Wait -PassThru -ErrorAction Stop }
                    catch { throw 'Elevation cancelled or failed' }
                } else { $process = Start-Process -FilePath $installer -ArgumentList $arguments -Wait -PassThru }
                if ($process.ExitCode -in @(3010, 1641)) { $rebootRequired = $true }
                elseif ($process.ExitCode -ne 0) { throw "Installer exited with code $($process.ExitCode)" }
                if (-not (Test-Path -LiteralPath $tool._path)) { throw 'Installed executable was not detected' }
                try { $installed = [version]((Get-Item -LiteralPath $tool._path).VersionInfo.ProductVersion -replace '[^0-9.].*$', '') } catch { $installed = $null }
                if ($null -eq $installed -or $installed -lt [version]$tool.minimum_version) { throw "Installed version is below minimum $($tool.minimum_version)" }
        } catch { $warnings.Add("$($tool.name): $_") }
    }
    $form.Dispose()
}
if ($warnings.Count) { [IO.File]::WriteAllLines($warningFile, $warnings) }
else { Remove-Item -LiteralPath $warningFile -Force -ErrorAction SilentlyContinue }

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (@($userPath -split ';') -notcontains $AppDir) {
    [Environment]::SetEnvironmentVariable('Path', (([string]$userPath).TrimEnd(';') + ';' + $AppDir), 'User')
}
if ($rebootRequired) { [IO.File]::WriteAllText($rebootFile, '1') }
