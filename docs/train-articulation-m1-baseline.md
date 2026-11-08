# Train articulation M1 — existing assembly baseline

Date: 2026-11-06. Source baseline: `bcc5add` (native Windows, Rust 1.99.1,
`x86_64-pc-windows-msvc`). This pass characterizes the existing solver; it does
not implement wheel-level fitting, wheel contact, velocity projection, a new
mechanical solver, UI, or persistence changes. The earlier PDF assessment is
task background; this report's measurements come from the Rust implementation.

## 1. Existing architecture and invariants

The authoritative inputs remain the persisted component/connection graph in
`qx-document`. Component origins, mass centers, guide points, mechanical anchors,
mounting frames, dimensions and inertia are separate concepts. Stable component
IDs resolve graph edges; car/bogie associations enable a fast seed but do not
restrict the general graph to two bogies per car.

1. [`consist::seed`](../crates/qx-physics/src/consist.rs) validates the train and
   resolves explicit track offsets, legacy car/gear placement and ordered or
   out-of-order mount dependencies. Placements are initial guesses, not welds.
2. [`assembly`](../crates/qx-physics/src/assembly.rs) optionally fits an individual
   two-pivot car with a symmetric rigid chord. It skips this optimization for
   off-axis gear anchors, translated gear placements and already-seeded shared
   gear. Averaged endpoint frames and the pivot chord orient the rigid car.
3. Mounts are re-evaluated, then guides and running-gear rail attitudes are built.
   Assembly can synthesize a legacy guide for running gear without explicit
   references; playback initialization uses explicit guide references only.
4. Sequential position/orientation projection closes actual mechanical edges,
   centerline guides and angular constraints. Translation weights are unit
   weights; rotation weights use inverse squared component radius, with radius
   at least 0.25 m. These are numerical weights, not physical masses or springs.
   Scalar translation corrections clamp to 0.2 m; rotation increments clamp to
   0.15 rad. Each iterate is a rigid pose; no anchors, dimensions or track are
   edited or scaled. One guide's station is pinned to remove longitudinal drift.
5. Convergence is checked after sweeps 1, 9, 17, ..., 2393. The limits remain
   **1e-5 m positional** and **1e-5 rad angular**, with **2400 maximum sweeps**.
   The existing scalar `worst` comparison shares the numerical value of these
   limits across units. New diagnostics report each physical unit separately.
6. `consist::initialize` converts component-origin poses to mass-center poses,
   subtracts local COM offsets from anchors/guides, and initializes independent
   bodies with **zero linear and angular velocity**. Its historical 0.05 m and
   0.05 rad rejection gates are looser than assembly's closure criteria.
7. [`solver`](../crates/qx-physics/src/solver.rs) independently integrates rigid
   bodies and constraint impulses at **1/240 s**. `TrackSimulation` still uses
   32 impulse iterations and four substeps. Assembly stations are not used to
   prescribe vehicle motion. Failed dynamic steps remain transactional.

The conventional fixture has one front roll-lock bearing, one rear ball bearing,
two centerline guide points per gear, and independent 1 m / 20 kg drawbars with
two spherical bearings. Below-track fixtures retain free suspension roll;
rotating seats have separate limited hinges and offset mass centers. Fixed,
hinge, ball and swing/twist roll constraints remain intact. Opposed roll axes
remain an explicit singularity. See the existing
[roll correction](rigid-body-roll-correction.md) and
[integrated articulation](integrated-train-articulation-and-ride-cameras.md)
documents; older audit descriptions predating these changes are not the current
solver contract.

Existing bounds retained: 2048 bodies, 4096 mechanical/angular joints and guide
references where checked; development consists are representable up to 256
cars. Representability does not guarantee geometric assembly convergence.
Local station projection retains its 12 iterations, 2 m per-correction bound,
and 1e-10 m stopping thresholds. Assembly additionally bounds station drift by
`max(8 * component_radius, 4 m)`. Open ends clamp local projection and reject
out-of-range sampling; there is no circuit wrapping in initial assembly.

## 2. Diagnostic API, metrics and units

[`assembly_diagnostics`](../crates/qx-physics/src/assembly_diagnostics.rs) contains
transient observations, without serde, file output, timers or global state.
`consist::inspect_with_diagnostics` returns the same `Result<ConsistInspection>`
as `inspect`, with a caller-owned report retained on failure. Each call resets
the report. The normal path instantiates the same implementation with
`TRACE = false`; collecting residual histories is compile-time opt-in and no
diagnostic call was added to the playback step.

| Metric | Definition | Unit |
| --- | --- | --- |
| Rigid attachment error | Maximum absolute change of pairwise distances among a component's origin, authored joint anchors, guide points and wheel reference points after its rigid transform | m |
| Mechanical anchor gap | Maximum world distance between the two anchors of each connection | m |
| Centerline guide residual | Maximum full distance from a transformed guide to its current solver station, including the pinned reference's longitudinal component | m |
| Angular joint error | Existing fixed rotation-vector norm, hinge swing/limit violation, or absolute roll-lock swing/twist error; free hinge travel excluded | rad |
| Rail attitude error | Absolute running-gear twist relative to its local authored rail frame | rad |
| Chord residual | Signed Euclidean chord length minus authored pivot separation | m |
| Chord work | Bracket probes, bisections, searched span, first bracket, solved stations, car ID when applicable, termination and error | counts; spans/stations in m |
| Assembly work/outcome | Completed sweeps, guide/attitude counts, seed residuals, convergence-check history, actual terminal residuals, phase, termination and error | counts and unit-specific residuals |

Terminations distinguish convergence, sweep exhaustion and early abort; chord
reports distinguish invalid length, absent bracket and failed track query.
`Converged` for a chord means its existing fixed 48 bisections completed, not
that a newly introduced residual stopping test passed. Partial assembly aborts
retain prior checkpoints and have no terminal measurement. Observation errors
are reported separately and never change the original solver result. Derived
nonfinite diagnostic values are errors rather than NaN maxima being hidden.

`diagnose_chord` exposes the production bounded search for focused development
tests. `velocity_residuals` accepts immutable world/guide/attitude references and
reports:

- Joint anchor relative point speed, `v + omega × r`, in **m/s**.
- Guide point transverse speed relative to its projected centerline, in **m/s**.
- Angular constrained speed using the existing impulse Jacobians, in **rad/s**;
  free hinge rotation and inward motion at unilateral limits are excluded.
- Rail twist rate, including spatial bank derivative, station projection and
  offset-point Jacobians, in **rad/s**.

These are velocity-level `J*v` measurements, excluding Baumgarte bias, motors,
forces and acceleration consistency. A resting assembly can satisfy them while
still needing gravity reactions and positional stabilization on its first step.
Assembly guide distances and playback's transverse guide residuals have different
definitions and must not be conflated.

Opt-in observations allocate bounded history (at most 300 convergence samples,
plus seed/terminal samples) and inspect attachment pairs. Attachment transforms
are cached within each measurement, but pairwise work is quadratic in attachment
count on a single component. This developer API is not intended as per-tick
telemetry or for high-density contact meshes.

## 3. Fixtures and measured results

New tests reuse `Train::development_consist` and `Train::development_fixture`.
The shared [test helper](../crates/qx-physics/tests/support/articulation.rs) uses
existing geometry generation at its unchanged default 0.1 m maximum step.
Fixtures are numerical development data, not owner-designed coaster layouts.

The matrix uses a 600 m straight or a 500 m radius-45 m arc, initial bank
0.25 rad, and front-car station 200 m. The changing-bank variant adds roll rate
linearly from 0 to 0.004 rad/m over the section. With changing roll this is not
assumed to be an exact planar circle. Every row compares ordinary/observed
results exactly, repeats the observed solve and compares the full report, and
checks that train and generated track remain equal to their initial clones.

### Conventional consists

All residuals below are actual terminal measurements. Failure rows are retained
for characterization, not accepted as assembled trains.

| Fixture / cars | Bodies / connections | Result | Sweeps / checks | Joint gap (m) | Guide residual (m) | Angular joint error (rad) |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| Straight / 2 | 7 / 6 | Converged | 1 / 1 | 5.636e-16 | 8.447e-18 | 0 |
| Straight / 8 | 31 / 30 | Converged | 1 / 1 | 5.636e-16 | 8.447e-18 | 0 |
| Straight / 16 | 63 / 62 | Converged | 1 / 1 | 5.636e-16 | 8.447e-18 | 0 |
| Straight / 32 | 127 / 126 | Converged | 1 / 1 | 1.444e-14 | 1.971e-17 | 0 |
| Curved / 2 | 7 / 6 | Converged | 41 / 6 | 9.144e-6 | 1.805e-7 | 1.688e-8 |
| Curved / 8 | 31 / 30 | Converged | 825 / 104 | 9.744e-6 | 9.805e-8 | 5.143e-8 |
| Curved / 16 | 63 / 62 | Sweep limit | 2400 / 300 | 2.292e-5 | 2.271e-7 | 1.267e-7 |
| Curved / 32 | 127 / 126 | Sweep limit | 2400 / 300 | 1.075e-4 | 1.061e-6 | 6.015e-7 |
| Changing bank / 2 | 7 / 6 | Converged | 41 / 6 | 9.627e-6 | 1.790e-7 | 2.087e-8 |
| Changing bank / 8 | 31 / 30 | Converged | 817 / 103 | 9.743e-6 | 9.781e-8 | 5.143e-8 |
| Changing bank / 16 | 63 / 62 | Sweep limit | 2400 / 300 | 2.272e-5 | 2.243e-7 | 1.253e-7 |
| Changing bank / 32 | 127 / 126 | Sweep limit | 2400 / 300 | 1.083e-4 | 1.065e-6 | 6.039e-7 |

For both eight-car curved cases, chord seeding starts with approximately
6.222e-3 m joint gap and 4.444e-4 m guide residual. Changing banking also starts
with 1.920e-3 rad angular joint error. Projection reduces these below their
existing limits without changing dimensions. The 16/32-car failures demonstrate
bounded convergence limitations for these fixtures, not proof of geometric
infeasibility or a general eight-car capacity limit.

### Mechanical variants

Curved variant fixtures use radius 12 m, initial bank 0.6 rad, 120 m length and
station 25 m. Asymmetric pivots match the existing off-axis regression's local
anchors `(1.5,-0.7,0.1)` / `(-0.8,-0.9,-0.05)` on the car and
`(0.05,0.15,0)` / `(-0.03,0.1,0)` on the gear, in metres.

| Fixture | Result | Sweeps / checks | Joint gap (m) | Guide residual (m) | Angular error (rad) |
| --- | --- | ---: | ---: | ---: | ---: |
| Off-axis asymmetric pivots, one car | Converged; chord seed skipped | 41 / 6 | 7.786e-6 | 2.241e-6 | 2.489e-7 |
| Shared gear / three gear associations, straight | Converged | 1 / 1 | 3.760e-15 | 1.445e-17 | 0 |
| Same redundant shared-gear graph, curved | Sweep limit | 2400 / 300 | 1.520e-1 | 2.454e-2 | 5.411e-3 |
| Below-track, two cars | Converged | 113 / 15 | 7.155e-6 | 2.370e-7 | 0 |
| Rotating seats, two cars | Converged | 145 / 19 | 8.398e-6 | 4.929e-7 | 3.143e-7 |
| Legacy direct offset hitch, straight | Converged | 1 / 1 | 0 | 0 | 0 |
| Legacy direct offset hitch, curved | Converged | 97 / 13 | 6.618e-6 | 3.972e-15 | 0 |
| Fixed front bearing, straight, one car | Converged | 1 / 1 | 6.746e-16 | 8.447e-18 | 1.395e-16 |
| Intentionally incompatible extra guide, straight | Sweep limit | 2400 / 300 | 1.577e-15 | 2.000e-1 | 0 |

The shared-gear fixture is exactly the existing three-association test: it adds
an explicit car-2 connection to the existing `gear-1-2` with local anchor
`(2.8,-0.8,0)` m. Shared identity remains one body. Its straight success does
not establish a production Jacobs-bogie mechanism or compatibility on curvature.
Those specialized arrangements, manufacturer geometry, individual wheels and
mechanically driven seat profiles remain unimplemented or unverified. Existing
tests still reject unimplemented mechanical-profile transmission dynamics.

Across measured assembly fixtures the rigid attachment errors stay at f64
roundoff (less than 2.3e-14 m at these coordinate scales), including failed
closure attempts. Failed closure and non-rigid deformation are distinct.
Successful fixtures have rail-attitude residuals at most 8.882e-16 rad; the
failed curved shared-gear fixture has 1.061e-11 rad rail-attitude error despite
its much larger mechanical and angular-joint gaps.

## 4. Chord and projection work statistics

The unchanged chord search rejects lengths below 1e-8 m, scans **96 equally
spaced spans** from zero to `min(4 * attachment_length, 2 * endpoint_margin)`,
then performs **48 bisections** after the first sampled nonnegative residual.
There is no derivative test, tangency test, adaptive bracket refinement or
residual-based early stopping. A chord is symmetric about its seed station;
general graph projection may subsequently move guides.

- Straight interior lengths 0.01, 0.4, 2.4, 7 and 20 m match analytic spacing
  within 2e-12 m; near-end successes use an explicit 1e-8 m interior margin.
  Probe counts are 24 or 25 in the interior and 96 near the tested endpoint.
- Nine circular tests use radii 2, 8 and 30 m with lengths 0.05, 0.6 and 1.9
  times radius. The independent reference is
  `span = 2 * R * asin(attachment_length / (2 * R))` on its first branch.
  The largest measured span error is **4.253285e-8 m** (R=2 m, chord=3.8 m).
  The test allows 2e-6 m for generation/interpolation error, independently of
  its 1e-12 m chord-residual check; actual values are printed for all nine cases.
- Conventional curved 2/8/16/32-car seeds use respectively 50/200/400/800
  bracket probes and 96/384/768/1536 bisections. Every chord in those cases uses
  25 probes and 48 bisections. Straight 32-car seeding uses 787 probes because
  some brackets include an exactly zero sampled residual.
- The asymmetric case uses no chord solve; the shared-gear case uses one;
  the legacy two-car fixture uses two. All use the same graph projection.
- Exhaustion returns the original error based on sweep **2393**. Diagnostics
  separately measure the actual final poses after sweep **2400**; these are not
  substituted into the original convergence decision or error string.

## 5. Identified numerical weaknesses

1. **Missed near-tangential bracket.** For `R=10 m`, requested chord
   `19.9999 m`, center 40 m on a 100 m circle segment, the search returns
   `NoBracket` after 96 probes. An independently queried span `pi * R` has
   positive residual **9.999989e-5 m**, and the analytic first root is inside the
   search bound. Thus this failure is not infeasibility. A 20 m tangential chord
   and an infeasible 20.1 m chord also return `NoBracket`; the current solver
   cannot distinguish these situations. Track interpolation error also affects
   an exact tangency, so that case alone is not used to prove a missed bracket.
2. **Later branch after a missed interval.** A deliberately folded unit-speed
   polyline fixture has a first independently verified root at span **1.02 m**.
   A narrow positive interval is missed; the solver brackets `[1.0416667,
   1.0833333] m` and returns span **1.05 m** instead. The fixture has corners
   isolated from the roots and is diagnostic data, not a smooth coaster design.
   The old “nearest root, never remote branch” comment was therefore corrected;
   the search algorithm itself is unchanged.
3. **Endpoint rounding.** On the 600 m straight, center 599.995 m and length
   0.01 m produce maximum span `0.009999999999990905 m`, and return no bracket
   for a mathematically feasible endpoint chord. At center 0.2 m in the closed
   circle fixture, the final probe can round just outside the track and return
   a query-domain error. Both failures remain explicit.
4. **Initial seam wrapping is absent.** A radius-10 m complete circle passes the
   existing topology closure test, but chord solves at stations 0, 0.2,
   `length-0.2` and `length` fail; an interior solve succeeds. Runtime circuit
   guidance supports wrapping separately. That does not provide seam-aware
   initial placement or assembly.
5. **Long-chain convergence.** The measured curved 16/32-car graphs reach the
   sweep limit with positional gaps above 1e-5 m. Small angular and guide errors
   do not make their mechanical closure successful. The sequential projection
   and single pinned guide suggest a chain-conditioning issue, but this pass
   does not prove the cause or change weights, order, limits or budgets.
6. **Stale final check and local bounds.** The original failure message can
   describe a state seven sweeps earlier than the terminal iterate. Local
   projection/search bounds and component ordering are part of today's contract;
   they are not global feasibility or branch-uniqueness guarantees.

## 6. Initial pose and velocity consistency

The immutable checks confirm `initialize` produces zero `J*v` residual in all
five tested resting straight/curved/changing-bank/rotating-seat cases. Poses meet
assembly tolerances, not exact zero positional error. COM-to-component-origin
round trips agree within 1e-12 m, and orientation values match exactly.

`TrackSimulation::with_operations` subsequently assigns each body's linear
velocity from its inspection station's track tangent times the requested speed;
angular velocity stays zero. The test reproduces this assignment and compares
the entire resulting world with the real `TrackSimulation` initial world.

At **12 m/s**, before any step or velocity projection:

| Fixture | Anchor relative speed (m/s) | Guide transverse speed (m/s) | Angular joint speed (rad/s) | Rail twist rate (rad/s) |
| --- | ---: | ---: | ---: | ---: |
| Straight / 2 cars | 0 | 0 | 0 | 0 |
| Radius-45 curve / 2 cars | 0.53328944 | 0.10666561 | 0 | 6.011e-8 |
| Radius-45 curve / 8 cars | 0.53328944 | 0.10666561 | 0 | 1.052e-8 |
| Changing bank / 2 cars | 0.53328922 | 0.10666561 | 0 | 0.01931589 |
| Changing bank / rotating seats, 2 cars | 0.53328922 | 0.10666561 | 0 | 0.01931598 |

Independent point-velocity calculations agree with the reported joint metric.
The rail-rate Jacobian is checked against centered finite differences using a
straight centerline with exact spatial twist 0.002 rad/m; the observed rates
are approximately 0.596000000006 and 0.596000000027 rad/s. The test also checks
anchor derivatives, free hinge rotation, forbidden swing, inward/outward limit
directions, immutable input state and explicit overflow/invalid-input rejection.
Existing curved-track frame derivatives use a 0.001 m finite-difference stencil;
they are not assumed to be exact derivatives of the Hermite/slerp query.

**Finding:** rest initialization is velocity-consistent. Nonzero-speed playback
seeding is generally not constraint-consistent on curvature or changing bank.
The existing impulse solver seats these velocities during early playback, as
already acknowledged by the roll-stability energy test's warm-up. No new seating,
velocity projection, energy change or user-facing behavior was implemented.

## 7. Release performance observations

Measurements use an Intel Core i7-14700KF, native Windows MSVC target, Rust
1.99.1 / LLVM 23.1.1. The ignored `release_assembly_baseline` test generates
tracks outside the timer, warms ordinary assembly once per fixture, then records
seven batches of five solves per path. Times include train validation, placement
seeding, assembly and returned inspection allocation, excluding track generation
and rendering. These are wall-clock observations on a shared interactive machine,
not isolated benchmarks, CI thresholds or normal playback frame timings.

The **before** run used the unmodified solver at `bcc5add` plus only the timing
test/helper. All seven exact f64 pose/station/anchor fingerprints match after
instrumentation. Before the change, internal assembly sweep histories were
unavailable; they cannot be reconstructed as historical measurements. Normal
and instrumented timing now run as separate tests so the ordinary workload
matches the original baseline. Units below are **microseconds per solve**;
brackets contain the minimum/maximum of the seven batch averages.

| Fixture | Before ordinary median [min, max] (us) | After ordinary median [min, max] (us) | Opt-in observations median [min, max] (us) |
| --- | ---: | ---: | ---: |
| Straight / 2 | 34.500 [34.480, 35.680] | 34.860 [34.180, 36.500] | 43.540 [43.360, 45.540] |
| Straight / 8 | 142.720 [141.400, 146.220] | 144.880 [143.300, 151.920] | 186.860 [185.100, 213.380] |
| Straight / 16 | 287.340 [285.260, 289.400] | 287.860 [286.020, 292.540] | 383.260 [372.740, 400.900] |
| Straight / 32 | 589.220 [580.720, 610.800] | 594.360 [583.300, 608.560] | 793.000 [783.440, 815.880] |
| Curved / 2 | 484.980 [483.700, 492.500] | 486.320 [484.720, 487.880] | 518.860 [518.320, 521.000] |
| Curved / 8 | 37792.200 [37721.080, 37834.480] | 37714.040 [37477.280, 38006.620] | 65270.600 [50032.840, 79497.260] |
| Changing bank / 8 | 37404.180 [37319.840, 37476.540] | 37178.780 [37131.540, 37469.400] | 64367.240 [64151.000, 65598.260] |

No material ordinary-path slowdown is demonstrated in this sample. This is not
a universal performance guarantee. A preliminary run interleaving ordinary and
observed solves had changing-bank ordinary batches ranging from 53.234 to
70.928 ms, versus 37.132 to 37.469 ms in the separated final ordinary test.
The source of that variability was not established. The last two opt-in timing
rows also show substantial variation across runs; they do not isolate a stable
percentage cost of instrumentation. Use isolated, longer profiling if that
cost becomes a target. The trace API intentionally performs extra residual
measurements and allocations and should remain off during routine playback.

Eight-car curved initialization already takes approximately 37 ms in ordinary
Release mode on this machine, compared with approximately 0.49 ms for two cars.
That is initialization work, not a measured per-tick cost or viewport FPS.
Timing limits for 16/32-car failed curved solves, a 256-car solve, full wheel
fitting, and full rigid-body playback under load were not benchmarked here.

## 8. Regression checks and reproduction

Run from the dedicated worktree root:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p qx-physics -p qx-document --tests
cargo test --release -p qx-physics --test assembly_diagnostics --test chord_characterization --test initialization_diagnostics -- --nocapture --test-threads=1
cargo test --release -p qx-physics --test articulation_baseline -- --ignored --nocapture --test-threads=1
git diff --check
```

The new regression sources are
[assembly characterization](../crates/qx-physics/tests/assembly_diagnostics.rs),
[chord characterization](../crates/qx-physics/tests/chord_characterization.rs),
[initialization checks](../crates/qx-physics/tests/initialization_diagnostics.rs)
and the [opt-in timer](../crates/qx-physics/tests/articulation_baseline.rs).
Known-failure assertions intentionally lock down today's contract; a later
approved solver fix should update their expected result and retain independent
geometry/residual checks, rather than simply deleting the fixture.

Executed results:

- Before instrumentation: the existing physics/document test command passed
  (52 physics tests and 41 document tests).
- Final physics/document tests: **105 passed, 0 failed; 2 timing tests ignored**
  in the ordinary test run. This includes 12 new characterization tests.
- Native Release characterization: **12 passed**, with the same numerical
  fixture outcomes and printed residuals as debug tests.
- Native Release timing command: **2 passed**, explicitly running the two
  ignored performance-observation tests.
- Workspace `cargo check`, strict workspace all-target Clippy, workspace format
  check and `git diff --check`: **passed**.

No existing test assertions or test tolerances were changed. The new endpoint,
seam, tangency and branch fixtures assert explicit current limitations rather
than accepting them as successful solutions. Diagnostic and ordinary results
match on success and failure, and report reuse clears prior observations.

The project schema remains version 7. No document/model/serialization source, project fixtures, dependency, outer
timestep or substep configuration changed. Existing tests cover persistence
round trips, migrations, unknown-field rejection, undo/redo, rigid-body replay,
roll stability, joints, COM conversion, mounts and transactional endpoint failures.
New report types are transient and cannot appear in project JSON.

**Not manually verified:** desktop interaction, rendering, ride cameras, DPI and
perceived playback smoothness. No GUI was controlled or launched for this pass.
There is no changed UI to exercise. An owner can optionally run the existing
application, open an existing train project, inspect/play/pause/reset and
save/reopen it; these observations are not substituted for numerical checks.
Full workspace tests and a new application Release build are outside this
physics-only verification scope; the workspace build check and targeted native
Release tests cover the changed code.

## 9. Prioritized Pass 2 recommendations

**Proceed with an adjusted Pass 2, preserving the general mechanical graph.**
Wheel-level bogie fitting remains a valid missing capability, but it should not
be expected to cure the measured seed, chain-convergence or velocity issues.

1. Separate bounded chord-search robustness from wheel fitting: explicit
   topology/end policy, endpoint arithmetic, narrow/tangential-root handling and
   nearest-local-branch criteria. Preserve bounded work and explicit failure;
   do not silently select another branch or reshape track to obtain a root.
2. Investigate the 16/32-car projection failures using this residual history and
   independently feasible chain fixtures. Evaluate seeding, ordering and
   conditioning before changing budgets or replacing the graph solver. Any
   numerical behavior change needs its own reviewed acceptance criteria.
3. Keep future wheel-level geometric fitting an opt-in initialization adapter
   producing the same rigid component graph/poses. Define wheel reference
   semantics, objective weights, failure thresholds and representative supported
   geometry first. Do not treat fitted wheel locations as wheel-contact physics
   or replace the independent body integration.
4. Schedule nonzero-speed velocity consistency as a separate bounded pass.
   Measure joint/guide/rail `J*v`, initial reaction impulses and energy seating
   before and after any future velocity projection. This M1 implements none.
5. Reuse the current fixtures and unit-specific metrics to establish Pass 2
   acceptance envelopes. Add true shared-bogie mechanisms and seam-aware cases
   with deliberately authored mechanical geometry; do not infer support from
   schema acceptance or from the redundant shared-gear straight fixture.

UX status: no owner-approved workflow was changed; no developer UI was added;
no UX design decision is required for this diagnostic API. Existing UI approvals
and provisional controls retain their prior status.

Git: work is isolated on `codex/train-articulation-m1`, based on `bcc5add`, in a
separate worktree. The original `codex/vscode-rust-environment-cleanup` checkout
and its README / `.vscode` / development-tooling changes are preserved. Work is
intentionally uncommitted; no commit, push, PR or merge is authorized by this
request. Remote state was fetched successfully using Git's Windows certificate
backend for that command; no persistent Git configuration was changed and no
remote changes were merged into either local branch.
