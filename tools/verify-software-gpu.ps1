# R0.4 hosted-runner experiment. Requires PowerShell 7; never builds the app.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'software-gpu-evidence.ps1')
$root = Split-Path -Parent $PSScriptRoot
$exe = (Resolve-Path -LiteralPath $Executable).Path
if ($env:QX_SOFTWARE_GPU -cne 'warp') { throw 'Set QX_SOFTWARE_GPU=warp explicitly' }
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Use a new output directory to prevent stale evidence' }
$output = (New-Item -ItemType Directory -Path $OutputDirectory).FullName
$records = [Collections.Generic.List[object]]::new()
$summary = [ordered]@{
    experiment = 'R0.4'; source_commit = (git -C $root rev-parse HEAD)
    run_id = $env:GITHUB_RUN_ID; run_attempt = $env:GITHUB_RUN_ATTEMPT
    image = $env:ImageOS; image_version = $env:ImageVersion
    os = [Environment]::OSVersion.VersionString
    requested_adapter = $env:QX_SOFTWARE_GPU; cases = $records; ok = $false
    manual_visual_verification = $false
}
function Save-Summary {
    $summary | ConvertTo-Json -Depth 25 | Set-Content -LiteralPath (Join-Path $output 'summary.json') -Encoding utf8
}
Save-Summary
$cases = @(
    @{ name = 'offscreen-welcome'; kind = 'offscreen'; page = 'welcome'; timeout = 120
       args = @('render-welcome', (Join-Path $output 'welcome.png')) },
    @{ name = 'offscreen-editor'; kind = 'offscreen'; page = 'editor'; timeout = 120
       args = @('render-editor', (Join-Path $root 'examples/foundation.qx.json'), (Join-Path $output 'editor.png'), 'track', '1280', '800') },
    @{ name = 'native-welcome'; kind = 'native'; page = 'welcome'; timeout = 45; args = @('--gui-smoke-test', 'welcome') },
    @{ name = 'native-editor'; kind = 'native'; page = 'editor'; timeout = 45; args = @('--gui-smoke-test', 'edit') },
    @{ name = 'native-simulator'; kind = 'native'; page = 'simulator'; timeout = 45; args = @('--gui-smoke-test', 'simulator') }
)
foreach ($case in $cases) {
    $record = [ordered]@{ name = $case.name; kind = $case.kind; page = $case.page
        args = $case.args; timeout_s = $case.timeout; timed_out = $false; exit_code = $null
        ok = $false; adapter = $null; report = $null; error = $null }
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $process = [Diagnostics.Process]::new()
    try {
        $start = [Diagnostics.ProcessStartInfo]::new($exe)
        $start.WorkingDirectory = $root
        $start.UseShellExecute = $false
        $start.CreateNoWindow = $true
        $start.RedirectStandardOutput = $true
        $start.RedirectStandardError = $true
        # Isolated application state is provided by the existing smoke entry
        # points; explicit assets keep the temporary executable reproducible.
        $start.Environment['QX_ASSETS_ROOT'] = Join-Path $root 'assets'
        foreach ($argument in $case.args) { $start.ArgumentList.Add($argument) }
        $process.StartInfo = $start
        $null = $process.Start()
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($case.timeout * 1000)) {
            $record.timed_out = $true
            $process.Kill($true)
            if (-not $process.WaitForExit(5000)) { throw 'Child did not terminate after timeout' }
        }
        $stdout = $stdoutTask.GetAwaiter().GetResult()
        $stderr = $stderrTask.GetAwaiter().GetResult()
        $stdout | Set-Content -LiteralPath (Join-Path $output "$($case.name).stdout.txt") -Encoding utf8
        $stderr | Set-Content -LiteralPath (Join-Path $output "$($case.name).stderr.txt") -Encoding utf8
        $record.exit_code = $process.ExitCode
        $diagnostics = @($stderr -split '\r?\n' | Where-Object { $_.StartsWith('QX_SOFTWARE_GPU_ADAPTER ') })
        if ($diagnostics.Count -gt 0) {
            $record.adapter = $diagnostics[-1].Substring('QX_SOFTWARE_GPU_ADAPTER '.Length) | ConvertFrom-Json
        }
        if ($case.kind -eq 'native' -and $stdout.Trim()) { $record.report = $stdout | ConvertFrom-Json }
        if ($record.timed_out) { throw "Process exceeded $($case.timeout) s" }
        if ($record.exit_code -ne 0) { throw "Process exit code $($record.exit_code); see raw stdout/stderr" }
        $captureValid = $false
        if ($case.kind -eq 'offscreen') {
            $png = Join-Path $output "$($case.page).png"
            $metadata = Join-Path $output "$($case.page).capture.json"
            if ((Test-Path -LiteralPath $png) -and (Test-Path -LiteralPath $metadata)) {
                $bytes = [IO.File]::ReadAllBytes($png)
                $capture = Get-Content -LiteralPath $metadata -Raw | ConvertFrom-Json
                $captureValid = $bytes.Length -gt 24 -and
                    [Convert]::ToHexString($bytes[0..7]) -eq '89504E470D0A1A0A' -and
                    $capture.size[0] -eq 1280 -and $capture.size[1] -eq 800 -and
                    $stdout -match '[1-9][0-9]* visible pixels'
            }
        }
        Assert-SoftwareGpuEvidence $record.adapter $case.kind $case.page $record.report $captureValid
        $record.ok = $true
    } catch {
        $record.error = $_.Exception.Message
    } finally {
        $watch.Stop()
        $record.elapsed_s = [math]::Round($watch.Elapsed.TotalSeconds, 3)
        $process.Dispose()
        $records.Add($record)
        $record | ConvertTo-Json -Depth 25 | Set-Content -LiteralPath (Join-Path $output "$($case.name).json") -Encoding utf8
        Save-Summary
        Write-Host "$($case.name): ok=$($record.ok); exit=$($record.exit_code); timeout=$($record.timed_out); $($record.error)"
    }
}
$summary.ok = $records.Count -eq 5 -and @($records | Where-Object { -not $_.ok }).Count -eq 0
Save-Summary
if ($env:GITHUB_STEP_SUMMARY) {
    @('### R0.4 WARP experiment (non-blocking)', '', '| Path | Passed | Exit | Timeout |', '| --- | --- | --- | --- |') +
        @($records | ForEach-Object { "| $($_.name) | $($_.ok) | $($_.exit_code) | $($_.timed_out) |" }) +
        @('', 'Offscreen readback and native hidden-window presentation are separate evidence. No visual/performance claim.') |
        Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY
}
if (-not $summary.ok) { throw 'Software GPU experiment failed; inspect summary.json and per-path logs' }
