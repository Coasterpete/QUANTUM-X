# Train articulation M4B — rail-normal guidance

Date: 2026-11-07. Native Windows x86_64 MSVC. Existing isolated worktree
`C:/Users/coast/.codex/worktrees/train-articulation-m1/QUANTUM-X`, branch
`codex/train-articulation-m1`, HEAD `bcc5add`. All accumulated M1–M4A work is
preserved. No production implementation, dependency, document schema or UI changes.

**Decision: the explicitly authored B1 bilateral mechanism has consistent
curved/banked position, velocity and acceleration equations and admits bounded
rigid-body impulse experiments. This is evidence for further isolated research,
not authorization to replace production guidance or a validation of wheel contact.**
One changing-bank experiment stops at an interpolation corner; tensile running
reactions, nonzero wheel errors and energy drift remain visible. B1 is not
uniformly better aligned or better conditioned than B0.

Preparation included `AGENTS.md`, the [M1](train-articulation-m1-baseline.md),
[M2A](train-articulation-m2a-chord-robustness.md),
[M2B](train-articulation-m2b-convergence.md),
[M3A](train-articulation-m3a-wheel-geometry.md),
[M3B](train-articulation-m3b-bogie-fitting.md) and
[M4A](train-articulation-m4a-mechanical-guidance.md) reports; the physical rail
sampler, document mechanisms, assembly/COM conversion, guidance, angular joints,
rigid-body integration, initialization and prior independent research fixtures.
No external coaster code, GLB geometry or generic wheel markers are used.

## 1. Implemented experimental mechanical model

All new executable code is **test-only**, renderer independent and transient:

- [rail_normal_guidance.rs](../crates/qx-physics/tests/rail_normal_guidance.rs):
  geometry, finite poses, derivatives, mobility, forces, replay and timing tests.
- [m4b_surface.rs](../crates/qx-physics/tests/support/m4b_surface.rs): bounded
  rail projection and exact interpolation jets for authored support datums.
- [m4b_contract.rs](../crates/qx-physics/tests/support/m4b_contract.rs): one
  residual/Jacobian contract, explicit carrier ownership, bearing frames,
  initialization and bounded dense reference reactions.
- [m4b_dynamics.rs](../crates/qx-physics/tests/support/m4b_dynamics.rs): standalone
  rigid-body sequential impulses, velocity initialization and transactional steps.
- [verify-train-guidance-m4b.py](../tools/verify-train-guidance-m4b.py): independent
  NumPy SVD and mass-weighted conditioning; optional developer tooling only.

The conventional graph reuses M4A's independently constructed B0 seed: one car,
two rigid bogie frames and four rigid axle carriers, seven bodies total. Each
bogie bridges two axle-bearing origins 0.6 m apart. Its front bearing is a
RollLock, its rear bearing a Ball. The car bridges bogie pivots 2.4 m apart with
one front RollLock and one rear Ball. Conventional car anchors are
`(+/-1.2,-0.8,0) m`. These are explicitly constructed synthetic mechanics, not
an assertion that every conventional or manufacturer's bogie has this topology.

Body origins are COMs in this experiment. Support points belong to their actual
axle carrier; bogie/car grouping supplies no extra body or degree of freedom.
Bearing anchors, both local bearing rotations, support frames and diagnostics
are distinct. Offset bogie pivots are explicitly `(0.05,0.12,0.03) m`.
Identity bearing rotations define the primary fixtures; a separate Jacobian
test uses nonidentity rotations on both sides.

Each axle owns these five scalar bilateral supports:

| Role / face identity | Carrier-local points, m | Explicit rail-datum offsets, m | Initial normal |
| --- | --- | --- | --- |
| Running / `running-plane` | `(-.12,0,-.6)`, `(.12,0,-.6)` | `(0,0,-.6)`, rail `left` | +Y |
| Running / `running-plane` | `(0,0,.6)` | `(0,0,.6)`, rail `right` | +Y |
| Side / `side-plane` | `(-.12,-.08,.6)`, `(.12,-.08,.6)` | `(0,-.08,.6)`, rail `right` | +Z |

The 0.24 m footprint is newly declared for these axle modules. It is not M4A's
single-body 0.6 m straight prototype retroactively relabeled. B0 and B1 use the
**same** new mechanism, attachment points, supports and diagnostic wheel offsets.
B0 retains its full-attitude guidance equations as the control. B1 replaces
only that test model's axle guidance with these five normal equations.

Rail/face identity, owning body, local point, surface role, initial station and
bounded neighborhood are explicit. The main support windows remain within
`seed +/-0.8 m`; station is eliminated geometrically and has no mass or prescribed
motion. Research limits are 16 bodies, 64 supports, 128 dense rows and 96 velocity
coordinates. Bearer relative-rotation magnitude must stay strictly below the
declared synthetic 0.7 rad envelope. Reaching it is `BearingLimit`, not a new
equality, free hinge or softened constraint. This prototype does not implement
active hinge-limit impulses; retained M1/M3B limit tests remain unchanged.

## 2. Rail-normal residual and Jacobian derivation

Coordinates are +X forward, +Y up, +Z lateral, metres/radians/seconds, f64.
`s` is the physical centerline station, not generally the arc length of an
offset support path. The existing generated-track interpolation defines `C(s)`
by cubic Hermite endpoint positions/tangents and `R(s)` by shortest quaternion
interpolation. Let `k` be its **world** angular derivative per station:

```text
R' a = k cross (R a)
P(s)  = C(s) + R(s) a
P'    = C' + k cross (R a)
P''   = C'' + k' cross (R a) + k cross (k cross (R a))
```

Hermite derivatives are evaluated from the actual cubic coefficients. For the
small-angle normalized-lerp quaternion branch, with `u=(s-s0)/h`, rotation
vector `r`, `d=cos(|r|/2)` and `N=1-2u(1-u)(1-d)`:

```text
k  = r * 2 sin(|r|/2) / (|r| h N)
k' = k * 2(1-2u)(1-d) / (h N)
```

The zero-angle limit is handled explicitly. Larger-angle slerp uses `k=r/h`
and `k'=0`. Substituting the frame tangent for `C'` or differentiating nlerp as
constant-rate slerp would give a different constraint.

Running support uses authored moving width `w=R Z`; side support uses `w=R Y`.
The oriented datum normals and derivatives are:

```text
running b = w cross P'; side b = P' cross w
n = b / |b|; w' = k cross w
n' = (b' - n (n dot b')) / |b|
```

These normals follow offset-path curvature, changing banking and inversion.
They are not constant world directions or unconditionally `R Y` / `R Z`.

For rigid support point `x = p + Q l`, choose a bounded local closest-datum
station by `F=(x-P) dot P'=0`. Write `delta=x-P` and
`D=P' dot P' - delta dot P''`. At a regular interior local minimum:

```text
ds/dx = P' / D
phi   = delta dot n(s(x))                         [m]
g     = n + P' * (delta dot n' - P' dot n) / D
J     = [g, (Q l) cross g]
phi_dot = J v                                    [m/s]
```

`g/|g|` is the constraint's oriented normal. At nonzero transverse offset even
a zero `phi` can have a station-coupled term. Freezing `n`, dropping `ds/dx`,
or applying only a rail torque would fail the displaced-pose derivative tests.
Generalized reaction is `J^T lambda`; a support multiplier has units N, but the
world point force is **g * lambda**, not always a unit-normal force of magnitude
`lambda`. Its normal component along `n` has that sign because `n dot g=1`.

**Surface interpretation:** `phi` defines an ideal moving normal-coordinate
guide chart about explicitly authored rail/support datums. It is not exact
closest distance to a finite-width swept tread/rail surface. At finite width,
that contact surface needs its own two-coordinate/profile projection and normal
derivative. The guide's exact normal is `g`, rather than an assertion that the
datum normal `n` is the physical contact normal everywhere. No real tread,
clearance, radius or contact patch is inferred. This limits physical claims,
while retaining a mathematically identical guide for assembly and dynamics.

### Bearings and B0 control

Every bearing has three anchor equalities, each with opposite point Jacobians
on the two owning bodies. RollLock adds the same swing/twist X-angle residual
and exact quaternion twist gradient used in M4A, transformed by the authored
local bearing frames. Ball adds no angular equality. Relative anchor offsets
remain inside the rotational columns; they are never fitted as loose points.

B0 constrains two normal coordinates of `p-C(s(p))` and three components of
`2 vector(Q R(s)^-1)`. This is a local full-attitude chart, not wheel contact.
With quaternion error `(w,z)`, its derivative is
`w(omega-k ds/dt) + (omega+k ds/dt) cross z`. It includes rail orientation's
linear-velocity contribution. It rejects the remote attitude chart rather than
claiming global quaternion coordinates. Both models have five guide rows per
axle, not different equation counts.

### Acceleration and units

The shared contract is `c=0`, `Jv=0`, `Ja + Jdot*v=0`. `Jdot*v` is a centered
directional difference of the **same** J along spatial rigid-body perturbations;
this is research tooling, not an analytic production Hessian. The step is halved
up to twelve times to keep both evaluations in the same interpolation pieces.
Independent second differences of **c**, not J, and step refinement check it.
Linear rows have m, m/s, m/s²; angular rows use local radians, rad/s, rad/s².
Compact maxima in raw logs compare numerical magnitudes only; each equation's
assertion applies in its own unit. No mixed-unit RMS establishes closure.

## 3. Bounded projection, topology and failure contract

Projection enumerates descending roots of F over 64 local bins and uses 40
bisections per bracket. It selects a regular interior minimum, rejecting
`D<=1e-8`, degenerate normals, window edges, open endpoints and equally close
separate roots. It is **sampled local enumeration**, not certified detection of
every root or a global nearest-rail algorithm. Narrow unresolved minima can be
missed; certified/adaptive branch continuation remains a production gate.

Open geometry never wraps. Circuit sampling requires explicit Circuit intent,
existing topology validation and tighter 1e-9 m/rad seam closure in this adapter.
A topology-tolerated 0.5 mm seam gap is rejected. Circuit station results stay
unwrapped. A complete-circle query across the seam passes; full-consist seam
initialization and seam-crossing impulse traversal are not established here.

One-sided jets are compared near projection targets. Derivative/normal jumps
above 1e-6 are `Corner`; no world-up reset, averaged bank frame, invented smooth
curve or silent distant-branch selection repairs them. The short impulse study
can cross other interpolation knots using their actual piece equations; that
does not validate an impact/contact rule for discontinuous geometry.

Failure variants distinguish invalid input, endpoint, inaccessible local target,
ambiguous target, corner, singular local/reaction system, bearing envelope,
failed nonlinear convergence and failed dynamic residual envelope. They do not
turn numerical nonconvergence into a proof of global mechanical impossibility.
The initializer works on copies and returns no failed candidate; an entire outer
dynamic step, including late failures, rolls back poses and velocities.

## 4. Independent geometric and finite-configuration validation

Straight/inverted lines, R=8 m horizontal turns, changing bank
`.02s+.001s²`, helices with rise `.3 m/m` and bank `.4+.04s`, and R=8 m vertical
loops are sampled at 0.025 m. Main lead station is 20.0137 m; dedicated loop
witnesses use near-top and near-vertical stations. These are synthetic research
fixtures, not authored production coasters. The station offset keeps ordinary
local differentiability tests away from intentional interpolation knots.

Validation is independent in several ways:

1. Existing M3A `PhysicalRailGeometry` agrees with new P/P' within 1e-12.
   Position, tangent derivative and normal derivative are independently
   differenced at 1e-5 and 1e-6 m, with a 2e-8 gate.
2. Explicit trigonometric basis vectors check geometry without quaternion/frame
   transformations. Solved B1 poses are measured against analytic tracks using
   an independent 90-iteration golden-section station search. Maximum analytic
   support gaps: turn `1.724e-12 m`, bank `1.004e-7 m`, helix `5.026e-10 m`, loop
   `1.952e-12 m`, inverted `7.348e-17 m`. The analytic/interpolated difference is
   separate from numerical closure, gated at 2e-6 m.
3. Full B0/B1 J matrices are checked against independent centered differences of
   all position equations at 1e-5/1e-6, both closed and displaced by 0.002-scaled
   metre/radian perturbations. Largest observed discrepancy is about 1.6e-9;
   the gate is 3e-7. Nonidentity bearing frames have a separate 1e-8 gate.
4. Twenty-four independent finite B1 assemblies per turn/bank/helix/loop sample
   stations 5.0137 through 48.7137 m. Worst closed residuals are respectively
   `2.621e-15`, `4.511e-15`, `2.848e-13`, `2.460e-15` in each row's unit.
   This samples almost a full horizontal turn and a loop across inversion;
   it is not a proof of continuous traversal between samples.

M4A's independent chord/bearing construction supplies seeds, not solutions to
B1. A bounded dense minimum-norm Newton correction then solves the **same** c/J
used by impulses: at most 20 iterations, 0.2 coordinate step cap, four line-search
scales, no rank-restoring damping and no new physical longitudinal lock. Every
returned individual residual is below 2e-11 in its own unit.

## 5. B0 versus B1 on identical support geometry

Maximum individual support gaps, diagnostic center separations and directed
rolling-axis errors are separately measured. The declared diagnostic centers
are each running support point plus carrier-local `0.18 Y m`; their target
loci use the same explicit offset in the track frame. This is a synthetic
radius/reference convention, not a contact-law assertion. There are three
running diagnostics per carrier. These are **not** M4A's eight x=0 wheel-center
diagnostics, so the historical tables must not be compared as the same wheels.

| Fixture | B0 gap / B1 gap, m | B0 center / B1 center, m | B0 rolling / B1 rolling, rad | B0 / B1 Newton iterations |
| --- | --- | --- | --- | --- |
| Straight | 0 / 0 | 1.066e-14 / 1.066e-14 | 0 / 0 | 0 / 0 |
| Tight turn | 9.729e-4 / 2.469e-15 | 8.372e-4 / 9.730e-4 | .013953 / .013954 | 1 / 2 |
| Changing bank | 4.786e-3 / 2.001e-15 | .005203 / .002065 | .050264 / .075378 | 1 / 3 |
| Helix | .001047 / 2.129e-13 | .001131 / .000539 | .016218 / .013996 | 1 / 2 |
| Loop top | .000900 / 1.790e-15 | .000921 / .000196 | .015344 / .015346 | 1 / 2 |
| Offset pivots, helix | .001047 / 2.128e-13 | .001131 / .000540 | .016218 / .013996 | 1 / 2 |
| Shared middle bogie, bank | .004786 / 1.422e-15 | .005203 / .002065 | .050264 / .075378 | 1 / 3 |

Both mechanical anchor and angular equalities close below 2e-11 individually.
Maximum relative bearing attitude is about .151 rad in the tight turn and
.206 rad in the changing-bank graph, inside the explicit .7 rad envelope.
The near-vertical loop, inverted and suspended witnesses also pass.

B0 forces the whole axle frame to equal the track frame at its origin. B1
lets its authored distributed support normals determine attitude, with travel
still coupled to the bearings. B1 closes its support equations more accurately
than B0's origin-attitude assumption, but has **worse** center error in the tight
turn and worse rolling-direction error on changing bank. Zero normal gap is not
zero wheel-center or no-slip error. Steering, tread geometry or lateral freedom
must be mechanically specified before making those additional promises.

## 6. Mobility, rank and conditioning

Twenty-nine emitted matrices are independently checked by NumPy SVD at absolute
thresholds 1e-5, 1e-7 and 1e-9. Angular columns use a **1 m characteristic length**;
these are metre-scale fixture results, not scale-independent rank certificates.

| Graph / restriction | Rows / body coordinates | Rank | Mobility / dependent rows |
| --- | --- | --- | --- |
| Conventional B0 or B1 | 41 / 42 | 41 | 1 travel / 0 |
| Suspended, rear/front Ball car bearings | 40 / 42 | 40 | 2, travel and car swing / 0 |
| Two cars, one shared middle bogie | 65 / 66 | 65 | 1 travel / 0 |
| All conventional car columns fixed | 41 / 36 | 36 | 0 / 5 restricted zero/dependent rows |
| One B1 carrier | 5 / 6 | 5 | 1 travel / 0 |
| Same, zero or 1e-10 m half-footprint | 5 / 6 | 3 numerical | 3 / 2 |
| Same, zero gauge | 5 / 6 | 4 | 2 / 1 |
| Conventional with duplicated or opposed support | 42 / 42 | 41 | 1 / 1 |
| All carrier footprints collapsed | 41 / 42 | 33 | 9 / 8 |

The tiny-footprint rank is numerical deficiency at these thresholds, distinct
from the nonzero-footprint mathematical rank. A dependent row makes individual
reaction multipliers nonunique; the dense reference solver rejects it. Two
coincident supports assigned parallel planes .01 m apart are independently
incompatible: at least one gap is .005 m or larger in every pose. This geometric
proof is separate from the initializer's singular/nonconvergent outcome.

Typical retained singular-value ratios of J and condition numbers of
`J M^-1 J^T`, respectively:

| Fixture | B0 J / Gram | B1 J / Gram |
| --- | --- | --- |
| Straight | 8.751 / 87.60 | 15.182 / 156.67 |
| Tight turn | 8.893 / 100.61 | 15.204 / 157.92 |
| Changing bank | 8.890 / 100.31 | 15.390 / 162.06 |
| Offset helix | 9.245 / 121.14 | 15.523 / 164.58 |
| Shared bank | 9.166 / 178.66 | 15.820 / 176.81 |

Mass weighting uses 100 kg / isotropic 20 kg m² for each body. B1 is generally
less well conditioned here, with a small shared-graph exception in the Gram
measure. Neither row counts nor local rank predict complete-element mobility,
sequential-impulse convergence or production performance. Fixing the car leaves
no independent conventional bogie fitting coordinates in either model.

## 7. Isolated rigid-body physics experiment

Geometry and Jacobian tests passed before the dynamics module was introduced;
the initial six-test evidence is retained in `target/m4b/geometry-first.txt`.
The standalone experiment preserves the production solver and never installs
poses into `World`, playback or M3B fitting. It uses rigid COM translation and
quaternion rotation, external gravity and impulses; poses are not assigned from
stations. All bodies have 100 kg, isotropic 20 kg m² inertia, so gyroscopic torque
is identically zero **for these fixtures**. General inertia is not validated.

Velocity initialization mass-projects a seed motion into the actual J nullspace
and normalizes the front carrier tangent speed. No construction projection
impulse is applied to a running simulation. Initial `max |Jv|` is below 1.6e-14
at 6 m/s. Raw per-body tangent velocities with zero angular velocity instead
produce about .68 m/s worst constraint speed and 63–74 N s maximum ideal seating
multiplier in curved fixtures. That mismatch is measured separately.

Instantaneous reference reactions solve
`(J M^-1 J^T) lambda = -J M^-1 f - Jdot*v`. Acceleration residuals pass 1e-7;
directional refinement passes 1e-6 and independent position second differences
pass 2e-5. Internal bearing linear rows cancel across the graph. External rail
rows exchange momentum with the world. Ideal reaction power is below 4.7e-12 W
in the measured nonzero-speed cases. With gravity off, stationary reference
loads are zero and 6 m/s reference loads are four times 3 m/s loads, within 1e-5.
Those speed-squared curvature loads are legitimate, not initialization seating.

Time integration keeps **1/240 s, four substeps and 32 impulse sweeps**. At each
substep it integrates gravity, builds the same c/J, and solves sequential rows
with target `J v_new = -dt * Jdot*v - .2 c/dt`, then advances rigid poses.
This includes a measured numerical stabilization bias, not physical compliance.
There is no post-step pose fitting or incompatible correction for old guides to
undo. The 32-sweep solve is approximate, with separately gated residuals.

For offset-pivot graphs, 24 outer steps are attempted at initial 0/3/6 m/s:

| Path, 6 m/s | Completed steps | Max equation position residual | Max equation speed residual | Max absolute energy drift, J |
| --- | --- | --- | --- | --- |
| Straight | 24 | 7.942e-7 | 1.513e-4 | .00119 |
| Tight turn | 24 | 1.226e-5 | .002344 | .04050 |
| Changing bank | 23, then Corner rollback | 9.818e-6 | .001952 | .03672 |
| Helix | 24 | 1.093e-5 | .001954 | .28723 |
| Loop, near inverted attitude | 24 | 1.171e-5 | .002227 | 2.63316 |
| Inverted straight | 24 | 7.942e-7 | 1.513e-4 | .00119 |

Units for linear and angular equations remain separate as defined above. Across
all eighteen runs, 17 complete 0.1 s and one completes 23/240 s. The changing-bank
6 m/s next step encounters a sampled normal-frame corner and leaves the entire
prior state intact. Its expected failure/step count is asserted explicitly.
All 0 and 3 m/s runs complete 24 steps. Maximum linear momentum imbalance against
gravity and accumulated external rail impulses is below 8.2e-10 N. Two independent
states and their complete step reports are exactly equal throughout, including
the corner failure. Invalid velocities and a late excessive-speed failure also
leave state unchanged. The shared offset-pivot graph separately completes eight
3 m/s steps with one physical middle bogie, six axles and 30 supports; maximum
equation residual is 2.974e-6. Its complete replay is also identical.

This is short-horizon evidence. Positive/negative integration and stabilization
work are not eliminated by ideal instantaneous zero power. The loop's 2.63 J
drift is retained, not rounded away. Long-run energy conservation, full loop
dynamic traversal, high speed, mass ratios, general inertia and production
impulse convergence remain unvalidated. Fixed local windows intentionally limit
travel; they are not a complete station-tracking controller.

## 8. Reactions and the boundary with actual contact

Straight gravity support totals **6867 N**, the weight of seven 100 kg bodies,
with largest running multiplier 891.075 N. Internal anchor reactions transfer
car/bogie weight to the supported axle bodies. At finite iteration count the
first-step largest multiplier is about 891.607 N; exact static values are not
assumed for the iterative time integrator. Gravity-free straight first-step
loads stay below 1e-6 N/N m: roundoff-sized anchor residuals are amplified by
stabilization/dt² and are not literally zero.

Bilateral running support is physically inadmissible as actual running-wheel
contact whenever its multiplier is negative. Examples from the same equations:

| Fixture / speed | Minimum running multiplier, N |
| --- | ---: |
| Bank transition, 0 m/s | -18.458 |
| Helix, 0 / 3 m/s | -219.465 / -162.237 |
| Loop near inverted attitude, 0 / 3 / 6 m/s | -792.410 / -692.201 / -391.576 |
| Inverted straight, every tested speed | -891.075 |

These loads are exposed and the inverted sign is tested. They cannot establish
wheel separation, an up-stop load or a valid contact active set. No unilateral
complementarity, impact, re-engagement, running/up-stop selection, friction,
wheel spin, suspension compliance or manufacturer tolerance is implemented.
Those belong to model C. Removing tensile rows from assertions, inventing an
up-stop at the same point, or changing the geometry to keep loads positive
would hide rather than resolve this missing physical definition.

## 9. Performance and regression verification

The ignored `research_phase_timing` measures setup, nonlinear initialization
and one 1/240 s B1 tick separately, with a warm-up and seven batches of five.
Setup includes 2401 physical samples, rail validation and independent mechanical
seed construction. Initialization excludes setup, includes projection, dense
Newton work and pose copies. Tick excludes setup/initialization but includes a
state clone, four substeps, all local projections/Jdot differences and 32 sweeps.
It repeatedly starts from the same valid 3 m/s state; compilation/logging are
outside timers. Timings are shared-machine observations, not CI thresholds.

Native timing host: Intel Core i7-14700KF, Rust 1.99.1 / LLVM 23.1.1, Windows
MSVC. Times below are microseconds, median [minimum, maximum] of batch means:

| Fixture | Setup | B0 initialize | B1 initialize | B1 rigid-body tick |
| --- | --- | --- | --- | --- |
| Straight | 70.42 [69.78,74.94] | 83.58 [82.82,89.60] | 390.64 [387.50,413.84] | 6090.44 [6029.16,6119.48] |
| Changing bank | 483.52 [480.52,486.88] | 374.12 [371.08,377.38] | 3933.06 [3896.76,3967.96] | 11235.26 [11131.86,11284.76] |
| Helix | 454.50 [444.34,467.32] | 370.76 [367.02,415.90] | 2807.76 [2797.10,2864.28] | 11280.54 [11102.92,11318.74] |

The prototype repeatedly scans bounded station neighborhoods and differences
whole matrices for Jdot. Its 6–11 ms tick exceeds the 4.167 ms outer-step interval
even for this seven-body graph on the measured CPU. This **research implementation**
does not meet a real-time 240 Hz simulation budget. No production frame rate is
estimated. B0 simulation ticks were not newly benchmarked; the timing comparison
between B0 and B1 is setup/initialization only.

The retained ordinary M1 assembly timer ran before any M4B source was written
and again after the complete regression suite, separately from other test runs.
Track generation and all research work are excluded. Median microseconds:

| Ordinary assembly | Before M4B | After M4B |
| --- | ---: | ---: |
| Straight / 2 cars | 18.440 | 18.380 |
| Straight / 8 | 74.260 | 75.100 |
| Straight / 16 | 149.080 | 153.040 |
| Straight / 32 | 323.080 | 320.500 |
| Curved / 2 | 457.560 | 467.480 |
| Curved / 8 | 7228.920 | 7279.820 |
| Changing bank / 8 | 7228.520 | 7290.100 |

All seven pose/station/anchor fingerprints are identical before/after. These
shared-machine variations are not a controlled performance-equivalence proof.
There is no new call or allocation on the ordinary assembly or simulation path.

Commands executed from the isolated worktree:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --release -p qx-physics --test assembly_convergence --test assembly_diagnostics --test chord_characterization --test initialization_diagnostics --test wheel_geometry --test bogie_fitting --test mechanical_guidance --test rail_normal_guidance -- --nocapture --test-threads=1
cargo test --release -p qx-geometry --test rail_geometry
python tools/verify-train-guidance-m4a.py target/m4b/characterization-release.txt
python tools/verify-train-guidance-m4b.py target/m4b/characterization-release.txt
cargo test --release -p qx-physics --test articulation_baseline -- --ignored --nocapture --test-threads=1
cargo test --release -p qx-physics --test assembly_convergence --test wheel_geometry --test bogie_fitting -- --ignored --nocapture --test-threads=1
cargo test --release -p qx-physics --test rail_normal_guidance -- --ignored --nocapture --test-threads=1
git diff --check
```

Final ordinary verification:

- Full workspace: **484 passed, zero failed, nine ignored**. One unrelated GPU
  graph-precision exporter remains ignored. The other eight ignored articulation
  characterization/timing tests were explicitly executed in native Release:
  **eight passed, zero failed**, including all seven retained tests and the new
  research phase timer. Original long-chain successes and explicitly feasible
  asymmetric 32-car failures remain characterized by the retained tests.
- Native Release characterization: **74 passed**, zero failed: 65 physics
  tests including all retained M1–M4A characterization and 12 new M4B tests,
  plus nine retained rail-geometry tests.
- Independent SVD: **38/38 retained M4A matrices** and **29/29 M4B matrices**
  pass all three rank thresholds and nullspace residual checks.
- Formatting, workspace check, strict all-target workspace Clippy and tracked
  diff checks pass. All six new files also receive Git no-index whitespace
  checks. Every relative report link resolves.
- SHA-256 preservation manifest: **468/468 pre-existing nonignored files are
  unchanged**, including all production code, dependencies, prior tests, fixtures
  and milestone reports. No historical assertion or tolerance was weakened.
  Exactly six files are added. The other tooling checkout retains its original
  status. Nothing is staged.

Raw logs, matrices, timing intervals and preservation manifests are retained
under ignored `target/m4b/`; they are not runtime assets. The normal Release
characterization overlapped the debug workspace run; **timing runs did not**.
No production FPS or long-chain B1 tick performance is inferred.

## 10. Remaining requirements for production integration and M4C

**Recommended M4C: strengthen the isolated bilateral model and geometry contract;
keep production replacement and model C contact as separate authorization gates.**

1. Specify the intended real carrier/bearing and rail/tread geometries with
   provenance, support sides, permitted steering/slip and actual travel limits.
   The synthetic guide chart cannot substitute for exact finite-profile contact.
   Continue distinguishing bearing articulation from frozen-car fitting freedom.
2. Provide continuous/adaptively certified rail projection neighborhoods,
   explicit knot/seam derivatives and branch tracking. Reproduce the bank corner
   without smoothing it away. Decide how nonsmooth authored geometry is rejected
   or handled, with independent proofs/fixtures for supported smooth pieces.
3. Replace research directional Jdot and dense reference allocation with a
   tested bounded analytic/sparse implementation if integration cost warrants it.
   Preserve independent finite differences, exact same c/J across initialization,
   velocity initialization, impulses, reactions, reset and diagnostics.
4. Require longer deterministic runs, complete-element dynamic traversal,
   speed sweeps, general inertia/COM offsets, mass ratios, shared/long-chain
   mechanisms, active bearing limits, energy envelopes and per-row reaction
   convergence. The current 0.1 s / 6 m/s evidence is not that acceptance suite.
5. Gate production opt-in on target-platform profiling and full pipeline
   integration: topology-aware initial assembly, COM adapters, stable support IDs,
   station cache invalidation, snapshots and backward-compatible old guidance.
   No current guide or M2B setting should be silently reinterpreted or deleted.
6. For model C, require exact signed contact gaps, compressive signs and an
   explicit complementarity/active-set model before testing separation, impact,
   up-stop engagement or friction. Keep inadmissible tensile B reactions in the
   evidence. Numerical rank alone cannot choose contact states.

Production `World` currently accepts specialized historical guidance rows; it
does not consume this generic mechanical contract. Installing only these poses
would let its existing constraints drive a different manifold. A future adapter
must update the complete initialization/dynamics/diagnostics path together.
M4B therefore stops at a standalone experiment and recommends no production rewrite.

UX status: no owner-approved behavior changed, no temporary developer UI added,
and no visual/workflow design finalized. Future mechanical metadata editing and
diagnostic presentation require human design. No desktop application was launched
or controlled. Rendering, cameras, DPI and interactive playback were not manually
verified. Optional owner regression: `cargo run -p quantum-x --release -- edit`,
open an existing interior train project, Inspect/Initialize, Play/Pause/Reset and
save/reopen. M4B adds no editor command to try.

Git status: local additions only on the existing branch; nothing staged,
committed, pushed, merged or submitted as a PR. The separate tooling checkout
`C:/Dev8/QUANTUM-X` is untouched.
