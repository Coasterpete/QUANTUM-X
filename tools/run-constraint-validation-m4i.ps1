# Native offline M4I evidence, including opt-in long experiments. No desktop control.
$ErrorActionPreference = 'Stop'
$repositoryPath = Split-Path -Parent $PSScriptRoot
Push-Location $repositoryPath
try {
    New-Item -ItemType Directory -Force target/m4i | Out-Null
    & cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance_m4i -- --include-ignored --nocapture --test-threads=1 > target/m4i/validation.log 2>&1
    if ($LASTEXITCODE -ne 0) { throw 'M4I native tests failed; see target/m4i/validation.log' }
    & python tools/verify-constraint-validation-m4i.py target/m4i/validation.log > target/m4i/independent.log 2>&1
    if ($LASTEXITCODE -ne 0) { throw 'M4I independent verification failed; see target/m4i/independent.log' }
    Get-Content target/m4i/independent.log
} finally {
    Pop-Location
}
