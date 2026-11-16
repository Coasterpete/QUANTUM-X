# R0.4 — hosted software GPU feasibility experiment

This is the non-blocking Tier 2 experiment from
[production readiness §7.1 and R0](roadmaps/production-readiness-v1.md#71-ci-feasibility-and-test-tiers).
It neither replaces hardware validation nor changes UI, physics, schemas or
normal adapter preferences. R0.5 and R1 are outside this pass.

## Procedure

The separate [workflow](../.github/workflows/software-gpu-experiment.yml) runs two
independent trials on standard `windows-2025` hosted runners. It runs on relevant
pull requests and by manual workflow dispatch. Each job has `continue-on-error:
true`, no other job depends on it, and it must remain outside branch protection
requirements. Tier 1's CPU, replay and package workflow is unchanged.

The workflow restores compilation caches without saving them, builds the existing
debug CLI, and copies that executable into a temporary directory. WARP is never
installed into the package, system directories or cached build tree. It downloads
Microsoft.Direct3D.WARP **1.0.20** from the versioned NuGet endpoint, checks the
pinned archive SHA-256 and Microsoft's DLL signature, and places
`build/native/bin/x64/d3d10warp.dll` beside the temporary executable. The
installation JSON records source, hashes, file version and signer. There is no
system-wide installation and no added Cargo dependency.

This follows [wgpu's pinned WARP installer](https://github.com/gfx-rs/wgpu/blob/a1d029a731855a6c8d49b717b0601ff3e2f05817/xtask/src/install_warp.rs).
Microsoft documents WARP's
[DXGI enumeration and Direct3D 12 support](https://learn.microsoft.com/en-us/windows/win32/direct3darticles/directx-warp).

`QX_SOFTWARE_GPU=warp` restricts the instance to DX12, requests a fallback CPU
adapter, retains the caller's surface compatibility requirement, and rejects
anything other than `Microsoft Basic Render Driver` / `Dx12` / `Cpu`. Unknown,
empty or misspelled values fail explicitly. Unset means the original backend and
power preferences; no other wgpu environment overrides are newly enabled.
The shared helper covers all four existing adapter creation sites (native,
offscreen, recording and material probes). Only the paths below are exercised
in this experiment; routing an override is not proof a path ran.

On override requests, stderr records the selected adapter as
`QX_SOFTWARE_GPU_ADAPTER {JSON}` before device creation, including name, backend,
device type, driver and driver information. Selection failures include the
surface requirement, request error and enumerated DX12 candidates. Existing
stdout reports remain intact. Ordinary launches emit no new diagnostics.

## What the runner executes

| Path | Existing entry point | Evidence required |
| --- | --- | --- |
| Welcome offscreen | `render-welcome welcome.png` | WARP identity, exit 0, PNG readback, capture metadata, nonzero visible pixels |
| Editor offscreen | `render-editor examples/foundation.qx.json editor.png track 1280 800` | Same, using actual editor controls and synthetic reference track |
| Welcome native | `--gui-smoke-test welcome` | Hidden HWND, compatible surface/device, five presented frames, cleanup, successful JSON |
| Editor native | `--gui-smoke-test edit` | Same, using the existing blank development editor route |
| Existing Simulator mode | `--gui-smoke-test simulator` | Existing development fixture and Editor → Simulator → Editor script; presented frames, at least 48 physics ticks, Pause/Reset, original state restored |

Offscreen rendering uses GPU textures/readback without any native window or
swapchain. Native smokes use the existing hidden-window winit/wgpu surface and
call `present`; this is evidence of those calls and the existing smoke assertions,
not evidence of visible desktop composition or visual correctness. Simulator is
the **current same-window mode**, not the future R3 separate window. No timestep,
equation, tolerance, fixture or native smoke deadline is changed.

[verify-software-gpu.ps1](../tools/verify-software-gpu.ps1) runs every case even
when earlier cases fail. Each offscreen process has a 120 s outer limit; native
processes have a 45 s limit in addition to their unchanged internal 20 s soft /
30 s watchdog deadlines. Process-tree termination bounds hangs. Both streams,
exit codes, timeouts, adapter diagnostics and native reports are retained; a
partial summary is saved after each case. The job is limited to 30 minutes;
artifacts retain logs, reports and PNGs for **three days**. Installation/build
failures are visible in Actions logs even when no render summary exists.

The evidence validator has CPU-only negative tests for hardware/wrong-backend
adapters, absent reports/captures, zero presented frames, missing cleanup, and
incomplete Simulator transitions. It never launches the application. No software
GPU dependency is added to the default test suite.

## Reproduction and interpretation

Open a PR with the rendering/workflow changes to trigger hosted trials. Once this
workflow exists on the default branch, it can also be dispatched from the Actions
page for a selected ref. No local rebuild or desktop control is required from
the owner. An already-built executable on a disposable Windows test environment
can use the same versioned installation step, then:

```powershell
$env:QX_SOFTWARE_GPU = 'warp'
pwsh -File tools/test-software-gpu-evidence.ps1
pwsh -File tools/verify-software-gpu.ps1 -Executable C:/scratch/r04-app/quantum-x.exe -OutputDirectory C:/scratch/new-evidence
```

Inspect each job's step outcome and `summary.json`; a green non-blocking workflow
alone **does not mean the experiment passed**. A fail is never converted to an
offscreen-only success. The durable, small result record and verdict will be
linked here after actual hosted runs. Mixed repeat outcomes mean **Flaky**;
consistent failures mean **Not feasible in the tested environment**, scoped to
the failing paths. Successful trials mean **Works** only for the executed paths,
with the sample size stated. None meets the provisional 20-success promotion
criterion by itself.

Mesa Vulkan is the alternative, not an implicit fallback that could disguise a
WARP failure. [wgpu's Mesa procedure](https://github.com/gfx-rs/wgpu/blob/a1d029a731855a6c8d49b717b0601ff3e2f05817/.github/actions/install-mesa/action.yml)
installs Windows lavapipe and selects its ICD with `VK_DRIVER_FILES`. Investigate
that separately if WARP cannot exercise the required paths; do not silently
switch backends and attribute Vulkan results to WARP.

Unverified by this experiment: visible-window visual quality, timing/FPS, DPI,
resize/minimize/device recovery, hardware GPU drivers, full-ride dynamics,
recording/material-probe entry points and a future separate Simulator window.
All interactive owner-hardware checks remain Tier 3; no active desktop control
is used. No new product UI or UX decisions are introduced.
