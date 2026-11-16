param(
    [string]$OutputDirectory,
    # Reuse the single build from build-windows.ps1; this mode never invokes Cargo.
    [string]$BuildReceipt
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-build-common.ps1')
$workspace = Split-Path -Parent $PSScriptRoot
Push-Location $workspace
$oldAssetsRoot = $env:QX_ASSETS_ROOT
try {
    $identity = Get-Content -LiteralPath (Join-Path $workspace 'product.json') -Raw | ConvertFrom-Json
    if (-not $BuildReceipt) {
        # Explicit local fallback. Canonical packages come from Actions instead.
        $BuildReceipt = Join-Path ([IO.Path]::GetTempPath()) ("qx-build-{0}.json" -f [guid]::NewGuid().ToString('N'))
        & (Join-Path $PSScriptRoot 'build-windows.ps1') -ReceiptPath $BuildReceipt
    }
    $sourceState = Get-PackageSource
    $receipt = Read-WindowsBuildReceipt $BuildReceipt $sourceState
    $package = $receipt.build_info
    $source = $receipt.executable
    $embeddedText = & $source --build-info
    if ($LASTEXITCODE -ne 0) { throw 'Executable cannot report build provenance' }
    $embedded = $embeddedText | ConvertFrom-Json
    if (($embedded | ConvertTo-Json -Depth 6 -Compress) -cne ($package | ConvertTo-Json -Depth 6 -Compress)) {
        throw 'Executable embedded provenance does not match build receipt'
    }
    $info = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($source)
    if ($info.ProductName -ne $identity.name -or $info.ProductVersion -ne $package.version) { throw 'Embedded product metadata/version is incorrect' }
    $runtime = & (Join-Path $PSScriptRoot 'verify-windows-runtime.ps1') -Executable $source
    $iconVerifier = Join-Path $PSScriptRoot 'verify-windows-icon.ps1'
    $sourceIcon = Join-Path $workspace 'assets/icons/quantum-dynamics/QUANTUM_Dynamics.ico'
    # A missing/stale/different icon must fail packaging, including cached builds.
    $iconCheck = & $iconVerifier -Executable $source -Icon $sourceIcon
    if (-not $OutputDirectory) {
        $shortCommit = $package.source_commit.Substring(0, 12)
        $OutputDirectory = Join-Path $receipt.target_directory "distribution/QUANTUM Dynamics-$($package.version)-$shortCommit-windows"
    }
    $destination = [System.IO.Path]::GetFullPath($OutputDirectory)
    if (Test-Path -LiteralPath $destination) { throw "Destination exists; choose a new -OutputDirectory: $destination" }
    New-Item -ItemType Directory -Path $destination | Out-Null
    Copy-Item -LiteralPath $source -Destination (Join-Path $destination $identity.windows_executable)
    Copy-Item -LiteralPath $source -Destination (Join-Path $destination $identity.development_executable)
    Copy-Item -LiteralPath (Join-Path $workspace 'product.json') -Destination $destination
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'verify-package.ps1') -Destination $destination
    # R0.5 also works against older extracted packages when run from tools/.
    # Ship the same standalone runner for subsequent packages, without a build.
    foreach ($file in @('verify-desktop.ps1', 'desktop-verification.ps1', 'desktop-process.cs',
                        'desktop-manual-checklist.md', 'desktop-manual-checklist.json')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination $destination
    }
    $package | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $destination 'build-info.json') -Encoding utf8
    $runtime | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $destination 'runtime-check.json') -Encoding utf8
    $assetsDestination = Join-Path $destination 'assets'
    New-Item -ItemType Directory -Path $assetsDestination | Out-Null
    foreach ($folder in @('catalog', 'tracks', 'trains', 'development', 'materials')) {
        Copy-Item -LiteralPath (Join-Path $workspace "assets/$folder") -Destination $assetsDestination -Recurse
    }
    # Ship the existing numerical fixtures, never generate a demonstration ride.
    $installedProjects = Join-Path $workspace 'assets/projects'
    if (Test-Path -LiteralPath $installedProjects) {
        Copy-Item -LiteralPath $installedProjects -Destination $assetsDestination -Recurse
    }
    $samplesDestination = Join-Path $assetsDestination 'projects/samples'
    New-Item -ItemType Directory -Path $samplesDestination -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $assetsDestination 'projects/templates') -Force | Out-Null
    $sampleFiles = @(Get-ChildItem -LiteralPath (Join-Path $workspace 'examples') -File -Filter '*.qx.json')
    foreach ($sample in $sampleFiles) {
        $sampleTarget = Join-Path $samplesDestination $sample.Name
        if (Test-Path -LiteralPath $sampleTarget) {
            if ((Get-FileHash -LiteralPath $sampleTarget).Hash -ne (Get-FileHash -LiteralPath $sample.FullName).Hash) { throw "Installed sample collides with repository fixture: $($sample.Name)" }
        } else {
            Copy-Item -LiteralPath $sample.FullName -Destination $samplesDestination
        }
    }
    $sampleCount = @(Get-ChildItem -LiteralPath $samplesDestination -File -Filter '*.qx.json').Count
    $templateCount = @(Get-ChildItem -LiteralPath (Join-Path $assetsDestination 'projects/templates') -File -Filter '*.qx.json').Count
    # Include only license notices for the embedded editor assets, not the
    # owner's separate icon source checkouts or any unrelated/C++ project.
    $notices = Join-Path $destination 'notices'
    New-Item -ItemType Directory -Path $notices | Out-Null
    foreach ($file in @('LICENSE-MIT','LICENSE-APACHE')) {
        Copy-Item -LiteralPath (Join-Path $workspace $file) -Destination $notices
    }
    Copy-Item -LiteralPath (Join-Path $workspace 'assets/fonts/OverpassMono/OFL.txt') -Destination (Join-Path $notices 'OverpassMono-OFL.txt')
    Copy-Item -LiteralPath (Join-Path $workspace 'assets/editor-icons/LICENSE-LUCIDE') -Destination $notices
    Copy-Item -LiteralPath (Join-Path $workspace 'assets/editor-icons/LICENSE-CARBON') -Destination $notices
    Copy-Item -LiteralPath (Join-Path $workspace 'docs/licenses/egui-default-fonts') -Destination $notices -Recurse
    @"
$($identity.name) $($package.version) - development distribution

Open '$($identity.windows_executable)' to start at the Welcome Screen, with no document open.
New Project asks for a name, then opens the Editor with no assigned coaster style.
Edit > Coaster Properties configures name, description, family and numbered style.
Open validates a .qx.json before entering the Editor. Each project has one coaster.
Library includes $sampleCount existing sample projects and $templateCount installed templates.
File > Return to Welcome Screen respects unsaved changes and unapplied drafts.
Use quantum-x.exe edit [project.qx.json] for the direct development editor workflow.
The public executable and quantum-x.exe also accept all development CLI commands.
Keep the assets folder beside both executables. Projects remain .qx.json files.
Box Spined-100 integrates the owner's original swept spine and rigid cross-ties.
Its configurable 1.10 m rail centers / 0.13 m diameter / 1.50 m tie pitch are provisional.
Other track styles and all finished train models remain Planned. No engineering validation is claimed.
Run .\quantum-x.exe --self-check for a noninteractive catalog/identity check.
Run '.\QUANTUM Dynamics.exe' --startup-check for a noninteractive startup-state check.
Welcome and Editor follow the owner's colorful, high-contrast Inkscape references.
Setup configures the application; Editor offers New/Open when no project is open.
View > Development tools exposes experimental train tools and diagnostics.
Track Workspace > Operations provides topology, bounded completion, launches/brakes and physical settings.
Engineering plots use the Transition pane. Support Workspace edits solids and generates editable timber.
All construction/operation changes require Apply or Cancel and support undo and schema-7 saving.
Experimental playback applies dimensioned forces at 240 Hz; stations, lifts, blocks and structural analysis remain unavailable.
News is unavailable in this Alpha build.
This unsigned portable folder has no installer, updater or file associations.
Extract the complete folder before launching. Rust and Cargo are not required.
Before first launch, run powershell -NoProfile -ExecutionPolicy Bypass -File .\verify-package.ps1
SHA256SUMS covers every file except itself, in ordinal path order with LF and UTF-8 (no BOM).
It detects corruption; it is not a signature or a guarantee of publisher identity.
build-info.json and --build-info identify the exact source, compiler and workflow build.
This is an unsigned x64 Windows MSVC development build. A compatible graphics driver is required.
CPU-only package checks do not verify GPU startup or visual quality on your computer.
For R0.5 native hardware checks (no rebuild), run from PowerShell:
powershell -NoProfile -ExecutionPolicy Bypass -File .\verify-desktop.ps1 -PackageDirectory . -OutputDirectory C:\scratch\new-desktop-evidence -MachineLabel unspecified
Choose a NEW evidence folder outside this package. report.json and summary.md are automated evidence.
Complete the generated manual-checklist.md/json separately; native hidden-window smokes do not verify visual quality.
"@ | Set-Content -LiteralPath (Join-Path $destination 'README.txt') -Encoding utf8
    $distribution = [ordered]@{
        product = $identity.name
        version = $package.version
        repository = $identity.repository
        executable = $identity.windows_executable
        development_executable = $identity.development_executable
        sha256 = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
        icon_resource_id = $iconCheck.resource_id
        icon_source_sha256 = $iconCheck.source_icon_sha256
        content = 'development; original Box Spined-100 visual construction; provisional configurable assembly dimensions; other styles planned; no finished trains'
        startup = 'welcome; no automatic document'
        sample_projects = $sampleCount
        templates = $templateCount
    }
    $distribution | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $destination 'distribution.json') -Encoding utf8
    # Launch the real packaged PE through its noninteractive entry points only.
    # Clear overrides and run outside the checkout to prove adjacent assets work.
    $env:QX_ASSETS_ROOT = $null
    Push-Location $destination
    try {
    $public = Join-Path $destination $identity.windows_executable
    $legacy = Join-Path $destination $identity.development_executable
    $packagedIconChecks = @(
        & $iconVerifier -Executable $public -Icon $sourceIcon
        & $iconVerifier -Executable $legacy -Icon $sourceIcon
    )
    $packagedIconChecks | ConvertTo-Json -Depth 4 |
        Set-Content -LiteralPath (Join-Path $destination 'icon-check.json') -Encoding utf8
    & $public --version
    if ($LASTEXITCODE -ne 0) { throw 'Public executable failed to launch' }
    $report = & $public --self-check
    if ($LASTEXITCODE -ne 0) { throw 'Packaged catalog self-check failed' }
    $report | Set-Content -LiteralPath (Join-Path $destination 'library-check.json') -Encoding utf8
    & $legacy --help | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Development CLI failed to launch' }
    $startupText = & $public --startup-check
    if ($LASTEXITCODE -ne 0) { throw 'Public startup check failed' }
    $startup = $startupText | ConvertFrom-Json
    if ($startup.no_arguments -ne 'welcome' -or $startup.page -ne 'welcome' -or $startup.document_open) { throw 'Public no-argument launch must start at Welcome with no document' }
    if ($startup.sample_projects -ne $sampleCount -or $startup.templates -ne $templateCount) { throw 'Packaged project library is incomplete' }
    $startupText | Set-Content -LiteralPath (Join-Path $destination 'startup-check.json') -Encoding utf8
    $developerText = & $legacy --startup-check edit
    if ($LASTEXITCODE -ne 0) { throw 'Developer startup check failed' }
    $developer = $developerText | ConvertFrom-Json
    if ($developer.page -ne 'editor' -or -not $developer.document_open) { throw 'Direct edit workflow is broken' }
    if ($sampleFiles.Count -gt 0) {
        $samplePath = Join-Path $samplesDestination $sampleFiles[0].Name
        foreach ($exe in @($public, $legacy)) {
            $directText = & $exe --startup-check edit $samplePath
            if ($LASTEXITCODE -ne 0) { throw 'Direct project startup check failed' }
            $direct = $directText | ConvertFrom-Json
            if ($direct.page -ne 'editor' -or -not $direct.document_open) { throw 'Specified project did not open' }
            $fileText = & $exe --startup-check $samplePath
            if ($LASTEXITCODE -ne 0 -or ($fileText | ConvertFrom-Json).page -ne 'editor') { throw 'Explicit project-file launch is broken' }
        }
    }
    } finally { Pop-Location }
    # Recheck inputs after copying and executing the checks, before authorizing
    # upload. No cache, build directory or older distribution is modified.
    $after = Get-PackageSource
    if ($after.commit -cne $sourceState.commit -or $after.tree -cne $sourceState.tree) { throw 'Source changed during packaging' }
    $null = Read-WindowsBuildReceipt $BuildReceipt $after
    foreach ($name in @($identity.windows_executable, $identity.development_executable)) {
        if ((Get-FileHash -LiteralPath (Join-Path $destination $name) -Algorithm SHA256).Hash.ToLowerInvariant() -cne $receipt.executable_sha256) { throw "Packaged executable changed: $name" }
    }
    Write-PackageManifest $destination
    $integrity = & (Join-Path $PSScriptRoot 'verify-package.ps1') -Directory $destination
    if ($env:GITHUB_OUTPUT) {
        "directory=$destination" | Add-Content -LiteralPath $env:GITHUB_OUTPUT
        "artifact_name=$(Split-Path -Leaf $destination)" | Add-Content -LiteralPath $env:GITHUB_OUTPUT
    }
    if ($env:GITHUB_STEP_SUMMARY) {
        @"
### Verified Windows development package
- Source commit: $($package.source_commit)
- Workflow checkout: $($package.workflow_sha) (PR runs package the tested merge commit)
- Artifact: $(Split-Path -Leaf $destination)
- Uncompressed bytes: $($integrity.bytes)
- Files verified: $($integrity.files)
- SHA256SUMS SHA-256: $($integrity.manifest_sha256)
- One workspace release compilation; packaging reused its hashed, provenance-bound executable.
"@ | Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY
    }
    Write-Output "Portable distribution verified: $destination"
} finally {
    $env:QX_ASSETS_ROOT = $oldAssetsRoot
    Pop-Location
}
