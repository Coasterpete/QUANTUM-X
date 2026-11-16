# R0.5: extracted Windows package only; never calls Cargo or builds the app.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PackageDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidatePattern('^[A-Za-z0-9_.-]{1,64}$')][string]$MachineLabel = 'unspecified',
    [ValidateRange(1,120)][int]$CommandTimeoutSeconds = 30,
    [ValidateRange(1,120)][int]$NativeTimeoutSeconds = 45,
    [ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ExpectedManifestSha256
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'desktop-verification.ps1')
$result = Invoke-DesktopVerification -PackageDirectory $PackageDirectory -OutputDirectory $OutputDirectory `
    -MachineLabel $MachineLabel -CommandTimeoutSeconds $CommandTimeoutSeconds `
    -NativeTimeoutSeconds $NativeTimeoutSeconds -ExpectedManifestSha256 $ExpectedManifestSha256
Write-Host "Automated desktop verification: $($result.automated_status). Evidence: $OutputDirectory"
Write-Host 'Manual checks are not performed by this script. Complete manual-checklist.md/json separately.'
if ($result.automated_status -ne 'passed') { exit 1 }
