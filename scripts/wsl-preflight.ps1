param(
  [Parameter(Mandatory)][string]$Msi,
  [Parameter(Mandatory)][string]$StatusPath,
  [switch]$Admin
)
$ErrorActionPreference = 'Stop'
$logDir = Join-Path $env:LOCALAPPDATA 'SI OpenCode\logs'
New-Item -ItemType Directory -Path $logDir -Force | Out-Null
$log = Join-Path $logDir 'wsl-preflight.log'
function Write-Diagnostic([string]$Message) {
    $line = "$(Get-Date -Format o) $Message"
    $line | Add-Content -LiteralPath $log -Encoding utf8
    Write-Host $line
}
function Write-Status([int]$Code) {
    # rename instead of rewrite so the parent never reads a half-written file
    $tmp = "$StatusPath.tmp"
    Set-Content -LiteralPath $tmp -Value ([string]$Code) -Encoding ascii
    Move-Item -LiteralPath $tmp -Destination $StatusPath -Force
}
Write-Diagnostic "preflight start admin=$([bool]$Admin) msi=$Msi"
try {
  if (-not $Admin) {
    Remove-Item -LiteralPath $StatusPath -Force -ErrorAction SilentlyContinue
    $wslOutput = & "$env:WINDIR\System32\wsl.exe" --version 2>&1 | Out-String
    $wslExit = $LASTEXITCODE
    Write-Diagnostic "wsl --version exit=$wslExit output=$($wslOutput.Trim())"
    if ($wslExit -eq 0) {
      Write-Diagnostic 'WSL already healthy, no elevation needed'
      Write-Status 0
      exit 0
    }
    $childArgs = '-NoProfile -ExecutionPolicy Bypass -File "' + $PSCommandPath + '" -Msi "' + $Msi +
      '" -StatusPath "' + $StatusPath + '" -Admin'
    try {
      Start-Process -FilePath "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
        -ArgumentList $childArgs -Verb RunAs | Out-Null
    } catch {
      Write-Diagnostic "elevation request failed: $($_.Exception.Message)"
      Write-Status 1
      exit 1
    }
    Write-Diagnostic 'elevated preflight launched, waiting for status file'
    $deadline = (Get-Date).AddMinutes(25)
    while ($true) {
      if (Test-Path -LiteralPath $StatusPath) {
        $raw = (Get-Content -LiteralPath $StatusPath -Raw).Trim()
        if ($raw -ne '') {
          $code = 0
          if (-not [int]::TryParse($raw, [ref]$code)) { $code = 1 }
          Write-Diagnostic "elevated preflight result=$code"
          exit $code
        }
      }
      if ((Get-Date) -gt $deadline) {
        Write-Diagnostic 'timed out waiting for elevated preflight'
        Write-Status 1
        exit 1
      }
      Start-Sleep -Seconds 2
    }
  }
  $restart = $false
  foreach ($feature in @('Microsoft-Windows-Subsystem-Linux', 'VirtualMachinePlatform')) {
    $state = (Get-WindowsOptionalFeature -Online -FeatureName $feature).State
    Write-Diagnostic "feature $feature state=$state"
    if ($state -ne 'Enabled') {
      Enable-WindowsOptionalFeature -Online -FeatureName $feature -All -NoRestart | Out-Null
      $restart = $true
      Write-Diagnostic "feature $feature enabled, restart pending"
    }
  }
  $signature = (Get-AuthenticodeSignature -LiteralPath $Msi).Status
  Write-Diagnostic "wsl.msi signature=$signature"
  if ($signature -ne 'Valid') { throw 'WSL MSI signature invalid' }
  $msi = Start-Process msiexec.exe -ArgumentList ('/i "' + $Msi + '" /passive /norestart') -Wait -PassThru
  Write-Diagnostic "bundled WSL MSI exit=$($msi.ExitCode)"
  if ($msi.ExitCode -notin @(0, 3010)) { throw "Offline WSL MSI repair failed with exit code $($msi.ExitCode). See $log" }
  if ($restart -or $msi.ExitCode -eq 3010) {
    Write-Diagnostic 'restart required before WSL can be verified'
    Write-Status 3010
    exit 3010
  }
  $wslOutput = & "$env:WINDIR\System32\wsl.exe" --version 2>&1 | Out-String
  if ($LASTEXITCODE -ne 0) {
    Write-Diagnostic "WSL remained unhealthy exit=$LASTEXITCODE output=$($wslOutput.Trim())"
    throw "WSL remains unhealthy after bundled MSI repair. See $log"
  }
  Write-Diagnostic "WSL repair verified: $($wslOutput.Trim())"
  Write-Status 0
  exit 0
} catch {
  Write-Diagnostic "ERROR: $($_.Exception.Message)"
  Write-Status 1
  exit 1
}
