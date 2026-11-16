# CPU-only failure-path tests. No application, GPU, window or Cargo is required.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'desktop-verification.ps1')
. (Join-Path $PSScriptRoot 'windows-build-common.ps1')
$scratch = Join-Path ([IO.Path]::GetTempPath()) ('qx-desktop-tests-' + [Guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $scratch
$checks = 0
function Assert-True($Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
    $script:checks++
}
function Assert-Fails([scriptblock]$Action, [string]$Expected) {
    $failure = $null
    try { $null = & $Action } catch { $failure = $_.Exception.Message }
    Assert-True ($failure -like "*$Expected*") "Expected '$Expected'; got '$failure'"
}
function Write-TestText([string]$Path, [string]$Value) { [IO.File]::WriteAllText($Path, $Value) }
function New-TestPackage {
    $path = Join-Path $scratch ([Guid]::NewGuid().ToString('N') + ' package')
    $null = New-Item -ItemType Directory -Path (Join-Path $path 'assets/catalog') -Force
    foreach ($name in @('QUANTUM Dynamics.exe','quantum-x.exe')) { Write-TestText (Join-Path $path $name) 'synthetic-not-executable' }
    Write-TestText (Join-Path $path 'assets/catalog/styles.json') '{}'
    $script:build = @{ verified_build = $true; version = '0.1.0'; source_commit = 'a' * 40
        source_tree = 'b' * 40; build_id = 'c' * 32; profile = 'release'; target = 'x86_64-pc-windows-msvc' }
    Write-TestText (Join-Path $path 'build-info.json') ($build | ConvertTo-Json)
    Write-PackageManifest $path
    $path
}
$system = { param($Runner,$Directory,$Environment)
    @{ os = @{ Caption = 'Synthetic test Windows'; Version = 'test' }; gpu_inventory = @(@{ Name = 'Synthetic inventory' }) }
}
$script:mode = 'success'
$script:invocations = 0
$fake = {
    param($Executable,$Arguments,$Directory,$Environment,$Timeout)
    $script:invocations++
    Assert-True ([string]::IsNullOrEmpty($Environment['QX_SOFTWARE_GPU']) -and $Environment['QX_ASSETS_ROOT'] -eq (Join-Path $Directory 'assets')) 'Child environment is not isolated'
    $r = @{ Started = $true; TimedOut = $false; Terminated = $true; CaptureComplete = $true
        ExitCode = 0; Stdout = ''; Stderr = ''; StdoutBytes = 1; StderrBytes = 0; ElapsedSeconds = 0.001; Error = $null }
    switch ($Arguments[0]) {
        '--build-info' {
            $data = $script:build.Clone()
            if ($script:mode -eq 'identity') { $data.source_commit = 'd' * 40 }
            $r.Stdout = $data | ConvertTo-Json
        }
        '--version' { $r.Stdout = 'QUANTUM Dynamics 0.1.0 (repository QUANTUM-X; development CLI quantum-x)' }
        '--self-check' { $r.Stdout = @{ product = 'QUANTUM Dynamics'; version = '0.1.0'; catalog_version = 1; entries = @(@{}) } | ConvertTo-Json -Depth 6 }
        '--startup-check' {
            $editor = $Arguments.Count -gt 1
            $r.Stdout = @{ page = $(if ($editor) { 'editor' } else { 'welcome' }); no_arguments = 'welcome'; document_open = $editor } | ConvertTo-Json
            if ($script:mode -eq 'startup') { $r.ExitCode = 17; $r.Stderr = "Startup failed at $env:USERPROFILE\private-project.qx.json" }
        }
        '--gui-smoke-test' {
            $page = if ($Arguments[1] -eq 'edit') { 'editor' } else { $Arguments[1] }
            $data = @{ ok = $true; result = 'passed'; hidden_window = $true; page = $page
                frames_requested = 5; frames_presented = 8; resources_released = $true
                adapter = 'AdapterInfo { name: "Synthetic", device_type: DiscreteGpu, backend: Dx12 }'
                exit = @{ reason = 'diagnostic_complete'; window_created = $true; frames_presented = 8 }
                script = @{ name = 'simulator_round_trip'; step = 'Done'; simulator_frames_drawn = 3
                    paused_ticks = 48; reset_matches_initial = $true; editor_restored = $true } }
            if ($script:mode -eq 'software') { $data.adapter = 'AdapterInfo { device_type: Cpu, backend: Dx12 }' }
            if ($script:mode -eq 'frames') { $data.frames_presented = 0 }
            if ($script:mode -eq 'reset') { $data.script.reset_matches_initial = $false }
            if ($script:mode -eq 'privacy') { $data.exit.detail = 'diagnostic path C:\private\driver\"quoted"'; $data.adapter += ', device_pci_bus_id: "0000:01:00.0"' }
            $r.Stdout = $data | ConvertTo-Json -Depth 10
            if ($script:mode -eq 'timeout') { $r.TimedOut = $true; $r.ExitCode = -1 }
            if ($script:mode -eq 'inner-timeout') { $r.ExitCode = 3; $r.Stdout = '{"ok":false,"result":"hard_timeout","stage":"gpu"}' }
            if ($script:mode -eq 'launch') { $r.Started = $false; $r.Error = 'Win32Exception'; $r.ExitCode = $null }
            if ($script:mode -eq 'malformed') { $r.Stdout = 'not JSON' }
            if ($script:mode -eq 'flood') { $r.StdoutBytes = 70000 }
        }
    }
    [pscustomobject]$r
}
function Run-Fixture([string]$Package, [string]$Mode) {
    $script:mode = $Mode; $script:invocations = 0
    $script:evidence = Join-Path $scratch ([Guid]::NewGuid().ToString('N') + ' evidence')
    Invoke-DesktopVerification -PackageDirectory $Package -OutputDirectory $evidence -MachineLabel 'synthetic-test' `
        -ProcessRunner $fake -SystemCollector $system
}
try {
    $package = New-TestPackage
    $before = [IO.File]::ReadAllText((Join-Path $package 'SHA256SUMS'))
    $r = Run-Fixture $package success
    Assert-True ($r.automated_status -eq 'passed' -and $invocations -eq 8) ("Valid synthetic workflow failed: " + ($r.automated_cases | Where-Object { $_.status -eq 'failed' } | ConvertTo-Json -Depth 4 -Compress))
    Assert-True ($r.manual_status -eq 'not_performed') 'Automated result claimed manual verification'
    $manual = Read-DesktopJson (Join-Path $evidence 'manual-checklist.json')
    Assert-True (@($manual.checks | Where-Object { $_.status -ne 'not_performed' }).Count -eq 0) 'Manual checklist falsely passed'
    Assert-True ((Read-DesktopJson (Join-Path $evidence 'report.json')).automated_status -eq 'passed') 'Machine report cannot round trip'
    Assert-True ((Get-Content (Join-Path $evidence 'summary.md') -Raw) -match 'NOT PERFORMED') 'Human summary omitted manual boundary'
    Assert-True ([IO.File]::ReadAllText((Join-Path $package 'SHA256SUMS')) -ceq $before) 'Package was modified'
    Assert-Fails { Invoke-DesktopVerification $package $evidence } 'new evidence'
    Assert-Fails { Invoke-DesktopVerification $package (Join-Path $package 'evidence') } 'non-overlapping'
    Assert-Fails { Invoke-DesktopVerification $package $scratch } 'non-overlapping'
    Assert-Fails { Invoke-DesktopVerification 'C:\' (Join-Path $scratch 'root') } 'filesystem root'
    Assert-Fails { Test-DesktopPackage $package ('0' * 64) } 'expected CI'
    Write-TestText (Join-Path $package 'quantum-x.exe') 'corrupt'
    $r = Run-Fixture $package success
    Assert-True ($r.package_integrity.status -eq 'failed' -and $invocations -eq 0) 'Corrupt package executed code'
    $package = New-TestPackage
    $r = Run-Fixture $package privacy
    $saved = Read-DesktopJson (Join-Path $evidence 'report.json')
    Assert-True ($saved.automated_status -eq 'passed' -and @($saved.automated_cases).Count -eq 8) 'Sanitization corrupted JSON structure'
    $savedNative = @($saved.automated_cases | Where-Object { $_.name -eq 'native-welcome' })[0].evidence
    Assert-True ($savedNative.exit.detail -notmatch 'C:' -and $savedNative.adapter -notmatch '0000:01') 'Structured report exposed a path or PCI location'
    # Move only the synthetic fixture file within this unique test workspace.
    Move-Item -LiteralPath (Join-Path $package 'QUANTUM Dynamics.exe') -Destination (Join-Path $scratch 'held.exe')
    Write-PackageManifest $package
    $r = Run-Fixture $package success
    Assert-True ($r.package_integrity.error -like '*Missing executable*' -and $invocations -eq 0) 'Missing executable not reported'
    $r = Run-Fixture (Join-Path $scratch 'absent') success
    Assert-True ($r.automated_status -eq 'failed' -and $invocations -eq 0) 'Missing package did not fail closed'
    $package = New-TestPackage
    foreach ($mode in @('identity','startup','software','frames','reset','timeout','inner-timeout','launch','malformed','flood')) {
        $r = Run-Fixture $package $mode
        Assert-True ($r.automated_status -eq 'failed') "Invalid evidence accepted: $mode"
        Assert-True ($r.automated_cases.Count -eq 8) "Incomplete failure report: $mode"
        if ($mode -eq 'identity') { Assert-True ($invocations -eq 1) 'Identity failure did not block subsequent execution' }
        else { Assert-True ($invocations -eq 8) 'Failure prevented independent checks' }
        if ($mode -eq 'startup') {
            $log = Get-Content (Join-Path $evidence 'startup-welcome.stderr.txt') -Raw
            Assert-True (-not $log.Contains($env:USERPROFILE)) 'Log exposed private profile path'
        }
        if ($mode -eq 'inner-timeout') {
            $native = @($r.automated_cases | Where-Object { $_.name -eq 'native-welcome' })[0]
            Assert-True ($native.timed_out -and $native.application_timed_out -and -not $native.outer_timed_out -and $native.evidence.stage -eq 'gpu') 'Application watchdog stage/timeout was lost'
        }
    }
    $r = Invoke-DesktopVerification -PackageDirectory $package -OutputDirectory (Join-Path $scratch 'inventory-failure') `
        -ProcessRunner $fake -SystemCollector { throw 'Inventory timeout' }
    Assert-True ($r.automated_status -eq 'failed' -and $r.system.error -eq 'Inventory timeout') 'Inventory failure was hidden'
    $script:mode = 'success'
    # Public command failure produces reports and a nonzero exit, without
    # running any synthetic executable or needing injection in the public CLI.
    $hostExe = Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
    $cliOutput = Join-Path $scratch 'public-cli-failure'
    & $hostExe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'verify-desktop.ps1') `
        -PackageDirectory (Join-Path $scratch 'absent') -OutputDirectory $cliOutput | Out-Null
    Assert-True ($LASTEXITCODE -eq 1 -and (Read-DesktopJson (Join-Path $cliOutput 'report.json')).automated_status -eq 'failed') 'Public CLI did not return failure evidence'
    # Real process supervision, with Windows' own PowerShell as a tiny child.
    # Add-Type compiles only the stream helper, never QUANTUM-X.
    $envMap = [Collections.Generic.Dictionary[string,string]]::new()
    $r = Invoke-DesktopProcess (Join-Path $scratch 'no-such-executable.exe') @() $scratch $envMap 1
    Assert-True (-not $r.Started -and $r.Error) 'Real missing executable was not captured'
    $r = Invoke-DesktopProcess $hostExe @('-NoProfile','-NonInteractive','-Command','[Console]::Error.WriteLine("startup failure"); exit 23') $scratch $envMap 5
    Assert-True ($r.Started -and $r.ExitCode -eq 23 -and $r.Stderr.Contains('startup failure')) 'Real startup failure was not captured'
    $r = Invoke-DesktopProcess $hostExe @('-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 30') $scratch $envMap 1
    Assert-True ($r.TimedOut -and $r.Terminated -and $r.ElapsedSeconds -lt 10) 'Real timeout was not bounded/terminated'
    $command = '$child = Start-Process -FilePath (Join-Path $env:SystemRoot "System32/WindowsPowerShell/v1.0/powershell.exe") -WindowStyle Hidden -ArgumentList @("-NoProfile","-NonInteractive","-Command","Start-Sleep -Seconds 30") -PassThru; [Console]::WriteLine($child.Id); Start-Sleep -Seconds 30'
    $r = Invoke-DesktopProcess $hostExe @('-NoProfile','-NonInteractive','-Command',$command) $scratch $envMap 2
    $spawnedId = 0
    Assert-True ([int]::TryParse($r.Stdout.Trim(), [ref]$spawnedId) -and $spawnedId -gt 0) 'Timeout fixture did not spawn its child'
    $alive = Get-Process -Id $spawnedId -ErrorAction SilentlyContinue
    Assert-True ($r.TimedOut -and $r.Terminated -and -not $alive) 'Timeout left a descendant running'
    $r = Invoke-DesktopProcess $hostExe @('-NoProfile','-NonInteractive','-Command','[Console]::Out.Write(("x" * 200000)); [Console]::Error.Write(("y" * 200000))') $scratch $envMap 5
    Assert-True ($r.ExitCode -eq 0 -and $r.CaptureComplete -and $r.StdoutBytes -eq 200000 -and $r.StderrBytes -eq 200000) 'Flood pipes were not drained'
    Assert-True ($r.Stdout.Length -eq 65536 -and $r.Stderr.Length -eq 65536) 'Output retention is not bounded'
    $argument = 'space "quote" trailing\'
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes('[Console]::Write($env:QX_TEST_VALUE)'))
    $envMap['QX_TEST_VALUE'] = $argument
    $r = Invoke-DesktopProcess $hostExe @('-NoProfile','-NonInteractive','-EncodedCommand',$encoded) $scratch $envMap 5
    Assert-True ($r.Stdout -ceq $argument -and -not $env:QX_TEST_VALUE) 'Child environment leaked or changed text'
    $echoScript = Join-Path $scratch 'echo arguments.ps1'
    Write-TestText $echoScript 'param([string]$Value) [Console]::Write($Value)'
    $r = Invoke-DesktopProcess $hostExe @('-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',$echoScript,'space and value') $scratch $envMap 5
    Assert-True ($r.Stdout -ceq 'space and value') 'Quoted process argument was changed'
    Assert-True ([QuantumDesktopVerification.ProcessRunner]::Quote('trailing\') -ceq '"trailing\\"') 'Trailing slash quoting is invalid'
    Assert-True ([QuantumDesktopVerification.ProcessRunner]::Quote('a"b') -ceq '"a\"b"') 'Embedded quote escaping is invalid'
    "Desktop verification checks passed: $checks (synthetic evidence; no application/GPU/window)"
} finally {
    $resolved = [IO.Path]::GetFullPath($scratch)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (-not $resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path -Leaf $resolved) -notlike 'qx-desktop-tests-*') { throw 'Unsafe test cleanup path' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
