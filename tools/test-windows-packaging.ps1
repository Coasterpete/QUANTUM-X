# Lightweight regression tests: tiny synthetic files, no Cargo, PE or GPU build.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-build-common.ps1')
$verify = Join-Path $PSScriptRoot 'verify-package.ps1'
$scratch = Join-Path ([IO.Path]::GetTempPath()) ("qx-packaging-tests-{0}" -f [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scratch | Out-Null
$checks = 0
function Assert-True($Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
    $script:checks++
}
function Assert-Fails([scriptblock]$Action, [string]$Expected) {
    $failure = $null
    try { $null = & $Action } catch { $failure = $_.Exception.Message }
    Assert-True ($failure -and $failure -like "*$Expected*") "Expected failure containing '$Expected'; got '$failure'"
}
function Write-TestFile([string]$Path, [string]$Text) {
    [IO.File]::WriteAllText($Path, $Text, [Text.UTF8Encoding]::new($false))
}
try {
    $fixture = Join-Path $scratch 'manifest'
    New-Item -ItemType Directory -Path (Join-Path $fixture 'assets') -Force | Out-Null
    Write-TestFile (Join-Path $fixture 'z file.txt') 'last'
    Write-TestFile (Join-Path $fixture 'assets/test.txt') 'asset'
    Write-TestFile (Join-Path $fixture 'A.txt') 'first'
    Write-PackageManifest $fixture
    $manifestPath = Join-Path $fixture 'SHA256SUMS'
    $manifest = [IO.File]::ReadAllText($manifestPath)
    $result = & $verify -Directory $fixture
    Assert-True ($result.verified -and $result.files -eq 3) 'Valid package failed'
    Assert-True (-not $manifest.Contains("`r") -and [IO.File]::ReadAllBytes($manifestPath)[0] -ne 239) 'Manifest must use LF without BOM'
    Assert-True ($manifest.Split("`n")[0].EndsWith('  A.txt')) 'Manifest must sort paths ordinally'
    Write-PackageManifest $fixture
    Assert-True ($manifest -ceq [IO.File]::ReadAllText($manifestPath)) 'Repeated manifest generation changed bytes'
    Write-TestFile (Join-Path $fixture 'A.txt') 'modified'
    Assert-Fails { & $verify -Directory $fixture } 'SHA-256 mismatch'
    Write-TestFile (Join-Path $fixture 'A.txt') 'first'
    Move-Item -LiteralPath (Join-Path $fixture 'assets/test.txt') -Destination (Join-Path $scratch 'held.txt')
    Assert-Fails { & $verify -Directory $fixture } 'Missing package file'
    Move-Item -LiteralPath (Join-Path $scratch 'held.txt') -Destination (Join-Path $fixture 'assets/test.txt')
    Write-TestFile (Join-Path $fixture 'extra.txt') 'unlisted'
    Assert-Fails { & $verify -Directory $fixture } 'Unlisted package file'
    Move-Item -LiteralPath (Join-Path $fixture 'extra.txt') -Destination (Join-Path $scratch 'extra.txt')
    $hash = '0' * 64
    foreach ($badPath in @('../outside.txt', '/absolute.txt', 'assets//test.txt', 'assets/./test.txt', 'C:/absolute.txt', 'assets\test.txt', 'SHA256SUMS')) {
        Write-TestFile $manifestPath "$hash  $badPath`n"
        Assert-Fails { & $verify -Directory $fixture } 'Unsafe or duplicate'
    }
    Write-TestFile $manifestPath ($manifest + $manifest.Split("`n")[0] + "`n")
    Assert-Fails { & $verify -Directory $fixture } 'Unsafe or duplicate'
    $lines = $manifest.TrimEnd("`n").Split("`n")
    [Array]::Reverse($lines)
    Write-TestFile $manifestPath (($lines -join "`n") + "`n")
    Assert-Fails { & $verify -Directory $fixture } 'deterministic ordinal order'
    Write-TestFile $manifestPath ''
    Assert-Fails { & $verify -Directory $fixture } 'Empty package manifest'
    Write-TestFile $manifestPath 'not a manifest'
    Assert-Fails { & $verify -Directory $fixture } 'Invalid manifest entry'

    # These bytes are never executed. A valid receipt only authorizes the later
    # embedded-provenance, Windows-resource, catalog and startup checks.
    $exe = Join-Path $scratch 'quantum-x.exe'
    Write-TestFile $exe 'synthetic executable bytes'
    $source = [pscustomobject]@{ commit = 'a' * 40; tree = 'b' * 40 }
    $receiptPath = Join-Path $scratch 'receipt.json'
    $receipt = [ordered]@{
        schema_version = 1
        executable = $exe
        executable_sha256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
        build_info = [ordered]@{
            verified_build = $true
            source_commit = $source.commit
            source_tree = $source.tree
            build_id = 'c' * 32
            profile = 'release'
            target = 'x86_64-pc-windows-msvc'
            toolchain = Get-PinnedToolchain
        }
    }
    function Save-Receipt { $receipt | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $receiptPath -Encoding utf8 }
    Save-Receipt
    $null = Read-WindowsBuildReceipt $receiptPath $source
    $checks++
    foreach ($field in @('source_commit', 'source_tree', 'profile', 'target', 'toolchain', 'build_id', 'verified_build')) {
        $saved = $receipt.build_info[$field]
        $receipt.build_info[$field] = if ($field -eq 'verified_build') { $false } else { 'outdated' }
        Save-Receipt
        Assert-Fails { Read-WindowsBuildReceipt $receiptPath $source } 'does not match'
        $receipt.build_info[$field] = $saved
    }
    Save-Receipt
    Write-TestFile $exe 'unrelated replacement executable'
    Assert-Fails { Read-WindowsBuildReceipt $receiptPath $source } 'hash does not match'
    $receipt.executable = Join-Path $scratch 'missing/quantum-x.exe'
    Save-Receipt
    Assert-Fails { Read-WindowsBuildReceipt $receiptPath $source } 'missing or invalid'

    # A minimal synthetic x64 import table exercises runtime inspection without
    # building or running an executable. Offsets follow the PE32+ layout.
    $peBytes = [byte[]]::new(1024)
    function Put-U16([int]$Offset, [uint16]$Value) { [BitConverter]::GetBytes($Value).CopyTo($peBytes, $Offset) }
    function Put-U32([int]$Offset, [uint32]$Value) { [BitConverter]::GetBytes($Value).CopyTo($peBytes, $Offset) }
    Put-U16 0 0x5a4d
    Put-U32 60 128
    Put-U32 128 0x4550
    Put-U16 132 0x8664
    Put-U16 134 1
    Put-U16 148 240
    Put-U16 152 0x20b
    Put-U32 272 0x1000
    Put-U32 404 0x1000
    Put-U32 408 512
    Put-U32 412 512
    Put-U32 524 0x1040
    [Text.Encoding]::ASCII.GetBytes("KERNEL32.dll`0").CopyTo($peBytes, 576)
    [IO.File]::WriteAllBytes($exe, $peBytes)
    $runtimeVerifier = Join-Path $PSScriptRoot 'verify-windows-runtime.ps1'
    $runtime = & $runtimeVerifier -Executable $exe
    Assert-True ($runtime.static_crt_verified -and $runtime.imports -contains 'KERNEL32.dll') 'System DLL import check failed'
    [Text.Encoding]::ASCII.GetBytes("VCRUNTIME140.dll`0").CopyTo($peBytes, 576)
    [IO.File]::WriteAllBytes($exe, $peBytes)
    Assert-Fails { & $runtimeVerifier -Executable $exe } 'external Visual C++ runtime'
    Write-TestFile $exe 'not a PE'
    Assert-Fails { & $runtimeVerifier -Executable $exe } 'Missing DOS/PE header'
    Write-Output "Packaging regression checks passed: $checks (no compilation)"
} finally {
    # Only remove the fresh, uniquely named test directory created above.
    $resolved = [IO.Path]::GetFullPath($scratch)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (-not $resolved.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path -Leaf $resolved) -notlike 'qx-packaging-tests-*') { throw 'Unsafe test cleanup path' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
