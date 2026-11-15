# Shared by the single-build producer and the packaging consumer. No compilation.
$ErrorActionPreference = 'Stop'

function Get-PinnedToolchain {
    $pin = [regex]::Match((Get-Content -LiteralPath (Join-Path $PSScriptRoot '../rust-toolchain.toml') -Raw), '(?m)^channel\s*=\s*"([^"]+)"')
    if (-not $pin.Success) { throw 'Cannot read the pinned Rust toolchain' }
    $pin.Groups[1].Value
}

function Get-PackageSource {
    $commit = & git rev-parse HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify source commit' }
    $tree = & git rev-parse 'HEAD^{tree}'
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify source tree' }
    $changes = & git status --porcelain --untracked-files=no
    if ($LASTEXITCODE -ne 0 -or $changes) { throw 'Packaging requires a clean tracked source tree; commit changes first (untracked logs are preserved)' }
    # Untracked source/assets could otherwise enter a build without belonging to
    # the recorded commit. Unrelated local files such as startup-log.txt are OK.
    $untracked = & git ls-files --others --exclude-standard -- apps crates assets examples tools product.json Cargo.toml Cargo.lock rust-toolchain.toml
    if ($LASTEXITCODE -ne 0 -or $untracked) { throw 'Untracked build/package inputs must be committed before packaging' }
    [pscustomobject]@{ commit = "$commit"; tree = "$tree" }
}

function Read-WindowsBuildReceipt([string]$Path, $Source) {
    $receipt = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    $build = $receipt.build_info
    if ($receipt.schema_version -ne 1 -or -not $build.verified_build -or
        $build.source_commit -cne $Source.commit -or $build.source_tree -cne $Source.tree -or
        $build.profile -ne 'release' -or $build.target -ne 'x86_64-pc-windows-msvc' -or
        $build.toolchain -cne (Get-PinnedToolchain) -or
        $build.build_id -notmatch '^[0-9a-f]{32}$' -or $receipt.executable_sha256 -notmatch '^[0-9a-f]{64}$') {
        throw 'Build receipt does not match this clean source commit or the supported Windows release target; run tools/build-windows.ps1'
    }
    if (-not [IO.Path]::IsPathRooted($receipt.executable) -or
        [IO.Path]::GetFileName($receipt.executable) -cne 'quantum-x.exe' -or
        -not (Test-Path -LiteralPath $receipt.executable -PathType Leaf)) { throw 'Build receipt executable is missing or invalid' }
    $hash = (Get-FileHash -LiteralPath $receipt.executable -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($hash -cne $receipt.executable_sha256) { throw 'Executable hash does not match build receipt' }
    $receipt
}

function Write-PackageManifest([string]$Directory) {
    $root = (Get-Item -LiteralPath $Directory).FullName
    [string[]]$paths = @(Get-ChildItem -LiteralPath $root -Recurse -File -Force | ForEach-Object {
        $_.FullName.Substring($root.Length + 1).Replace('\', '/')
    } | Where-Object { $_ -cne 'SHA256SUMS' })
    [Array]::Sort($paths, [StringComparer]::Ordinal)
    $lines = foreach ($path in $paths) {
        $hash = (Get-FileHash -LiteralPath (Join-Path $root $path) -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $path"
    }
    [IO.File]::WriteAllText((Join-Path $root 'SHA256SUMS'), (($lines -join "`n") + "`n"), [Text.UTF8Encoding]::new($false))
}
