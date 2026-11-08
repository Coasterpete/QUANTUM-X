# Train articulation M4A — mechanical guidance and bogie freedom

Date: 2026-11-07. Native Windows x86_64 MSVC. Existing worktree:
`C:/Users/coast/.codex/worktrees/train-articulation-m1/QUANTUM-X`, branch
`codex/train-articulation-m1`, HEAD `bcc5add`. All five previous milestones and
their accumulated uncommitted changes are retained.

**Decision: investigate an explicit carrier/bearing graph with rail-normal
guidance in M4B; do not replace production guidance yet.** M4A establishes a
consistent *ideal bilateral* carriage/bearing architecture, finite assembly
witnesses, mobility and instantaneous reaction equations. It does not validate
a wheel-contact implementation or its stability in playback. The five-normal
wheel prototype is validated only on planar straight-track surfaces.

An important qualification: a conventional bogie need not have an arbitrary
rotational null direction while its car is fixed. Rail support and actual
bearings determine its attitude. Current car-to-bogie pitch/yaw articulation
already exists. What is missing is mechanically justified guidance at the actual
wheel/carrier supports, rather than forcing two virtual points on one rigid
gear to the centerline and fitting wheel centers afterward. Declaring a ball
joint, deleting a guide, or adding optimizer variables does not supply that
missing architecture.

## 1. Existing mechanical guidance architecture

Read before implementation: repository `AGENTS.md`,
[M1](train-articulation-m1-baseline.md),
[M2A](train-articulation-m2a-chord-robustness.md),
[M2B](train-articulation-m2b-convergence.md),
[M3A](train-articulation-m3a-wheel-geometry.md), and
[M3B](train-articulation-m3b-bogie-fitting.md). Source inspection covered
document mechanics/fixtures, assembly, diagnostics, initialization, body
integration, angular constraints, playback, wheel fitting and physical rail
queries. No source was imported from another coaster application.

### Position level

The component/connection graph is authoritative. Car/bogie grouping is not an
extra joint; a shared ID means one body. `Placement::Mounted` supplies an initial
pose, not a weld. Asset pivots and GLB corrections have no mechanical authority.

[`assembly.rs`](../crates/qx-physics/src/assembly.rs) constructs rigid initial
poses with chord seeding and the existing M2B projection. A connection first
enforces three anchor-coincidence equations. Its actual `JointModel` adds:

| Joint | Additional angular equalities | Physical interpretation / approximation |
| --- | ---: | --- |
| Ball | 0 | Ideal spherical bearing: three relative rotations before other constraints |
| Hinge | 2, plus active travel limit | Ideal revolute bearing about authored joint Z; one interior angular coordinate |
| RollLock | 1 | Zero swing/twist twist about joint X; two swing coordinates, not arbitrary Euler angles |
| Fixed | 3 | Ideal rigid attachment |

These are meaningful ideal mechanical connections when the authored mechanism
actually has those bearings. They omit clearance, flexibility and bearing
compliance. At a hinge limit admissible velocities form a one-sided cone; an
ordinary equality nullspace is insufficient. The new rank fixtures use interior
hinges; retained M1/M3B tests cover limits.

Each explicit guide point is locally projected to `C(s)`. Two transverse rows
constrain it to the centerline. Conventional development gear has points at
`(+/-.2,0,0)` m. Two separated points constrain lateral/heave displacement and
pitch/yaw. One additional `TrackAttitude` constrains twist against the rail
frame at the guide centroid. The local attitude frame compensates authored
track-placement rotation; it is not a wheel axle or tread frame.

The **first assembly guide** also pins longitudinal position to its seed. That
is an initialization gauge, not a physical brake. It is absent from playback
Jacobians and from the mobility counts below. Assembly can synthesize legacy
guides; initialization uses explicit references. This existing distinction is
preserved, and future guidance must not silently select different constraints
in the two paths.

### Velocity and force levels

[`consist::initialize`](../crates/qx-physics/src/consist.rs) converts component
origins and anchors to COM coordinates and creates independent rigid bodies.
It initially sets both velocities to zero. [`playback.rs`](../crates/qx-physics/src/playback.rs)
then assigns each COM a station-tangent linear velocity and leaves angular
velocity zero. This is the known nonzero-speed initialization mismatch.

[`solver.rs`](../crates/qx-physics/src/solver.rs) integrates applied forces and
gyroscopic rigid-body dynamics, solves velocity impulses, then advances each
body. It does not prescribe poses from stations. For a scalar point row with
world radius `r` and constraint normal `n`,

```text
J = [n, r cross n]; point velocity = v + omega cross r.
```

Mechanical connections apply opposite anchor impulses to both bodies; angular
joints apply opposite torques. Track guides exchange momentum with the static
rail/world. Rail attitude includes an important spatial-frame derivative:

```text
g = twist gradient(track_frame, body_rail_frame)
k = world rail-frame angular derivative per station
D = 1 - delta dot (k cross tangent)
Jlinear  = tangent * (-g dot k / D)
Jangular = g + r cross Jlinear
```

Both the force and torque are necessary: a torque-only moving bank target would
do inconsistent work. The code rejects near-singular projection denominators.
Current guide projection and normals use the interpolated frame tangent;
Hermite position derivatives are independently interpolated. Thus the existing
curved velocity rows are an approximation to the derivative of exact
point-on-Hermite-curve equations. This study checks their existing velocity
contract, not exact nonlinear integrability on every sampled curve. M3A already
provides exact sampler/offset-path derivatives for future guidance.

The scalar impulse includes `0.2 * position_error / substep_dt` stabilization.
That is numerical error correction, not suspension stiffness or a contact law.
Reported forces are accumulated impulses divided by the **outer** timestep;
they include stabilization and any initial velocity seating. Assembly's unit
translation/radius rotation weights are numerical weights, not these masses
or physical reaction forces.

## 2. Constraint rank and mobility analysis

New executable evidence is entirely under tests:

- [`mechanical_guidance.rs`](../crates/qx-physics/tests/mechanical_guidance.rs)
  contains ten independent characterization/feasibility tests.
- [`m4a_guidance.rs`](../crates/qx-physics/tests/support/m4a_guidance.rs)
  reconstructs the current signed impulse rows without mutating the world.
- [`m4a_mechanisms.rs`](../crates/qx-physics/tests/support/m4a_mechanisms.rs)
  defines explicit analytic geometry and bearing-closed witnesses.
- [`m4a_matrix.rs`](../crates/qx-physics/tests/support/m4a_matrix.rs)
  provides small test-only elimination, nullspace and centered differences.
- [`verify-train-guidance-m4a.py`](../tools/verify-train-guidance-m4a.py)
  independently checks the emitted matrices with NumPy SVD. NumPy is an
  optional research-verification dependency, not an application dependency.

Each rigid body has six *velocity* coordinates; quaternion components are not
counted as four independent rotational DOFs. Mobility is `n - rank(J)` at a
regular feasible configuration. Dependent rows are `m - rank(J)`; a dependent
row may cause nonunique multipliers without making a pose infeasible. Residuals,
rank, active inequalities and finite motion must be considered separately.

The reconstruction is checked against existing `velocity_residuals` for every
unit velocity column and a mixed velocity, including curved/banked cases.
Independent position differences on exact straight fixtures use steps
`1e-4/1e-5/1e-6`, with a `2e-8` maximum Jacobian discrepancy gate. B's augmented
position equations are differenced at `1e-5` and `1e-6` and agree within `2e-8`.
Every null vector is multiplied by the original matrix to verify closure.

All 38 exported matrices agree between Rust elimination and NumPy SVD at
absolute thresholds `1e-5`, `1e-7`, `1e-9`. These use metre-scale fixtures and
an explicit **1 m characteristic length** for angular scaling. They are not
scale-independent rank certificates. For example a `1e-10 m` gauge is
numerically rank deficient at these thresholds, although its ideal nonzero-
gauge matrix has mathematical rank five.

### Current model A

| Exact straight mechanism / scope | Rows | Coordinates | Rank | Mobility | Meaning of null directions |
| --- | ---: | ---: | ---: | ---: | --- |
| One car, two conventional bogies | 17 | 18 | 17 | 1 | Whole-assembly longitudinal travel |
| Same, suspended body with two ball bearings | 16 | 18 | 16 | 2 | Travel and passenger-body swing about the bearing chord |
| Conventional plus M3B leaf hinge carrier | 25 | 24 | 22 | 2 | Travel and actual extra carrier hinge rotation |
| Front conventional bogie, all other bodies fixed | 17 | 6 | 6 | 0 | No independent fitting rotation |
| Front suspended bogie, all other bodies fixed | 16 | 6 | 6 | 0 | Passenger swing does not rotate the bogie freely |
| Added M3B carrier, all other bodies fixed | 25 | 6 | 5 | 1 | Authored hinge rotation |
| Isolated single guide at pivot plus rail roll | 3 | 6 | 3 | 3 | Travel plus pitch/yaw; wheel alignment not assured |

Rows that do not involve a selected body's columns become zero in a
parent-fixed restriction; their apparent dependence is not redundancy in the
whole mechanism. With separated conventional guides alone, four rows have
rank four, and rail roll raises this to five. The remaining travel direction on
a curve includes orientation change; it is not independent pitch/yaw fitting.

The radius-8 turn and radius-12 changing-bank checks both give rank five for
the isolated front-gear guidance columns and rank six after fixing the car and
including its bearing. Full current matrices have rank 17 at thresholds
`1e-3/1e-5/1e-7`. These are linearizations at tolerance-closed production
assemblies, not exact-manifold proofs from their small nonzero residuals.

NumPy's retained condition ratios are 8.8292 for the straight conventional
graph and 12.8334 for the graph with the extra carrier, under the stated scaling.
There are three dependent rows in the latter graph: its pivot guide/attitude
and bearing/parent guidance overlap. No automatic row-removal inference is made.

## 3. Necessary, redundant and incompatible restrictions

The current five independent guidance directions are reasonable for an *ideal
captive rigid carriage*. They are not intrinsically an algebraic defect.
They become an unsuitable physical abstraction when virtual centerline points
and a separately imposed roll target stand in for a different wheel/bearing
mechanism. Adding wheel-center constraints to them can overconstrain that
mechanism instead of improving it.

Specific findings:

1. Two distinct centerline points plus roll already determine all gear attitude
   at a given travel coordinate. A ball suspension does not restore it.
2. A single pivot guide plus roll leaves two rotational directions, but a finite
   `.1 rad` yaw moves an authored off-axis wheel away from its declared rail.
   Removing a guide without replacing its mechanical function is not a fix.
3. Front roll reference plus rear spherical bearing accommodates differential
   bank. Adding a rear roll lock to the B bank-transition witness produces a
   **.03311480319 rad** violation at those same closed stations. This proves
   incompatibility of that pose with the additional bearing, not global
   infeasibility over every possible station/pose.
4. Four wheels, each constrained in two transverse center-locus directions,
   supply eight rows with straight-track rank five: **three redundant rows**.
   On curvature/twist, these equalities need not remain compatible. In a
   radius-8 horizontal fixture, same-rail targets at `s=20+/-.3` have pair
   separations **.644848838754 / .554869931021 m** for the two rails, while
   both rigid local pairs are `.6 m`. Those prescribed targets cannot all be
   achieved rigidly. Independent stations in a true nearest query are variables;
   this is not a global impossibility proof over them.
5. Running and opposed up-stop normal rows at identical zero-clearance supports
   can be linearly dependent. Their bilateral multipliers are nonunique; real
   finite-clearance contacts have distinct active sets and signs.
6. Zero wheelbase leaves yaw/pitch unlocated in the five-normal prototype
   (rank three); zero gauge loses roll location (rank four). Tiny dimensions
   produce conditioning problems well before an exact mathematical singularity.
7. At a circular diameter chord, the station derivative vanishes, but a finite
   `.01 m` relative-station perturbation shortens the chord by approximately
   `3.125e-6 m`. A first-order flex there is not a finite articulation mode.

Unconstrained drawbar spin can be legitimate for a spherical-ended rigid link
but has no wheel-alignment objective. Shared gear can reduce mobility through
extra actual attachments; adding another grouping reference alone changes none.
The retained M2B contradictory-edge fixtures additionally supply a global
incompatibility check: two connections share one child anchor but their anchors
on the same rigid parent differ by `.1 m`, forcing at least one gap to be
`.05 m` or larger in every pose. Their near-tolerance variant and incompatible
extra-guide fixture are also retained and rerun, rather than relaxing them to
make any new model pass.

## 4. Candidate guidance architectures

| Candidate | Mechanical consistency / freedom | Numerical conditioning | Complexity / integration |
| --- | --- | --- | --- |
| A: existing paired centerline guides + rail twist | Consistent ideal captive carriage with explicit car bearings; cannot guarantee wheel alignment or extra fitting freedom | Five independent directions per ordinary gear; twist/projection singularities and long-chain iteration remain | Lowest cost; existing initialization, impulse solver and diagnostics already agree on this approximation |
| B0: rail-guided axle carriages + explicit bogie/car bearings | Finite consistent graph; separate axle rows can follow rail bank while the bogie frame bridges their pivots; full axle attitude is still an ideal bearing assumption | Tested regular rank and instantaneous reactions; requires spatial orientation derivatives with translational terms | More bodies/rows; a useful isolated control model, not a production contact model |
| B1: role-specific scalar rail-normal/bearing guidance | Restricts only supported directions of authored surfaces and carriers; articulation follows the actual graph, not free pose fitting | Straight five-normal prototype is full row rank; curved normals, contact branches, clearance and changing rank still need validation | Credible next research target; needs common residual/Jacobian definitions across initialization and dynamics |
| C: explicit running/side-guide/up-stop wheel contacts | Most direct route to geometric support, separation and re-engagement; requires contact surfaces/normals, bearing geometry and a valid active set | Redundant simultaneous contacts, impacts, near-parallel normals and active-set switching complicate conditioning | Highest complexity; current bilateral impulse infrastructure alone is insufficient |

B0 is a useful reference because it exposes every physical body and joint and
its idealizations are explicit. It is **not** a claim that ordinary coaster axle
modules follow full rail attitude, or that adding bodies automatically models
a real bogie. Its guided axle modules themselves have no independent attitude
at a fixed station. In fact its conventional tight-turn graph has **rank 36
over 36 moving-body columns when the car is held fixed**. It must not be offered
as a new free-coordinate M3B optimizer on a frozen car.

### B0 equations and explicit fixture geometry

Each axle carriage at world origin `x` and attitude `Q` has a local rail station
parameter `s`, with ideal equations `x=C(s)` and `Q=R(s)`. Six equalities plus
one auxiliary station remove five physical velocity DOFs. The station has no
mass, velocity prescription or independently advancing playback coordinate.

For unit-speed analytic `C` and world frame derivative `k`, eliminating station
velocity gives two transverse position rows and three attitude rows:

```text
normal dot v = 0; lateral dot v = 0
omega - k * (tangent dot v) = 0
```

The second line includes linear contributions from curvature and banking.
Our tests independently condense the augmented position Jacobian and retain
these contributions. A future implementation must differentiate the actual
chosen curve and constraint coordinates, including offset pivots/COMs and
nonunit parameter derivatives; this unit-speed formula is not a universal API.

Each rigid bogie bridges two axle origins with anchors at `+/-.3 X m`.
One front RollLock plus a rear Ball closes it without a second roll target.
The car bridges two bogie pivots `2.4 m` apart with local anchors
`(+/-1.2,-height,0) m`, height `.8 m` or `-1.4 m` for the suspended case.
Conventional cars have one front roll reference; suspended cars use two balls.
The offset-pivot fixture declares bogie-local `(.05,.12,.03) m` before solving.
The shared fixture has two cars, three bogies and six axle carriages: its
middle bogie and its two carriages exist only once.

An independent bounded bisection solves the `.6 m` axle chord, then the
`2.4 m` car pivot chord. Rigid transforms are constructed from these chords
and front-bearing twist. No M2B/chord/guide-projection solver supplies these
witnesses. Every anchor and angular equality is then re-evaluated.

### B1 and C: scalar gaps versus center alignment

The isolated straight B1 prototype has three declared running-normal supports
at `(-.3,0,-.6)`, `(.3,0,-.6)`, `(0,0,.6) m`, normal +Y, and two side-guide
supports at `(+/-.3,0,0) m`, normal +Z. A support contributes one plane-gap
equation and `[n,r cross n]`, not two wheel-center-locus equalities. These five
rows locate heave, lateral, pitch, yaw and roll, leaving travel. They are
synthetic planar bilateral bearing/surface constraints; tread shape and wheel
radius have **not** been inferred from these points.

For future C, a physical normal contact needs a signed wheel/rail surface gap
`phi >= 0`, nonnegative compressive normal reaction, and contact/separation
complementarity. A running wheel cannot pull a detached vehicle toward a rail;
an up-stop can support the corresponding inverted load at a different contact.
Contact point, axle/tread geometry, normal, clearance, rail identity and carrier
ownership must be authored explicitly. Wheel-center alignment supplies none of
the separation/impact/load-selection rules. Lateral rolling/no-slip, friction,
compliance and wheel spin are separate issues and are not implemented here.

The distinction between bilateral position/velocity/acceleration equations and
unilateral contact follows the general constrained-dynamics formulation in
[MIT's multibody dynamics notes](https://underactuated.csail.mit.edu/multibody.html).
Our coaster-specific graphs, row counts and fixture measurements are independent
derivations from this repository, not manufacturer data or external code.

## 5. Independent feasibility results

Analytic tracks use +X forward, +Y up, +Z lateral, SI metres/radians and f64.
Straight and inverted cases are exact lines. Turns and loops have radius 8 m.
The bank-transition turn has bank `.02*s+.001*s*s`; the helix has vertical rise
`.3 m` per metre and bank `.4+.04*s`. Lead bogie center is 20 m, except loop
top `8*pi` and vertical tangent `4*pi`. These are numerical fixtures, not
production coaster designs or manufacturer geometry.

| B0 witness | Bodies / auxiliary stations | Augmented rows / rank | Physical mobility | Maximum equation residual | Continuation max `abs(J*v)` |
| --- | --- | --- | ---: | ---: | ---: |
| Straight | 7 / 4 | 45 / 45 | 1 | 1.421e-14 | 0 |
| Tight horizontal turn | 7 / 4 | 45 / 45 | 1 | 5.329e-15 | 5.931e-11 |
| Bank transition | 7 / 4 | 45 / 45 | 1 | 3.553e-15 | 5.931e-11 |
| Helix | 7 / 4 | 45 / 45 | 1 | 1.244e-14 | 1.180e-10 |
| Vertical loop top / inverted attitude | 7 / 4 | 45 / 45 | 1 | 6.662e-15 | 4.734e-11 |
| Vertical loop tangent | 7 / 4 | 45 / 45 | 1 | 8.882e-15 | 5.327e-11 |
| Inverted straight | 7 / 4 | 45 / 45 | 1 | 1.421e-14 | 0 |
| Suspended, changing bank | 7 / 4 | 44 / 44 | 2 | 3.553e-15 | 5.931e-11 |
| Offset pivots, helix | 7 / 4 | 45 / 45 | 1 | 1.599e-14 | 1.180e-10 |
| Shared middle bogie, changing bank | 11 / 6 | 71 / 71 | 1 | 1.066e-14 | 5.931e-11 |

Residual maxima above mix the numeric values of metre anchor/rail errors and
radian angular errors solely for compact display; **each** underlying equation
is gated separately by absolute value `1e-12` in its own unit. There is no
mixed-unit objective or tradeoff. Finite continuation constructs independent
witnesses at lead station `+/-1e-4 m`; the table is a derivative per unit lead
station speed. It is not evidence of time integration or nonzero-speed playback.

After station elimination, conventional B0 has **41 independent rows / 42 body
coordinates**; suspended B0 has 40/42; shared B0 has 65/66. In the suspended
case an additional finite `.2 rad` car rotation about the two-bearing world
chord preserves every constraint while moving its origin by more than `.1 m`.
This verifies real passenger swing rather than merely labeling a null vector.
Typical conventional car/front-bogie relative angle is `.1506751 rad` on the
radius-8 turn/loop and `.1369749 rad` on the helix. These relative angles are
bearing-closed orientations, not independent frozen-car fitting coordinates.

## 6. Expected wheel-alignment improvements and limits

Each axle module owns a single left/right wheel row with local centers
`(0,.18,+/-.6) m` and rolling +X. Wheel centers are never fitted individually.
Declared rails are at lateral `+/-.6 m`, with explicit `.18 m` center offset.
An independent trigonometric world basis verifies the transformed wheel
centers. M3A's **existing** physical rail adapter then samples and independently
queries the same declared loci on an analytic track sampled every `.005 m`.
All nearest queries must be resolved and within `5e-6 m` full separation.

The baseline A comparison assembles the unchanged conventional one-car train
on that same sampled track and observes its eight rigid-gear wheel centers.
B0 has eight centers on four new axle bodies. This is a comparison of different,
explicitly authored mechanisms, **not** an improvement to the same rigid gear
obtained by loosening constraints or changing its local wheel coordinates.

| Fixture | Current A RMS / max center error, m | B0 max analytic-center error, m | B0 max discrepancy to sampled locus, m | B0 max rolling-direction error, rad |
| --- | --- | ---: | ---: | ---: |
| Straight | 0 / 0 | 0 | 0 | 0 |
| Tight turn | .003184505 / .003580268 | 0 | 5.735e-13 | 7.538e-11 |
| Bank transition | .011948082 / .014675985 | 1.780e-15 | 6.587e-9 | .040694819 |
| Helix | .003057598 / .003768135 | 1.986e-15 | 6.161e-9 | .002786338 |
| Loop top | .003253618 / .003254214 | 1.777e-15 | 1.080e-13 | 5.320e-11 |
| Inverted straight | 0 / 0 | 8.327e-17 | 0 | 0 |

The bank row uses a different twist law from M3A/M3B's original generated
fixture; its baseline must not be compared as if it were that historical row.
The sampled-locus discrepancies measure interpolation error separately from
analytic mechanical closure. Initial `.02 m` samples did not meet the new
`1e-7 m` sampler comparison gate on changing bank; reducing *test track*
spacing to `.005 m` meets it. No production sampling setting was changed.

Zero center error is not zero axle-direction error. Different offset wheel
paths can have different tangents on twisting track. The two wheels on a
rigid axle cannot generally both have rolling +X exactly tangent to those
different paths. A future mechanism may have steering carriers, tread/contact
freedom or lateral slip; those must be modeled rather than asserting an exact
no-slip fit. B0 validates a useful position reference, not complete wheel contact.

## 7. Compatibility with dynamics, velocities and reaction forces

At the continuous level a chosen bilateral guidance manifold needs:

```text
c(q) = 0
J(q) v = 0
J(q) a + Jdot(q,v) v = 0
M a = f + J^T lambda
(J M^-1 J^T) lambda = -J M^-1 f - Jdot v
```

This is our proposed compatibility contract. The existing impulse denominator
is the corresponding scalar `J M^-1 J^T`; future rows must use the *same*
residual and derivatives for assembly, initialization, impulses and diagnostics.
Internal bearing rows must retain opposite reactions; rail rows have an external
world reaction. A static ideal bilateral reaction should have zero instantaneous
power on an admissible velocity. Numerical stabilization need not do so.

Independent B0 tests condense massless stations out before forming the physical
reaction matrix. For each body they declare 100 kg and isotropic 20 kg m²,
with world gravity `(0,-9.81,0)`. Isotropic inertia removes gyroscopic torque
for these particular checks; this is not a validation of general inertia.
Independent continued witnesses at `+/-0.0005 m` supply velocity, acceleration
and the changing Jacobian. At unit lead-station speed:

| Fixture | Max acceleration-equation check from continued geometry | Max solved acceleration residual | Absolute reaction power, W |
| --- | ---: | ---: | ---: |
| Straight | 2.842e-8 | 3.638e-14 | 0 |
| Bank transition | 3.925e-8 | 8.374e-15 | 2.965e-8 |
| Helix | 4.548e-8 | 2.904e-14 | 1.095e-6 |
| Loop | 3.467e-8 | 4.287e-15 | 1.098e-8 |

Translation rows use m/s², angular rows rad/s²; table maxima are numeric maxima
with separate unit-specific equation gates, as above. These results establish
consistent instantaneous ideal reactions. They do **not** demonstrate stable
sequential-impulse integration, energy behavior over time, high-speed reactions,
contact active-set changes or convergence at extreme mass ratios.

The straight B1 five-normal prototype independently supports a 100 kg body
with three running-normal loads **245.25, 245.25, 490.5 N**, zero side loads,
981 N total lift and zero net torque to roundoff. Reversing required support
gives three **negative** running-only loads. That solution is legal for bilateral
equations but physically inadmissible for unilateral running contact. Up-stop
geometry and active-set selection are necessary for inverted support.

### Existing playback characterization, unchanged

The current one-car straight World was stepped eight times with its existing
32 iterations, four substeps and no applied motion. Repeated worlds and reports
are identical. Independent momentum balance agrees with external guide plus
attitude force and gravity within `1e-8 N`; observed maximum discrepancy is
approximately `3.04e-12 N`. The first reported support force is 7847.813862 N,
settling toward 7848.000519 N at step eight for 800 kg. Initial assumptions of
exact static support at every finite-iteration step were replaced by this actual
momentum-balance check. Position residuals remain measured, not rounded to zero.

The existing two-car changing-bank radius-45 fixture was initialized exactly as
playback does, without projection:

| Initial speed | Initial anchor speed, m/s | Initial guide transverse speed, m/s | Initial rail twist speed, rad/s | First-step max joint force, N | First-step max guide force, N |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 m/s | 0 | 0 | 0 | 2999.146 | 1989.971 |
| 12 m/s | .533289218 | .106665612 | .019315893 | 16609.727 | 10681.656 |

After the 12 m/s first step, maxima are `.000901744 m/s` anchor speed,
`.002516978 m/s` transverse guide speed and `.004987953 rad/s` rail twist rate.
Force differences include legitimate speed-dependent track reactions as well
as initialization seating and positional stabilization. This two-row experiment
does not isolate how much comes from each cause and does not certify energy
consistency. A zero initial `J*v` also does not eliminate gravitational reaction.

Future B/C adoption must revise velocity initialization, initial pose assembly,
constraint definitions, impulse Jacobians, force/torque diagnostics, reset and
snapshot station handling together. Updating only a fitted pose would let old
guides immediately drive it toward the previous manifold. M4A implements no
velocity projection, modifies no playback code and does not install M3B fits.

## 8. Numerical risks and performance implications

- **Rank versus finite mobility:** residual-induced near singular values, tangent
  chord roots, limits and changing contact sets need explicit classifications.
  Numerical damping must not be treated as restored physical rank.
- **Gauge and units:** remove the longitudinal assembly gauge from physical
  mobility/reactions. Use characteristic lengths and mass-weighted conditioning
  in later solvers; current raw SVD ratios do not predict impulse convergence.
- **Projection and branches:** offset-path tangents, rail curvature, derivative
  corners, self-crossings, seams and lost local tracking can make a target
  unavailable. Preserve bounded neighborhoods and explicit errors.
- **Twist coordinates:** the existing opposed-axis singularity is real. Full
  quaternion attitude error charts likewise require local branch management.
  Never hide a singular frame with a world-up reset through inversions.
- **Redundant reactions:** `J M^-1 J^T` is singular for dependent bilateral rows.
  Multipliers may be nonunique even if total generalized force is unique.
  Coincident running/up-stop rows are particularly unsuitable as load sensors.
- **Time integration:** exact position witnesses and instantaneous forces do not
  establish long-run stability. Future tests need force/energy transients,
  deterministic replay, residual envelopes, mass ratios, speed sweeps and
  unilateral separation/re-engagement. Retain the baseline timestep/iterations
  until there is evidence for any separately authorized change.
- **Cost:** current one-car A has 3 bodies and 17 scalar rows; conventional B0
  has 7 bodies and 41 condensed scalar rows. More carriers increase integration,
  joint, projection and matrix bandwidth costs. At the existing 32 iterations
  and four substeps, 17 versus 41 rows imply 2176 versus 5248 scalar-row visits
  per tick before other work; this is a structural count, **not a measured timing**.
  Actual many-car scaling depends on shared-body topology and solver order.
- **Research overhead:** new matrices are bounded to 256 rows / 128 coordinates;
  dense finite differences/SVD are verification tools, not a proposed per-tick
  implementation. Test logging is opt-in via `--nocapture`. No new allocations,
  work or calls were added to production assembly, dynamics or rendering.

No new physics-tick, FPS or application performance claim is made. Retained
Release characterization/timing tests are rerun for regression, not evidence
that the proposed richer mechanism meets an interactive budget.

## 9. Migration and backwards compatibility

M4A adds no module exports, library APIs, project fields, styles, assets,
dependencies, controls or serialization changes. Schema 7 is unchanged.
Every new mechanism lives in tests and is labeled synthetic/experimental.
The five previous reports, fixtures, M2B solver, constraints, M3B fitter,
1/240 s timestep, 32 playback iterations and four substeps remain intact.

If a later production milestone adopts B/C:

1. Preserve the old guide model for existing documents. Do not reinterpret
   `guide_references_m`, generic `wheel_references_m`, grouping IDs or GLB
   geometry as newly authored physical wheel surfaces.
2. Require explicitly versioned rail/support, carrier/bearing, contact-side,
   wheel/tread/axle and clearance metadata. A migration must be deliberate,
   previewable and reversible; no invented manufacturer geometry.
3. Share one transient mechanical constraint description across pose assembly,
   velocity initialization, physical row construction and diagnostics. Keep
   component origins, COMs, local joint frames and wheel frames distinct.
4. Invalidate cached stations/fits when geometry, topology or metadata changes.
   Preserve open/shuttle behavior, explicit circuit intent and unwrapped branch
   identity. Whole-consist seam initialization remains separately unresolved.
5. Retain existing M1–M3B reference results. New guidance requires its own
   acceptance suite; old wheel-center errors must remain reproducible under A.

No automatic migration or final project/editor design is authorized by this
study. Future metadata editing and diagnostic presentation require human UX.

## 10. Recommended design and acceptance gates for M4B

**Proceed with a bounded, isolated B research implementation, using B0 as its
control and B1 as the next physical approximation. Do not authorize a production
replacement merely from these results.**

1. Keep a component/carrier/bearing graph, with each wheel owned by its actual
   rigid piece. Start with the explicit two-bogie geometry and shared identity
   above. Do not demand arbitrary frozen-car yaw/pitch/roll from a constrained
   conventional bogie or add those as unconstrained optimization variables.
2. Implement common, differentiable scalar rail-normal/bearing equations in an
   opt-in research adapter. Specify authored surface points/normals, admissible
   axle/carrier rotation, guidance locations and rail identity. Keep B0 full-
   attitude carriage equations available as a labeled comparison, never as
   implicit wheel contact. Do not retain an old rail-roll equality if the new
   support rows already supply that moment, unless rank analysis justifies it.
3. Validate B1 on curved and twisting rails before any playback integration:
   finite closure at each support, exact target derivatives, constraint rank,
   contact-normal geometry, every wheel's center **and direction** diagnostics,
   limits and deliberately incompatible supports. A smaller center RMS is
   insufficient. Supported slip/steering assumptions must be stated explicitly.
4. Test one shared residual/Jacobian contract through initialization, velocity
   consistency and an isolated impulse experiment, with measured seating impulses,
   residuals, energy and deterministic replay. This future work needs simultaneous
   initialization/dynamics/diagnostics changes; M4A has made none of them.
5. Gate contact C separately: compressive signs, actual running/side/up-stop
   surfaces, finite clearances, active-set changes, separation and re-engagement.
   Retain bilateral B only as an ideal captive-guide approximation. Do not add
   friction, wheel spin or compliance just to mask incompatible hard geometry.
6. Require long-chain/offset/shared regression and target-platform profiling
   before considering opt-in production support. Preserve M2B's known feasible
   asymmetric 32-car failures and all earlier explicit unsupported outcomes.

Unresolved mechanical decisions: actual target bogie/carrier topology and bearing
travel; mechanical rail/tread surface conventions; clearance/contact sides;
whether a particular mechanism permits steering or lateral slip; and which
mechanism families M4B should initially support. No manufacturer-accurate design
is inferred from synthetic examples. These decisions do not prevent the isolated
B1 investigation, but they block a claim of realistic production bogie physics.

## 11. Verification, preservation and UX/Git status

Reproduce from the existing worktree; no GUI or desktop control is required:

```powershell
New-Item -ItemType Directory -Force target/m4a | Out-Null
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --release -p qx-physics --test mechanical_guidance -- --nocapture --test-threads=1 > target/m4a/mechanical-final-release.txt 2>&1
python tools/verify-train-guidance-m4a.py target/m4a/mechanical-final-release.txt
cargo test --release -p qx-geometry --test rail_geometry
cargo test --release -p qx-physics --test assembly_convergence --test assembly_diagnostics --test chord_characterization --test initialization_diagnostics --test wheel_geometry --test bogie_fitting -- --test-threads=1
cargo test --release -p qx-physics --test assembly_convergence --test articulation_baseline --test wheel_geometry --test bogie_fitting -- --ignored --test-threads=1
git diff --check
```

Executed results:

- Final full workspace: **472 passed, zero failed, eight ignored**. The unrelated
  graph-precision GPU evidence exporter remains ignored; all seven articulation
  ignored characterization/timing tests were explicitly executed in Release
  and passed. No test or existing tolerance was weakened.
- Native Release characterization: **62 distinct tests passed**: 43 retained
  M1–M3B physics characterization tests, ten new M4A tests, and nine retained
  rail-geometry tests. The retained suite ran in `fixtures-release.txt`, rail
  tests in `rail-release.txt`, and the finalized M4A source was separately rerun
  in `mechanical-final-release.txt` after the last fixture refinement.
- Independent NumPy SVD: **38/38 matrices passed**, including original guidance,
  frozen-parent restrictions, added hinge carrier, augmented/condensed B0 graphs,
  scalar normal supports, redundant wheel-center constraints and degeneracies.
- Final formatting, workspace check, strict all-target workspace Clippy and
  tracked diff checks passed. All six new untracked files received separate
  no-index whitespace checks. All 14 local report links resolve.
- SHA-256 preservation manifest covers **462 pre-existing nonignored files**:
  **all 462 unchanged**, including every library/production implementation,
  prior report, fixture, test, dependency declaration and document model. The
  pre-existing tracked binary diff is identical to its initial snapshot.
- Exactly **six new files**: this report, one test harness, three test-only
  helpers and the independent SVD verifier. No existing file was edited. The
  separate tooling checkout retains its initial branch and status.

Local raw logs/matrix evidence and preservation manifests are under ignored
`target/m4a/`, not runtime assets. Retained timing tests were allowed to overlap
the workspace regression run; their fresh durations are not used as controlled
performance comparisons or as evidence of B's future per-tick cost.

UX status: no owner-approved behavior changed; no temporary developer UI was
introduced. No new interaction or visual design was finalized. Future mechanical
metadata editing, research controls and diagnostic presentation need human design.

Not manually verified: desktop interaction, rendering, cameras, DPI, subjective
playback, a new application Release package, manufacturer mechanics, C contact
dynamics, or B integration over time. No GUI was launched or controlled. Optional
owner regression checklist: `cargo run -p quantum-x --release -- edit`, open an
existing interior train project, inspect/initialize, Play/Pause/Reset and
save/reopen. This exercises existing behavior; M4A adds no editor feature to try.

Git status: all additions remain local and uncommitted on the existing branch.
Nothing staged, committed, pushed, merged, published or submitted as a PR.
The separate `C:/Dev8/QUANTUM-X` tooling checkout was not edited.
