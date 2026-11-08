# Train articulation M3B — mechanical freedom and bounded bogie fitting

Date: 2026-11-07. Native Windows x86_64 MSVC, Rust 1.99.1 / LLVM 23.1.1.
Local work continues on `codex/train-articulation-m1` at `bcc5add`, in the existing
isolated train-articulation worktree. M1–M3A and their accumulated changes remain
uncommitted and preserved.

**Result:** a deliberately narrow, opt-in, one-coordinate geometric fitting
prototype improves an explicitly authored, limited-hinge wheel carrier without
changing its mechanical graph or guidance. It also works with the existing
below-track arrangement. Conventional paired-guide bogies have no independent
rotational fitting freedom with their parent held fixed; they are not fitted.
This is initialization research, not a replacement for guidance or wheel contact.

## 1. Preparation and mechanical freedom audit

Read repository `AGENTS.md`, the [M1](train-articulation-m1-baseline.md),
[M2A](train-articulation-m2a-chord-robustness.md),
[M2B](train-articulation-m2b-convergence.md), and
[M3A](train-articulation-m3a-wheel-geometry.md) reports, document train/mechanism
definitions, assembly, consist initialization/snapshot, angular constraints,
track guides/attitudes, rigid-body playback, and M3A wheel/rail queries.

All positions/lengths below use metres, station means physical centerline arc
coordinate, and angular coordinates/errors use radians. Component +X/+Y/+Z
are forward/up/lateral; joint frames and wheel frames are independently authored.

| Connection / configuration | Motion before guidance | Remaining fitting freedom |
| --- | --- | --- |
| Ball at one attachment | Three relative rotations about the coincident anchors | Only the intersection with every other joint, guide and attitude; not three independent bogie rotations after those constraints |
| Limited hinge | One rotation about joint-frame Z, within the authored interval | Available if the actual guide geometry and rail attitude permit that axis; at a bound only inward travel is available |
| Roll-lock bearing | Two swing coordinates; zero swing/twist-decomposition twist about joint X | Not arbitrary Euler pitch/yaw with roll corrected afterward; general fitting coordinates are not implemented here |
| Fixed connection | No relative rotation or translation | None; moving one side would violate the frame and/or anchors |
| Offset pivot | Same angular freedoms as its joint; component-origin translation follows rotation about the authored attachment | Not a license to translate the bogie independently |
| Two separated centerline guides + rail attitude | Guides constrain transverse motion of two rigid points; rail attitude constrains remaining twist | On a straight track, pitch/yaw/roll are all constrained. A remaining station slide is not independent angular fitting and is coupled through mechanical attachments/the assembly pin |
| Single guide at a hinge pivot | Guide point is invariant under hinge rotation | A swing hinge can retain one useful coordinate; a roll hinge is blocked by the rail attitude |
| Shared gear | One rigid body, potentially attached to several cars | Multiple attachments constrain the same body. Grouping references do not add bodies or free transforms |
| Suspended running gear | Gear remains rail-guided; passenger body can swing at a ball suspension | Passenger swing is not extra rotation of a paired-guide wheel carrier; an additional authored swing carrier can retain a useful coordinate |
| Coupled graph / closed loop | Depends on all edge/guide Jacobians | No tree or independent-leaf assumption is valid for a moving component with several connections; unsupported by this prototype |

The straight rotational audit independently perturbs all three world axes. For
guide points at `+/-a X`, pitch and yaw produce transverse derivatives of norm
`a`; rail twist observes roll with derivative one. A three-row rotational minor
has determinant magnitude `a²` (`a=.2 m` here). This establishes no independent
rotational null direction about that pivot. On curves the constraint intersection
must still be evaluated; schema acceptance alone is not a mobility calculation.
Coincident/redundant guide points must not be counted as separated constraints.

An unguided leaf at a ball joint can also have freedom in the current data model,
but that does not justify pretending that the existing guided bogies are free.
Ball/roll-lock parameterizations and moving subtrees are outside this prototype.
Free drawbar roll may have no wheel objective at all. A passenger body swinging
below the track does not move wheels owned by a different rigid component.

## 2. Guide compatibility and feasibility checkpoint

The conventional development gear has local guides at `(+/-.2,0,0)` m and a
separate running-gear twist constraint. Removing either guide, changing the
component kind to avoid rail attitude, softening tolerances, or moving its car
would change the problem. None is done. `Placement::Mounted` is an initial
guess, not an extra weld; actual mechanical connections remain authoritative.

The checkpoint passed **before the optimizer existed**, using a separate
analytic witness in `tests/bogie_fitting.rs`. The new fixture **adds** an authored
leaf carrier to the front gear of the unchanged one-car conventional or
below-track fixture. It does not delete or replace any existing guides/edges.
The carrier has:

- One limited free hinge, joint Z mapped to parent +Y by nonidentity joint frames.
- Parent anchor at the front gear origin, child anchor `(.07,0,-.03)` m.
- One explicit centerline guide at that child anchor and its normal running-gear
  rail attitude. Rotation is swing about +Y, not prohibited rail roll.
- Four explicit M3A wheel definitions on the actual carrier, with local centers
  `pivot + Ry(-.12) * (x,.18,z)`, `x=+/-.3`, `z=+/-.6` m. Targets are declared
  rail origins `(0,0,z)` plus `(0,.18,0)` m, with independent station windows.

On straight track, the independent witness uses `R=Ry(.12)` and
`origin=parent_anchor-R*child_anchor`. Direct sine/cosine pivot rotation checks
anchor closure; the relative quaternion has zero X twist. All four target
stations are `26.2+x` m at lead station 25 m. The analytic witness reduces
carrier-only RMS/max from `.0361719337365 / .0402284806744 m` to roundoff
(`5.55e-17 / 1.11e-16 m` in the initial run), with zero-to-roundoff joint/guide/
angular/rail errors. Both conventional and below-track cases pass.

This is a synthetic, explicitly authored mechanical configuration supported by
the existing guide and impulse models, not a claimed manufacturer bogie. It is
the reason a narrow optimizer is justified. It does **not** establish independent
fitting compatibility for the conventional paired-guide gear.

## 3. Implemented algorithm and ownership

New files:

- `qx-physics/src/bogie_fitting.rs`: immutable `FitSeed`, bounded scalar fitting,
  options, reports and the analytic frozen-target Jacobian.
- `qx-physics/src/bogie_fitting/mechanics.rs`: actual joint-coordinate adapter,
  invariant-guide gate, full existing-constraint observations, derived residual
  refresh and rigid-dimension checks.
- `qx-physics/tests/bogie_fitting.rs` and `tests/support/bogie_fixtures.rs`:
  analytic witnesses, compatibility/failure/regression tests and Release timing.

`FitSeed::assemble(train, physical_track, station)` first obtains a **successful**
measured M2B assembly and validates the same immutable physical track for M3A
queries. Private seed fields prevent fabricated pose/evidence pairs. Fitting
borrows that seed and returns a selected inspection/report; no world is modified.
The normal assembly, initializer, simulation worker and physics step do not call it.

Only the B component of a selected **mechanical leaf**, attached by one free
limited hinge, is moved. All parent/other poses stay fixed. A parent may belong
to a larger coupled graph because its pose is not an optimization variable.
Moving subtrees, multiple child attachments, unrestricted hinges, drives, balls,
roll locks and fixed edges are explicitly unsupported by the fitting adapter.

For parent world joint quaternion `Qa`, authored child joint frame `Fb`, child
anchor `b`, world attachment `A` and permitted joint angle `theta`:

```text
Qchild(theta) = Qa * Rz(theta) * inverse(Fb)
Pchild(theta) = A - Qchild(theta) * b
```

This preserves anchor closure by construction and uses authored frames on both
sides. No component dimensions, anchors, guides or wheel definitions change.
The initial hinge coordinate uses the existing principal-angle convention.
An initial angle fractionally outside its exact authored interval is rejected,
even if M2B's angular tolerance allowed it; limits are not silently enlarged.

Every moving guide must lie on the authored hinge axis (radial allowance
`1e-10 m`). This is tested relative to the **child anchor**, so M2B's residual
inter-body gap is not mistaken for an off-axis guide. Rail attitude remains a
separate hard acceptance gate; a guide-compatible roll hinge can still fail.

Each wheel is projected onto its own declared path and original window by the
bounded M3A query. No wheel row or rail shares a forced station. For world wheel
point `p`, target `w`, unit target tangent `t`, pivot `A` and unit hinge axis `n`:

```text
r = (I - t*t^T) * (p-w)                         [m]
J = (I - t*t^T) * (n cross (p-A))               [m/rad]
g = sum(J dot r); H = sum(J dot J)
delta = -g / (H * (1 + damping))               [rad]
```

This is a scalar damped Gauss–Newton model with a bounded angular step. Initial
damping is `1e-6` times `H`; accepted proposals reduce it by ten down to `1e-9`,
rejected proposals increase it by ten with a minimum of one. Targets and all
constraints are re-evaluated for each proposal. `H<=1e-16 m²/rad²` is classified
as rank deficient **before damping**. Damping cannot create observable motion.
No matrix solver or third-party optimizer dependency is added.

The Jacobian freezes the target/tangent for its local model. It is **not** the
exact derivative of a curved nearest-path projector at finite residual. Tests
compare it with independent Rodrigues-formula centered differences at steps
`1e-4/1e-5/1e-6 rad` (error below `3e-9 m/rad`), and independently reprojected
straight targets. Actual curved candidates never rely on that model for success.
There is no claimed curved-projector derivative validation or convergence proof.

## 4. Bounds and acceptance

Defaults: 24 outer iterations, 32 total wheel-report evaluations (baseline
included), `.05 rad` step, `.35 rad` total travel from the original angle,
`.25 m` station travel from **each wheel's original resolved station**, and the
authored hinge interval. Options cap these at 64 iterations, 96 evaluations,
`.1 rad` step, `.5 rad` total travel and 1 m station travel. Invalid/nonfinite
options are errors. A failed proposal never advances the current coordinate.

Fit snapshots are bounded to 128 components, 256 connections, 256 wheels and
32 guide/reference markers per component. M3A retains 4096 segments per query
and 262144 charged segments per report, plus its attachment budget. Thus even
rejected searches and diagnostic allocations have bounded work. Seed assembly
retains its existing larger domain limits and **2400-sweep** cap.

Every proposal is compared with the **original M2B wheel report**, not merely
the preceding iterate. Acceptance requires:

1. Every joint gap at most `1e-5 m`, angular violation at most `1e-5 rad`, every
   guide at most `1e-5 m`, rail twist at most `1e-5 rad`; original guide pin and
   bounded M2B guide-station neighborhoods retained.
2. Exact selected-hinge limit clipping, rigid pair/dimension discrepancies at
   most `1e-10 m`, including guide/reference points and component box diagonals.
   M3A independently measures wheel/anchor/origin rigid pairs.
3. Every wheel has one interior, smooth, unambiguous target. A boundary result,
   missing locus, exhausted search, ambiguity or tangent corner rejects it.
4. No individual observed transverse wheel error increases by more than
   `1e-10 m` roundoff allowance, and each original station-travel bound holds.
5. The current least-squares objective decreases, and the **returned** fitted
   pose improves both original RMS and original maximum by more than `1e-5 m`.

The micrometre-scale M3A target-query uncertainty remains in every row. The
per-wheel comparison is a comparison of these measured errors, not an interval
proof about exact track geometry or manufacturer contact surfaces. RMS and max
are reported independently; neither stands in for individual-wheel checks.

The report preserves the baseline and every evaluated proposal, wheel rows,
joint/guide/angular/rail measurements and rejection reasons. Connection fields
in a fitted inspection are refreshed from its actual anchors; they are not stale
M2B values. `accepted=false` returns the original inspection exactly. Invalid
inputs or measurement errors can instead return `Err`, without mutating the seed.

## 5. Fixtures and before/after results

All fixtures are development mechanics, not designed coasters or release assets.
The M3A analytic rail matrix and M2B independently constructed mechanical
witnesses remain intact. Curved, tight, bank-transition, helix, offset-pivot and
below-track paired-guide cases retain their M2B poses and measured wheel errors;
the new optimizer does not manufacture freedom for them.

Native Release observations (all errors in metres):

| Fixture / wheels measured | Original RMS / max | Returned RMS / max | Iterations / evaluations | Outcome |
| --- | --- | --- | --- | --- |
| Added hinge carrier, conventional / 12 | .020883876 / .040228481 | 2.9811e-15 / 5.2181e-15 | 5 / 5 | Accepted, Stationary |
| Added hinge carrier, below-track / 12 | .020883876 / .040228481 | 2.9811e-15 / 5.2181e-15 | 5 / 5 | Accepted, Stationary |
| Distinct joint frames, bank .4 / 12 | .008667465 / .015743595 | 6.1261e-12 / 1.0611e-11 | 3 / 3 | Accepted, Stationary |
| Distinct joint frames, inverted / 12 | .008667465 / .015743595 | 6.1262e-12 / 1.0611e-11 | 3 / 3 | Accepted, Stationary |
| Hinge upper limit .04 rad / 4 | .036171934 / .040228481 | .024051086 / .025893384 | 2 / 2 | Accepted partial improvement, AngularBound |
| Coincident incompatible wheels / 2 | .6 / .6 | .6 / .6 | 1 / 1 | Original retained, RankDeficient |
| Initially correct fifth wheel / 5 | .032353161 / .040228481 | .032353161 / .040228481 | 11 / 11 | Original retained, NoAdmissibleStep |
| Severe straight twist / 4 | .155481962 / .185324650 | .155481962 / .185324650 | 11 / 11 | Original retained, NoAdmissibleStep |

The 12-wheel rows include eight unmoved parent-gear wheels with zero error.
Carrier-only RMS in the first two rows is `.0361719337365 m` initially and
approximately `5.2e-15 m` finally. The report never hides the four carrier wheels
inside the whole-train RMS. Their individual errors in declaration order are:

| Wheel (x,z), m | Original error | Final error, conventional and below-track |
| --- | ---: | ---: |
| (-.3,-.6) | .0315988436990 | 5.2180482e-15 |
| (-.3,+.6) | .0402284806744 | 5.2180482e-15 |
| (+.3,-.6) | .0402284806744 | 4.9960036e-15 |
| (+.3,+.6) | .0315988436990 | 5.2180482e-15 |

At the `.04 rad` hinge limit, the four remaining errors are
`[.0220554319723, .0258933844092, .0258933844092, .0220554319723] m`.
This is an allowed bounded improvement with substantial residual, not an exact
fit. The distinct-frame fixtures begin at `.07 rad` component yaw and recover
`.12 rad` component yaw through `.05 rad` **joint** travel; the `.4 rad` bank
and inversion remain intact. This checks both frames rather than assuming the
hinge axis, component axes and world axes coincide.

For unsupported conventional gear, returned errors are unchanged:

| Paired-guide fixture | RMS / max (m) | Joint gap / guide residual (m) |
| --- | --- | --- |
| Straight, one car | 0 / 0 | 0 / 0 |
| R=12, bank .6 | .002100882 / .002280540 | 5.619406e-6 / 5.113446e-7 |
| Tight R=8 | .003184319 / .003580408 | 9.274240e-6 / 9.195943e-7 |
| Bank transition, R=12 | .002107052 / .002336383 | 4.906421e-6 / 4.480339e-7 |
| Helix R=8, rise .3, twist .04 | .003058665 / .003767272 | 7.489632e-6 / 9.313489e-7 |
| Offset/asymmetric pivots, changing bank | .002107209 / .002336350 | 7.761056e-6 / 2.232742e-6 |
| Two below-track cars, changing bank | .002106144 / .002336182 | 9.847947e-6 / 3.067562e-6 |

Each of these performs one baseline wheel evaluation and zero fitting
iterations. `UnsupportedGraph` describes the adapter, not mechanical
infeasibility; all seven M2B assemblies succeeded. The separate limited-hinge
fixture with an additional off-axis guide directly tests the guide-freedom
rejection rather than relying only on the ball/roll-lock adapter rejection.

Mechanical validation of accepted fits:

| Accepted fixture | Maximum joint / guide (m) | Angular / rail (rad) | Wheel/anchor rigid discrepancy (m) |
| --- | --- | --- | --- |
| Conventional carrier | 0 / 0 | 0 / 0 | 2.109424e-15 |
| Below-track carrier | 0 / 0 | 0 / 0 | 2.109424e-15 |
| Distinct frames, bank .4 | 1.091464e-20 / 1.091464e-20 | 1.665335e-16 / 0 | 1.443290e-15 |
| Distinct frames, inversion | 4.372931e-20 / 4.372931e-20 | 2.220446e-16 / 0 | 1.443290e-15 |
| Active hinge limit | 0 / 0 | 2.220446e-16 / 0 | 1.332268e-15 |

Dimension/guide/reference pair tests independently stay below `1e-12 m` for
these accepted fixtures. Parent/other rigid poses, component identities and all
authored local data remain unchanged. The severe-twist fallback retains its
original joint gap `1.752798e-6 m`, guide residual `3.266755e-6 m`, angular
violation `4.888812e-6 rad` and rail residual `4.440892e-16 rad`.

Deterministic fixture acceptance rates: each of the five accepted rows passed
**2/2 identical calls**; incompatible-center, individual-veto, severe-twist and
rail-attitude-veto cases each passed **0/2**. Off-axis guide, additional coupled
edge, ball, fixed and roll-lock adapter fixtures each passed **0/2**. Missing,
boundary, excessive-window and ambiguous-target fixtures each passed **0/2**.
The seven conventional-pose matrix entries each passed **0/1**, explicitly
unsupported. These are fixed repeatability observations, not random-trial
success probabilities or estimates of support across arbitrary mechanisms.

## 6. Numerical failure handling and unsupported configurations

Terminations distinguish a stationary wheel model, no admissible step under
hard gates, angular/travel bounds, zero scalar rank, iteration/evaluation limits,
invalid targets, unsupported graphs and nonfinite model arithmetic. A tiny step
caused by heavy rejection damping is **NoAdmissibleStep**, not evidence that the
wheel objective is stationary. Budget exhaustion may retain an already accepted
valid improvement; `accepted` is independent of the stopping reason. It never
makes an unvalidated last proposal the returned pose.

Tests include active hinge limits; missing, boundary, excessive-window and truly
ambiguous repeated-track targets; malformed wheel geometry/options; a failed
M2B seed; individual-wheel veto despite lower RMS; rail-twist veto despite lower
RMS; exact coincident-center incompatibility/rank deficiency; off-axis paired
guides; extra shared attachments; fixed/ball/roll-lock adapters; severe twist;
and deterministic repeat reports. The repeated-circle query uses a separate
small two-body mechanism; it does not relax a conventional production train to
make an otherwise failed assembly usable.

Two coincident rigid wheel centers assigned to parallel loci 1.2 m apart cannot
both fit: the maximum separation error is at least .6 m by the triangle
inequality. The fixture is already at this minimax lower bound and has zero
observable hinge column; it returns the original pose and `RankDeficient`.

The severe straight-twist track rotates 1 rad/m. At prescribed stations
`s0+/-.3`, a same-rail target pair has distance
`sqrt(.6² + 4*(.18²+.6²)*sin(.3)²)`, whereas its rigid wheel pair is .6 m apart.
This independently proves an exact fit to those prescribed targets impossible.
Stations are variables in the actual fit, so that pair argument is **not** a
global lower-bound proof over every pose/station combination. The bounded fit
attempts are rejected when individual wheels worsen, leaving large measured
residuals explicit. Neither failure nor a smaller objective certifies a global
best fit or general mechanical infeasibility.

General ball/roll-lock fitting, several moving joints, shared moving gear,
closed-loop kinematics, branch-continuation proofs between finite proposals,
manufacturer mechanisms and suspension travel/compliance remain unsupported.
Every queried candidate must be unambiguous in its unchanged local windows;
there is no global nearest-branch or global optimization guarantee.

## 7. Physics authority, integration and UX

Example developer integration (no editor command or schema fields):

```rust,ignore
let seed = qx_physics::bogie_fitting::FitSeed::assemble(&train, &physical_track, 25.)?;
let report = seed.fit("carrier-bearing", &explicit_m3a_wheels, Default::default())?;
// report.pose is the validated candidate only when report.accepted is true;
// otherwise it is the original successful assembly. Inspect the reports first.
```

The fitter neither installs poses into an ongoing world nor projects velocities.
No contact forces, wheel spin, friction, compliance, impulses or dynamics are
added. M2B's 64 unrelaxed sweeps / 1.85 relaxation, 2400-sweep budget, original
closure tolerances, fixed **1/240 s**, 32 impulse iterations and four playback
substeps are unchanged. Whole-consist seam initialization is still unsupported.

Regression compares two independent playback worlds at nonzero initial speed
5 m/s and nonzero carrier COM offset, while fitting the immutable seed between
steps. Four successive steps have identical worlds, diagnostics and snapshots.
This confirms observation has no playback effect; it does not validate starting
a new simulation from the fitted pose, nonzero-speed velocity consistency or
contact dynamics. Production playback still assembles through its original path.

No owner-approved interaction behavior changed, no developer UI was introduced,
and no new UX decision is required for this transient API. Future mechanical
metadata editing, fitting commands and diagnostic presentation need human design.
No document, style, asset, renderer, dependency or project-schema change is made.

## 8. Performance and verification

Native Release fit timing uses a preassembled immutable seed and already
validated geometry, warms up, then measures seven batches of five fits with
`black_box`. It includes all baseline/candidate wheel queries, constraint checks,
report allocations and pose copies. It excludes assembly, track generation,
geometry validation, compilation and physics ticks. Values are microseconds per
fit, median [minimum, maximum] of batch means:

| Fit fixture | Result | Iterations / evaluations | Fit duration (us) |
| --- | --- | --- | --- |
| Conventional hinge carrier, four wheels | Accepted | 5 / 5 | 150.280 [148.660,155.860] |
| Below-track hinge carrier, four wheels | Accepted | 5 / 5 | 149.180 [148.640,162.900] |
| Individual-wheel veto, five wheels | Original retained | 11 / 11 | 387.700 [385.600,396.740] |
| Severe twist, four wheels | Original retained | 11 / 11 | 2938.560 [2931.500,2967.980] |

The fit timer's four-wheel set differs from the 12-wheel whole-fixture reports
above; unchanged parent wheels are not included in that timer. Both sets use
the same constraints and exact carrier solution. Timings are shared-machine
observations on the existing Intel Core i7-14700KF system, not CI thresholds,
controlled CPU-frequency measurements or performance guarantees for other graphs.

Unchanged M1 ordinary assembly timing ran **before editing** and after M3B.
It warms up and measures seven batches of five complete ordinary inspections,
excluding track generation and all fitting/diagnostic work:

| Ordinary assembly | Before M3B (us) | After M3B (us) |
| --- | --- | --- |
| Straight / 2 cars | 18.040 [17.680,19.140] | 17.480 [17.360,19.220] |
| Straight / 8 | 75.800 [73.980,84.500] | 74.300 [73.400,76.880] |
| Straight / 16 | 154.100 [150.400,171.000] | 147.980 [146.980,152.660] |
| Straight / 32 | 332.480 [325.640,347.800] | 319.100 [310.300,339.220] |
| Curved / 2 | 457.900 [455.700,473.420] | 457.240 [454.000,471.060] |
| Curved / 8 | 7227.640 [7193.140,7334.260] | 7239.380 [7202.840,7344.960] |
| Changing bank / 8 | 7213.480 [7194.740,7226.660] | 7433.820 [7375.760,7499.520] |

All seven exact pose/station/anchor fingerprints match before/after. No new
call exists on the ordinary assembly or simulation path. The changing-bank
timing increase and other variation are not a controlled proof of performance
equivalence or of an implementation-induced slowdown. M2B's unchanged long-chain
timer also passes: curved 16/32-car medians are 49654.933 / 359516.133 us;
changing-bank 16/32 medians are 48808.933 / 355884.533 us, with the original
521/1881 and 513/1873 successful sweep counts. These are assembly costs, not
physics-tick costs. Physics-tick time was not newly benchmarked for M3B.

Commands executed from the existing isolated worktree:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p qx-physics -p qx-document --tests
cargo test --release -p qx-geometry --test rail_geometry -- --nocapture
cargo test --release -p qx-physics --test assembly_convergence --test assembly_diagnostics --test chord_characterization --test initialization_diagnostics --test wheel_geometry --test bogie_fitting -- --nocapture --test-threads=1
cargo test --release -p qx-physics --test assembly_convergence --test articulation_baseline --test wheel_geometry --test bogie_fitting -- --ignored --nocapture --test-threads=1
git diff --check
```

Results:

- Full workspace: **462 passed, zero failed, eight ignored** in the ordinary
  run. The unrelated graph-precision GPU evidence exporter remains ignored;
  all seven articulation ignored tests were explicitly run in Release.
- Final physics/document suite: **136 passed, zero failed, seven ignored**;
  all seven ignored tests are covered by the explicit Release run.
- Native Release characterization: **52 passed, zero failed**: 43 physics
  tests (including 12 M3B tests and all 31 retained M1–M3A physics
  characterization tests) and nine M3A rail-geometry tests.
- Native Release ignored characterization/timing: **seven passed**, zero failed:
  all six retained tests and the new fit timer.
- The analytic feasibility witness was executed and passed **before adding
  the fitting implementation**; its raw output is retained separately.
- Workspace check, strict all-target workspace Clippy, formatting and tracked
  diff checks passed. New untracked M3B files also received separate Git
  no-index whitespace checks.
- Preservation hashes cover **311** pre-existing source/test/document files:
  **310 unchanged**, with only `qx-physics/src/lib.rs` gaining the module export.
  In particular, every prior milestone report, fixture, test, assembly/chord/
  guide/impulse implementation, document/style source and playback implementation
  is byte-for-byte unchanged. No earlier test/tolerance was weakened.

The workspace run was already in progress when the final no-admissible-step
termination label and Release timing coverage were refined. The final physics/
document suite and native Release characterization were run separately against
the finished source; no application/runtime integration changed in that refinement.
Local raw logs and preservation manifests are under ignored
`target/m3b-evidence/`; no generated artifacts are runtime dependencies.

**Not manually verified:** desktop interaction, rendering, cameras, DPI,
subjective playback, a new application Release package or a simulation initialized
from a returned fitted inspection. No GUI was launched or controlled. Optional
owner checklist: run `cargo run -p quantum-x --release -- edit`, open an existing
interior train project, inspect/initialize, exercise Play/Pause/Reset, then
save/reopen. This remains a regression checklist for existing UI; M3B adds no
interactive fitting command and does not make conventional gear independently fit-able.

## 9. Focused next-milestone recommendations

1. Review **mechanical guidance** before attempting conventional bogie fitting.
   Decide which actual physical attachment/axle constraints replace or coexist
   with paired centerline guides and rail twist. Require independent feasible
   witnesses and initial/playback constraint consistency. Do not simply delete
   one guide or soften it to lower wheel error.
2. Expand joint-coordinate support only against authored mechanisms with useful
   null directions: ball/roll-lock swing charts, multi-part wheel carriers and
   coupled closure. Detect full Jacobian rank/limits before introducing a sparse
   multi-variable solver. The current scalar rank test cannot certify graph mobility.
3. Retain the explicit per-wheel veto, original-pose rollback, hard mechanical
   gates and M1–M3A regressions. Specify whether future fitting should allow
   clearly presented individual tradeoffs; this prototype does not.
4. Keep wheel/rail mechanical metadata, actual contact physics, nonzero-speed
   velocity initialization and topology-aware circuit initialization separate.
   The M1 velocity mismatch and M2B asymmetric 32-car failures remain visible.

Git: all work is local and uncommitted on `codex/train-articulation-m1`.
Nothing staged, committed, pushed, merged, published or submitted as a PR.
The separate `C:/Dev8/QUANTUM-X` tooling checkout is untouched.
