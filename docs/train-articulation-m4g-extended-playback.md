# M4G — extended source-guided dynamics and diagnostic playback

Native Windows/MSVC, 2026-11-13. Experimental integration tests and standalone
developer evidence only. No production simulator, schema, editor or train assets
are changed. No push, merge or PR is authorized.

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
| Physical evaluation/projection | 5.5417 | 6.2698 | 8.5339 | see raw summary |

Both measured series missed the 4.166667 ms interval on **2,400/2,400** steps.
The nominal simulation timestep is unchanged; offline physics does not require
meeting that wall-clock interval. This is a distribution on one shared machine,
not a worst-case bound, a guaranteed sustained rate, a speedup against M4F or
an editor FPS measurement. Timing hooks and interleaved controls can affect
cache/scheduling behavior. Full phase distributions, including exclusive
mechanical constraints/reactions, impulse integration, post-step validation/
tracking and miscellaneous overhead, are in `stage-b-cost-final.summary.json`.
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
