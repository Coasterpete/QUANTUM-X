# Produces exactly one workspace release build and a receipt for package-windows.
param([string]$ReceiptPath)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-build-common.ps1')
$workspace = Split-Path -Parent $PSScriptRoot
Push-Location $workspace
$oldProvenance = $env:QX_BUILD_PROVENANCE
$oldRustflags = $env:CARGO_ENCODED_RUSTFLAGS
try {
    $source = Get-PackageSource
    $metadataText = & cargo metadata --locked --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
    $metadata = $metadataText | ConvertFrom-Json
    $package = $metadata.packages | Where-Object name -eq 'quantum-x'
    if (-not $package) { throw 'Application package quantum-x not found' }
    $rustc = & rustc -Vv
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify Rust toolchain' }
    $cargo = & cargo -V
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify Cargo version' }
    if ($rustc -notcontains 'host: x86_64-pc-windows-msvc') { throw 'Use the native x64 Windows MSVC toolchain' }
    $toolchain = Get-PinnedToolchain
    if ($rustc -notcontains "release: $toolchain") { throw "Use the pinned Rust $toolchain toolchain" }
    $build = [ordered]@{
        schema_version = 1
        verified_build = $true
        build_id = [guid]::NewGuid().ToString('N')
        source_commit = $source.commit
        source_tree = $source.tree
        version = $package.version
        profile = 'release'
        target = 'x86_64-pc-windows-msvc'
        toolchain = $toolchain
        rustflags = @('-C', 'target-feature=+crt-static')
        rustc = $rustc -join "`n"
        cargo = "$cargo"
        built_at_utc = [DateTime]::UtcNow.ToString('o')
        repository = 'https://github.com/Coasterpete/QUANTUM-X'
        workflow_sha = $env:GITHUB_SHA
        run_id = $env:GITHUB_RUN_ID
        run_attempt = $env:GITHUB_RUN_ATTEMPT
        runner_os = $env:RUNNER_OS
        runner_image = $env:ImageOS
        runner_image_version = $env:ImageVersion
    }
    $env:QX_BUILD_PROVENANCE = $build | ConvertTo-Json -Depth 5 -Compress
    # Bundle the MSVC C runtime into the executable. An explicit target keeps
    # these target flags away from host build scripts and procedural macros.
    $env:CARGO_ENCODED_RUSTFLAGS = '-C' + [char]31 + 'target-feature=+crt-static'
    # Capture Cargo's actual artifact rather than guessing target/release. The
    # per-build ID forces fresh embedded provenance even after restoring a cache.
    $messages = & cargo build --workspace --release --locked --target x86_64-pc-windows-msvc --message-format=json-render-diagnostics
    if ($LASTEXITCODE -ne 0) { throw 'Workspace release build failed' }
    $artifact = $messages | ForEach-Object { $_ | ConvertFrom-Json } | Where-Object {
        $_.reason -eq 'compiler-artifact' -and $_.package_id -eq $package.id -and
        $_.target.name -eq 'quantum-x' -and $_.target.kind -contains 'bin' -and $_.executable
    } | Select-Object -Last 1
    if (-not $artifact -or -not $artifact.executable.EndsWith('.exe')) { throw 'Cargo did not report a native Windows executable' }
    $after = Get-PackageSource
    if ($after.commit -cne $source.commit -or $after.tree -cne $source.tree) { throw 'Source changed during build' }
    if (-not $ReceiptPath) { $ReceiptPath = Join-Path $metadata.target_directory 'windows-release-build.json' }
    $receipt = [ordered]@{
        schema_version = 1
        executable = $artifact.executable
        executable_sha256 = (Get-FileHash -LiteralPath $artifact.executable -Algorithm SHA256).Hash.ToLowerInvariant()
        target_directory = $metadata.target_directory
        build_info = $build
    }
    $receipt | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $ReceiptPath -Encoding utf8
    Write-Output "Windows build receipt: $ReceiptPath"
} finally {
    $env:QX_BUILD_PROVENANCE = $oldProvenance
    $env:CARGO_ENCODED_RUSTFLAGS = $oldRustflags
    Pop-Location
}
