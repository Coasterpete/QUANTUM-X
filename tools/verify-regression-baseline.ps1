# R0.3 CPU-only baseline. Does not build/launch the application or package it.
[CmdletBinding()]
param(
    [ValidateSet('Exact', 'Tolerant')][string]$Comparison = 'Exact',
    [string]$OutputDirectory,
    [string]$MachineLabel = 'unspecified'
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$root = Split-Path -Parent $PSScriptRoot
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $root ('target/regression-r03/' + [Guid]::NewGuid().ToString('N'))
}
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a new evidence output directory.' }
$null = New-Item -ItemType Directory -Path $OutputDirectory
$OutputDirectory = (Resolve-Path -LiteralPath $OutputDirectory).Path
$originalComparison = $env:QX_REPLAY_COMPARISON
$results = [System.Collections.Generic.List[object]]::new()
$report = [ordered]@{
    format_version = 1
    comparison = $Comparison.ToLowerInvariant()
    exact_gate = ($Comparison -eq 'Exact')
    source_commit = $null
    working_tree_status = $null
    machine_label = $MachineLabel
    rustc = $null
    cargo = $null
    rustflags = $env:RUSTFLAGS
    encoded_rustflags = $env:CARGO_ENCODED_RUSTFLAGS
    os = [System.Environment]::OSVersion.VersionString
    processor = $env:PROCESSOR_IDENTIFIER
    utc = [DateTime]::UtcNow.ToString('o')
    checks = $results
    passed = $false
    exact_gate_passed = $false
}
function Invoke-Check([string]$Name, [string]$Program, [string[]]$Arguments) {
    & $Program @Arguments 2>&1 | Tee-Object -FilePath (Join-Path $OutputDirectory "$Name.log") | Write-Host
    $code = $LASTEXITCODE
    $results.Add([ordered]@{ name = $Name; program = $Program; arguments = $Arguments; exit_code = $code })
    if ($code -ne 0) { throw "$Name failed with exit code $code. Evidence: $OutputDirectory" }
}
Push-Location $root
try {
    $report.source_commit = (& git rev-parse HEAD | Out-String).Trim()
    $report.working_tree_status = @(& git status --porcelain)
    $report.rustc = (& rustc -Vv | Out-String).Trim()
    $report.cargo = (& cargo -V | Out-String).Trim()
    $env:QX_REPLAY_COMPARISON = $Comparison.ToLowerInvariant()
    Invoke-Check 'fixture-integrity-before' 'python' @('tools/verify-regression-fixtures.py')
    Invoke-Check 'compatibility' 'cargo' @('test', '-p', 'qx-document', '--test', 'compatibility_corpus', '--locked', '--', '--nocapture')
    Invoke-Check 'replay-debug' 'cargo' @('test', '-p', 'qx-physics', '--test', 'golden_replay', '--locked', '--', '--nocapture')
    Invoke-Check 'replay-release' 'cargo' @('test', '-p', 'qx-physics', '--release', '--test', 'golden_replay', '--locked', '--', '--nocapture')
    Invoke-Check 'fixture-integrity-after' 'python' @('tools/verify-regression-fixtures.py')
    $report.passed = $true
    $report.exact_gate_passed = ($Comparison -eq 'Exact')
    if ($Comparison -eq 'Tolerant') { Write-Host 'DIAGNOSTIC ONLY: this result does not pass the exact regression gate.' }
} finally {
    $env:QX_REPLAY_COMPARISON = $originalComparison
    Pop-Location
    $report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'result.json') -Encoding UTF8
    Write-Host "Regression evidence: $OutputDirectory"
}
