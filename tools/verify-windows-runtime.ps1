# Inspect PE imports as data. Static CRT packages must not need a separately
# installed Visual C++ redistributable. Windows 10/11 system DLLs remain external.
param([Parameter(Mandatory = $true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$bytes = [IO.File]::ReadAllBytes($Executable)
function Read-U16([int]$Offset) { [BitConverter]::ToUInt16($bytes, $Offset) }
function Read-U32([int]$Offset) { [BitConverter]::ToUInt32($bytes, $Offset) }
if ($bytes.Length -lt 64 -or (Read-U16 0) -ne 0x5a4d) { throw 'Missing DOS/PE header' }
$pe = [int](Read-U32 60)
if ((Read-U32 $pe) -ne 0x4550 -or (Read-U16 ($pe + 4)) -ne 0x8664) { throw 'Expected an x64 Windows PE executable' }
$sectionCount = Read-U16 ($pe + 6)
$optional = $pe + 24
if ((Read-U16 $optional) -ne 0x20b) { throw 'Expected PE32+ optional header' }
$sections = $optional + (Read-U16 ($pe + 20))
function Convert-Rva([uint32]$Rva) {
    for ($i = 0; $i -lt $sectionCount; $i++) {
        $section = $sections + 40 * $i
        $start = Read-U32 ($section + 12)
        $size = Read-U32 ($section + 16)
        if ($Rva -ge $start -and $Rva -lt ([long]$start + $size)) {
            return [int]((Read-U32 ($section + 20)) + $Rva - $start)
        }
    }
    throw "Unmapped PE import RVA: $Rva"
}
$importRva = Read-U32 ($optional + 120)
if ($importRva -eq 0) { throw 'PE import table is missing' }
$descriptor = Convert-Rva $importRva
$imports = @()
for ($i = 0; $i -lt 4096; $i++) {
    $nameRva = Read-U32 ($descriptor + 12)
    if ($nameRva -eq 0) { break }
    $offset = Convert-Rva $nameRva
    $end = $offset
    while ($end -lt $bytes.Length -and $end -lt $offset + 256 -and $bytes[$end] -ne 0) { $end++ }
    if ($end -ge $bytes.Length -or $end -eq $offset + 256) { throw 'Invalid PE import name' }
    $name = [Text.Encoding]::ASCII.GetString($bytes, $offset, $end - $offset)
    if ($name -match '^(vcruntime|msvcp|msvcr|concrt).*\.dll$') { throw "Executable requires an external Visual C++ runtime: $name" }
    $imports += $name
    $descriptor += 20
}
if ($i -eq 4096) { throw 'Unterminated PE import table' }
[pscustomobject]@{ architecture = 'x64'; static_crt_verified = $true; imports = $imports }
