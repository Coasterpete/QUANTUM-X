# M4G — extended source-guided dynamics and diagnostic playback

Native Windows/MSVC, 2026-11-13. Experimental integration tests and standalone
developer evidence only. No production simulator, schema, editor or train assets
are changed. No push, merge or PR is authorized.

Toolchain/hardware: rustc/Cargo 1.99.1, LLVM 23.1.1,
`x86_64-pc-windows-msvc`, Intel Core i7-14700KF. Performance observations are
on a shared desktop, with no CPU affinity, isolated scheduling or deadline bound.

## Stage 0 — preserved baseline

Read repository `AGENTS.md`, the M4F report, and relevant M4D/M4E-A/B/C research.
The clean M4F checkpoint is `87d561b38ece1e1e2ee8603417640501d2d9f0ef`;
its three stages are `751865a` (continuous source geometry), `6a541ca`
(experimental B1 coupling), and `87d561b` (actual-pose offline viewer).
Those commits, report, original viewer and evidence remain intact.

Created the managed worktree
`C:/Users/coast/.codex/worktrees/train-articulation-m4g/QUANTUM-X` directly at
that checkpoint, then branch `codex/train-articulation-m4g`. All previous
worktrees and the original checkout's README/.vscode/development-tooling edits
are preserved. Explicit task instructions supersede the repository's automatic
publication workflow: local stage commits only.

Before implementation, native Release `graph_dense_extension` passed all four
tests. The focused physics baseline (`source_guidance`, `mechanical_guidance`,
`rail_normal_guidance`, `guidance_robustness`, `guidance_continuity`,
`source_rail_audit`) passed 45 tests, with six opt-in cases ignored.
The unchanged M4F independent dense fixture/derivative and source reaction tools,
and M4A/B/C/D independent matrix/reaction verifiers passed. Baseline logs are
under ignored `target/m4g/baseline-*`; no inherited gate or equation was edited.
Cargo uses a command-local `CARGO_TARGET_DIR` at the existing M4E-B build cache;
the older source/evidence is untouched. Omit that variable for an independent
new build cache.

## Stage A — PASS within the tested scope

Qualification is required before extending the viewer. Initial duration probes
use the exact original 12 m R8/.04 rad/m twist source, initial bank .6 rad,
lead 8.025 m, shared middle bogie, 6 m/s and gravity (0,-9.81,0) m/s².
A second independent case uses the unchanged foundation project's first two
sections, physical interval [0,72] m, lead 11.625 m, traversing the compatible
12 m source join. No source length, control, banking, mass, bearing or support
equation is changed. Outer timestep remains 1/240 s, four internal substeps and
32 impulse sweeps. Both cases pass the 120/240/480-tick ladder with exact
state/report/hint/envelope replay. There is no incomplete duration target or
first failing tick in either qualification case. The R8 train reverses under
gravity before reaching the endpoint; motion is not prescribed or repeated.

Dense preparation uses unchanged defaults: requested maximum step .1 m, fine
grid .05 m, 200,000-sample budget and .15 rad maximum source rotation per step.
R8 is fixture index 2 in `qx-geometry/tests/support/m4e_c_extension.rs`: section
1, radius 8 m, plane angle 0, constant roll .04 rad/m and initial bank .6 rad.
Foundation uses `qx_document::sample::foundation_project().sections[..2]`:
12 m level lead-in and the original 60 m force-vector rise/bank with its two
30 m authored profiles. These are unchanged diagnostic fixtures, not newly
designed coaster layouts. The 12 m source suffices for the R8 duration; the
longer fixture supplies separate multi-chart/performance evidence. Tokens are
scoped to each independent fixture run; the trace fixture/source/chart/section
tuple identifies its reference.

`support/m4g_extended.rs` retains states in memory, then separately extracts
diagnostics. Each of 481 records per case contains all 11 actual body poses,
66 velocities, 65 position/velocity constraint rows, 30 support datums/normals,
source chart/section identities, projection F/D, station continuation envelopes,
bearing anchors/swing/roll, instantaneous reactions, outer-step average impulse
multipliers, reaction power, momentum closure and energy drift. Instantaneous
forces use the actual eliminated-constraint gradient times the independently
solved multiplier. Average impulse multipliers are a different observation,
not asserted to be instantaneous or unit-normal force magnitudes.

| Two-second observation | R8/.04 twist, 12 m | Foundation prefix, 72 m |
| --- | ---: | ---: |
| Maximum position row, m/rad | 1.271846e-5 | 2.204655e-6 |
| Maximum velocity row, m/s or rad/s | .002260358 | .0003348022 |
| Maximum signed support gap magnitude, m | 1.271477e-5 | 9.150822e-7 |
| Maximum bearing anchor distance, m | 3.908771e-6 | 2.718779e-6 |
| Maximum bearing roll error, rad | 1.943187e-6 | 8.156978e-8 |
| Maximum relative bearing swing, rad | .1864516 | .008921047 |
| Maximum absolute energy drift, J | 29.749221 | .312288 |
| Final signed energy drift, J | -29.749221 | +.312288 |
| Maximum instantaneous support reaction, N | 1710.4843 | 1057.6939 |
| Maximum absolute reaction power, W | 21.4072 | .6325093 |
| Support station range, m | [2.752468,11.836265] | [6.405,24.037216] |
| Source charts visited | 0 | 0,1 (12 m join) |

Unchanged qualification gates remain position <2e-5, velocity <.003, bearing
swing <.7 rad, |F| <2e-13 and D >1e-8. Maximum observed |F| is 1.936e-13;
minimum D is .9963993. Linear momentum closure is below 1.61e-9 N. Station
changes satisfy the original velocity-driven bounded continuation envelopes in
both directions. All support points remain inside the physical domain; complete
body-center diagnostic projections also succeed there. No endpoint extrapolation
or remote station selection is used. The foundation's 42 m join is not reached.

The unchanged stabilization method has finite constraint velocity and nonzero
reaction work; energy drift is an observed limitation, not energy conservation.
The R8 loss grows from 4.558 J at .5 s through 13.652 J at 1 s to 29.749 J at
2 s despite bounded residuals. This is not a full wheel/contact or articulated
production train validation. Reactions are bilateral and can pull as well as
push; there is no unilateral separation, suspension, friction or impact model.

The front R8 support reaches its largest station at recorded tick 258 (1.075 s),
then retreats. This is the independently observed turnaround, not a prescribed
playback event. Support reaction multiplier ranges are [-908.6023,1710.4843] N
for R8 and [-12.89759,1057.6939] N for foundation; normal-gradient scaling is
accounted for separately in the force-norm maxima above. Independent energy
reconstruction from all poses and 66 velocity components agrees within
1.456e-11 J. Initial kinetic energies are 19011.2296 J and 19800 J; maximum
absolute drift/initial-kinetic ratios are .001564824 and .00001577210.

Stage A native Release tests pass (duration/replay, transactional rejection,
explicit full trace). The Python verifier independently reconstructs support and
bearing world points, checks all 962 states and continuation/chart ownership,
and solves 12 long-run reaction systems with NumPy at initial, 24, 120, 240,
360 and 480 ticks. Maximum load/acceleration-equation/mass-weighted solution
discrepancies are 1.450e-11 / 1.784e-13 / 1.877e-13. Finite-difference J and
directional Jdot refinement retain 3e-7 / 1e-6 gates at these long-run states.
An explicitly doubled-speed negative rejects its first candidate (position
2.944828e-5, velocity .004741681) and preserves the last accepted state.
An invalid internal-step configuration also preserves state, hints, windows,
envelopes and tick count. Neither negative is an incomplete six-m/s target.

Evidence: `stage-a-probe.log`, `stage-a-final.log`, `stage-a-independent.log`
and `stage-a-final.summary.json`, ignored under `target/m4g/`.

## Stage B — PASS as a measurement, not a real-time qualification

Historical M4F observation: maximum solver 3.156 ms, diagnostic/record work
2.561 ms, combined 5.642 ms, against a nominal 4.167 ms outer interval. Those
short-fixture observations are preserved as context, not rerun or treated as a
direct comparison with the longer multi-chart and expanded-diagnostic cases.

Native serial Release observations: one complete 480-tick warm-up followed by
five complete 480-tick runs on the unchanged 72 m foundation source. Preparation,
initialization, compilation, control comparisons and printing are outside step
timers. Each profiled tick is paired with an uninstrumented inherited engine;
reports, poses, velocities, hints and continuation envelopes match exactly.
Optional adapter timers default off. The instrumented copies of the M4D
dynamics/tracking helpers add measurements only; their original controls and
all bearing/support equations are preserved. These are test helpers, not a
second production backend. Nested rail work is subtracted from the other phase
categories; phase sums are checked against complete-step observations.

| 2,400 step observations | Median ms | P95 ms | P99 ms | Maximum ms |
| --- | ---: | ---: | ---: | ---: |
| Uninstrumented complete step | 6.0843 | 7.1780 | 9.3649 | 13.5179 |
| Instrumented complete step | 6.1262 | 6.9691 | 9.4094 | 17.4725 |
| Physical evaluation/projection | 5.5417 | 6.2698 | 8.5339 | 16.7457 |
| Exclusive constraints/reactions | .3132 | .4217 | .6449 | 3.5534 |
| Impulse integration | .1572 | .1786 | .2519 | 1.4987 |
| Exclusive validation/tracking | .0864 | .1051 | .1940 | 2.0268 |
| Other complete-step overhead | .0218 | .0549 | .0634 | .2007 |

Both measured series missed the 4.166667 ms interval on **2,400/2,400** steps.
The nominal simulation timestep is unchanged; offline physics does not require
meeting that wall-clock interval. This is a distribution on one shared machine,
not a worst-case bound, a guaranteed sustained rate, a speedup against M4F or
an editor FPS measurement. Timing hooks and interleaved controls can affect
cache/scheduling behavior. Full phase distributions, including exclusive
mechanical constraints/reactions, impulse integration, post-step validation/
tracking and miscellaneous overhead, are in `stage-b-cost-final.summary.json`.
The uninstrumented complete-step observed range is 4.5337–13.5179 ms.
The constraints phase covers J/constraint and directional Jdot assembly;
sequential reaction impulses are solved and applied in the impulse phase.
Independent instantaneous dense reaction solves belong to the diagnostic phase.
Rail queries dominate this observation. No numerical shortcut, widened gate or
cache that could hide source changes was introduced for a latency claim.

Recording is now buffered and separated into physical integration, diagnostic
calculations, numeric serialization and output. All ticks retain full physical
verification even when replay skips display frames. For 481 records per case:

| Recording observation | R8/.04 twist | Foundation |
| --- | ---: | ---: |
| Diagnostic calculations total, ms | 7942.873 | 11001.519 |
| Slowest per-tick diagnostic, ms | 266.031 | 316.987 |
| Serialization total, ms | 68.257 | 76.663 |
| Slowest per-tick serialization, ms | 2.816 | .741 |
| Buffered trace output total, ms | 149.507 | 164.691 |
| Trace bytes (including matrices/scene) | 11510048 | 11511908 |

Diagnostics include instantaneous dense reaction solves every tick and extra
finite-difference J/Jdot checks at six checkpoints, which explain large maxima.
They do substantially more work than M4F's alignment-only record pass; these
figures are not a like-for-like regression benchmark against 2.561 ms. Output
timing measures stdout write/flush through the Cargo/redirection pipeline, not
a durable-storage guarantee. Scene/reference preparation belongs to diagnostic
totals, not per-tick samples. Timing observations are excluded from exact replay
comparison; scene and all 962 diagnostic/state records remain identical to
Stage A. Separating this work prevents it blocking physical integration and
makes later bounded geometry/reaction caching independently reviewable.

## Stage C — PASS as a provisional developer inspector

The original M4F viewer/report remain unchanged. New tools package both genuine
481-state clips into `target/m4g/source-guidance-m4g.html`, about 13.22 MB.
The exporter first runs full-tick physical/continuation/reaction validation;
only then does it omit dense matrices/unused raw rows from the display package.
The original full trace retains those observations. There is no interpolated,
repeated or prescribed motion; each primitive box is transformed by its recorded
position/quaternion, and shared middle bogie 3 has a single identity.

Implemented developer controls: tick slider and numeric physical timestamp;
play/pause/reset with .1/.25/.5/1/2/4 times replay speed; side/top/three-quarter
presets; yaw/elevation, pan and zoom through controls plus drag/Shift-drag/wheel;
role filtering, body selection and isolation; independent IDs, bearing anchors,
support datums, normals and source-reference visibility. Replay follows elapsed
physical timestamps and can skip displayed records; it never interpolates poses.
All recorded ticks remain inspectable. Timer cancellation prevents stale replay
callbacks surviving pause/reset. The initial requestAnimationFrame-driven QA
did not advance in the headless virtual-time run; a timestamp-driven cancellable
timer now exercises the actual replay in both normal and headless contexts.

Camera bounds are computed from the entire recorded clip, not the present frame.
Label leader lines and collision avoidance improve overlap; the body table also
provides an accessible selection path. Source reference uses dashed lines;
cars/bogies/carriers have distinct restrained colors and labeled roles. Selected
bodies have a heavy dashed outline as well as a selected table row. No wheel
graphics or train/manufacturer branding is introduced. Original provisional
dimensions are retained and micrometre gaps are not geometrically magnified.
Normal glyph length is explicitly .22 m. Actual error values, observed maxima
and relevant gates are shown beside timestamp histories of normalized position/
velocity error and signed energy drift. Graph scales are explicitly labeled.

Headless Edge uses isolated profiles, hidden process creation and no connection
to the owner's active browser/desktop. Real DOM control checks pass for both
clips, endpoint reset, numeric timestamps, scrubbing, timed replay advancement
at 1x and 4x, pause, keyboard stepping, camera controls/presets, filtering,
shared-bogie isolation, body-list selection, every visibility switch and history
generation. Stable framing and all 66 pose-derived body faces are checked.
Three 1600x1400 screenshots were exported and visually inspected: the R8
three-quarter state at 1 s, foundation top view at 1 s, and isolated middle bogie
at 1 s. Evidence filenames are `source-guidance-m4g.png`,
`source-guidance-m4g-foundation-top.png`, `source-guidance-m4g-shared-bogie.png`,
`stage-c-viewer-dom.html`, `stage-c-headless.log` and `stage-c-export.log`.
The viewer is standalone with no network assets or server requirement.

These are headless and screenshot checks, not manual native desktop, high-DPI,
touch-device, arbitrary-size or live application qualification. The whole-clip
framing can make a single body small; zoom/isolation controls permit closer
inspection. Source lines can leave the viewport. This layout is explicitly
temporary developer UI, not an approved main-editor design.

## Stage D — independent verification details

**PASS for the stated experimental scope.** Executed native verification:

| Check | Result |
| --- | --- |
| Default full debug workspace | 502 passed, zero failed, 13 ignored |
| Feature-enabled full debug workspace | 525 passed, zero failed, 18 ignored |
| Native Release geometry, experimental source feature | 112 passed, zero failed, two ignored |
| Native Release physics, experimental guidance feature | 142 passed, zero failed, 15 ignored |
| M4G duration/replay, transactional negative and explicit export | Passed |
| Repeated complete-step characterization | Warm-up plus 2,400 measured paired steps passed numerical comparisons |
| Final strengthened ladder in debug | One passed, zero failed; both fixtures finish 480 ticks |
| Independent M4F derivatives/reactions; M4A/B/C/D matrices/reactions | Passed again against final Release physics evidence |
| Independent M4G full-tick/replay/energy/reaction checks | 962 states and 12 long-run systems passed |
| Formatting and workspace check | Passed, including final rerun |
| Strict all-target Clippy, default and experimental features | Passed, including final rerun |
| Python/Node syntax, exporter negatives, viewer export/headless interactions | Passed |
| Git staged/unstaged whitespace and preservation audit | Passed |

The full feature workspace preceded the final assertion-only strengthening of
the ladder; that exact test was then rerun in debug, and the complete native
Release physics run also includes the stronger assertion. No integration
equations or physical data changed after the full workspace runs. Final syntax,
export and headless checks include the independent energy/face verifiers.
Final packaging observed 215.119 ms JSON serialization and 63.152 ms HTML/JSON
file writes; HTML is 13,221,072 bytes. These offline packaging observations are
separate from Rust trace serialization/output and from physical integration.

Final evidence under ignored `target/m4g/`: `workspace-default.log`,
`workspace-feature.log`, `geometry-release.log`, `physics-release.log`,
`ladder-debug-final.log`, `fmt-final.log`, `check-final.log`,
`clippy-default-final.log`, `clippy-feature-final.log`, `final-m4*-independent.log`,
`stage-d-independent-final.log`, `stage-d-export-negatives.log`,
`stage-d-export-final.log`, `stage-d-headless-final.log`,
`preservation-final.log` and `platform.log`, plus the Stage A/B evidence above.
Original M4F HTML/JSON/PNG evidence remains in its unchanged clean worktree.

The original dense geometry/derivative tools pass on the preserved source:
585 pose queries in 117 cells, 48 adjacent limit pairs, two station spacings,
and 70/90-digit rolling controls. The unchanged physical-datum verifier checks
42 cases: maximum position reconstruction discrepancy 2.446e-15 m, first
derivative discrepancy 2.624e-16 and normal discrepancy 3.620e-16. These measure
agreement with the retained numerical representation, not certified error against
the exact graph ODE. Source approximation/refinement limits remain those of M4F.
All geometry source and geometry tests are byte-unchanged from M4F. A normalized
body comparison also confirms the differential and bounded-projection bodies
are unchanged beneath the optional timing wrappers.

Long-run independent checks add SI energy reconstruction for every state and
force-vector reconstruction from independently solved multipliers at the 12
reaction checkpoints. The headless verifier independently transforms all 66
body faces using explicit quaternion rotation matrices and compares their SVG
coordinates to recorded tick 240 at the declared dimensions/camera. This checks
actual poses separately from the viewer's cross-product rotation implementation.
Full trace verification also rejects deliberately corrupted residual, quaternion,
endpoint and nonfinite records before viewer export. The ordinary duration test
requires **both** fixtures to finish 480 ticks; any future early stop fails it.

The original .4 rad/m rolling-line `NoConvergence` seating negative is retained
unchanged. Its geometry is valid; it is not a new positive mechanical/playback
case. The original sampled changing-bank corner rejection, derivative restart
counterexample and M4A–M4E controls remain executable. No failing positive gate
was waived or tolerance widened. Native application Release packaging, manual
desktop/high-DPI interaction, live FPS and production B1 integration are not
verified or claimed. Apart from the explicitly executed M4G export/profiler,
other inherited ignored long-run/profiling/GPU exporter cases remain opt-in.

## Reproduction

Run from the M4G worktree. Create ignored `target/m4g` first. An ordinary target
cache also works; no application default feature enables this experiment.
PowerShell redirection is intentional so exact per-tick evidence is retained.

```powershell
New-Item -ItemType Directory -Force target/m4g
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features qx-physics/experimental-source-guidance -- -D warnings
cargo test --workspace
cargo test --workspace --features qx-physics/experimental-source-guidance
cargo test --release -p qx-geometry --features experimental-graph-source
cargo test --release -p qx-physics --features experimental-source-guidance -- --nocapture --test-threads=1
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance extended:: -- --include-ignored --nocapture --test-threads=1 > target/m4g/trace.log 2>&1
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance extended::export_extended_states -- --ignored --nocapture --test-threads=1 > target/m4g/replay.log 2>&1
python tools/verify-extended-guidance-m4g.py target/m4g/trace.log target/m4g/replay.log
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance performance::repeated_complete_step_costs -- --ignored --nocapture --test-threads=1 > target/m4g/cost.log 2>&1
python tools/summarize-guidance-cost-m4g.py target/m4g/cost.log
python tools/render-source-guidance-m4g.py target/m4g/trace.log target/m4g/cost.summary.json
node --check tools/verify-source-viewer-m4g.mjs
node tools/verify-source-viewer-m4g.mjs target/m4g/source-guidance-m4g.html
git diff --check
git diff --cached --check
```

Run profiling alone after other builds/tests finish. `trace.log`/`replay.log`
are complete logs, not project documents. The exporter validates before display
field filtering. Both fixtures and all 481 poses per fixture remain in the
single HTML. Python requires the existing NumPy independent-check environment;
viewer packaging otherwise uses the standard library. Node QA uses an installed
Edge/Chromium binary; its optional second argument supplies another executable.
The HTML itself has no third-party library or runtime dependency.

Optional owner-only manual checklist (not manually verified here): open the
generated HTML in a browser; scrub both clips from 0 to 2 s; inspect the R8
turnaround and middle bogie 3; choose side/top/three-quarter, zoom and pan;
select/isolate each role, toggle overlays, seek through the time histories;
confirm play/pause/reset and keyboard navigation at multiple replay speeds.
The native application regression command is `cargo run -p quantum-x --release
-- edit`; opening an existing project, initialize/play/pause/reset and save/reopen
remains an owner-executable production regression, not a new B1 workflow.

## Remaining blockers and M4H recommendation

The two-second fixture qualification establishes bounded residuals and repeatable
source coupling, not a production contact model or realistic whole train. The
same provisional 100 kg/isotropic 20 kg m² body parameters and ideal bilateral
datums remain. They omit wheel spin, unilateral wheel/rail contact, suspension,
friction, collision/impact and realistic vehicle mass/inertia. Projection is a
bounded local chart search, not global nearest-root certification. Native curves,
rider-reference inversion, periodic seams and general authored-track acceptance
remain outside this experimental continuous-source subset. Caller-owned identity
tokens must be retired on source edits; this offline immutable trace has no live
edit lifecycle. No safety, all-layout, real-time or ride-quality claim follows.

Recommend M4H first investigate energy/work consistency on these exact retained
fixtures, including turnaround, step refinement with the same 1/240 s outer
timestep, impulse/reaction work and stabilization accounting. Treat that as a
separate numerical gate. Then profile bounded exact-key geometry reuse and
reaction/diagnostic reuse against the unchanged control, including stale-source
negatives and bitwise or explicitly quantified numerical comparisons. Do not
promote B1 into production merely because the trace is longer or replay is more
usable. Production workflows and any future simulator presentation still require
human design; M4G finalizes no production UX choice.

## UX and Git status

Implemented owner-requested behavior is confined to the listed standalone
developer inspection controls. All new viewer layout/styling remains provisional.
No production UI/UX, simulator selector, schema, train authoring, app dependency
or production geometry/physics source file is changed. No manual desktop control
was used. Stage checkpoints are local: Stage 0 `8069a5e`, A `9dcffa4`, B `ce03070`,
C `a60e03e`; final validation is recorded in a subsequent local D commit.
No push, merge or PR is performed. Existing worktrees and unrelated work remain
preserved. Generated traces, HTML/JSON, screenshots, logs and browser profiles
are ignored artifacts; they are intentionally not committed as large binaries.
