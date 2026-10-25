param([string]$OutputDirectory)
$ErrorActionPreference = 'Continue'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $env:USERPROFILE 'Desktop' }
$outDir = Join-Path $OutputDirectory "si-opencode-diag-$stamp"
New-Item -ItemType Directory -Path $outDir -Force | Out-Null

function Save-Text([string]$Name, [string]$Text) {
    Set-Content -LiteralPath (Join-Path $outDir $Name) -Value $Text -Encoding utf8
}

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
Save-Text 'context.txt' @(
  "collected=$((Get-Date).ToString('o'))"
  "computer=$env:COMPUTERNAME user=$env:USERNAME elevated=$isAdmin"
  "os=$([System.Environment]::OSVersion.VersionString)"
)

# 1. installer diagnostics
$logDir = Join-Path $env:LOCALAPPDATA 'SI OpenCode\logs'
if (Test-Path $logDir) {
  Copy-Item -Path (Join-Path $logDir '*') -Destination $outDir -ErrorAction SilentlyContinue
} else {
  Save-Text 'missing-installer-logs.txt' "No $logDir directory"
}

# 2. WSL state
$wsl = "$env:WINDIR\System32\wsl.exe"
$report = @()
foreach ($arg in @('--version', '--status', '--list --verbose')) {
  $out = & $wsl ($arg -split ' ') 2>&1 | Out-String
  $report += "=== wsl $arg (exit=$LASTEXITCODE) ==="
  $report += $out.Trim()
}
Save-Text 'wsl-state.txt' ($report -join "`r`n")

# 3. optional features (needs elevation to be readable)
$featureReport = @()
foreach ($feature in @('Microsoft-Windows-Subsystem-Linux', 'VirtualMachinePlatform', 'Microsoft-Hyper-V-All')) {
  try {
    $state = (Get-WindowsOptionalFeature -Online -FeatureName $feature -ErrorAction Stop).State
    $featureReport += "$feature = $state"
  } catch {
    $featureReport += "$feature = ERROR: $($_.Exception.Message)"
  }
}
Save-Text 'optional-features.txt' ($featureReport -join "`r`n")

# 4. WSL appx packages and services
$appx = Get-AppxPackage -Name '*WindowsSubsystemForLinux*' -ErrorAction SilentlyContinue |
  ForEach-Object { "$($_.Name) $($_.Version) $($_.InstallLocation)" }
Save-Text 'wsl-appx.txt' (($appx -join "`r`n") + "`r`n`r`n" +
  ((Get-Service -Name 'LxssManager', 'WslService', 'vmcompute' -ErrorAction SilentlyContinue |
    ForEach-Object { "$($_.Name) = $($_.Status) ($($_.StartType))" }) -join "`r`n"))

# 5. WSL MSI product registration
$msi = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue |
  Where-Object { $_.DisplayName -match 'Windows Subsystem for Linux' } |
  ForEach-Object { "$($_.DisplayName) $($_.DisplayVersion) $($_.Publisher)" }
Save-Text 'wsl-msi.txt' (($msi -join "`r`n") + "`r`n")

# 6. recent Lxss/WSL event log
try {
  $events = Get-WinEvent -FilterHashtable @{LogName='Application'; StartTime=(Get-Date).AddDays(-3)} -ErrorAction Stop |
    Where-Object { $_.ProviderName -match 'Lxss|WSL|Hyper-V' -or $_.Message -match 'LxssManager|WSL' } |
    Select-Object -First 40 |
    ForEach-Object { "$($_.TimeCreated) $($_.LevelDisplayName) $($_.ProviderName) $($_.Message)" }
  Save-Text 'eventlog.txt' (($events -join "`r`n") + "`r`n")
} catch {
  Save-Text 'eventlog.txt' "event log unavailable: $($_.Exception.Message)"
}

# 7. installed SI OpenCode build
$appDir = Join-Path $env:LOCALAPPDATA 'Programs\SI OpenCode'
if (Test-Path $appDir) {
  Save-Text 'installed-files.txt' ((Get-ChildItem $appDir -Recurse -Depth 1 -ErrorAction SilentlyContinue |
    Select-Object -First 200 | ForEach-Object { "$($_.FullName) $($_.Length) $($_.LastWriteTime)" }) -join "`r`n")
} else {
  Save-Text 'installed-files.txt' "No $appDir directory"
}
$runOnce = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\RunOnce' -ErrorAction SilentlyContinue |
  Select-Object -Property * -ExcludeProperty PS*
Save-Text 'runonce.txt' (($runOnce.PSObject.Properties | ForEach-Object { "$($_.Name) = $($_.Value)" }) -join "`r`n")

# 8. pack
$zip = "$outDir.zip"
Compress-Archive -Path (Join-Path $outDir '*') -DestinationPath $zip -Force
Write-Output $zip
