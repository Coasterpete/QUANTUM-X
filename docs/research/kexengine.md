# KexEngine engineering reference — M1.1

Reviewed 2026-11-03 against upstream commit
[`9847fea0568f1c5a59e4464eb8431013dec8aa9b`](https://github.com/IndividualKex/kexengine/tree/9847fea0568f1c5a59e4464eb8431013dec8aa9b).
The separate checkout is `C:/Dev8/reference-kexengine-m1.1`; its working tree
remained clean. These are source observations and engineering proposals, not
claims that either engine has been independently certified or benchmarked against
the other. No KexEngine source, dependency, FFI, or solver was added to QUANTUM-X.

## Scope and provenance

All nine requested files exist at that revision and were inspected. Additional
checks covered `sim/forces.rs`, `sim/curvature.rs`, the manifest, README, and tree.
`cargo test --locked --target-dir C:/Dev8/QUANTUM-X/target/m1.1-evidence/kex-target`
passed **143 upstream unit tests**; one documentation example was ignored.
Log: `target/m1.1-evidence/kex-tests.log`. Passing those tests is evidence about
their tested contracts, not a cross-engine accuracy comparison.

The [manifest](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/Cargo.toml#L1-L21)
declares MIT, edition 2021, `cdylib`/`rlib`, default FFI, and `approx`.
The [README](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/README.md)
also declares MIT. **No standalone license/copyright notice is present in this
checked tree.** Before any future source reuse, obtain/verify the applicable
copyright and full MIT notice, preserve both in distributions containing copied
substantial portions, and review provenance. A manifest declaration alone is not
a complete attribution record. This milestone uses conceptual study only.

## Coordinate, unit, and model boundaries

| Quantity | KexEngine at reviewed commit | QUANTUM Dynamics at M1.0/M1.1 |
| --- | --- | --- |
| Scalar precision | `f32` positions, forces, times, quaternions | `f64` authored geometry, scalar profiles, generation and vehicle dynamics |
| Default frame | tangent `(0,0,-1)`, normal `(0,-1,0)`, lateral `(1,0,0)` | tangent `+X`, rider normal `+Y`, lateral `+Z` |
| Orientation | three vectors rotated by quaternions; Gram-Schmidt correction | normalized quaternion state; derived orthonormal frame |
| Force authoring | normal/lateral values in g; standard gravity `9.80665` | normal/lateral profiles in g; configured positive gravity |
| Authoring integration | fixed `DT=0.01 s`, 100 Hz; distance sections still step through time | spatial RK4, knot-aware stations; section time/distance domain converted using explicit section design speed |
| Design speed | driven velocity curve, or gravity/friction/drag energy update | explicit section reference speed; no adoption of Kex's energy evolution |
| Vehicle dynamics | reviewed modules generate paths from authored inputs | independent rigid bodies/constraints, outer timestep `1/240 s`, optional internal substeps |

Sources: [Kex frame](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/src/sim/frame.rs#L20-L102),
[physics constants](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/src/sim/physics.rs#L1-L9),
QUANTUM [`math.rs`](../../crates/qx-geometry/src/math.rs),
[`authoring.rs`](../../crates/qx-geometry/src/authoring.rs),
[`solver.rs`](../../crates/qx-geometry/src/solver.rs), and
[`qx-physics/solver.rs`](../../crates/qx-physics/src/solver.rs).
The Kex frame comment describes an upward normal, but its default and cross
products use a downward one. Compare executable conventions, not just names.

For future comparisons, a proper world rotation mapping a Kex vector to QUANTUM
coordinates is `(-z, y, x)`. It preserves world up and maps the default tangent to
`+X`. It maps the Kex default normal to `-Y`, so normal vectors and signed rates
must be interpreted separately. In the default pose, inspection of the force
rotation equations predicts equal normal commands and opposite lateral-command
signs for the same initial bending direction. Positive roll is also applied with
opposite axial rotation signs. These are **inferences from the equations**, not
a validated global channel-conversion API; banking, steering and noncommuting
rotation order need explicit paired fixtures before using such a mapping.

No trajectory error ranking is asserted here. A valid comparison must align
frames, heartline versus spine reference, g versus SI acceleration, radians versus
degrees, domain units, driven-speed assumptions and exact interval endpoints.
Use each method's native step size, convergence sweeps and declared tolerances;
do not compare identical raw array indices or assume `f32` and `f64` tolerances
should be interchangeable.

## A. Force-vector generation and numerical stability

[`nodes/force.rs:8–125`](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/src/nodes/force.rs#L8-L125)
projects gravity plus commanded force into the preceding frame, derives angular
increments from acceleration, velocity and 100 Hz, applies normal/lateral
quaternions, then reorthonormalizes. Estimated speed uses preceding heartline
advance when available. Minimum velocity guards prevent division by near zero;
per-step bending angles are clamped to ±0.5 rad. Those guards alter the computed
path in extreme cases; they are not accuracy guarantees.

Position uses old/new tangents with half-step travel and an offset correction
between heartline and spine. Roll is applied afterward. Heart and spine advances
are accumulated separately. Non-driven energy uses the change in a center
position placed at **0.9 times heart offset**, plus spine travel for friction.
That center fraction is a model assumption, not an identified vehicle mass model.

[`sim/physics.rs:31–48`](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/src/sim/physics.rs#L31-L48)
updates squared speed from local elevation/friction work and a cubic-speed drag
loss, then clamps nonpositive squared speed to zero. Working with local height
differences avoids subtracting large absolute potential energies. The drag
coefficient is not sufficient evidence of a calibrated aerodynamic model without
its units and parameter provenance.

The [time and distance builders](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/src/nodes/force.rs#L191-L350)
evaluate keyframes in different input domains. Time loops use floored point counts
and exclude the final loop index; distance loops can cross the requested terminal
station. Minimum speed, maximum speed/force and iteration limits terminate
generation, generally by returning the points produced so far rather than a
structured completion error. These behaviors matter in any endpoint comparison.

QUANTUM's [`Context::omega`, RK4 step](../../crates/qx-geometry/src/solver.rs)
instead derives spatial curvature from `(commanded specific acceleration +
gravity projection) / design_speed²`. It integrates position/quaternion with
one-sided endpoint evaluations and rejects excessive rotation per step rather
than silently clamping authored forces. [`heartline.rs`](../../crates/qx-geometry/src/heartline.rs)
keeps offset path jets and point-motion terms distinguishable from track generation.
Kex's non-driven design-speed feature could inspire an opt-in future design-speed
model; it must not replace QUANTUM's independent vehicle solver.

## B. Geometric and circular authoring

[`nodes/geometric.rs:78–244`](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/src/nodes/geometric.rs#L78-L244)
uses rates times `1/100 s` for time sections, and rates times `v/100` for distance
sections. The [step](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/src/nodes/geometric.rs#L246-L365)
has two steering paths: one removes accumulated roll, rotates the unrolled frame,
and reapplies roll; the other rotates the current frame, applies a heart/spine
offset correction, then adds roll. Yaw is about world up. Pitch axes depend on
direction and the chosen sign of up. This is not an automatic equivalence to
QUANTUM's rider-local pitch/yaw vector-curvature definition.

QUANTUM distinguishes rider-local (`normal*pitch + lateral*yaw`) from
world-geometric rates and explicitly rejects undefined gravity-referenced axes
near vertical. Comparisons should include level, banked, vertical-approach and
reversing poses, with convergence checks rather than a single attractive path.

[`nodes/curved.rs:9–229`](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/src/nodes/curved.rs#L9-L229)
handles radius/arc/plane-axis inputs, with degrees for arc increments and plane
axis and radians for quaternion rotations and roll rates. Lead-in/out use
smoothstep damping and an empirical `1.997` distance factor. The method restores
roll relative to a world-up frame and falls back at a near-vertical lateral axis.
This is useful research material for explicitly requested lead-ins, not authority
to reshape owner-authored arc regions or add a transition type.

## C. Keyframes, derivatives and continuity

[`sim/keyframe.rs:60–163`](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/src/sim/keyframe.rs#L60-L163)
binary-searches sorted keyframes and supports constant, linear and weighted cubic
Bezier segments. Weighted tangents define a 2D time/value curve, so evaluation
inverts its time coordinate using up to eight Newton iterations, clamped to
`[0,1]`, before evaluating its value coordinate. The time residual stopping
threshold is `1e-6`; a derivative magnitude below `1e-9` stops iteration.

That evaluator does not return an achieved inversion residual or a derivative
jet, and this module does not enforce a C1/C2 contract. Ordering, duplicate times
and monotonicity of the weighted time curve require caller assumptions. The
weighted 2D Bezier representation is not QUANTUM's scalar rational B-spline.

QUANTUM's [`profile.rs`](../../crates/qx-geometry/src/profile.rs) and
[`profile_transition.rs`](../../crates/qx-geometry/src/profile_transition.rs)
already provide degree 3–5 positive-weight rational profiles, explicit knot
continuity, analytic first/second derivatives, exact subdivision and constrained
editing. Evaluation has an explicit normalized input; there is no weighted
abscissa inversion to borrow. M1.1 fixes display approximation of this existing
math. It does not expose mathematical implementation names as new authoring UX.

## D. Arc length and resampling

[`track/spline.rs:44–222`](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/src/track/spline.rs#L44-L222)
resamples an existing point sequence using its accumulated spine arc: choose
`ceil(length/resolution)+1` stations, binary-search each interval, linearly
interpolate spine positions, and normalize interpolated frame vectors
independently. Physics telemetry is interpolated separately.

This approximates an already discretized path. It does **not** integrate the
arc length of a native NURBS or prove continuous-curve error. Independently
normalizing three interpolated vectors need not preserve mutual orthogonality.
Empty/single/zero-length input is handled, but `resolution` is not validated here
before sample-count arithmetic/allocation. Do not transplant that input boundary.

QUANTUM already has span-wise adaptive speed quadrature and inversion in
[`arc_length.rs`](../../crates/qx-geometry/src/arc_length.rs), plus generated-track
distance queries using Hermite position and quaternion slerp. Its Simpson error
estimate is documented as an estimate, not a certified bound. Useful future work
is a common comparison suite for positional error, frame orthogonality and
telemetry alignment across these two distinct resampling problems. Uniform
resampling cannot recover a feature omitted by the original source samples;
M1.1 therefore starts from native scalar knot spans.

## E. Internal evaluation graph

[`graph/traversal.rs:5–118`](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/src/graph/traversal.rs#L5-L118)
finds sources/sinks, deduplicates predecessor/successor node IDs, queues ready
nodes and rejects cycles when traversal cannot visit every node. It is Kahn-style
topological traversal with repeated array lookups rather than an indegree table.
[`track/evaluate.rs:6–30`](https://github.com/IndividualKex/kexengine/blob/9847fea0568f1c5a59e4464eb8431013dec8aa9b/src/track/evaluate.rs#L6-L30)
evaluates that order into an `EvaluationResult`; absent/unmapped node types are
skipped. A successful `Option` does not itself establish complete output for every
unknown node.

QUANTUM's ordered Regions already define upstream pose dependencies. If profiling
justifies incremental generation, stable Region IDs and explicit dependency
invalidation are useful ideas. They do not require a node-based user interface
or a generalized graph framework. Preserve the current Region workflow.

## Prioritized M2 proposals — no M2 functionality implemented

| Priority | Kind | Independently testable proposal |
| --- | --- | --- |
| P1 | Correctness, existing QUANTUM findings | Address the separately reported transaction-scope and transition-length issues in their own approved pass. Preserve drafts/history and neighboring Region invariants; add the original reproductions. M1.1 found no blocker requiring their implementation. |
| P1 | Numerical verification | Add paired analytic force/geometric fixtures with documented frame/sign transforms, constant-speed assumptions, heart/spine references and step-convergence tables. Include flat 1 g, planar curvature, banking and vertical approaches. Keep QX `f64` and 240 Hz vehicle dynamics. |
| P1 | Numerical diagnostics | Audit cancellation, frame drift, near-zero speed, exact terminal stations and incomplete generation reports. Prefer explicit errors/residuals to silent clamps. Kex's delta-energy approach is a useful reference, not a reason to alter speed semantics in place. |
| P2 | Correctness coverage | Extend native-curve arc-length and generated-path resampling fixtures with orthonormality, opposing-frame, extreme-weight, repeated-knot and telemetry alignment checks. Validate all resampling allocation budgets. |
| P2 | Optional model | Propose a distinct, opt-in evolving design-speed model with SI resistance parameters and energy-work tests. Any visible workflow needs owner design. It remains separate from rigid-body simulation. |
| P3 | Optional performance | Measure plot preparation and generation costs, then consider revision-keyed plot caches and ordered-Region prefix reuse. Test invalidation on knots, weights, view changes, undo and reopening before adopting caches. |
| P3 | Optional analysis | Expand derivative/continuity diagnostics using existing analytic profile jets. Use OpenFVeiD/FVD++ for research with their licensing boundaries; do not copy GPL implementation into this project. |

These are proposals. They neither finalize UX nor authorize automatic smoothing,
new node controls, replacement solvers, dependency additions or source reuse.
