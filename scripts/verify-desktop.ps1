param([Parameter(Mandatory)][string]$Path)
$signature = Get-AuthenticodeSignature -LiteralPath $Path
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'Anomaly Innovations') {
    Write-Error 'OpenCode Desktop is not signed by the expected publisher'
    exit 1
}
