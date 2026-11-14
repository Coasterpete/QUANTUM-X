# M4K.1 — Windows simulation-worker timing correction

Date: 2026-11-14 · Branch `claude/m4k-simulator-mode`, on top of M4K
`653d6e3` and the investigation report `989ead1`.
Hardware for every measurement: the owner's Windows 11 laptop, Intel integrated
GPU, Vulkan driver 101.8724, 240 Hz panel.

**Scope.** The playback worker's *wait* is replaced. Nothing else changes:
the physics model, the 1/240 s step, solver sweeps, tolerances, the 8-step
catch-up cap, presentation interpolation (`PRESENTATION_DELAY_TICKS = 2`), the
project format and the Editor ↔ Simulator workflow. There is still exactly one
worker.

## 1. Root cause

The worker keeps absolute monotonic deadlines (`next += dt`). It used
`mpsc::Receiver::recv_timeout(next − now)` to sleep between them. On Windows that
call parks with a millisecond timeout, and the kernel only expires that timeout on the system
clock interrupt, which is about 15.6 ms by default. Measured on this machine:
`recv_timeout(4.167 ms)` returned after a median of **15.6 ms** (minimum 8.1 ms).

So the worker woke about 64 times per second instead of 240. Each wake caught
up 3–4 steps in a burst:

- About two of every three rendered frames had no new physics state ("starved").
- Whenever bursts plus step cost exceeded the 8-step cap, wall-clock debt was
  discarded, and simulated time fell behind real time.
- The legacy wait was very sensitive to step cost. Runs where the CPU was in a
  slower state (step mean ≥ 1.7 ms) produced most of the catch-up events.

## 2. Selected mechanism

**One high-resolution waitable timer per worker thread**:
`CreateWaitableTimerExW(.., CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS)`,
then `SetWaitableTimer` with a relative due time and `WaitForSingleObject`
([playback_clock.rs](../../apps/quantum-x/src/authoring/playback_clock.rs)).

- **Deadline-based.** Each wait is `deadline − now` against the same absolute
  `Instant` deadlines as before, so errors never accumulate. Every wait is
  bounded by 50 ms, and the OS wait by 50 ms + 100 ms, so a timer that never
  signals cannot hang the worker.
- **Requests.** While running, the channel is polled before and after every
  timed wait, so Pause/Reset/Clear/shutdown are handled within one step
  (≤ 4.17 ms). While paused or idle, the worker blocks in `recv_timeout(60 s)`
  exactly as before: no timer and no CPU use.
- **Shutdown and cleanup.** If the sender is dropped, the worker sees
  `Disconnected` and its loop ends. The timer handle is an owned, thread-local
  RAII value closed in `Drop`.
- **No process-wide side effect.** `timeBeginPeriod` is not called, so the
  process and system timer resolution are unchanged.
- **Fallback.** If timer creation fails (Windows before 10 1803), or any
  set/wait fails, the worker permanently uses the previous `recv_timeout` wait.
  Non-Windows builds always use it. The active mechanism is published in the
  worker timing record and shown in the Simulator panel.
- **FFI.** Six kernel32 functions are declared by hand, each `unsafe` call with
  a SAFETY note. Enabling extra `windows-sys` features would instead rebuild
  winit/wgpu and their dependents.

**Alternatives evaluated:**

| Option | Verdict |
| --- | --- |
| `recv_timeout` (previous) | 15.6 ms granularity (measured) |
| `timeBeginPeriod(1)` | Rejected. It raises the process timer resolution, which this task forbids. |
| Busy-wait or spin to the deadline | Rejected. It costs CPU continuously. |
| `std::thread::sleep` | Measured equal (p50 4.57 ms for a 4.167 ms request), because std currently uses the same timer internally. Rejected: it relies on an undocumented std detail, creates and closes a kernel timer on every call, and cannot report a fallback. |
| Explicit high-resolution waitable timer | **Selected.** Probe: 240 Hz deadline cadence, interval p50 4.14 ms, lateness p50 0.38 ms, p99 1.31 ms. |

**Developer A/B switch.** Set `QX_PLAYBACK_WAIT=legacy` to force the previous wait.
Any other value refuses playback with an explanatory error, following the same
pattern as `QX_EXPERIMENTAL_TRAIN_VELOCITY`. It is launch-only, never persisted,
and never shown as a setting.

## 3. Before/after measurements

**Method.** A new `--gui-smoke-test simulator-timing <project> <seconds>`
drives the **real Simulator composition** through the native hidden-window
event loop, using the production worker, egui and Vulkan, and the same commands
as the UI.

- Sequence: enter → Play for 12 s → Pause → hold paused 3 s → Reset → Return
  to Editor → three re-entries that each **close the Simulator while running**
  → offline replay.
- Each configuration ran legacy and precise twice, in alternating order, on the
  same binary and hardware.
- Process CPU was read from `Process.TotalProcessorTime` after each run.
- The hidden loop draws on a ~5 ms poll (frame p50 5.75–6.25 ms), not on display
  vsync. Frame freshness shows worker pacing; it is not a display benchmark.

**Projects.** Development fixtures, not coaster designs: a 400 m straight with
the default three-car gray-box train (11 bodies, 2440 kg) seated at mid-track.

- `flat-v0`: level, 0 m/s
- `flat-v3`: level, 3 m/s
- `down10`: 10° downhill, 0 m/s
- `launch1`: level, Launch 0–400 m at 1 m/s²
- `down10-short-error`: an 80 m version of `down10` that runs off the open end

### 3.1 Summary over eight 12 s runs per mechanism

| Metric | Legacy (`recv_timeout`) | High-resolution timer |
| --- | --- | --- |
| Wake interval p50 / p90 / p99 | 15.75 / 16.25–31.0 / 16.75–32.0 ms | **4.25 / 4.75 / 4.75–5.0 ms** |
| Wake interval worst | 16.7–47.6 ms | 5.0–16.3 ms (one OS scheduling outlier) |
| Wake lateness past deadline, p50 / p99 | 8.5–9.5 / 12.25–15.75 ms | **0.5 / 0.75–1.0 ms** |
| Steps per wake | mostly 3–4, up to 8 | **exactly 1 on ≥ 99.7 % of wakes** |
| Physics steps per second | 237.6–240.0 | **239.9–240.0** |
| Simulated/wall ratio | 0.9898–0.9999 | **0.9998–0.9999** |
| Drift after 12 s (wall − simulated) | 1.8–122.3 ms | **1.4–2.8 ms** |
| Catch-up limit events | 0–22 per run (50 total) | **0 in every run** |
| Dropped wall-clock time | 0–26.6 ms per run | **0 ms** |
| Frames with fresh physics data | 30.9–36.2 % | **99.7–100 %** |
| Presentation-starved frames | 63.9–72.6 % | **0–0.4 %** |
| Step mean / p99 / max | 1.31–2.11 / 1.75–3.0 / 2.6–4.4 ms | 1.32–2.31 / 1.75–3.0 / 2.5–5.5 ms |
| Over-budget steps (> 4.167 ms) | 0–7 per run | 0–3 per run |
| Worker CPU, % of one core, while running | 17–65 (mean 41.5) | 28–60 (mean 40.0) |
| Whole-process CPU, % of one core | 32–66 (mean 48.6) | 40–63 (mean 47.4) |
| Worker CPU over a 3 s paused hold | 0.0 ms | 0.0 ms |

A residual drift of ~1.5–3 ms is expected even on pace. Drift is sampled right
after a step is published, so it equals that step's wake lateness plus its
solver time. It does not grow with run length.

### 3.2 Short track ending in a simulation error (6.7–6.9 s of play)

| | Legacy | High-resolution |
| --- | --- | --- |
| Drift when the error stopped play | 232.6 ms | **2.7 ms** |
| Catch-up events / dropped wall time | 39 / 66.3 ms | **0 / 0 ms** |
| Fresh frames | 25.4 % | **100 %** |
| Error tick, speed, distance | 1615, 11.4631 m/s, 38.568 m | **identical** |

### 3.3 Per-run detail

| Config | Mode | Rep | Drift ms | Catch-up | Dropped ms | Wake p50/p99 ms | Fresh % | Worker CPU % |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| flat-v0 | legacy | 1 | 9.1 | 1 | 1.7 | 15.75/16.75 | 36.2 | 17.2 |
| flat-v0 | precise | 1 | 1.6 | 0 | 0 | 4.25/4.75 | 99.7 | 35.8 |
| flat-v3 | legacy | 1 | 3.9 | 0 | 0 | 15.75/16.75 | 35.9 | 43.7 |
| flat-v3 | precise | 1 | 1.6 | 0 | 0 | 4.25/4.75 | 100 | 28.3 |
| down10 | legacy | 1 | 2.7 | 0 | 0 | 15.75/16.75 | 36.2 | 53.5 |
| down10 | precise | 1 | 1.8 | 0 | 0 | 4.25/4.75 | 100 | 39.1 |
| launch1 | legacy | 1 | 1.8 | 0 | 0 | 15.75/16.75 | 35.9 | 36.4 |
| launch1 | precise | 1 | 1.4 | 0 | 0 | 4.25/4.75 | 100 | 38.4 |
| flat-v0 | precise | 2 | 1.7 | 0 | 0 | 4.25/5.0 | 100 | 36.6 |
| flat-v0 | legacy | 2 | 3.7 | 0 | 0 | 15.75/16.75 | 36.0 | 21.9 |
| flat-v3 | precise | 2 | 1.9 | 0 | 0 | 4.25/5.0 | 100 | 28.4 |
| flat-v3 | legacy | 2 | 43.4 | 8 | 10.1 | 15.75/32.0 | 33.0 | 64.5 |
| down10 | precise | 2 | 2.3 | 0 | 0 | 4.25/4.75 | 100 | 53.8 |
| down10 | legacy | 2 | 99.5 | 19 | 17.9 | 15.75/31.75 | 30.9 | 49.1 |
| launch1 | precise | 2 | 2.8 | 0 | 0 | 4.25/4.75 | 100 | 59.6 |
| launch1 | legacy | 2 | 122.3 | 22 | 26.6 | 15.75/32.0 | 33.0 | 45.9 |

Stationary vs moving: `flat-v0` (stationary) ticked at the same 240 steps/s as
the moving cases. A stationary train is never evidence of a stalled simulation.

## 4. CPU and power implications

- **Same solver work.** Both mechanisms execute the same number of steps, so
  physics CPU is unchanged. Measured worker CPU means (41.5 % vs 40.0 % of one
  core) and process CPU means (48.6 % vs 47.4 %) are equal within run-to-run
  noise.
- **Measurement noise.** Windows charges thread CPU at clock-tick granularity,
  and CPU frequency state varies between runs. Step mean ranged 1.3–2.3 ms on
  the same input.
- **More wake-ups.** While playing, the worker wakes 240 times per second
  instead of ~64. Each extra wake costs one timer arm plus a context switch
  (microseconds). On battery this may keep the worker's core out of deeper idle
  states during playback; that power cost was not measured with a meter.
- **Idle and paused.** Unchanged. The timer is used only while running. Paused
  and idle workers block on the channel and use no CPU; the measured paused CPU
  was 0.0 ms over 3 s, below Windows' ~15.6 ms accounting resolution.
- **No busy-waiting and no spin phase.**
- **System-wide.** The high-resolution timer does not change system timer
  resolution, so other processes and the system's power policy are unaffected.

## 5. Windows compatibility and fallback

| Platform | Behavior |
| --- | --- |
| Windows 10 1803+ / Windows 11 | High-resolution waitable timer |
| Earlier Windows, or any timer failure | Falls back to `recv_timeout` (pre-M4K.1 behavior) and reports `channel timeout` |
| Non-Windows | `recv_timeout` (unchanged; ordinary OS timers are typically fine-grained) |

Not tested: power-throttled (EcoQoS) background processes, and Windows on ARM.
The hidden, inactive smoke window is a reasonable proxy for an occluded window,
and pacing there was on target.

## 6. Deterministic physics regression

- **Bit-identical replay.** Every timing run (18 runs, both mechanisms)
  replayed the paused tick count offline with `TrackSimulation::with_operations`
  (the worker's constructor) and required bit-identical poses. All 18 matched,
  at 2853–2882 ticks and at the error tick 1615.
- **Physical results:**
  - Level 0 m/s stays at 0.
  - Level 3 m/s holds 3.0000 m/s.
  - 10° downhill accelerates at 1.7035 m/s² (= g·sin 10°), reaching 20.4348 m/s at tick 2879/2880 in both modes.
  - A 1 m/s² launch gives exactly 1.0000 m/s².
  - The error run stopped at tick 1615 with identical speed and distance in both modes.
- **State handling:**
  - Reset matched the initial poses in all 18 runs.
  - Reset cleared the open-end simulation error (recovery).
  - Pause held the tick count constant for 3 s.
  - The Editor state (document, dirty, undo/redo, path, selection, profile,
    layouts) was restored after every exit: 72 checks, including 54 closes
    while running.
- **Test suites.** The workspace suite passes (§8). `qx-physics`, `qx-document`,
  `qx-geometry` and `qx-rendering` are unchanged by M4K.1.

## 7. Remaining performance limitations

- **Step cost.** Mean 1.3–2.3 ms, p99 up to 3.0 ms, worst 5.5 ms in these runs
  (7–17 ms spikes in the earlier investigation), against a 4.167 ms budget, for
  one 11-body train. A step that overruns delays only its own wake; the 8-step
  cap absorbs it without dropping time. Sustained slower steps would bring catch-up back. Reducing
  solver cost (128 sweeps per tick regardless of convergence) is a **separate
  optimization problem**. Sweeps were not reduced here.
- **OS scheduling outliers.** One precise run had a single 16.3 ms wake gap.
  It was absorbed without catch-up or dropped time.
- **Presentation not changed.** `PRESENTATION_DELAY_TICKS = 2` is unchanged by
  design. With on-time worker wakes, starvation fell from ~68 % to ≤ 0.4 % of frames.
- **Not measured.** Visible-window display pacing at 240 Hz, battery power
  draw, and multi-train scenes.

## 8. Verification commands

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace --no-fail-fast` | pass: 530 passed, 0 failed, 16 ignored. The app binary has 202 passed / 1 ignored, including 3 new `playback_clock` tests and 1 new layout-comparison test. |
| `--gui-smoke-test welcome` / `edit` / `simulator` (release, final binary) | all `passed`. Simulator round trip: 48 ticks, reset equal to initial, editor restored. |
| `--gui-smoke-test simulator-timing` campaign (18 runs) | all `ok: true` (tables above) |
| Final-binary confirmation, `flat-v3`, 12 s | High-resolution: drift 3.0 ms, 0 catch-up events, 0 ms dropped, wake p50/p99 4.25/5.0 ms, 99.9 % fresh frames. Legacy (warm CPU after the test suite): drift 732.3 ms, 110 catch-up events, 271.1 ms dropped, wake p50 16.25 ms, 25.9 % fresh frames. Both runs had bit-identical replay and restored the editor. |
| Invalid `QX_PLAYBACK_WAIT=bogus` | entry refused with the actionable message; fails closed |
| `simulator_tests` repeated after the fix | 0 failures in 150 runs |

## 10. Intermittent document-preservation test failure (resolved)

The open M4K item (`simulator-mode-m4k.md` §5.2) **reproduced** during this pass.

- **Failure rate before the fix:** 1/20, 1/80 and 2/27 repeated runs of the
  `simulator_tests` module.
- **Affected tests:** `editor_simulator_editor_round_trip_preserves_the_open_document`
  and `failed_seating_returns_to_the_editor_without_a_partial_simulator`.
- **Failing field.** The field-level assertion named `["layouts"]` every time.
- **Diagnostics.** Temporary diagnostics printed both layouts, and in every
  failure they were **structurally equal**. Only the JSON object key order
  differed.

**Cause.** `egui_tiles` stores tiles in an `ahash::HashMap`. Its iteration order,
and so its serialization order, depends on per-instance hash keys and insertion
history, so an unchanged layout can serialize differently. The test compared
`serde_json::to_string` output. This was a test-harness defect, not lost editor
state. It is not related to the timing change: the same failure was documented
at `653d6e3`, before M4K.1.

**Fix.** The test, and the native smoke `Baseline`, now compare
`serde_json::Value`. Object key order is ignored; every tile, ID, share, active
tab and child order is still compared exactly. The new test
`layout_comparison_ignores_only_hash_map_key_order` pins these semantics: a
changed pane, child order or active tab is still detected. No assertion was
removed or loosened. After the fix: 0 failures in 150 module runs and in 3 full
app-binary runs.

## 9. Manual desktop verification (owner)

Not manually verified by the agent: no desktop control was used.

1. Build: `cargo build --release -p quantum-x`.
2. Open or create a project with a train associated (Train Workspace ›
   Gray-box train › Add and associate) on a track of at least ~20 m. Set
   Operations › Physical simulation settings › Initial velocity to 3 m/s.
3. Window / Layout › Simulator Mode (development)… → Play. In the right panel,
   check the line `Worker: high-resolution waitable timer · wake p50 4.25 ms ·
   drift … ms`. Drift should stay within a few ms and should not grow. No
   catch-up notice should appear.
4. Watch the train in Chase or Free camera at the panel's 240 Hz and note
   whether motion is smooth.
5. Pause: the tick count stops. Reset returns to tick 0. Return to Editor
   restores the editor exactly. Re-enter, Play, then Return while running.
6. Comparison: close the app, then in PowerShell
   `$env:QX_PLAYBACK_WAIT='legacy'; .\target\release\quantum-x.exe`, and repeat
   step 3. The line should read `channel timeout · wake p50 15.75 ms`, often
   with a growing drift and catch-up notices. Remove the variable afterwards:
   `Remove-Item Env:QX_PLAYBACK_WAIT`.
7. Optional repeatable measurement:
   `.\target\release\quantum-x.exe --gui-smoke-test simulator-timing <project.qx.json> 12`
   prints the JSON evidence used above.
