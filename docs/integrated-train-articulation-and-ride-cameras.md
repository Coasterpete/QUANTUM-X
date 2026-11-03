# Integrated train articulation, ride cameras and recording

Updated 2026-11-01. This is the existing integrated implementation, including the recording-quality correction. No new milestone, UI redesign, physics rewrite, project schema change, dependency, artwork or publication was introduced.

## What changed

The earlier delivery was explicitly hardcoded to 24 frames at **12 FPS, 960x640**. FFmpeg received that same input rate. This was a poor capture setting for assessing motion, not an encoder limit or a native performance measurement.

- The default ride-camera scenario now records at **60 FPS, 1440x960**. A persistent viewport recorder retains the device, PBR pipelines, meshes, materials and readback buffers. Every output image is newly rasterized from independent 1/240-second physics and the actual presentation code. FFmpeg uses passthrough timestamps; no duplicated frames, optical flow or image synthesis is used.
- Native animation no longer adds a 16 ms post-render wait. Active playback requests continuous repaint and remains paced by the existing AutoVsync swapchain. Idle/worker polling remains bounded.
- Committed snapshots use fixed-step scheduled timestamps rather than variable solver completion timestamps. An eight-pose history provides a **two-tick / 8.333 ms presentation buffer**. Each UI frame latches one timestamp and one pose sample shared by camera and vehicle instances. Paused/single-step views show committed state; Reset clears the history. Exhausted history holds the last committed pose and reports starvation, without extrapolation. Catch-up limits remain observable.
- Opt-in CLI diagnostics separately measure native frame intervals, UI work, pose/instance updates, camera math, swapchain waits, worker steps and capture GPU/readback/PNG costs. They create a hidden, nonactivating native window and never control the active desktop or save user preferences.

## Measurements

Windows, NVIDIA RTX 4070, Vulkan, driver 617.42, monitor reporting **100 Hz**, AutoVsync. Each native run lasted eight seconds and exercised Play, Pause, Reset and Play again. The app window was 1440x960, with its existing approximately 742x755 central 3D pane. These are real winit/wgpu native-surface draws; visible scanout and subjective desktop smoothness were not manually verified.

Steady playback measurements below are milliseconds; warm-up and control-transition intervals are excluded.

| Native presentation target | Frame interval median / p95 | Pose + instance update median | Camera math median | Worker step mean / max |
| --- | --- | --- | --- | --- |
| Legacy post-render wait | 17.510 / 18.188 | 0.0124 | 0.0013 | 0.545 / 1.255 |
| 30 Hz | 33.492 / 33.564 | 0.0095 | 0.0010 | 0.344 / 1.214 |
| 60 Hz | 16.581 / 17.055 | 0.0135 | 0.0014 | 0.632 / 1.712 |
| 144 Hz requested | 9.996 / 10.443 | 0.0077 | 0.0007 | 0.327 / 0.616 |
| 144 Hz requested, rotating passenger / original GLBs | 10.006 / 11.463 | 0.0224 | 0.0008 | 1.266 / 4.559 |

The 144 Hz runs reached about **100 Hz**, constrained by the vsynced surface on the reported 100 Hz monitor; median acquisition wait was about 9.3 ms. Logical presentation at 144 Hz is separately covered by timing tests, but native 144 FPS was not demonstrated.

There were **zero starved presentation frames and zero discarded catch-up batches** in these measured runs. The rotating-passenger run had one worker step above the 4.167 ms budget. The 60 Hz run had two long native intervals, **26.31 and 33.66 ms**, associated with **25.10 and 31.99 ms swapchain-acquisition waits**. These remain native rendering stalls; the worker and pose clock continued, and the buffered camera/vehicle sample stayed coherent. This is not a claim of perfectly uniform visible playback on all hardware.

Camera eye and orientation basis matched the independently recalculated component transform within 1e-8 tolerance, including GLB-local corrections for the separately rotating passenger mesh. Steady pose-time increments matched sample-clock increments within 0.001 ms. Pause held both camera and pose exactly; Reset restored the original state. Tests also cross inversions and quaternion sign changes without camera flips.

Full-viewport hill capture GPU time at 1440x960 was **1.014 ms median / 2.056 ms p95**. Capture readback was 6.667 ms median and PNG writing 15.037 ms median. Export is intentionally offline: its filesystem/readback cost is not native viewport performance or the movie frame rate.

## Recordings and executable

Paths are relative to `C:/Dev8/QUANTUM-X`:

- Longer POV: `target/ride-quality-60fps-hill/physics-rider-pov-60.mp4` -- **353 unique rendered and decoded frames, 5.883 seconds, 60 FPS, 1440x960**. Uses the existing 90 m numerical hill fixture and existing gray-box mechanism.
- Same-layout comparison with the old delivery: `target/ride-quality-60fps-bank/physics-rider-pov-60.mp4` -- **233 unique rendered and decoded frames, 3.883 seconds, 60 FPS, 1440x960**. Uses the original banked source unchanged.
- Updated Windows app: `target/distribution/QUANTUM Dynamics-articulation-quality-20261101/QUANTUM Dynamics.exe`. Keep its adjacent assets folder.
- Each recording directory includes `inspection.qx.json`, source PNGs, camera/pose data, GPU timings, `evidence.json` and `verification.json`. Native raw traces are under `target/ride-quality-native-{legacy,30,60,144}-final/` and `target/ride-quality-native-passenger-verified/`.

Both recordings stop at the actual open-track boundary. They do not wrap, extend the track, duplicate a final frame or alter the authored layout. Rider POV retains genuine body rotation; it is not horizon-smoothed to hide vehicle behavior. The original 12 FPS artifacts remain historical outputs.

Reproduce with the updated executable:

```powershell
.\target\release\quantum-x.exe ride-recording <project.qx.json> <new-directory>
ffmpeg -framerate 60 -i <new-directory>/pov-frames/%04d.png -c:v libx264 -crf 16 -pix_fmt yuv420p -fps_mode passthrough <new-directory>/physics-rider-pov-60.mp4
python tools/verify-ride-quality.py <new-directory> <native-profile-directory> ...
.\target\release\quantum-x.exe ride-native-profile <project.qx.json> <new-profile-directory> 60
```

For owner-operated visible acceptance, open this exact project, choose Train (Ctrl+2), select Rider POV, expand Experimental physics playback, set initial speed to 2 m/s, then Initialize and exercise Play/Pause/Reset. Repeat on a 144 Hz monitor when available:

```powershell
& '.\target\distribution\QUANTUM Dynamics-articulation-quality-20261101\QUANTUM Dynamics.exe' edit '.\target\ride-quality-60fps-bank\inspection.qx.json'
```

## Verification and retained boundaries

Formatting, workspace check, strict all-target Clippy, **309 workspace tests**, workspace Release build, portable packaging and original icon verification passed. Added tests cover timing at 30/60/144 Hz under variable completion latency, starvation without extrapolation, same-frame pose latching, and continuous inversions/quaternion sign changes. Existing assembly, independent dynamics, endpoint rollback, GLB replacement, undo/redo, project compatibility and save/reopen tests remain passing. FFprobe confirms 60 FPS and exact timestamps; raw-image hashes and FFmpeg decoded-frame hashes confirm every recorded frame is unique. The full six-camera scenario is also exercised through the updated recorder.

Rigid assembly still preserves dimensions and closes supported graph connections to 1e-5 m / 1e-5 rad using bounded chord search and graph projection. Shared, single and multiple running gear remain supported where geometrically compatible. Unsupported arrangements return residual errors; only labeled development diagnostics may show an unclosed seed. The physics equations, gray placeholders, original Blender/GLB assets, PBR construction, owner palette, workspace organization and transient camera controls are retained.

Centerline guides remain experimental bilateral constraints, without complete wheel/rail contact, suspension or manufacturer validation. Large-scene performance, visible desktop scanout and native 144 Hz operation on suitable hardware remain unverified. Existing development controls remain provisional; this correction introduces no new product UX requiring owner approval.

All changes remain local and uncommitted on `codex/m1-track-pbr-materials`. Existing work was preserved. No push, PR, merge or publication occurred.
