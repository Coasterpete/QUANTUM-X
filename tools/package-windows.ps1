param([string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
Push-Location $workspace
try {
    $identity = Get-Content -LiteralPath (Join-Path $workspace 'product.json') -Raw | ConvertFrom-Json
    $metadataText = & cargo metadata --locked --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
    $metadata = $metadataText | ConvertFrom-Json
    $package = $metadata.packages | Where-Object name -eq 'quantum-x'
    if (-not $package) { throw 'Development package quantum-x not found' }
    # Cargo supplies the exact artifact. Never pick an arbitrary executable or
    # mutate target/release/quantum-x.exe. A cached build still validates freshness.
    $messages = & cargo build --locked --release -p quantum-x --message-format=json-render-diagnostics
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    $artifact = $messages | ForEach-Object { $_ | ConvertFrom-Json } |
        Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'quantum-x' -and $_.executable } |
        Select-Object -Last 1
    if (-not $artifact -or -not $artifact.executable.EndsWith('.exe')) { throw 'A native Windows executable is required' }
    $source = $artifact.executable
    $info = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($source)
    if ($info.ProductName -ne $identity.name) { throw 'Embedded product metadata is incorrect' }
    $iconVerifier = Join-Path $PSScriptRoot 'verify-windows-icon.ps1'
    $sourceIcon = Join-Path $workspace 'assets/icons/quantum-dynamics/QUANTUM_Dynamics.ico'
    # A missing/stale/different icon must fail packaging, including cached builds.
    $iconCheck = & $iconVerifier -Executable $source -Icon $sourceIcon
    if (-not $OutputDirectory) {
        $OutputDirectory = Join-Path $metadata.target_directory "distribution/QUANTUM Dynamics-$($package.version)-windows"
    }
    $destination = [System.IO.Path]::GetFullPath($OutputDirectory)
    if (Test-Path -LiteralPath $destination) { throw "Destination exists; choose a new -OutputDirectory: $destination" }
    New-Item -ItemType Directory -Path $destination | Out-Null
    Copy-Item -LiteralPath $source -Destination (Join-Path $destination $identity.windows_executable)
    Copy-Item -LiteralPath $source -Destination (Join-Path $destination $identity.development_executable)
    Copy-Item -LiteralPath (Join-Path $workspace 'product.json') -Destination $destination
    $assetsDestination = Join-Path $destination 'assets'
    New-Item -ItemType Directory -Path $assetsDestination | Out-Null
    foreach ($folder in @('catalog', 'tracks', 'trains', 'development')) {
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
        if (Test-Path -LiteralPath (Join-Path $workspace $file)) { Copy-Item -LiteralPath (Join-Path $workspace $file) -Destination $notices }
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
Library styles are planned; no production models or engineering validation are supplied.
Run .\quantum-x.exe --self-check for a noninteractive catalog/identity check.
Run '.\QUANTUM Dynamics.exe' --startup-check for a noninteractive startup-state check.
Welcome and Editor follow the owner's colorful, high-contrast Inkscape references.
Setup configures the application; Editor offers New/Open when no project is open.
View > Development tools exposes experimental train tools and diagnostics.
News, support generators and ride operations are unavailable in this Alpha build.
This unsigned portable folder has no installer, updater or file associations.
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
        content = 'development; planned catalog; no production train/track assets'
        startup = 'welcome; no automatic document'
        sample_projects = $sampleCount
        templates = $templateCount
    }
    $distribution | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $destination 'distribution.json') -Encoding utf8
    # Launch the real packaged PE through its noninteractive entry points only.
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
    Write-Output "Portable distribution verified: $destination"
} finally {
    Pop-Location
}
