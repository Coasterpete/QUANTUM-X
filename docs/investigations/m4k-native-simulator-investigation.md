# M4K native Simulator investigation — stationary train and catch-up notices

Date: 2026-11-14 · Checkpoint: `653d6e3` on `claude/m4k-simulator-mode` ·
Machine: owner's Windows 11 laptop, Intel integrated GPU (Vulkan), 240 Hz panel.
No production code, tests, thresholds or project files were changed.

## 1. Which project was simulated

The exact document cannot be recovered: `%LOCALAPPDATA%\QUANTUM-X\recent-v1.json`
does not exist and no `*.qx.json` was saved under the user profile, so the
session was an **unsaved project** (Welcome › New, or `edit` with no file). None
of the bundled samples (`examples/*.qx.json`) contains a saved train, and
Simulator readiness refuses a project without one, so a train was added in the
session. The three-car train matches **Train Workspace › Gray-box train › Add
and associate gray-box train** with its default `Cars 3`
(`Train::development_consist(.., 3, Conventional)`: 11 rigid bodies, 2440 kg).

Defaults that apply unless changed in **Operations › Physical simulation settings**:

| Setting | Default | Source |
| --- | --- | --- |
| Track | one 10 m level straight from a level initial frame | `startup.rs` `Setup::project`, `session::blank_project` |
| Initial velocity | 0 m/s | `PhysicalSettings::default` |
| Gravity | 9.81 m/s² (physics); authored `initial.gravity_mps2` 9.80665 is authoring-only | `operations.rs` |
| Rolling coefficient / Cd×A | 0 / 0 m² | `PhysicalSettings::default` |
| Devices | none | `Operations::default` |
| Seating station | track length × 0.5 unless picked | `simulator.rs` `request_entry` |

The default 10 m track **cannot seat** the three-car train (it fails with
`distance query outside generated track`; 12 m also fails, 20 m succeeds). The
train you saw seated, so your track was at least ~20 m. **Please save the
project and share the path** if you want the exact geometry confirmed.

## 2. What is implemented in operations

| Capability | Status |
| --- | --- |
| Initial velocity (all bodies along local tangent) | implemented |
| Gravity on rigid bodies | implemented |
| Rolling resistance, aerodynamic drag | implemented (default 0) |
| Launch device (constant, legacy keys, or Transition curve; force cap; reverse) | implemented |
| Brake device | implemented |
| Lift / chain / tire drive, station, transport, block system | **not implemented** (no `DeviceKind` exists) |

**Expected behavior on level track with v0 = 0 and no devices: the train stays
stationary.** No force acts along the track. This is correct physics, not a defect.

## 3. Controlled reproductions (production `TrackSimulation::with_operations`)

An isolated scratch harness outside the repository generated tracks with
`Project::generate` and ran the same constructor the playback worker uses, with
the default three-car gray-box train seated at mid-track, for 2400 ticks (10 s).

| Case | Result | Analytic check |
| --- | --- | --- |
| A0 · 10 m level, v0 0 | seating fails: `distance query outside generated track` | train longer than half the track |
| A1 · 80 m level, v0 0 | speed 0.0000 m/s, distance 0.0000 m for 10 s | stationary ✔ |
| B · 80 m level, v0 3 m/s | 3.0000 m/s constant, 30.0000 m in 10 s | no resistance → constant ✔ |
| C · 80 m, 10° downhill, v0 0 | a = +1.7035 m/s²; stops at open end, tick 1615 | g·sin 10° = 1.7035 ✔ |
| D · 80 m level, Launch 1 m/s² | a = +1.0000 m/s², device force 2440.0 N; stops at open end, tick 2108 | m·a = 2440 N ✔ |

The physics responds correctly to every supported input tested.

## 4. Timing measurements

**Raw OS wait granularity.** `mpsc::recv_timeout(4.167 ms)` on this machine:
min 8.1, p50 15.6, p90 16.0, max 16.6 ms. That is the default Windows timer
resolution. The worker asks to wake every 4.17 ms but actually wakes every ~15.6 ms.

**Physics step cost (release, three-car train, warm runs).** Mean 2.0–2.5 ms,
p99 3.0–3.4 ms, spikes 7–17 ms, against a 4.167 ms budget. One cold run
averaged 3.6 ms. Cost does not depend on whether the train moves: the solver
always runs 4 substeps × 32 iterations (`consist::initialize(.., 32, 4)`).
While playing, the worker keeps **50–90 % of one CPU core** busy even for a
stationary train. Snapshot publication costs about 0.006 ms per step and is negligible.

**Copy of the worker loop, no rendering, 10 s wall time:**

| Case | Simulated / wall | Catch-up limit hits (`lag_events`) | Wakes that hit the 8-step cap |
| --- | --- | --- | --- |
| A1 level, stationary | 9.387 s / 10.012 s = 0.938 | 95 | 137 / 417 |
| B level, 3 m/s | 9.521 s / 10.011 s = 0.951 | 67 | 97 / 480 |

**Real application path (`quantum-x ride-native-profile`, hidden native window,
real worker + egui + wgpu, 60 Hz target, v0 forced to 1 m/s by that tool):**

- Simulated/wall ratio: 0.990 in the first play segment, 0.979 after Reset and replay.
- 15 catch-up limit hits in about 5.5 s of playing. `over_budget_steps` 0, step max 3.99 ms, max lateness 18.6 ms.
- **76 % of presented frames were starved** (264 / 347): the viewport held the newest committed pose and waited for the next one.
- Rendering is cheap: frame CPU p50 1.44 ms, UI p50 0.74 ms, upload p50 0.08 ms. Rendering and geometry are not what slows the worker.
- Not measured: the visible window at the 240 Hz panel rate. While playing, the app repaints continuously, so the UI thread runs more often than in this 60 Hz profile.

**Interpretation**

- Ticks keep advancing during playback. The worker is not stalled, and the UI is not showing a stale snapshot.
- Simulation time runs **1–6 % slower than wall-clock time**. Each wake must catch up ~4 steps. Step cost plus timer overshoot sometimes pushes the backlog past 8 steps, and the debt is then discarded (`next = now + dt`). Ticks are never enlarged or skipped.
- The catch-up notice therefore appears even when the train is stationary. It is about scheduling, not motion.

## 5. Window behavior

Simulator Mode is **same-window**. `panels.rs:58` hands the one native window
(one surface, one egui pass) to `simulator_ui::workspace`, which replaces the
Editor composition, and Return to Editor swaps it back. The M4K architecture
doc says "Share window/surface/device/queues". **Gap:** your intended design is
a separate native Simulator window with the Editor kept open. That needs
multi-window support in `qx-rendering` (a second surface, an event loop routing
by `WindowId`, per-window egui contexts), plus a decision on how editing while
simulating interacts with the epoch-retirement rule. Not started here.

## 6. Confirmed defects

1. **Presentation starvation on Windows (timing; confirmed by measurement).**
   `PRESENTATION_DELAY_TICKS = 2` (8.3 ms) is shorter than the real
   worker publish interval (~15.6 ms). About 3 of every 4 frames hold the
   last pose, so moving trains will visibly hitch. The visual effect was not
   manually verified.
2. **Worker falls behind real time (timing; confirmed).** Coarse
   Windows waits plus a step cost of 50–90 % of budget leave too little headroom.
   The 8-step catch-up cap is reached about 3–10 times per second, and simulated
   time runs 1–6 % slow.
3. **Misleading notice wording (minor).** "simulation time is retained, not
   skipped" is accurate about ticks, but it hides the fact that playback runs
   slower than real time.
4. **Unhelpful seating error (minor).** A train longer than half the track
   fails entry with `Simulator could not start: distance query outside
   generated track`. The message does not say the track is too short for
   the train at the chosen station.

Not a defect: a stationary train on level track with v0 = 0 and no devices.

## 7. Recommended next steps, ordered by impact

1. **Owner action, no code:** in Operations › Physical simulation settings,
   set an initial velocity, add a Launch device, or author elevation. These are
   supported today. A lift does not exist yet.
2. **Windows wake precision in the playback worker.** Use a high-resolution
   waitable timer (`CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`), or scoped
   `timeBeginPeriod(1)` while playing, or a short spin for the final
   sub-millisecond, so waits match the 4.17 ms step. This should remove most
   catch-up hits and starvation without changing the 1/240 s step.
3. **Presentation delay tied to measured publish interval** (presentation
   only). For example, hold at least one observed worker wake interval plus a
   margin, so interpolation always has a bracketing pair.
4. **Profile the qx-physics step.** 128 solver sweeps per tick cost ~2 ms for
   11 bodies, before any multi-train or longer-consist scenario. Measure first,
   then consider convergence-based early exit only with accuracy tests at the
   current thresholds. Do not loosen any tolerances.
5. **Clarify the two messages** (catch-up wording; seating failure naming the
   train length vs the space available at the station).
6. **Separate Simulator window:** design and plan it as its own pass after an
   owner UX decision.

## Reproduction

- Harness (scratch, not in the repo): `m4k-diag` crate in the session
  scratchpad. It depends on `qx-physics`, `qx-document`, `qx-geometry` and
  `qx-style` by path, and contains a line-for-line copy of the worker's
  Play loop.
- Native profile: `target\release\quantum-x.exe ride-native-profile <flat80-3car.qx.json> <new-dir> 60`
  (hidden, inactive window; no desktop interaction).
- Manual check still owed on the desktop: watch a moving train (Case B or D
  settings) in Simulator Mode at 240 Hz and note whether motion hitches and
  how often the catch-up notice increments.
