# Train articulation M4D — continuity and station tracking

Date: 2026-11-08. Native Windows x86_64 MSVC, Rust 1.99.1 / LLVM 23.1.1.

M4D establishes a smooth **test-only source-derived rail reference**, reproduces
the original sampled changing-bank failure, and investigates bounded continuation
at 3/6/12/24 m/s. It does not repair generated tracks or replace production
guidance. Higher-speed station tracking and higher-speed integration accuracy are
separate results: retaining the M4C mechanical-quality envelope requires finer
experimental internal steps. Gravity-driven helix energy conservation remains a
known failing characterization.

## Preparation and preservation

Read `AGENTS.md` and the M1, M2A, M2B, M3A, M3B, M4A, M4B and M4C reports.
The requested local M4C checkpoint already existed when this task started:
`0ea81a02dbaa6f404ba5838bb4c086f0e5ef042f`. Its clean worktree and exact six-file
addition were reviewed, and its unchanged Release B1 tests and independent
matrix/reaction verifiers passed before M4D implementation. That verified commit
was reused rather than manufacturing a duplicate empty checkpoint. Generated
evidence is excluded from Git.

The managed worktree-creation call did not return; after checking that no new
worktree had appeared, a separate local Git worktree was created at
`C:/Users/coast/.codex/worktrees/train-articulation-m4d-local/QUANTUM-X`, branch
`codex/train-articulation-m4d`, directly from the checkpoint. The M1 and M4C
worktrees remain intact. The separate tooling checkout retains its modified
README and untracked `.vscode` / development-tooling files. A command-local
Schannel fetch succeeded; `origin/main` remains `59bcba882a31aea8f97613d7bd76945f20dbf6e8`.
No certificate checks or Git configuration were changed.

## 1. Geometric continuity contract

Coordinates remain +X forward, +Y up, +Z lateral, SI/f64. Station is physical
centerline distance, **not** offset-rail arc length. Let `C(s)` be the actual
centerline interpolant, `R(s)` its actual frame rotation, `a` an explicit local
datum offset, and `k` the world angular derivative per station:

```text
P = C + R a
R' a = k cross R a
P' = C' + k cross R a
P'' = C'' + k' cross R a + k cross (k cross R a)

running: w = R Z; b = w cross P'
side:    w = R Y; b = P' cross w
n = b / |b|
F = (x-P) dot P' = 0
D = P' dot P' - (x-P) dot P'' > 1e-8
ds/dx = P'/D
c = (x-P) dot n
g = n + P' * ((x-P) dot n' - P' dot n)/D
J = [g, (Q local_point) cross g]
```

This is the inherited ideal bilateral normal-coordinate **guide chart**. It is
not closest distance to a finite-width contact surface. `g*lambda` is the point
reaction; its magnitude is `|g|*|lambda|`, and its torque is the point lever arm
cross that force. Multipliers are not universally unit-normal force magnitudes.

| Continuity level | What it establishes / what else is required |
| --- | --- |
| Position `C`, physical datum `P` | No positional teleport; insufficient for any velocity/acceleration promise |
| Centerline tangent `C'` | Hermite endpoint tangents agree; this alone says nothing about an offset datum's tangent |
| Full frame `R`, including bank | An orientation-continuous track can still have discontinuous angular rate |
| Angular rate `k` | Together with `C'`, makes `P'` continuous for fixed offsets; special offset/axis cancellations are possible |
| Offset derivative `P'`, support normal `n` | Needed for a smooth stationary projection/chart; a visible C0 join is insufficient |
| `P''`, `n'`, regular `D` | Generic differentiable projection and the existing J in a neighborhood of finite support offsets |
| `P'''`, `n''` | Generic differentiability of J, hence the acceleration equation `J a + Jdot*v = 0` across boundaries |

A useful sufficient upstream contract for this **particular** chart is `C,R` of
class C3, regular nonzero `P'` and `b`, a locally unique branch, and positive D.
Because n itself uses P', C2 position alone does not generally establish a C2
eliminated constraint. Some derivative terms cancel at special exactly coincident
datums; initialization and finite-residual dynamics cannot rely on those
cancellations throughout a neighborhood. These are local smoothness requirements,
not a proof of global nearest-root uniqueness, mechanical feasibility or safety.

The unchanged M4B acceleration checker refines its directional stencil to remain
inside one chart. M4D's analytic rail has **one global smooth chart**, with only
domain endpoints in its transient `track` metadata. This is accurate chart
identity, not hiding interpolation boundaries: this adapter never interpolates
that metadata. The sampled control still exposes all its actual sample pieces.

## 2. Why current interpolation violates the contract

`GeneratedTrack::sample_at_distance` uses cubic Hermite positions and independent
shortest-quaternion frame interpolation. M4C differentiates exactly those
polynomials and both quaternion branches; the audit executes that unchanged
implementation on two-sample restrictions to obtain exact one-sided limits.

Hermite shares position and first derivative at knots, but does not impose
matching second or third derivatives. Independently interpolated frames share
orientation, but generally not their angular derivatives. For a piece of width h
with relative rotation vector r, `u=(s-s0)/h`, and `d=cos(|r|/2)`:

```text
large-angle slerp: k = r/h; k' = 0
small-angle normalized lerp (d > .9995):
N = 1 - 2u(1-u)(1-d)
k = r * 2 sin(|r|/2) / (|r| h N)
k' = k * 2(1-2u)(1-d) / (h N)
```

Even uniform rotations with equal one-sided k have opposite endpoint k' in the
normalized-lerp branch. Curved Hermite position derivatives and interpolated
frame tangents also need not agree between endpoints. Replacing C' by `R X`, or
pretending small-angle interpolation has constant rate, changes the constraint.

The audit traverses every internal boundary in the six 60 m / .025 m fixtures.
Position/frame closure and centerline C1 agreement are at f64 roundoff. Examples
of measured **maximum one-sided jumps** over the full station domain:

| Fixture | `|P'_L-P'_R|` | `|P''_L-P''_R|` | `|k_L-k_R|` | `|k'_L-k'_R|` | `|n'_L-n'_R|` |
| --- | ---: | ---: | ---: | ---: | ---: |
| Straight | 4.44e-16 | 2.22e-14 | 0 | 0 | 0 |
| R=8 turn | 1.33e-14 | 1.465e-5 | 2.22e-14 | 2.441e-5 | 2.31e-11 |
| Changing bank | 2.620e-4 | 5.074e-5 | 4.402e-4 | 8.259e-5 | 9.509e-5 |
| Helix | 6.885e-5 | 1.343e-5 | 1.137e-4 | 1.844e-5 | 7.121e-6 |
| Vertical loop | 1.78e-15 | 1.953e-6 | 2.22e-14 | 2.441e-5 | 2.11e-11 |

Offset `(0,-.08,.6) m`, both support roles; each column is an independent maximum,
not necessarily at the same knot. Straight/inverted lines meet the contract to
roundoff. Turn/loop second-derivative jumps remain real even where the guide
normal has special cancellations. They are not advertised as universally C3.

The audit additionally runs actual `generate` output, using the retained M1
fixtures/default generation settings, without modifying inputs:

| Actual generated fixture | Offset P' jump | Offset P'' jump | k jump | n' jump |
| --- | ---: | ---: | ---: | ---: |
| Straight, 600 m | 6.66e-16 | 3.33e-14 | 0 | 0 |
| R=12 arc, bank .6, 80 m | 2.32e-15 | 1.736e-5 | 4.12e-15 | 2.15e-12 |
| Same, roll rate 0 to .004 rad/m | 3.027e-6 | 1.739e-5 | 5.000e-6 | 5.002e-6 |

Thus fine sampling and viewport smoothness do not supply the needed derivative
contract. Existing topology checks also do not certify derivative-periodic seams.

## 3. The 20.425 m corner, retained

The original bank source is `.02s+.001s²`. The original sampled guide accepts
23 ticks at 6 m/s and rejects the 24th with `Corner`, including under M4D's
speed-aware continuation. Its failing **side** support datum is at 20.425 m:

| Witness | Value |
| --- | ---: |
| Left F | +1.144148137e-7 |
| Right F | -9.907278062e-8 |
| P' jump | 9.891869736e-5 |
| n jump | 3.680735884e-6 |

Squared-distance derivative is `-2F`: this is a genuine corner minimum with no
smooth stationary root between those limits. M4C's independent position-only
golden minimizer and expected failure remain unchanged. Repeated attempts leave
state, every hint and tick count intact. No averaged normals, tolerance inflation,
impacts, branch fallback or owner-geometry edits are introduced.

On the **separate source-derived smooth reference**, the same source bank law
has well-defined derivatives through 20.425 m. A 120-tick gravity run and the
longer 480-tick run pass through it. This does not claim that the original
Hermite/quaternion curve was repaired: the two representations differ between
samples, and the original one still fails.

## 4. Alternative upstream architectures

1. **Source-derived physical datums and jets.** Carry the authoring solver's
   continuous physical position/frame and derivatives into a rail evaluator.
   This preserves authored intent when those derivatives exist, but requires
   section joins, native curves, rider-to-physical conversion, distance inversion
   and seams to share an explicit derivative contract. M4D implements only six
   closed-form development references, not this production pipeline.
2. **Derivative-constrained orientation/position interpolation.** Match rates
   and sufficient higher derivatives, with explicit one-sided source data and
   periodic boundary conditions. Improving orientation alone leaves Hermite C''
   joins unresolved. Cubic rotation splines with continuous angular rate and
   acceleration also do not automatically supply generic third derivatives.
3. **An independently authored physical rail-datum representation.** Give each
   physical support path/surface its own regular derivatives and provenance.
   Centerline, frame and datum must remain compatible; independently fitting them
   cannot silently rewrite authoring intent or produce different initialization
   and dynamic constraints.
4. **Validation/rejection and explicit discontinuities.** Reject unsupported
   smooth-guide intervals and distinguish authored discontinuity from generated
   derivative defects. A discontinuity needs an explicit model/policy; this
   bilateral smooth chart provides no impact law. Rejecting is valid; inventing
   a normal average is not.

For background, the official [SciPy Hermite documentation](https://docs.scipy.org/doc/scipy/reference/generated/scipy.interpolate.CubicHermiteSpline.html)
identifies its C1 contract. [RotationSpline's documentation](https://docs.scipy.org/doc/scipy/reference/generated/scipy.spatial.transform.RotationSpline.html)
describes continuous angular rate/acceleration, while [Slerp](https://docs.scipy.org/doc/scipy/reference/generated/scipy.spatial.transform.Slerp.html)
uses a fixed axis/rate within each interval. No SciPy interpolator or external
coaster implementation is imported. The stronger neighborhood requirements above
are derived from QUANTUM-X's actual residual/J, not asserted by those sources.

## 5. Independently checked geometry and mechanics

New executable files are all under integration tests and optional developer tools:

- `guidance_continuity.rs`: interpolation/source continuity, original failure,
  finite-difference mechanics, rollback, speed and opt-in characterization tests.
- `support/m4d_reference.rs`: third-order scalar derivatives of explicit source
  trigonometric bases; P/P'/P''/P''', n/n'/n'', k/k'.
- `support/m4d_surface.rs`: global analytic chart and bounded projection, plus
  the unchanged M4C sampled control and exact one-sided piece restriction.
- `support/m4d_tracking.rs`: speed-aware transactional continuation.
- `support/m4d_dynamics.rs`: isolated 4/16/64 internal-step experiment; four-step
  state/reports compare **exactly** with the preserved M4C core.
- `support/m4d_characterization.rs`: individual support, body/wheel, reaction,
  station, rigid-dimension, energy and replay observations.
- `tools/verify-train-guidance-m4d.py`: independent position-only derivatives,
  NumPy SVD and LAPACK reactions.

The actual M4B `Model`, position initializer, residual/Jacobian, bearing equations,
velocity initializer, directional Jdot, dense reaction solve and energy function
are compiled unchanged against the new adapter. No initialization/dynamics rail
interpretation differs. Seven 100 kg bodies / isotropic 20 kg m² inertia remain;
the shared graph has eleven bodies and one actual middle bogie. Five scalar
supports per axle and authored bearing frames/limits are unchanged.

Geometry checks cover straight, R=8 turns, changing bank, helix, full vertical
loops/inversions, both support roles, zero and nonzero longitudinal/normal/lateral
offsets, old sample boundaries (including 20.425 m), and explicit periodic seams.
Analytic endpoint jets agree through P'''/n''/k' within 2e-14 at turn/loop seams.
Bidirectional unwrapped projection traverses more than a lap. An authored abrupt
roll-rate change exposes P'/n/k jumps; .5 mm or .5 mrad circuit gaps are rejected.
Inputs remain unchanged.

Independent NumPy verification uses explicit sine/cosine positions and bases,
complex-step first derivatives, and overdetermined degree-12 position/normal
polynomials on a .2 m stencil. It consumes no Rust derivative code, quaternion
interpolator or Newton projector. All **216** cases pass: maximum errors are
2.85e-14 for P, 1.30e-13 for P', 9.53e-12 for P'', 1.54e-9 for P''', 1.13e-12
for n'', and 1.18e-15 for k'. The tolerances are 2e-8 for P''' and 2e-9 for
the other emitted quantities. Rust independently differences position, normal,
rate and derivative jets at .001/.0001 m.

All residual/J columns are checked at closed and .002-scaled displaced poses
against position-equation differences (3e-7 gate). Directional acceleration
refinement retains 1e-6; independent second differences of c retain 2e-5.
Eighteen reaction systems cover all six references at the former corner and near
loop inversion, plus each shared graph. They have rank 41/42 or 65/66 at SVD
thresholds 1e-5/1e-7/1e-9. Independent loads, acceleration equations, ideal
bilateral power and force/lever-arm directions pass their existing gates.
Maximum independent load discrepancy is 1.188e-11 N/N m, acceleration residual
2.504e-13 in each equation's unit, and ideal bilateral reaction power 1.513e-11 W.
Straight/inverted support totals are respectively +/-6867 N (seven bodies) or
+/-10791 N (eleven). Tensile reactions remain visible.

## 6. Bounded speed-aware continuation

The M4C .04 m innovation gate already excludes constant 12 m/s travel
(`12/240=.05 m`). It is a search/tracking limit, not a physical speed limit.
For each actual rigid support point at the previously accepted station, M4D uses:

```text
u = v_COM + omega cross (x-COM)
rate = P' dot u / D
motion = abs(rate) / 240
innovation = 1.25 * motion + .002 m
half_window = innovation + .010 m
```

The .002 m is **station prediction uncertainty**, not support clearance or a
relaxed mechanical gap. The extra .010 m is search guard space. Stations are
still reprojected from actual rigid poses; no station prescription moves a body.
Both total innovation and signed prediction error are checked after the core.
Windows remain centered on the previous accepted station, not a predicted remote
target. They must be at most .20 m half-width, and
`(|P''|/|P'|+|k|)*half_window <= .15`. This is a **local turning estimate**, not
a certified supremum over arbitrary unseen geometry. At nominal 3/6/12/24 m/s,
half-windows are .027625/.04325/.0745/.137 m before local geometry corrections.
The speed-independent cap implies about 36.1 station-m/s maximum even on a line;
48 m/s is explicitly rejected. This is a tested envelope, not a universal cap
or guarantee for every configuration below it.

Analytic projection scans bins at most .025 m wide, at most 128 bins over a
1.6 m half-window for initialization, with 48 safeguarded Newton iterations per
bracket. Continuation's .20 m cap needs at most 16 bins per query. F remains below
2e-13 and D above 1e-8; window edges/endpoints, singular normals and every
distinct competing local minimum are failures, including unequal-distance laps.
This is sampled local root enumeration, **not certified interval root isolation**.
It cannot promise arbitrary narrow-root detection or global branch uniqueness.

Candidates include state, stations and continuation observations. Windows are
restored on every exit; state/hints/ticks/previous envelopes commit together only
after all post-step gates. Tests prove late rollback after a successful full core
step: at zero initial speed, a real 500 m/s² longitudinal test acceleration moves
5.4 mm, within the search window but outside the 2 mm innovation gate. No retry
widens the window. Invalid inputs, excessive speed, ambiguity, endpoints and
original corners retain explicit failures. Seek/edit/reset and geometry identity
invalidation remain future production-adapter work.

## 7. Dynamic characterization and supported accuracy

The ordinary sweep attempts 24 ticks on each of the six continuous references
at 3/6/12/24 m/s with gravity disabled. Every run is independently initialized
twice; complete states, reports, hints and continuation envelopes agree exactly.
Support stations are independently checked against position-only local golden
minimization, without F/D/J/Newton, within 3e-7 m.

Four internal substeps retain the M4C equations and solver acceptance gates, but
unrefined 12/24 m/s curved runs fail its **tighter quality** gates of 2e-5 for
position rows and .003 for velocity rows. This expected limitation is retained
as executable evidence. For example, an unrefined 24 m/s loop reaches about
1.98e-4 position / .0375 velocity: correct station selection is not sufficient.

The explicit experimental comparison uses 4 steps at <=6 m/s, 16 at 12 m/s and
64 at 24 m/s. This approximately follows velocity-squared curvature acceleration
when bounding velocity error per internal step. It is a declared fixture policy,
not an adaptive production integrator. Every refined run meets the original
2e-5/.003 quality envelope; 32 impulse sweeps, stabilization .2 and the
**1/240 s outer step** remain unchanged. More internal work is not hidden in
performance claims. Gravity-free instantaneous curvature loads scale with speed
squared (comparison within .003 N/N m at the highest speed).

Maximum observations across the six 24-tick gravity-free fixtures at each speed:

| Initial speed m/s | Internal steps | Max position row | Max velocity row | Max energy drift J | Max accepted innovation bound m |
| --- | ---: | ---: | ---: | ---: | ---: |
| 3 | 4 | 3.092e-6 | .0005860 | .002452 | .01764 |
| 6 | 4 | 1.237e-5 | .002344 | .03924 | .03328 |
| 12 | 16 | 3.092e-6 | .002344 | .03924 | .06459 |
| 24 | 64 | 7.731e-7 | .002344 | .03924 | .12725 |

Per-run raw output retains every support gap and velocity row; each body's full
orientation relative to an independently nearest centerline frame; every running
diagnostic wheel's own projected center offset and rolling-axis error; bearing
closure/angular speed; actual support-point force magnitude; tensile running
multipliers; accumulated outer-tick impulses; station errors; rigid distances;
speed range and energy drift. These are not extra alignment constraints. The
.18 Y wheel-center convention applies only to the three running diagnostics per
axle, never the side-guide datums. Full-body alignment is sampled every 24 ticks
and at termination; support/wheel observations are taken every tick.

Native Release long runs (all have exact independent replay):

| Run / gravity | Outer ticks / time | Internal steps | Travel m | Max position row | Max velocity row | Max energy drift J |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Bank, initial 6 m/s / gravity | 480 / 2 s | 4 | 12.17607443 | 1.417e-5 | .002522 | .03619 |
| Helix, initial 6 m/s / gravity | 480 / 2 s | 4 | 6.043330758 | 1.093e-5 | .001954 | **6.26894** |
| Closed loop, 12 m/s / none | 1200 / 5 s | 16 | 60.00342266 | 3.095e-6 | .002344 | .03922 |
| Closed turn, 12 m/s / none | 1200 / 5 s | 16 | 60.00734453 | 3.067e-6 | .002344 | .03926 |
| Shared-carrier bank, 6 m/s / none | 480 / 2 s | 4 | 12.02517461 | 1.259e-5 | .002386 | .06084 |
| Closed loop, 24 m/s / none | 600 / 2.5 s | 64 | 60.00338720 | 7.738e-7 | .002344 | .03922 |

Each closed run starts at lead station 45.0137 m and advances farther than the
50.26548 m circumference, crossing actual modulo seams with the whole consist.
This establishes seam traversal only for these globally periodic references,
not for production sampled-track seams or arbitrary source joins. Full loop
traversal here is gravity-free; it does not establish a gravity-driven loop with
energy-consistent production trains.

Linear rows use m/m/s and angular rows rad/rad/s. Maxima compactly compare their
numeric magnitudes; every row is individually gated. Largest independently
measured station discrepancy is 6.44e-9 m; support-owner rigid-distance discrepancy is below
3.0e-15 m. Largest bearing-coordinate gap is 3.10e-6 m. Minimum running reaction
in the 12 m/s closed turn is **-1608.36 N**; bank/helix also retain tensile rows.
The 24 m/s loop reaches about 6413 N maximum point-support force and 26.72 N s
maximum accumulated outer-tick multiplier. No physical wheel-load limit follows.

Geometry alignment remains distinct from closure. The bank run has up to
.003269 m diagnostic wheel-center error / .10554 rad rolling-axis error and
.12496 rad full-body frame misalignment, despite small support gaps. The helix
has .0005434 m / .01400 rad wheel errors. A loop has about .0001964 m center
error and .01535 rad maximum rolling-axis error. These are the declared rigid
mechanism's observations, not quantities silently optimized away.

The M4C conservation gate is retained: `max(.005 J, 1e-5 * initial kinetic
energy)`. All declared runs except the gravity helix pass it. That helix has a
.12260 J allowance and **fails**, explicitly asserted as an expected failure;
it is not accepted as an energy-conserving result. Its station-speed range is
.05816–6.00232 m/s; the gravity-bank range is 5.99104–6.20141 m/s. Thus changing
speeds exercise continuation without a station trajectory. The helix energy
defect needs a separate integrator/stabilization investigation; source continuity
does not cure it. No claim of general dynamic energy fidelity is made.

## 8. Complete-step performance and helix variability

Host verified as Intel Core i7-14700KF. The final unpinned serial profiler uses
one warm-up and 35 individual first-tick measurements, resetting to the same
valid state outside each timer. Construction, initialization, reset, compilation
and logging are excluded. The timer includes continuation-envelope construction,
all transactional copies, the entire 4/16/64-step core, contract/curvature
evaluation, 32 sweeps per internal step, final station checks, window restoration
and commit. These initial profiling ticks use gravity `(0,-9.81,0)`.

Milliseconds per complete successful outer tick, median [minimum, maximum]:

| Speed / internal steps | Straight | Changing bank | Helix |
| --- | --- | --- | --- |
| 3 m/s / 4 | .350 [.331,.456] | .799 [.779,.844] | .775 [.768,.805] |
| 6 m/s / 4 | .374 [.358,.410] | .854 [.841,.892] | .847 [.827,.928] |
| 12 m/s / 16 | 1.649 [1.595,1.747] | 3.952 [3.900,4.149] | 3.868 [3.798,4.009] |
| 24 m/s / 64 | 8.395 [8.252,8.663] | 21.261 [20.950,21.752] | 20.810 [20.478,21.254] |

The preceding separate serial run measured 4.090 ms median / 4.378 ms maximum
for the 12 m/s bank and 3.915 / 4.189 ms for the helix. The pinned helix run
measured 4.181 / 4.634 ms. Therefore the 12 m/s cases near the **4.167 ms**
interval do **not** establish a reliable deadline. The refined 24 m/s cases
clearly exceed it. Even a straight line is expensive here because the fixture
policy deliberately applies the same 64-step research refinement. Geometry-aware
error control is future work, not an optimization silently enabled in these data.

The explicit late-rollback case also times a complete attempt: the zero-speed
straight core finishes under a longitudinal test acceleration, and the post-step
innovation check rejects it. Median **.266 ms [.260,.284]**, including copies,
candidate dynamics and restoration. State/hints/ticks are checked unchanged
outside each timer. This is one measured late-failure case, not a worst-case
abort-time bound.

The unchanged M4C profiler and long-run fixtures were rerun. Its original
complete-wrapper medians were 2.092/3.858/3.608 ms for straight/bank/helix in the
unpinned pass; its former report remains unchanged. Its helix sparse core was
3.439 ms. The large timing changes from M4C's historical table affect multiple
phases and fixtures, not just helix continuation. M4C's numerical long-run
values reproduce (e.g. turn energy .03923849 J), despite different wall times.

For a separate helix investigation, M4D runs the **unchanged sampled M4C core and
wrapper**, alternates which is timed first, and repeats three rounds. It uses
identical reset states and station windows, checks deterministic jet counts in
every sample, and times the whole wrapper. In the final unpinned rounds:

| Round | Core median ms | Wrapper median ms | Median paired extra ms |
| --- | ---: | ---: | ---: |
| 0 | 2.048 | 2.163 | .120 |
| 1 | 2.032 | 2.157 | .121 |
| 2 | 2.057 | 2.177 | .122 |

Core work is always **12,120 jets**; wrapper work **12,732**, the extra 612 jets
coming from final station projection. There is no intermittent extra root search
or changed support configuration in these samples. The same M4C equations are
compiled through M4D's sampled-control adapter, so comparing different binaries
alone cannot isolate code generation from execution placement.

To constrain placement, both Release executables were separately launched
noninteractively with their child-process affinity mask verified as **1**, i.e.
logical CPU 0. `Start-Process -WindowStyle Hidden` and the process-only
`ProcessorAffinity` property were used; user applications and system settings
were not changed. Affinity was applied before the later helix phases. Original
M4C helix core/wrapper became **2.080/2.238 ms**. M4D's paired sampled-control
rounds measured cores **2.160/2.077/2.080 ms**, wrappers **2.235/2.197/2.158 ms**,
and paired extra medians **.087/.108/.093 ms**. Outliers remain, including a
4.053 ms core sample; a negative paired difference can occur when one run is
interrupted and does not mean negative wrapper work.

Much of the cross-executable discrepancy disappears on the same logical CPU.
This is evidence that execution context contributes to the observed variation,
not proof of a particular P/E-core assignment, CPU-frequency or scheduler cause.
The formerly large helix-specific wrapper surcharge is not reproducible as
deterministic extra query work. No affinity policy is proposed for the application.

No other test/compile job from this pass overlapped these timing runs. They
remain observations on an interactive shared machine, not isolated hardware
benchmarks, CI limits, production FPS or hard real-time guarantees. The new
analytic costs describe a **different, explicitly continuous representation**
and smaller speed-aware windows; they must not be called equivalent performance
improvements on the original discontinuous sampled curve. Numerical tolerances
were not weakened to meet a timing target.

## 9. Verification and reproduction

Run from the M4D worktree root. Evidence goes into ignored `target/m4d/`.
This pass reused the M4C worktree's Cargo target cache through command-local
`CARGO_TARGET_DIR`; neither the old source nor its prior evidence was removed.
The commands work with a separate default target directory as well.

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --release -p qx-physics --test articulation_baseline --test assembly_convergence --test assembly_diagnostics --test chord_characterization --test initialization_diagnostics --test wheel_geometry --test bogie_fitting --test mechanical_guidance --test rail_normal_guidance --test guidance_robustness --test guidance_continuity -- --nocapture --test-threads=1 > target/m4d/characterization.log 2>&1
cargo test --release -p qx-geometry --test rail_geometry
python tools/verify-train-guidance-m4a.py target/m4d/characterization.log
python tools/verify-train-guidance-m4b.py target/m4d/characterization.log
python tools/verify-train-guidance-m4c.py target/m4d/characterization.log
python tools/verify-train-guidance-m4d.py target/m4d/characterization.log
cargo test --release -p qx-physics --test articulation_baseline --test assembly_convergence --test wheel_geometry --test bogie_fitting --test rail_normal_guidance --test guidance_robustness -- --ignored --nocapture --test-threads=1
cargo test --release -p qx-physics --test guidance_continuity long_continuous -- --ignored --nocapture --test-threads=1
cargo test --release -p qx-physics --test guidance_continuity complete_step_costs_and_helix_variability -- --ignored --nocapture --test-threads=1
git diff --check
git diff --cached --check
```

Executed results:

- Full workspace: **498 passed, zero failed, 13 ignored**. All inherited
  M1–M4C regressions remain intact. The ignored tests are twelve numerical
  characterization/timing cases and the inherited GPU evidence exporter.
- Native ordinary Release characterization: **88 passed, zero failed**:
  79 physics tests and nine rail-geometry tests. Twelve opt-in physics cases
  remain ignored by the ordinary command.
- **All twelve** opt-in numerical cases explicitly passed in Release: ten
  retained M1–M4C cases, M4D's long runs, and M4D's complete-step profiler.
  The GPU exporter was not explicitly executed.
- Independent NumPy verification: **38 M4A matrices, 29 M4B matrices, six M4C
  reaction systems, 18 M4D reaction systems, and 216 M4D geometric cases pass**.
- Formatting, workspace check, strict all-target workspace Clippy and Git
  staged/unstaged whitespace checks pass. Final Clippy and targeted debug
  refinement/geometry checks were rerun after adding the explicit unrefined
  accuracy characterization and the finite-station guard. The full workspace
  run had started before those last assertions/guard; completed Release
  characterization and long-run evidence use the finished geometry/dynamics/
  tracking implementation. The final profiler (including late rollback) was
  executed separately, followed by final format/check/strict Clippy checks.
- All **480 inherited Git-tracked files** are unchanged. Exactly eight new
  files are added; no earlier report, fixture, assertion, tolerance, library
  export, dependency or production implementation is modified. The original
  tooling checkout and M1/M4C worktrees retain their original source/status.

The expected geometry corner, unrefined high-speed quality failure and
gravity-helix conservation failure are assertions of known limitations, not
accepted physical results or disabled failing regressions. No run was reported
as successful merely because a matrix solve/build returned success.

Primary local evidence: `workspace.log`, `characterization.log`, `rail.log`,
`independent.log`, `m4a-independent.log`, `m4b-independent.log`,
`m4c-independent.log`, `long-release.log`, `retained-opt-in.log`,
`profile-serial.log`, `profile-rollback-final.log`, `profile-core0-control.log`, `profile-core0-research.log`,
and the final debug/fmt/Clippy logs. Baseline reruns are in the M4C worktree's
ignored `target/m4d-baseline/`. Generated logs/build products are not committed.

## 10. Production blockers and M4E scope

Production `GeneratedTrack` does not yet provide the generic C3 physical
datum/frame contract required here. It needs source-derived derivatives or an
explicit derivative-constrained representation, validation of native/section
joins and physical-reference conversion, and periodic jets at authored seams.
No automatic resampling/smoothing fixes that contract. Physics must reject or
explicitly model discontinuities rather than interpreting them as smooth guides.

M4E should first investigate a bounded **source-derived physical geometry
adapter**, with independent one-sided derivatives, chart identity, explicit
discontinuity rejection and geometry-edit invalidation. Then investigate analytic
constraint curvature, better energy behavior and bounded error-controlled
internal stepping against unchanged references and residuals. A higher-speed
integrator needs complete-step profiling; it must not trade numerical correctness
for the 4.167 ms interval. General masses/inertia, longer graphs, active bearing
bounds and source curves/joins require separate evidence.

Production support metadata, topology-aware whole-consist initialization,
reset/snapshot adapters, certified branch handling and realistic tread/rail
surface definitions still block production integration. Bilateral supports can
pull; they are not unilateral wheel contact, up-stop engagement, impact, friction,
wheel spin, suspension compliance or ride-safety/manufacturer validation.

## UX, manual verification and Git scope

No approved UI/interaction behavior changes, no temporary developer UI, and no
new UX design is finalized. Future physical metadata/diagnostics presentation
still needs human design. No desktop application was launched or controlled.
Rendering, cameras, DPI, interactive playback and a new application Release
package are **not manually verified**. Optional owner regression checklist:
`cargo run -p quantum-x --release -- edit`; open an existing interior train
project, Inspect/Initialize, Play/Pause/Reset, then save/reopen. M4D adds no
editor feature for that checklist to demonstrate.

All new executable code is isolated research. No production source, dependency,
schema, M2B assembly limit, prior report/test/assertion or production timestep is
changed. Work remains local; no push, merge, PR or external message is authorized
or performed.
