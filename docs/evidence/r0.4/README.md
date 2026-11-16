# R0.4 hosted Windows software GPU result

**Verdict: Works**, scoped to the paths below on two independent
`windows-2025` runners. Observed 2026-11-15 (America/New_York; Actions timestamps
are 2026-11-16 UTC). This is feasibility evidence, not promotion to a required
gate or a graphics-quality/performance qualification.

The final [hosted experiment run](https://github.com/Coasterpete/QUANTUM-X/actions/runs/43782066066)
tested PR head `c4ee6c99403ded6518798f7931f3e220cd7387db` via GitHub's synthetic
merge checkout `11878aaae96efea4a3ac14594c9878e9382c32be`. Its base is R0.3 merge
`ee213a1ad6c0069a07cce0de90edebf8301e0743`, whose
[four main-branch CI jobs passed](https://github.com/Coasterpete/QUANTUM-X/actions/runs/43780619760)
before R0.4 started. Later documentation-only commits do not change the tested
rendering code or harness.

## Adapter and execution results

Both runners selected **Microsoft Basic Render Driver / Dx12 / Cpu / driver
1.0.20.0**, using wgpu 30.0.1 and runner image `win25-vs2026`, version
`20261103.298.1`, Windows NT `10.0.26100.0`. The versioned NuGet archive and
Microsoft-signed DLL hashes matched in both installations. Every render process
exited **0**, every outer timeout was **false**, and both actual experiment
steps (not merely their non-blocking parent checks) concluded **success**.

| Rendering path | Trial 1 | Trial 2 | What executed |
| --- | --- | --- | --- |
| Welcome offscreen | Pass | Pass | Actual egui/wgpu output, 1280×800 PNG readback and capture metadata |
| Editor offscreen | Pass | Pass | Actual Editor plus existing foundation fixture, 1280×800 PNG readback |
| Welcome native | 5 presented frames | 5 presented frames | Hidden native HWND, surface/device, swapchain presentation, resource cleanup |
| Editor native | 5 presented frames | 5 presented frames | Existing blank-editor smoke route and native surface |
| Simulator native | 11 frames; 72 paused ticks | 12 frames; 64 paused ticks | Existing same-window Editor → Simulator → Editor script; Play/Pause/Reset; editor restored |

Both Simulator scripts finished `Done`, drew eight/nine Simulator frames
respectively, matched initial state after Reset and restored the editor. Tick/frame counts can differ
because the existing script waits for asynchronous snapshots and requires **at
least 48 fixed ticks** and **at least five presented frames**. The solver remains
at 1/240 s; these counts are not a physics determinism comparator or timing gate.

The small [summary.json](summary.json) preserves the adapter/installation
diagnostics, per-path results, exact source and artifact digests after artifacts
expire. Raw output, native reports, capture metadata and PNGs are available for
**three days** in
[trial 1](https://github.com/Coasterpete/QUANTUM-X/actions/runs/43782066066/artifacts/14188284911)
and [trial 2](https://github.com/Coasterpete/QUANTUM-X/actions/runs/43782066066/artifacts/14187949156).
Run/job logs remain linked from the workflow. Download requires GitHub access.

## Initial failure and correction

The [initial run](https://github.com/Coasterpete/QUANTUM-X/actions/runs/43781841008)
had **failed experiment steps in both trials**, although its non-blocking
workflow appears green. All five application processes in each trial exited 0;
each native app report was a pass. The new evidence wrapper wrongly required
exactly five frames and rejected both successful 12-frame Simulator round trips
(64 paused ticks). This was a deterministic wrapper defect, not an observed
software-GPU/windowing flake.

The wrapper was corrected to match the existing `Smoke::complete` minimum-frame
contract and to require agreement between the report and exit frame counts.
Negative tests still reject insufficient/zero frames, inconsistent counts,
missing cleanup and incomplete Simulator transitions. The existing renderer,
native smoke assertions/deadlines, physics, golden fixtures and tolerances were
unchanged. The verdict above uses the fresh two-trial run after that correction;
the earlier failure is retained rather than relabeled as a passing workflow.

## Recommendation and limits

**Continue hosted WARP testing as a separate, non-blocking Tier 2 experiment.**
It exercises useful application paths on standard hosted Windows runners without
paid GPU runners, self-hosted hardware or owner rebuilds. Keep it outside branch
protection and collect repeated results before considering the roadmap's
provisional 20-success promotion criterion and owner acceptance. Two final trials
do not establish a flake rate or cover future runner image changes.

WARP exercised all requested paths, so installing Mesa Vulkan was unnecessary
in this pass. Its upstream procedure is documented as an alternative; **Mesa is
unverified**, and the override deliberately cannot silently switch to it.

Offscreen success proves GPU texture rendering/readback. Native results prove
the existing hidden-window surface/present and cleanup paths; they do not prove
visible Windows desktop composition or visual quality. No hosted windowing
failure was observed in these trials, but the environment is not a substitute
for Tier 3 hardware testing. Visible-window FPS, hardware drivers, DPI/focus,
resize/minimize, multi-monitor/device recovery, full-ride physics qualification,
recording/material-probe entry points and the future separate Simulator window
remain unverified. No active desktop control or manual owner validation occurred.

No UI/UX behavior was added or redesigned, no developer UI was introduced, and
this experiment requires no new human UX decision. See the
[procedure and implementation](../../software-gpu-r0.4.md) for reproduction,
timeouts, diagnostics and isolation from the existing CPU/package gates.
