param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$uiExecutable = (Resolve-Path -LiteralPath $Executable).Path
$uiOutput = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $uiOutput) { throw "Choose a new verification output directory: $uiOutput" }
New-Item -ItemType Directory -Path $uiOutput | Out-Null
$uiChecks = [Collections.Generic.List[string]]::new()
function Invoke-UiCheck([string]$Name, [string]$Program, [string[]]$Arguments) {
    Write-Output "Running $Name"
    & $Program @Arguments *> (Join-Path $uiOutput "$Name.log")
    if ($LASTEXITCODE -ne 0) { throw "$Name failed; inspect $uiOutput/$Name.log" }
    $uiChecks.Add($Name)
}
# Run outside the checkout: all supplied paths are absolute and content must
# resolve beside the packaged executable. No interactive command is invoked.
Push-Location ([IO.Path]::GetTempPath())
try {
    Invoke-UiCheck 'physics' $uiExecutable @('physics-diagnostic')
    Invoke-UiCheck 'workflow' $uiExecutable @('editor-workflow-scenario', (Join-Path $uiOutput 'workflow'))
    Invoke-UiCheck 'workflow-verifier' 'python' @((Join-Path $PSScriptRoot 'verify-editor-workflow.py'), (Join-Path $uiOutput 'workflow'))
    Invoke-UiCheck 'authoring' $uiExecutable @('authoring-scenario', (Join-Path $uiOutput 'authoring'))
    Invoke-UiCheck 'authoring-verifier' 'python' @((Join-Path $PSScriptRoot 'verify-m6-exports.py'), (Join-Path $uiOutput 'authoring'))
    Invoke-UiCheck 'track' $uiExecutable @('track-scenario', (Join-Path $uiOutput 'track'))
    Invoke-UiCheck 'track-verifier' 'python' @((Join-Path $PSScriptRoot 'verify-m7-exports.py'), (Join-Path $uiOutput 'track'))
    Invoke-UiCheck 'train-assets' $uiExecutable @('train-asset-scenario', (Join-Path $uiOutput 'authoring/project.qx.json'), (Join-Path $uiOutput 'train-assets'))
    Invoke-UiCheck 'train-assets-verifier' 'python' @((Join-Path $PSScriptRoot 'verify-m81-exports.py'), (Join-Path $uiOutput 'train-assets'))
    Invoke-UiCheck 'playback' $uiExecutable @('train-playback-scenario', (Join-Path $uiOutput 'playback'))
    Invoke-UiCheck 'playback-verifier' 'python' @((Join-Path $PSScriptRoot 'verify-m82-exports.py'), (Join-Path $uiOutput 'playback'))
    Invoke-UiCheck 'ui' $uiExecutable @('ui-reference-scenario', (Join-Path $uiOutput 'train-assets/train-inspection.qx.json'), (Join-Path $uiOutput 'ui'))
    Invoke-UiCheck 'ui-verifier' 'python' @((Join-Path $PSScriptRoot 'verify-ui-reference.py'), (Join-Path $uiOutput 'ui'))
    [ordered]@{
        executable = $uiExecutable
        sha256 = (Get-FileHash -LiteralPath $uiExecutable -Algorithm SHA256).Hash
        completed = @($uiChecks)
        manual_desktop_tested = $false
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $uiOutput 'checks.json') -Encoding utf8
    Write-Output "All noninteractive verification passed: $uiOutput"
} finally { Pop-Location }
