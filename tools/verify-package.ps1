# Portable integrity check: Windows PowerShell 5.1+; no Rust/Cargo or modules.
param([string]$Directory = $PSScriptRoot)
$ErrorActionPreference = 'Stop'
$root = (Get-Item -LiteralPath $Directory).FullName
if (-not (Test-Path -LiteralPath (Join-Path $root 'SHA256SUMS') -PathType Leaf)) { throw 'SHA256SUMS is missing' }
$seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$previous = $null
foreach ($line in [IO.File]::ReadAllLines((Join-Path $root 'SHA256SUMS'))) {
    if ($line -cnotmatch '^([0-9a-f]{64})  (.+)$') { throw "Invalid manifest entry: $line" }
    $hash, $relative = $Matches[1], $Matches[2]
    if ($relative.Contains('\') -or $relative.Contains(':') -or
        $relative -match '(^|/)(\.|\.\.|)(/|$)' -or $relative -ieq 'SHA256SUMS' -or
        -not $seen.Add($relative)) { throw "Unsafe or duplicate manifest path: $relative" }
    if ($null -ne $previous -and [StringComparer]::Ordinal.Compare($previous, $relative) -ge 0) { throw 'Manifest is not in deterministic ordinal order' }
    $previous = $relative
    $path = Join-Path $root $relative
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing package file: $relative" }
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $hash) { throw "SHA-256 mismatch: $relative" }
}
if ($seen.Count -eq 0) { throw 'Empty package manifest' }
$files = @(Get-ChildItem -LiteralPath $root -Recurse -Force)
foreach ($item in $files) {
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Package contains a link/reparse point: $($item.FullName)" }
    if (-not $item.PSIsContainer) {
        $relative = $item.FullName.Substring($root.Length + 1).Replace('\', '/')
        if ($relative -cne 'SHA256SUMS' -and -not $seen.Contains($relative)) { throw "Unlisted package file: $relative" }
    }
}
[pscustomobject]@{
    verified = $true
    files = $seen.Count
    bytes = ($files | Where-Object { -not $_.PSIsContainer } | Measure-Object Length -Sum).Sum
    manifest_sha256 = (Get-FileHash -LiteralPath (Join-Path $root 'SHA256SUMS') -Algorithm SHA256).Hash.ToLowerInvariant()
}
