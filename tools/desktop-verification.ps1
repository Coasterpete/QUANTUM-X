# R0.5 implementation and test seams. Dot-sourcing does not start the app.
$desktopTools = $PSScriptRoot

function Initialize-DesktopProcessRunner {
    if (-not ('QuantumDesktopVerification.ProcessRunner' -as [type])) {
        Add-Type -Path (Join-Path $desktopTools 'desktop-process.cs')
    }
}

function Invoke-DesktopProcess($Executable, [string[]]$Arguments, $WorkingDirectory, $Environment, $TimeoutSeconds) {
    Initialize-DesktopProcessRunner
    [QuantumDesktopVerification.ProcessRunner]::Run($Executable, $Arguments, $WorkingDirectory,
        $Environment, ($TimeoutSeconds * 1000), 65536)
}

function Assert-DesktopPlainPath([string]$Path) {
    $cursor = [IO.Path]::GetFullPath($Path)
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            if ((Get-Item -LiteralPath $cursor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw 'Links/reparse points are not supported in package or evidence paths'
            }
        }
        $cursor = Split-Path -Parent $cursor
    }
}

function Read-DesktopJson([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Missing metadata: $([IO.Path]::GetFileName($Path))" }
    if ((Get-Item -LiteralPath $Path).Length -gt 65536) { throw 'Package metadata exceeds 64 KiB' }
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Test-DesktopPackage([string]$Directory, [string]$ExpectedManifestSha256) {
    Assert-DesktopPlainPath $Directory
    if (-not (Test-Path -LiteralPath $Directory -PathType Container)) { throw 'Package directory is missing or is not an extracted folder' }
    # Walk without following links; bound the input before the existing trusted
    # integrity verifier hashes it. Use the verifier beside this runner, never
    # a script selected by the target package's metadata.
    $pending = [Collections.Generic.Stack[string]]::new()
    $pending.Push($Directory)
    $count = 0; $bytes = 0L
    while ($pending.Count) {
        foreach ($item in Get-ChildItem -LiteralPath $pending.Pop() -Force) {
            $count++
            if ($count -gt 10000) { throw 'Package exceeds 10000 entries' }
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Package contains a link/reparse point' }
            if ($item.PSIsContainer) { $pending.Push($item.FullName) }
            else { $bytes += $item.Length }
            if ($bytes -gt 4GB) { throw 'Package exceeds 4 GiB' }
        }
    }
    $manifest = Join-Path $Directory 'SHA256SUMS'
    if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) { throw 'SHA256SUMS is missing' }
    if ((Get-Item -LiteralPath $manifest).Length -gt 2MB) { throw 'SHA256SUMS exceeds 2 MiB' }
    if ($ExpectedManifestSha256 -and
        (Get-FileHash -LiteralPath $manifest -Algorithm SHA256).Hash -ine $ExpectedManifestSha256) {
        throw 'Manifest hash does not match the expected CI summary hash'
    }
    $integrity = & (Join-Path $desktopTools 'verify-package.ps1') -Directory $Directory
    foreach ($name in @('QUANTUM Dynamics.exe', 'quantum-x.exe')) {
        if (-not (Test-Path -LiteralPath (Join-Path $Directory $name) -PathType Leaf)) { throw "Missing executable: $name" }
    }
    if (-not (Test-Path -LiteralPath (Join-Path $Directory 'assets/catalog/styles.json') -PathType Leaf)) {
        throw 'Package catalog is missing'
    }
    $build = Read-DesktopJson (Join-Path $Directory 'build-info.json')
    if ($build.verified_build -ne $true -or $build.source_commit -cnotmatch '^[0-9a-f]{40}$' -or
        $build.source_tree -cnotmatch '^[0-9a-f]{40}$' -or $build.build_id -cnotmatch '^[0-9a-f]{32}$' -or
        $build.version -cnotmatch '^\d+\.\d+\.\d+([+-][A-Za-z0-9.+-]+)?$' -or
        $build.target -cne 'x86_64-pc-windows-msvc' -or $build.profile -cne 'release') {
        throw 'Invalid or unsupported Windows development build metadata'
    }
    $hash = (Get-FileHash -LiteralPath (Join-Path $Directory 'QUANTUM Dynamics.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    if ((Get-FileHash -LiteralPath (Join-Path $Directory 'quantum-x.exe') -Algorithm SHA256).Hash.ToLowerInvariant() -cne $hash) {
        throw 'Public and development executables differ'
    }
    [pscustomobject]@{ integrity = $integrity; build = $build; executable_sha256 = $hash }
}

function Assert-DesktopNativeEvidence($Report, [string]$Page) {
    if (-not $Report -or $Report.ok -ne $true -or $Report.result -cne 'passed' -or
        $Report.hidden_window -ne $true -or $Report.page -cne $Page -or
        $Report.frames_requested -lt 5 -or $Report.frames_presented -lt $Report.frames_requested -or
        $Report.exit.frames_presented -ne $Report.frames_presented -or
        $Report.resources_released -ne $true -or $Report.exit.reason -cne 'diagnostic_complete' -or
        $Report.exit.window_created -ne $true) { throw 'Native smoke did not prove startup, presentation and clean exit' }
    # Inventory is not selected-adapter evidence. Require the actual native
    # report to identify a hardware adapter, on either owner configuration.
    if ($Report.adapter -cnotmatch 'device_type: (IntegratedGpu|DiscreteGpu)' -or
        $Report.adapter -cnotmatch 'backend: (Dx12|Vulkan|Gl)' -or
        $Report.adapter -match 'Microsoft Basic Render Driver|llvmpipe|lavapipe|SwiftShader') {
        throw 'Native smoke did not identify a supported hardware GPU adapter'
    }
    if ($Page -eq 'simulator' -and
        ($Report.script.name -cne 'simulator_round_trip' -or $Report.script.step -cne 'Done' -or
         $Report.script.simulator_frames_drawn -lt 1 -or $Report.script.paused_ticks -lt 48 -or
         $Report.script.reset_matches_initial -ne $true -or $Report.script.editor_restored -ne $true)) {
        throw 'Simulator Play/Pause/Reset/Return round trip was incomplete'
    }
}

function Get-DesktopSystemEvidence($ProcessRunner, [string]$WorkingDirectory, $Environment) {
    # Explicit allowlist: no hostname, username, serial, PNP/device ID, display
    # EDID, network configuration, installed-program list or full dxdiag dump.
    $command = @'
$ErrorActionPreference = 'Stop'
$os = Get-CimInstance Win32_OperatingSystem -OperationTimeoutSec 10 | Select-Object Caption,Version,BuildNumber,OSArchitecture
$gpu = @(Get-CimInstance Win32_VideoController -OperationTimeoutSec 10 | Select-Object -First 8 Name,AdapterCompatibility,DriverVersion,@{n='DriverDate';e={if ($_.DriverDate) {$_.DriverDate.ToUniversalTime().ToString('o')} else {$null}}})
@{ os = $os; gpu_inventory = $gpu } | ConvertTo-Json -Depth 5 -Compress
'@
    $powershell = Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
    $r = & $ProcessRunner $powershell @('-NoProfile','-NonInteractive','-EncodedCommand',$encoded) $WorkingDirectory $Environment 30
    if (-not $r.Started -or $r.TimedOut -or $r.ExitCode -ne 0 -or -not $r.CaptureComplete -or $r.StdoutBytes -gt 65536) {
        throw 'Windows/GPU inventory failed or timed out (30 s limit)'
    }
    $r.Stdout | ConvertFrom-Json
}

function Invoke-DesktopVerification {
    param(
        [string]$PackageDirectory, [string]$OutputDirectory, [string]$MachineLabel = 'unspecified',
        [int]$CommandTimeoutSeconds = 30, [int]$NativeTimeoutSeconds = 45,
        [string]$ExpectedManifestSha256,
        # Test seams exist only in this dot-sourced implementation, never in the
        # public command. Synthetic tests cannot exercise the owner CLI by mock.
        [scriptblock]$ProcessRunner = ${function:Invoke-DesktopProcess},
        [scriptblock]$SystemCollector = ${function:Get-DesktopSystemEvidence}
    )
    if ([Environment]::OSVersion.Platform -ne 'Win32NT') { throw 'Desktop verification requires Windows' }
    $package = [IO.Path]::GetFullPath($PackageDirectory).TrimEnd('\','/')
    $output = [IO.Path]::GetFullPath($OutputDirectory).TrimEnd('\','/')
    if ($package -ieq [IO.Path]::GetPathRoot([IO.Path]::GetFullPath($PackageDirectory)).TrimEnd('\','/')) {
        throw 'Choose the extracted package folder, not a filesystem root'
    }
    if ($output -ieq $package -or $output.StartsWith($package + '\', [StringComparison]::OrdinalIgnoreCase) -or
        $package.StartsWith($output + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Evidence and package directories must be separate, non-overlapping folders'
    }
    Assert-DesktopPlainPath $output
    if (Test-Path -LiteralPath $output) { throw 'Use a new evidence output directory; existing evidence is never overwritten' }
    $null = New-Item -ItemType Directory -Path $output
    $cases = [Collections.Generic.List[object]]::new()
    $verifierFiles = @('verify-desktop.ps1','desktop-verification.ps1','desktop-process.cs',
        'verify-package.ps1','desktop-manual-checklist.md','desktop-manual-checklist.json') | ForEach-Object {
        @{ name = $_; sha256 = (Get-FileHash -LiteralPath (Join-Path $desktopTools $_) -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
    $report = [ordered]@{
        format_version = 1; milestone = 'R0.5'; started_utc = [DateTime]::UtcNow.ToString('o'); completed_utc = $null
        verifier = @{ files = @($verifierFiles); powershell_version = $PSVersionTable.PSVersion.ToString(); process_64bit = [Environment]::Is64BitProcess }
        machine_label = $MachineLabel; automated_status = 'running'; manual_status = 'not_performed'
        application = $null; package_integrity = [ordered]@{ status = 'not_run'; error = $null }
        system = [ordered]@{ status = 'not_run'; os = $null; gpu_inventory = @(); error = $null }
        policy = [ordered]@{ application_rebuilt = $false; interactive_desktop_control = $false
            software_gpu_override = 'cleared_in_child'; state = 'isolated'; log_limit_bytes_per_stream = 65536
            visible_visual_quality_verified = $false; performance_verified = $false }
        automated_cases = $cases
        additional_roadmap_checks = @(
            @{ name = 'simulator-timing'; status = 'not_run'; reason = 'Separate owner hardware timing campaign; not a smoke or FPS result' },
            @{ name = 'cross-machine-golden-replay'; status = 'not_run'; reason = 'R0.3 test runner is not in the development package; no Cargo invoked' },
            @{ name = 'installer-uninstaller'; status = 'not_applicable'; reason = 'Portable package; installer is a future milestone' }
        )
    }
    $privateStrings = @($package, $output, $env:USERPROFILE, $env:USERNAME, $env:COMPUTERNAME) | Where-Object { $_ -and $_.Length -gt 2 }
    function Protect-DesktopText([string]$Value, [int]$Limit = 65536) {
        foreach ($private in $privateStrings) {
            foreach ($variant in @($private.Replace('\','\\'), $private, $private.Replace('\','/'))) {
                $Value = [regex]::Replace($Value, [regex]::Escape($variant), '<redacted>', 'IgnoreCase')
            }
        }
        # Paths outside our known roots, including build-host paths in Rust
        # panics. Logs are diagnostic text; sanitized JSON lives in report.json.
        $Value = [regex]::Replace($Value, '(?i)[a-z]:[\\/][^\r\n"<>|]*', '<path>')
        $Value = [regex]::Replace($Value, 'device_pci_bus_id:[^,}]*', 'device_pci_bus_id: <omitted>')
        $Value = [regex]::Replace($Value, '[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]', '')
        if ($Limit -gt 0 -and $Value.Length -gt $Limit) { $Value = $Value.Substring(0,$Limit) }
        $Value
    }
    function ConvertTo-DesktopSanitizedValue($Value) {
        if ($null -eq $Value) { return $null }
        if ($Value -is [string]) { return (Protect-DesktopText $Value) }
        if ($Value -is [Collections.IDictionary]) {
            $clean = [ordered]@{}
            foreach ($key in $Value.Keys) { $clean[$key] = ConvertTo-DesktopSanitizedValue $Value[$key] }
            return $clean
        }
        if ($Value -is [pscustomobject]) {
            $clean = [ordered]@{}
            foreach ($property in $Value.PSObject.Properties) { $clean[$property.Name] = ConvertTo-DesktopSanitizedValue $property.Value }
            return [pscustomobject]$clean
        }
        if ($Value -is [Collections.IEnumerable]) {
            $clean = [Collections.Generic.List[object]]::new()
            foreach ($item in $Value) { $clean.Add((ConvertTo-DesktopSanitizedValue $item)) }
            return ,($clean.ToArray())
        }
        return $Value
    }
    function Write-DesktopText([string]$Name, [string]$Value) {
        # Sanitize JSON string values before serializing, so Windows paths with
        # escaped quotes/backslashes cannot corrupt the JSON escape sequences.
        # Never truncate a serialized report into an invalid document.
        if ($Name.EndsWith('.json')) {
            $parsed = $Value | ConvertFrom-Json
            $Value = ConvertTo-DesktopSanitizedValue $parsed | ConvertTo-Json -Depth 30
        } else { $Value = Protect-DesktopText $Value }
        [IO.File]::WriteAllText((Join-Path $output $Name), $Value, [Text.UTF8Encoding]::new($false))
    }
    function Save-DesktopReport {
        Write-DesktopText 'report.json' ($report | ConvertTo-Json -Depth 30)
        $selected = @($cases | Where-Object { $_.name -like 'native-*' -and $_.evidence.adapter } | ForEach-Object { $_.evidence.adapter } | Select-Object -Unique)
        $lines = @('# R0.5 desktop package verification', '',
            "Machine label: $MachineLabel", "Automated status: $($report.automated_status)",
            "Application: $($report.application.version); source: $($report.application.source_commit)",
            "Package integrity: $($report.package_integrity.status)", "Manifest SHA-256: $($report.package_integrity.manifest_sha256)",
            "Windows/GPU inventory: $($report.system.status)",
            "Windows: $($report.system.os.Caption) $($report.system.os.Version) $($report.system.os.OSArchitecture)",
            "Installed GPUs (inventory only): $((@($report.system.gpu_inventory) | ForEach-Object { $_.Name }) -join '; ')",
            "Selected native adapters: $($selected -join '; ')",
            '', '| Automated check | Status | Started | Exit | Timeout |', '| --- | --- | --- | --- | --- |')
        $lines += @($cases | ForEach-Object { "| $($_.name) | $($_.status) | $($_.started) | $($_.exit_code) | $($_.timed_out) |" })
        $lines += @('', 'Manual checks: NOT PERFORMED. Complete manual-checklist.md/json separately.',
            'Hidden-window presentation does not verify visible appearance, FPS, responsiveness, DPI or monitors.',
            'Timing, cross-machine golden replay and installer checks: see report.json additional_roadmap_checks.',
            '', 'Failures:')
        $lines += @($report.package_integrity.error, $report.system.error) | Where-Object { $_ }
        $lines += @($cases | Where-Object { $_.error } | ForEach-Object { "$($_.name): $($_.error)" })
        Write-DesktopText 'summary.md' ($lines -join "`n")
    }
    $environment = [Collections.Generic.Dictionary[string,string]]::new()
    $environment['QX_ASSETS_ROOT'] = Join-Path $package 'assets'
    $environment['QX_SOFTWARE_GPU'] = $null
    $environment['QX_PLAYBACK_WAIT'] = $null
    $environment['QX_EXPERIMENTAL_TRAIN_VELOCITY'] = $null
    $environment['QX_WORKBENCH_PREFS'] = Join-Path $output 'state/workbench.json'
    $environment['QX_RECENT_PROJECTS'] = Join-Path $output 'state/recent.json'
    $environment['RUST_BACKTRACE'] = '0'
    # Ensure the allowlisted state overrides cannot redirect into user files.
    $null = New-Item -ItemType Directory -Path (Join-Path $output 'state')
    $manual = Read-DesktopJson (Join-Path $desktopTools 'desktop-manual-checklist.json')
    Write-DesktopText 'manual-checklist.json' ($manual | ConvertTo-Json -Depth 10)
    Write-DesktopText 'manual-checklist.md' ([IO.File]::ReadAllText((Join-Path $desktopTools 'desktop-manual-checklist.md')))
    Save-DesktopReport
    try {
        try {
            $system = & $SystemCollector $ProcessRunner $output $environment
            if (-not $system.os -or @($system.gpu_inventory).Count -eq 0) { throw 'Windows/GPU inventory is incomplete' }
            $report.system = [ordered]@{ status = 'passed'; os = $system.os; gpu_inventory = @($system.gpu_inventory); error = $null }
        } catch { $report.system.status = 'failed'; $report.system.error = Protect-DesktopText $_.Exception.Message }
        try {
            $validated = Test-DesktopPackage $package $ExpectedManifestSha256
            $report.package_integrity = [ordered]@{ status = 'passed'; files = $validated.integrity.files
                bytes = $validated.integrity.bytes; manifest_sha256 = $validated.integrity.manifest_sha256
                expected_manifest_checked = [bool]$ExpectedManifestSha256; error = $null }
            $build = $validated.build
            $report.application = [ordered]@{ version = $build.version; source_commit = $build.source_commit
                source_tree = $build.source_tree; build_id = $build.build_id; target = $build.target
                profile = $build.profile; executable_sha256 = $validated.executable_sha256 }
        } catch { $report.package_integrity.status = 'failed'; $report.package_integrity.error = Protect-DesktopText $_.Exception.Message }
        Save-DesktopReport
        $definitions = @(
            @{ name = 'build-info'; args = @('--build-info'); executable = 'QUANTUM Dynamics.exe' },
            @{ name = 'version'; args = @('--version'); executable = 'QUANTUM Dynamics.exe' },
            @{ name = 'self-check'; args = @('--self-check'); executable = 'QUANTUM Dynamics.exe' },
            @{ name = 'startup-welcome'; args = @('--startup-check'); executable = 'QUANTUM Dynamics.exe' },
            @{ name = 'startup-editor'; args = @('--startup-check','edit'); executable = 'quantum-x.exe' },
            @{ name = 'native-welcome'; args = @('--gui-smoke-test','welcome'); executable = 'QUANTUM Dynamics.exe'; page = 'welcome' },
            @{ name = 'native-editor'; args = @('--gui-smoke-test','edit'); executable = 'quantum-x.exe'; page = 'editor' },
            @{ name = 'native-simulator'; args = @('--gui-smoke-test','simulator'); executable = 'quantum-x.exe'; page = 'simulator' }
        )
        $identityPassed = $false
        foreach ($case in $definitions) {
            $timeout = if ($case.page) { $NativeTimeoutSeconds } else { $CommandTimeoutSeconds }
            $record = [ordered]@{ name = $case.name; executable = $case.executable; arguments = $case.args
                status = 'not_run'; started = $false; timeout_s = $timeout; timed_out = $false
                exit_code = $null; terminated = $null; elapsed_s = $null; capture_complete = $null
                stdout_bytes = 0; stderr_bytes = 0; logs_truncated = $false; evidence = $null; error = $null }
            $cases.Add($record)
            if ($report.package_integrity.status -ne 'passed' -or ($case.name -ne 'build-info' -and -not $identityPassed)) {
                $record.error = 'Blocked by package integrity or executable identity failure'
                Save-DesktopReport
                continue
            }
            try {
                $r = & $ProcessRunner (Join-Path $package $case.executable) $case.args $package $environment $timeout
                $record.started = $r.Started; $record.timed_out = $r.TimedOut; $record.exit_code = $r.ExitCode
                $record.terminated = $r.Terminated; $record.elapsed_s = $r.ElapsedSeconds; $record.capture_complete = $r.CaptureComplete
                $record.stdout_bytes = $r.StdoutBytes; $record.stderr_bytes = $r.StderrBytes
                $record.logs_truncated = $r.StdoutBytes -gt 65536 -or $r.StderrBytes -gt 65536
                Write-DesktopText ($case.name + '.stdout.txt') $r.Stdout
                Write-DesktopText ($case.name + '.stderr.txt') $r.Stderr
                if (-not $r.Started) { throw "Process failed to start: $($r.Error)" }
                if ($r.TimedOut) { throw "Process exceeded $timeout s; termination attempted for its process tree" }
                if (-not $r.Terminated -or -not $r.CaptureComplete -or $r.Error) { throw 'Process termination or stream capture failed' }
                if ($r.ExitCode -ne 0) { throw "Process exit code $($r.ExitCode); see bounded logs" }
                if ($record.logs_truncated) { throw 'Diagnostic stream exceeded 64 KiB; incomplete evidence cannot pass' }
                if ($case.name -eq 'version') {
                    if (-not $r.Stdout.Trim().StartsWith("QUANTUM Dynamics $($build.version) (repository ", [StringComparison]::Ordinal)) {
                        throw 'Application version does not match package metadata'
                    }
                    $record.evidence = @{ version = $build.version }
                } else {
                    $data = $r.Stdout | ConvertFrom-Json
                    switch ($case.name) {
                        'build-info' {
                            foreach ($field in @('version','source_commit','source_tree','build_id','target','profile','verified_build')) {
                                if ($data.$field -cne $build.$field) { throw "Embedded build metadata mismatch: $field" }
                            }
                            $identityPassed = $true
                            $record.evidence = @{ embedded_build_identity_matches = $true }
                        }
                        'self-check' {
                            if ($data.product -cne 'QUANTUM Dynamics' -or $data.version -cne $build.version -or
                                $data.catalog_version -lt 1 -or @($data.entries).Count -lt 1) { throw 'Catalog self-check report is incomplete' }
                            $record.evidence = @{ catalog_version = $data.catalog_version; entries = @($data.entries).Count
                                construction_checks = @($data.construction_checks).Count }
                        }
                        'startup-welcome' {
                            if ($data.page -cne 'welcome' -or $data.no_arguments -cne 'welcome' -or $data.document_open -ne $false) {
                                throw 'Welcome startup route failed'
                            }
                            $record.evidence = @{ page = $data.page; document_open = $data.document_open }
                        }
                        'startup-editor' {
                            if ($data.page -cne 'editor' -or $data.document_open -ne $true) { throw 'Editor startup route failed' }
                            $record.evidence = @{ page = $data.page; document_open = $data.document_open }
                        }
                        default {
                            # Retain failure-stage/exit evidence even if assertions fail.
                            $record.evidence = $data
                            Assert-DesktopNativeEvidence $data $case.page
                        }
                    }
                }
                $record.status = 'passed'
            } catch {
                $record.status = 'failed'; $record.error = Protect-DesktopText $_.Exception.Message
            }
            Save-DesktopReport
            Write-Host "$($case.name): $($record.status); exit=$($record.exit_code); timeout=$($record.timed_out)"
        }
        $report.automated_status = if ($report.package_integrity.status -eq 'passed' -and $report.system.status -eq 'passed' -and
            $cases.Count -eq 8 -and @($cases | Where-Object { $_.status -ne 'passed' }).Count -eq 0) { 'passed' } else { 'failed' }
    } finally {
        if ($report.automated_status -eq 'running') { $report.automated_status = 'failed' }
        $report.completed_utc = [DateTime]::UtcNow.ToString('o')
        Save-DesktopReport
    }
    [pscustomobject]$report
}
