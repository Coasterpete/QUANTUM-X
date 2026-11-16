# Synthetic reports test false-positive rejection; no application/GPU is launched.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'software-gpu-evidence.ps1')
$adapter = [pscustomobject]@{
    accepted = $true; requested = 'warp'; backend = 'Dx12'; device_type = 'Cpu'
    name = 'Microsoft Basic Render Driver'
}
$report = [pscustomobject]@{
    ok = $true; hidden_window = $true; page = 'welcome'
    frames_requested = 5; frames_presented = 5; resources_released = $true
    adapter = 'AdapterInfo { name: Microsoft Basic Render Driver, backend: Dx12 }'
    exit = [pscustomobject]@{ reason = 'diagnostic_complete'; window_created = $true; frames_presented = 5 }
    script = [pscustomobject]@{
        name = 'simulator_round_trip'; step = 'Done'; simulator_frames_drawn = 6
        paused_ticks = 48; reset_matches_initial = $true; editor_restored = $true
    }
}
$checks = 0
function Assert-Rejected([scriptblock]$Action) {
    $rejected = $false
    try { & $Action } catch { $rejected = $true }
    if (-not $rejected) { throw 'Invalid rendering evidence was accepted' }
    $script:checks++
}
Assert-SoftwareGpuEvidence $adapter native welcome $report $false
$checks++
Assert-SoftwareGpuEvidence $adapter offscreen welcome $null $true
$checks++
Assert-Rejected { Assert-SoftwareGpuEvidence $null native welcome $report $false }
$adapter.device_type = 'IntegratedGpu'
Assert-Rejected { Assert-SoftwareGpuEvidence $adapter native welcome $report $false }
$adapter.device_type = 'Cpu'
$adapter.backend = 'Vulkan'
Assert-Rejected { Assert-SoftwareGpuEvidence $adapter native welcome $report $false }
$adapter.backend = 'Dx12'
Assert-Rejected { Assert-SoftwareGpuEvidence $adapter offscreen editor $null $false }
Assert-Rejected { Assert-SoftwareGpuEvidence $adapter native welcome $null $false }
$report.frames_presented = 0
Assert-Rejected { Assert-SoftwareGpuEvidence $adapter native welcome $report $false }
$report.frames_presented = 5
$report.exit.frames_presented = 4
Assert-Rejected { Assert-SoftwareGpuEvidence $adapter native welcome $report $false }
$report.exit.frames_presented = 5
$report.frames_presented = 4
Assert-Rejected { Assert-SoftwareGpuEvidence $adapter native welcome $report $false }
$report.frames_presented = 5
$report.resources_released = $false
Assert-Rejected { Assert-SoftwareGpuEvidence $adapter native welcome $report $false }
$report.resources_released = $true
$report.page = 'simulator'
# Real scripted native runs legitimately present more than the minimum five.
$report.frames_presented = 12
$report.exit.frames_presented = 12
Assert-SoftwareGpuEvidence $adapter native simulator $report $false
$checks++
$report.script.editor_restored = $false
Assert-Rejected { Assert-SoftwareGpuEvidence $adapter native simulator $report $false }
$report.script.editor_restored = $true
$report.script.simulator_frames_drawn = 0
Assert-Rejected { Assert-SoftwareGpuEvidence $adapter native simulator $report $false }
"Software GPU evidence validation: $checks checks passed (no GPU/window)"
