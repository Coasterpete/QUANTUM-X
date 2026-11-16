# Evidence validation only: safe to dot-source without creating a GPU or window.
function Assert-SoftwareGpuEvidence {
    param($Adapter, [string]$Kind, [string]$Page, $Report, [bool]$CaptureValid)
    if (-not $Adapter -or $Adapter.accepted -ne $true -or $Adapter.requested -cne 'warp' -or
        $Adapter.backend -cne 'Dx12' -or $Adapter.device_type -cne 'Cpu' -or
        $Adapter.name -cnotlike '*Microsoft Basic Render Driver*') {
        throw 'Missing or unexpected WARP adapter diagnostics'
    }
    if ($Kind -eq 'offscreen') {
        if (-not $CaptureValid) { throw 'Missing PNG readback or capture metadata' }
        return
    }
    if ($Kind -ne 'native') { throw 'Unknown rendering evidence kind' }
    if (-not $Report -or $Report.ok -ne $true -or $Report.hidden_window -ne $true -or
        $Report.page -cne $Page -or $Report.frames_requested -lt 1 -or
        $Report.frames_presented -ne $Report.frames_requested -or
        $Report.resources_released -ne $true -or $Report.exit.reason -cne 'diagnostic_complete' -or
        $Report.exit.window_created -ne $true -or
        $Report.adapter -cnotlike '*Microsoft Basic Render Driver*Dx12*') {
        throw 'Native report did not prove window creation, presentation and cleanup'
    }
    if ($Page -eq 'simulator' -and
        ($Report.script.name -cne 'simulator_round_trip' -or $Report.script.step -cne 'Done' -or
         $Report.script.simulator_frames_drawn -lt 1 -or $Report.script.paused_ticks -lt 48 -or
         $Report.script.reset_matches_initial -ne $true -or $Report.script.editor_restored -ne $true)) {
        throw 'Simulator round trip was incomplete'
    }
}
