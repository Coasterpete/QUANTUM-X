# Train articulation M3A — wheel geometry and alignment diagnostics

Date: 2026-11-07. Native Windows x86_64 MSVC, Rust 1.99.1 / LLVM 23.1.1.
Work continues on `codex/train-articulation-m1`, based on `bcc5add`, in the
existing isolated worktree. M1, M2A and M2B remain local and uncommitted.

**Implemented:** explicit transient wheel geometry, physical rail-reference and
wheel-center path queries, and immutable per-wheel diagnostics of existing rigid
poses. This establishes geometry for Stage 2; it does not fit bogies, change car
placement, or implement wheel contact. Section 10 is a future design proposal.

## 1. Existing wheel-related architecture

The [M1](train-articulation-m1-baseline.md),
[M2A](train-articulation-m2a-chord-robustness.md) and
[M2B](train-articulation-m2b-convergence.md) reports were read before editing.
Their source, diagnostic infrastructure, numerical witnesses and fixtures are
preserved. The Coasterman two-stage placement description supplies the milestone
context; no external implementation or mesh-derived mechanism was imported.

| Existing data / owner | What it can establish | What it cannot establish |
| --- | --- | --- |
| `qx-document::train::Component` | Stable rigid-component identity; component origin, local reference positions, dimensions, mass, COM and inertia | Wheel radius, tread, axle direction, contact side or mechanical rail association |
| `wheel_references_m` | Component-local markers, also included in M1 rigid-distance observations | They are explicitly references without contact semantics; no automatic conversion to mechanical wheels |
| `Car::bogie_ids` | Car grouping, including references to shared running gear | Additional physical joints or duplicated shared bodies/wheels |
| `Connection` / `JointModel` | Explicit anchor pairs and ball, fixed, hinge or roll-lock frames/limits | A universal two-bogie or manufacturer-specific mechanism |
| `Placement::Track` / `Mounted` | Initial component-local pose guesses | Welds or prescribed simulated motion |
| Guide points / `TrackAttitude` | Existing bilateral centerline and running-gear twist approximations | Wheel-level running surfaces or contact forces |
| `GeneratedTrack` | f64 physical stations, positions, full frames, joins; Hermite positions and shortest quaternion interpolation | A verified mechanical running surface or wheel-center locus by itself |
| `TrackConstruction::continuous` | Named `Role::Running` profile origins, track-frame offsets, profile geometry/rotation | Which face/tread a wheel contacts; radius, compliance, clearance, wear, contact patch or surface normal conventions |
| Validated construction map | Exact coverage and identical running members across structural variants | A rail morph, adapter surface, or automatic authored-to-physical station remapping |
| Component `Asset` / GLB placement | Visual pivot, translation, rotation and scale correction | Mechanical wheel locations, pivots, joints or mass properties |

Relevant implementations remain `qx-document/src/train.rs`, `train/mechanism.rs`,
`qx-physics/src/consist.rs`, `assembly.rs`, `solver.rs`, and
`apps/quantum-x/src/authoring/{session,train_preview,train_assets}.rs`.
The preview draws wheel markers as crosses. Its asset correction is separate
from the component-origin transform; M3A never opens or queries a GLB.

For rider-reference authoring, the application already derives its physical
track through `supports::physical_attachment_track`. Callers must pass that
derived view to the new API, as the existing train pipeline does. A raw
`GeneratedTrack` carries no role tag, so the adapter cannot detect an incorrectly
supplied rider path. A dedicated regression verifies the actual inverse-reference
adapter against an independent straight-track world solution.

## 2. Geometry types and ownership semantics — implemented

New modules:

- `qx-geometry/src/rail_geometry.rs`: `PhysicalRailGeometry`, `RailReference`,
  `WheelCenterPath`, `PathSample` and exact interpolation derivatives.
- `qx-geometry/src/rail_geometry/nearest.rs`: bounded local queries and explicit
  result classifications.
- `qx-physics/src/wheel_geometry.rs`: `WheelGeometry`, `WheelTarget`, optional
  `WheelRole`, world transforms, joint/attachment observations and aggregates.

All are transient Rust types, without serde, schema fields, caches in the
document, dependencies on graphics, or new third-party dependencies. Existing
project schema **7** and editor data are unchanged.

A wheel has its own ID, owning **rigid component ID**, component-local center in
metres, local unit quaternion, optional running/side-guide/up-stop role, and an
explicit target plus independent station window. Wheel-local +X is its declared
rolling direction, +Y a radial reference, and +Z its axle. These axes need not
coincide with component axes, joint frames or asset axes. Shared running gear
owns one wheel set even if several cars refer to it. Wheels on separate pieces
of a multipart assembly belong to their actual rigid pieces, not a fictitious
common transform.

`WheelTarget::Declared` requires an explicit wheel-center locus.
`WheelTarget::Unavailable(reason)` records missing mechanical metadata. Merely
having a persisted wheel marker never supplies that declaration. Roles are
descriptive; none introduces a contact law or assumes a side of a rail.
Ownership and arbitrary local frames permit suspended/flying/articulated
configurations without claiming production implementations of all of them.

## 3. Physical rail-target construction — implemented

For physical centerline position `C(s)` and its full frame rotation `R(s)`:

```text
rail profile-origin path: G(s) = C(s) + R(s) a
wheel-center locus:       W(s) = C(s) + R(s) (a + b)
derivative:               W'(s) = C'(s) + omega(s) cross R(s)(a + b)
```

`a` is the selected running member's geometric offset. `b` is a separately
declared reference-to-wheel-center offset. Both use track T/N/L metres. The
station `s` remains the **physical centerline arc coordinate**, not wheel-path
arc length. Longitudinal offsets are permitted and differentiated as well.

`RailReference::from_construction` selects a named running sweep, preserving its
authored offset; non-running/missing IDs fail. It does not use triangulated
circles, extract surfaces from asset sections, or treat profile rotation as a
wheel-contact specification. `from_map` uses the validated identical running
members and checks the supplied physical track length. Correct physical station
mapping/track identity remains a caller responsibility; equal length alone does
not prove two tracks are the same.

**Mechanical surface gap:** current profiles and running roles do not
unambiguously define a mechanical running surface. The characterization wheels
therefore use explicitly declared synthetic loci: generally rail origins at
`(0,0,+/-0.6)` m and running-wheel offsets `(0,0.18,0)` m. The 0.18 m is a
declared test center offset, not an inferred radius or validated contact model.
Additional side-guide/up-stop fixtures declare their own offsets and frames.

The evaluator differentiates the actual Hermite position polynomial, rather
than substituting its independently interpolated frame tangent. Frame rotation
includes both quaternion interpolation branches: constant-rate slerp and the
existing normalized-linear small-angle branch, whose angular speed varies with
station. The latter's angular acceleration is included in search error bounds.
Tangents therefore follow offset curves, banking transitions, helices and
inversions instead of incorrectly following the centerline direction.

At an exact interpolation knot the outgoing derivative is used (incoming at
the final open endpoint). A differing incoming tangent is also exposed when
directions differ by more than 1e-6 in unit-vector norm. At such a corner no
unique transverse plane or wheel-direction error is asserted. The diagnostic
uses point-to-curve separation there and leaves lateral/vertical components
unavailable. Interpolation derivative jumps can be numerical as well as authored;
the flag does not classify their cause or smooth them away.

### Bounded nearest-path queries

Each wheel supplies `[min_m,max_m]` in physical stations. Queries binary-search
only relevant interpolation pieces. Within each piece a second-derivative bound
`M` sets subdivision counts so the linearization deviation is bounded by
`M * ds² / 8`. The bound includes centerline curvature and frame rotation.
Each resulting line segment receives an independent closest-point projection;
the selected station is then evaluated on the actual interpolated locus.

Defaults: **16 m** maximum window span, **1e-6 m** linearization allowance,
**1e-5 m** branch-distance ambiguity allowance, **4096** segments per wheel.
Explicit options permit at most 1000 m span, 16384 segments and allowances in
`[1e-9,1e-3]` m. Stations are bounded to +/-1e7 m. Track validation is bounded
to one million samples. A scale-dependent `256 * EPSILON` roundoff margin must
remain below one quarter of the linearization allowance; otherwise the query
fails. Roundoff-sized clipped tails are measured without dividing by tiny
segment lengths. Nonfinite, singular, invalid-topology and exhausted searches
return errors rather than a plausible-looking remote target.

The reported distance uncertainty is twice the linearization allowance plus
roundoff and any accepted seam gap. This describes distance-to-curve accuracy,
**not a station error guarantee**. It uses analytic smooth-piece bounds and a
floating-point allowance; it is not an interval-arithmetic proof. Generated-track
approximation error is separate. Perpendicular metrics also retain the actual
longitudinal residual and full separation at the approximate station.

Near-minimum segments form connected station neighborhoods. Multiple separated
neighborhoods, or a flat/uncertain minimum reaching both ends, report
`Ambiguous`; no representative is silently designated the correct branch.
An endpoint within the uncertainty/ambiguity band reports `BoundaryMinimum`.
An open-end point can have zero tangent-normal distance while remaining metres
beyond the path; full separation and the boundary classification expose that.
`Resolved` means one interior neighborhood at this resolution, not a proof of
unique stationary roots inside that neighborhood or global track feasibility.

Open and Shuttle queries reject out-of-range windows. Circuit queries require
explicit Circuit intent and the existing position/tangent/frame/join closure
contract. Windows span less than one circuit; returned stations are unwrapped
and sampling alone uses modulo. Pieces split at the seam. A seam gap in the
declared locus above the linearization allowance is unavailable even if the
looser topology closure contract accepts it. No extrapolation, seam smoothing,
or topology-aware whole-consist initialization was added.

## 4. Per-wheel alignment metrics — implemented

`wheel_geometry::diagnose` accepts an immutable `ConsistInspection` at component
origins. For playback, use the existing `TrackSimulation::snapshot` /
`consist::snapshot` conversion from mass centers. It never invokes assembly.

For a transformed wheel center `P`, nearest target `W(s)`, unit target tangent
`t`, and `d = P-W(s)`:

```text
longitudinal residual = dot(d,t)                         [m]
transverse vector     = d - t*dot(d,t)                   [m]
per-wheel error       = length(transverse vector)       [m]
RMS                   = sqrt(sum(error_i²) / N)         [m]
maximum               = max(error_i)                    [m]
```

Each row retains world center/orientation, role, target station, station segment,
rail-origin and wheel-center targets, derivative/tangent, full separation,
longitudinal residual, uncertainty/work and classification. Lateral is the
track lateral projected into the target-normal plane; vertical is
`lateral cross tangent`. These are **rail-relative**, including inversions,
not gravity vertical. Degenerate axes/corners omit those components. A directed
rolling-axis-to-tangent angle is separately reported in radians when defined.

RMS/max are supplied only for a nonempty set whose **every wheel** has a resolved
target. Missing, invalid, ambiguous, boundary-limited or exhausted rows suppress
whole-set aggregates. Individual measurements remain inspectable. There is no
new wheel acceptance threshold or change to assembly acceptance; an average
never establishes that every wheel fits.

Joint anchor gap vectors and existing angular-constraint errors are recomputed
from the same immutable poses, independently of wheel errors. Pairwise rigid
distances among local wheel centers, origins and mechanical anchors produce a
separate attachment-roundoff metric. M1/M2B guide and rail-attitude residuals
remain in their existing report and are not relabeled wheel errors.

Work limits are 4096 wheels, **262144** charged search segments per report, and
**262144** attachment pairs. Failed searches conservatively charge their full
per-call allowance; successful work counts are exact. Exhaustion is explicit,
including unvisited rows. No timers, searches or allocation paths were installed
in ordinary inspection, assembly sweeps or simulation ticks.

## 5. Numerical fixtures and measured results

Tests live in `qx-geometry/tests/rail_geometry.rs`, its analytic fixture helper,
and `qx-physics/tests/wheel_geometry.rs`. They are development geometry, not
owner-authored coaster designs or manufacturer models. The conventional wheel
fixture has two longitudinal rows at +/-0.3 m and two declared rails per gear.

Native Release observations (metres):

| Existing pose fixture | Wheels | Assembly sweeps | Wheel RMS | Maximum wheel error | Maximum joint gap |
| --- | ---: | ---: | ---: | ---: | ---: |
| Straight, two conventional cars | 16 | 1 | 0 | 0 | 5.636e-16 |
| Tight horizontal R=8 m, one car | 8 | 17 | 3.184319e-3 | 3.580408e-3 | 9.274240e-6 |
| Banked R=12 m, bank .6 rad | 8 | 17 | 2.100882e-3 | 2.280540e-3 | 5.619406e-6 |
| Same arc, changing bank | 8 | 17 | 2.107052e-3 | 2.336383e-3 | 4.906421e-6 |
| Helix R=8 m, rise .3, twist .04 rad/m | 8 | 17 | 3.058665e-3 | 3.767272e-3 | 7.489632e-6 |
| Helix, bank `.4+.02*s+.001*s²` | 8 | 25 | 7.205663e-3 | 9.950053e-3 | 1.463739e-6 |
| Offset/asymmetric bogie pivots | 8 | 41 | 2.101014e-3 | 2.280650e-3 | 7.785653e-6 |
| Below-track two-car arrangement, changing bank | 16 | 97 | 2.106144e-3 | 2.336182e-3 | 9.847947e-6 |
| Vertical loop R=8 m, top attitude | 8 | 17 | 3.253618e-3 | 3.254214e-3 | 7.492059e-6 |
| Inverted straight | 8 | 1 | 0 | 0 | 0 |

Asymmetric local suspension anchors retain the M1 fixture coordinates. Separate
straight fixtures add side-guide and up-stop wheels with rotated axle frames.
Visual corrections and COM changes do not alter the component-origin wheel
observations. Shared car grouping does not duplicate an owner's wheels.

The original M2B curved and changing-bank train results remain:

| Cars / wheels | Curved sweeps | Curved wheel RMS / max (m) | Changing-bank sweeps | Changing-bank wheel RMS / max (m) |
| --- | ---: | --- | ---: | --- |
| 2 / 16 | 41 | 5.559132e-4 / 5.692211e-4 | 41 | 6.306936e-4 / 7.199076e-4 |
| 8 / 64 | 153 | 5.559637e-4 / 5.694364e-4 | 153 | 6.225535e-4 / 7.199387e-4 |
| 16 / 128 | 521 | 5.559774e-4 / 5.693788e-4 | 513 | 6.126202e-4 / 7.199378e-4 |
| 32 / 256 | 1881 | 5.559869e-4 / 5.693861e-4 | 1873 | 5.961033e-4 / 7.199373e-4 |

For the curved two-car fixture, the first gear's four target stations are
`200.904092990`, `200.896091720`, `201.496195586`, `201.504196551` m.
Both longitudinal rows and left/right wheels acquire their own stations.
Rigid attachment discrepancies across these measured fixtures stay below
2.3e-14 m. These wheel errors do not invalidate the independently reported
mechanical closure, and mechanical closure does not make them zero.

Independent checks include:

- Closed-form world positions for circles, helices with several twist laws,
  straight twisting offsets, vertical loops and inverted axes. The dense analytic
  helix matrix's maximum target-position discrepancy is 2.926e-9 m and maximum
  station discrepancy 2.901e-9 m. This measures those fixtures, not every query.
- Analytic offset derivatives and centered differences of the pre-existing
  Hermite/quaternion sampler, including both interpolation branches, very small
  rotations, and equivalent opposite-sign quaternion samples.
- An independent quaternion rotation matrix for world wheel centers/axles,
  plus analytic straight-car stations and physical rider-to-track inversion.
- Vertical-loop transverse offsets .04/.03 m give .05 m error, including the
  vertical tangents and top inversion. Abrupt banking derivatives expose both
  one-sided offset tangents instead of inventing a unique transverse frame.
- A 0.02/0.03 m outlier has 0.0360555 m maximum and 0.00901388 m RMS among
  sixteen wheels, with unchanged joint gaps. The maximum remains visible.
- Two coincident local wheel centers assigned to parallel loci 1.2 m apart
  cannot both fit in any rigid pose: the triangle inequality forces at least
  .6 m maximum error. The measured pose reports **1.2 m max / .8485281 m RMS**.
- A twice-traversed circle reports two near-minimum station neighborhoods;
  a circle-center point reports an ambiguous flat minimum. Local windows avoid
  silently choosing the other lap. Endpoints, insufficient coordinate precision,
  discontinuous seams, invalid offsets and bounded work failures are explicit.
- A 512-wheel dense-query fixture reaches the 262144-segment report budget,
  marks remaining rows unavailable by budget, and suppresses aggregates.
- Repeated complete reports are deterministic. Train, track, poses and wheel
  definitions remain unchanged. Four successive 1/240 s playback steps match
  an unobserved control world, snapshots and telemetry exactly, including a
  nonzero COM offset and nonzero initial speed.

## 6. Limitations on twisting track

A rigid gear cannot generally match every independently twisting wheel-center
path. In the severe straight-twist fixture, twist is 1 rad/m and wheel rows are
1 m apart. The unchanged test pose reports **.2643972 m** RMS/max. Tightening
linearization from 1e-6 to 1e-8 m leaves that result equal at printed precision.
This residual is not evidence that the nearest-path arithmetic failed.

At symmetric intended stations, an independently evaluated same-rail wheel pair
has a **.1665208 m** distance mismatch with the rigid local wheel pair. Thus
those prescribed targets cannot be obtained by a rigid transform. This is not
a proof that no other pose/station combination could improve the fit; M3A does
not optimize or certify global feasibility. Suspension travel, axle articulation,
real tread geometry and mechanical clearances would need explicit models.

The two-car suspended fixture is the existing below-track numerical arrangement,
not a manufacturer-accurate suspended train. Flying vehicles and general
multipart wheel carriers remain future model/metadata work. No wheel loads,
unilateral contact, friction, rolling constraints or ride-safety conclusions can
be inferred from center alignment alone.

## 7. Missing production metadata

Production support needs human/mechanically authored wheel and rail definitions:
stable wheel/axle/carrier identities; owning rigid frames; hub centers and axle
directions; radius/tread/profile and contact-reference conventions; intended
rail and contact side; running/guide/up-stop classification; nominal clearance,
permitted travel, suspension/compliance and articulation limits; and explicit
contact surface/normal geometry with compatible units and physical stations.

Track/style compatibility, metadata versions/provenance, discontinuous rail
adapters, shared assemblies, unsupported roles and error policy also need
definition. A visual profile origin, GLB mesh, component box or optional wheel
role is insufficient. M3A deliberately adds none of these to persisted projects.

## 8. Performance observations

Before editing, the unchanged M1 ordinary/instrumented inspection timers ran on
the accumulated M2B implementation. After implementation the same tests ran
again. Each timer warms up, then records seven batches of five solves. Track
generation, compilation and wheel diagnostics are excluded from ordinary
inspection timings. Values are microseconds per solve: median [min,max].

| Ordinary inspection | Before M3A (us) | After M3A (us) |
| --- | --- | --- |
| Straight / 2 | 17.440 [17.380,25.160] | 17.580 [17.160,19.520] |
| Straight / 8 | 75.500 [74.680,98.780] | 75.280 [74.200,77.180] |
| Straight / 16 | 148.660 [147.620,191.920] | 150.440 [148.760,154.420] |
| Straight / 32 | 311.260 [310.040,357.200] | 313.640 [311.440,324.260] |
| Curved / 2 | 455.740 [453.520,483.920] | 453.860 [453.720,455.720] |
| Curved / 8 | 7203.600 [7155.460,7330.340] | 7230.340 [7163.320,7364.300] |
| Changing bank / 8 | 7157.440 [7142.240,7242.120] | 7334.760 [7264.080,7395.180] |

All seven exact pose/station/anchor fingerprints are unchanged. Source-level
inspection confirms no ordinary-path call to the new layer. These shared-machine
samples, including the changing-bank timing shift, are not a controlled
performance-equivalence proof. Compilation/code placement, CPU frequency and
other machine load were not isolated.

Opt-in wheel report timing uses existing poses and seven batches of five calls.
It includes train/pose validation, independent joint/attachment measurements,
wheel queries and report allocations, with no assembly inside the timer. A
separate timer measures one-time physical geometry validation:

| Fixture | Wheels / segments | Wheel report (us) | Geometry setup (us) |
| --- | ---: | --- | --- |
| Straight / 8 | 64 / 1314 | 372.240 [365.100,412.240] | 94.200 [92.840,97.400] |
| Curved / 8 | 64 / 7744 | 1389.080 [1381.640,1409.460] | 169.760 [168.600,181.800] |
| Changing bank / 32 | 256 / 30972 | 5741.100 [5709.480,5752.540] | 230.360 [225.580,266.360] |

The wheel layer can cost substantially more than straight assembly. It should
remain opt-in, reuse the validated immutable track borrow, and run only on
requested snapshots. These are inspection/diagnostic times, not physics-tick
costs, contact-solver budgets, viewport FPS or a universal performance guarantee.
The unchanged M2B long-chain timer also passed: curved 16/32-car medians were
49198.733 / 356137.167 us; changing-bank 16/32-car medians were
48462.600 / 358227.733 us, with the original successful closure counts.

## 9. Regression verification and integration

Use `PhysicalRailGeometry::new(physical_track, layout)`, explicit wheel
declarations and `wheel_geometry::diagnose(train, snapshot, geometry, wheels,
options)`. The test sources provide complete reproducible declarations and
calls. Nothing is attached to an editor command or automatic playback path.

Verification commands, run from the isolated worktree:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p qx-physics -p qx-document --tests
cargo test --release -p qx-geometry --test rail_geometry -- --nocapture
cargo test --release -p qx-physics --test assembly_convergence --test assembly_diagnostics --test chord_characterization --test initialization_diagnostics --test wheel_geometry -- --nocapture --test-threads=1
cargo test --release -p qx-physics --test assembly_convergence -- --ignored --nocapture --test-threads=1
cargo test --release -p qx-physics --test articulation_baseline --test wheel_geometry -- --ignored --nocapture --test-threads=1
git diff --check
```

Results:

- Full workspace tests: **450 passed, zero failed, seven ignored** in the
  ordinary run. Targeted physics/document tests: **124 passed, zero failed,
  six ignored**. The unrelated opt-in graph-precision GPU evidence export
  remained ignored; the six articulation tests were run explicitly below.
- Native Release characterization: **40 passed, zero failed** (31 physics,
  including the retained 21 M1–M2B cases, and nine rail-geometry tests).
- All **six** articulation timing/characterization tests were explicitly run
  in Release and passed: three unchanged M2B tests, two unchanged M1 timers,
  and the new wheel timing test. Timings are observations, not CI thresholds.
- Workspace build check, strict all-target workspace Clippy and formatting
  check: **passed**.
- Preservation hashes cover 103 pre-existing source/test/report files. **101
  are byte-for-byte unchanged**; only the two module roots gained M3A exports.
  In particular, assembly, M2B relaxation, chord solving, consist initialization,
  dynamics, document/style sources, all prior fixtures/tests and all three
  milestone reports are unchanged. All seven ordinary inspection fingerprints
  also match their pre-edit values exactly.
- New source/report files were checked separately with Git's no-index
  whitespace checks; tracked `git diff --check` also passed. No file was staged.

Local raw evidence is under ignored `target/m3a-evidence/`; no logs or generated
build artifacts are runtime dependencies.

No assembly tolerance, relaxation strategy, sweep limit, graph solver, physical
playback, 1/240 s timestep, impulse iterations or substeps changed. The original
independently feasible asymmetric witnesses, asymmetric 32-car failures,
contradictory edges, shared-gear configurations, chord robustness, deterministic
replay and document compatibility tests remain in place.

**Not manually verified:** desktop UI, rendering, cameras, DPI, subjective
playback or full application Release packaging. No desktop control or GUI
launch was used. Optional owner checklist: run
`cargo run -p quantum-x --release -- edit`, open an existing interior train
project, inspect/initialize, Play/Pause/Reset, then save/reopen. M3A adds no UI
whose presence that checklist should be expected to demonstrate.

UX status: no owner-approved interaction behavior changed, no temporary
developer UI introduced, and no new UX decision is required for this API. Future
production wheel editing and diagnostics presentation require human design.

Git status: local and uncommitted on `codex/train-articulation-m1`. Nothing was
committed, pushed, merged, published or submitted as a PR. The separate tooling
checkout at `C:/Dev8/QUANTUM-X` was not edited.

## 10. Proposed M3B joint-space bogie fitting — not implemented

1. Start from a successful, independently checked M2B assembly. Require explicit
   mechanical wheel definitions and available target geometry. Keep the original
   poses intact as the transactional fallback; do not treat a failed assembly
   as an acceptable fitting seed or use wheel residuals to hide joint failures.
2. Define local coordinates from actual `JointModel` frames and admissible
   degrees of freedom: three relative rotational coordinates for a ball joint,
   allowed swing coordinates for a roll lock, one limited hinge coordinate,
   none for a fixed joint. Rotations act about authored attachment points.
   Do not add independent six-DOF bogie transforms that detach anchors.
3. Build a kinematic adapter for the supported subgraph. Trees can propagate
   rigid transforms from a fixed car frame. Shared gear, multiple attachments
   and closed loops need explicit compatibility/closure constraints or an
   unavailable classification; a sequential tree traversal cannot claim to
   solve every mechanical graph. Preserve car/gear identities and dimensions.
4. Minimize unit-explicit transverse wheel residual vectors with bounded
   independent station coordinates or variable projection onto the declared
   local paths. Longitudinal stations must not be shared across wheel rows or
   rails. Keep joint closure/limits as hard constraints, with wheel orientation
   terms separately specified rather than arbitrarily mixing radians and metres.
5. A later damped Gauss–Newton/trust-region implementation could use analytic
   rigid-point rotational Jacobians and target-path tangents, verified against
   independent finite differences. Rank deficiency, twist corners, near-zero
   tangents, multiple branches, active hinge limits and inconsistent targets
   must be detected explicitly. M3A implements no such optimizer/Jacobian solve.
6. Bound iterations, evaluations, angular steps, station travel and diagnostic
   work. Continuation must be keyed to the same track/topology/metadata and
   nearby pose; invalid hints require a bounded fresh solve, never a remote
   branch jump. Preserve open/shuttle boundaries and explicit circuit intent.
7. Review per-wheel maxima, RMS, every relevant joint/attachment/guide/angular
   residual and rigid dimensions independently. Specify supported tolerances
   and an honest best-attainable/unsupported outcome for twisting track before
   using any fit for initialization. A smaller objective alone is not success.
   Existing paired centerline guides and rail attitudes can already remove
   the available fitting degrees of freedom. Do not silently soften those
   constraints to lower the wheel objective. A fit incompatible with the
   existing playback guidance must remain unsupported; replacing guidance
   with wheel contact is separate work. Otherwise playback could immediately
   drive a fitted pose back toward its old guide manifold.
8. Test successful conventional/asymmetric/suspended/shared configurations,
   independent target witnesses, infeasible coincident targets, severe twist,
   topology seams, order/replay determinism and rollback. Compare costs with
   this M3A baseline. Keep fitting opt-in initialization work and preserve
   authoritative independent rigid-body integration during playback.

Physical contact, friction, spin, nonzero-speed velocity consistency/projection,
forces and suspension dynamics remain separate work. No timestep or M2B sweep
budget increase is part of this proposal.
