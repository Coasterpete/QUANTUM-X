# Multi-method authoring and native inspection

This extends the graph/timeline foundation in [core-foundation.md](core-foundation.md).
Ordered force-vector, straight, arc, world-geometric, and rider-local sections retain
their original equations and boundary semantics. Spline authoring is an additional
section method. Nothing in this pass simulates a vehicle or changes the physics
crate, its eventual rigid-body responsibilities, or the 1/240 s physics requirement.

## Run the integrated workflows

From the repository root on native Windows:

```powershell
cargo run -p quantum-x -- examples target/pass2-examples
cargo run -p quantum-x -- inspect examples/mixed-authoring.qx.json
cargo run -p quantum-x -- generate examples/spline-comparison.qx.json target/comparison 0.1
cargo run -p quantum-x -- generate examples/numerical-reference.qx.json target/reference 0.1
cargo run -p quantum-x -- generate examples/foundation.qx.json target/foundation 0.1
cargo run -p quantum-x -- move-point examples/spline-comparison.qx.json target/edited.qx.json 101 4 22.125 8.25 4.5
cargo run -p quantum-x -- inspect target/edited.qx.json
cargo run -p quantum-x --release -- benchmark
```

`examples` regenerates all three version-2 fixtures and exports into separate named
directories. The checked-in version-1 foundation fixture remains unchanged. The
numerical reference consists of lines of 30, 20, and 30 m, and a radius-20 m rational
quarter circle: total length `80 + 10*pi` m. The mixed example interleaves all four
splines with force-vector and geometric sections, including a final force section.
These are original development fixtures, not finished coasters or catalog assets.

`move-point` uses document commands, validates the edited document, generates it,
and only then saves the output. It accepts f64 section-local XYZ values and stable
section/control IDs. A handle edit that breaks G1 must be paired with an explicit
join fit using the Rust command API; the single-point CLI never silently repairs it.
The output parent directory must already exist. Existing output files are replaced
using the same recoverable sibling-file saving strategy as Pass 1.

## Representations and evaluation APIs

`qx_geometry::curve::Curve` preserves each representation's authored data. Every
control has an ID unique within its curve, distinct from its position and ordering.
All authoritative geometry, parameters, derivatives, distance, and orientation use
f64. Renderer and widget types never enter the domain crates.

| Representation | Native domain and evaluation |
| --- | --- |
| Cubic Bezier | `3*n+1` ordered controls with shared endpoints; each segment uses a unit span, so domain is `[0,n]`. Endpoint-adjacent controls are explicit tangent handles. De Casteljau evaluation and analytic first/second derivative curves. |
| B-spline | Degree 1 through 8, explicit knot vector of length `control_count + degree + 1`; domain `[knots[degree], knots[control_count]]`. De Boor evaluation and derivative control nets. Uniform unclamped, clamped, and nonuniform knots are supported. Controls generally approximate the curve, rather than interpolate it. |
| NURBS | Same degree/knot model, with positive weights and homogeneous control nets. The quotient rule gives rational derivatives; weights are never ignored. |
| Catmull-Rom | Interpolates all controls. Knot increments are `distance^alpha`: alpha 0 (uniform), 1 (chordal), or 1/2 (default centripetal). Global native domain uses cumulative increments. Reflected endpoint ghosts define one-sided endpoint tangents. |

`Curve::prepare()` validates and builds a `PreparedCurve`, owning reusable cubic
coefficients/control data or homogeneous derivative nets. Repeated sampling uses
that prepared evaluator without rescanning all authored controls. `evaluate(u, side)`
returns position, first derivative `dP/du`, and second derivative `d²P/du²`. Tangent
is the normalized first derivative. The curvature vector is

```text
T = P' / |P'|
dT/ds = (P'' - T*(T dot P'')) / |P'|²
curvature = |dT/ds|
```

For homogeneous position `A` and weight `w`, rational evaluation uses
`P=A/w`, `P'=(A'-P*w')/w`, and
`P''=(A''-2*P'*w'-P*w'')/w`. Derivative nets reduce degree by one per order;
zero knot denominators contribute zero, with explicit one-sided evaluation at
repeated knots. Interior values default to the right side; the last endpoint uses
the left side. Out-of-domain/nonfinite queries return errors.

Catmull-Rom uses the nonuniform interpolant's analytic Hermite tangents. On a
span of length `b`, with previous/next knot increments `a,c`, its start tangent
in normalized span coordinates is
`b*((P1-P0)/a - (P2-P0)/(a+b) + (P2-P1)/b)`, and similarly at the end.
The Hermite polynomial is evaluated through equivalent cubic controls internally;
the persisted Catmull-Rom points and parameterization remain authoritative.

Validation bounds controls to 2..4096, IDs to unique nonzero integers, control
position norm to 1e8 m, knots to finite nondecreasing values bounded by 1e6, and
positive knot spans to at least 1e-10. Parameter-domain width must exceed 1e-9.
Interior multiplicity is at most the degree, preserving position continuity;
endpoint multiplicity is at most degree+1. NURBS weights lie in `[1e-8,1e8]`.
Catmull-Rom adjacent points must be separated by more than 1e-9 m, including its
reflected ghosts. Track validation adds a 65536-total-control budget. Existing
project-file, transition, section, world-coordinate, and sample limits still apply.

Zero/tiny derivatives have valid position evaluations but cannot supply a track
tangent. Track creation rejects undefined endpoint tangents, non-G1 internal joins,
and excessive tangent/frame turns during generation. This is not a certified global
regularity proof: pathological stationary points narrower than sampling/quadrature
resolution remain a limitation. Extremely uneven weights/knots can be ill-conditioned;
the bounds and finite-value checks do not guarantee every allowed configuration is
well-conditioned. Reduce the spatial step or repair such authoring data when errors
report unresolved turns or exhausted quadrature budgets.

## Arc length, errors, and sampling

`ArcLengthTable::build(&prepared, options)` integrates speed `|P'(u)|` separately
on every smooth native knot/segment span. Adaptive Simpson quadrature compares one
whole interval against two half intervals, estimates error as `abs(delta)/15`,
and applies the Richardson correction `delta/15` to the accepted length. It divides
the absolute tolerance budget with parameter-span width and recursive subdivision;
the relative tolerance scales with the estimated length. Default tolerances are
1e-8 m absolute and 1e-10 relative, with depth 24 and 65536 leaves. Depth/leaf
exhaustion is an error, not an accepted low-quality table. Error estimates are not
certified bounds and may underestimate errors for unresolved pathological functions.

Accepted leaf endpoints form a strictly increasing parameter/distance table with
compensated distance accumulation. `distance_at_parameter` integrates the remaining
partial leaf. `parameter_at_distance` binary-searches a distance bracket, then uses
bracketed Newton iterations with a bisection fallback for tiny speed or an escaping
Newton step. Distance residual tolerance is `0.2*absolute + relative*total_length`.
`sample_at_distance` evaluates the actual curve at the inverted parameter. A table
must be used with the same prepared curve it was built from.

`GenerationOptions::spline_arc_length` exposes the tolerance/budget configuration
to the shared solver. Geometry-generation spacing is spatial and independent of
any simulation timestep. Outputs end exactly at native spline breakpoints and
roll-transition boundaries, with at least four intervals per smooth range and a
configured maximum spatial step. Exact analytic positions/tangents are evaluated
at those stations; uniform parameter steps are never treated as equal distances.

Spline frames use two shortest tangent rotations per interval, passing through
the exact midpoint tangent. This approximates rotation-minimizing (Bishop)
transport without a Frenet normal, remaining defined on regular straight spans,
inflections, loops, and vertical tangents. Transport is second-order in spatial
step for smooth resolved spatial curves. Simpson-integrated authored roll is
then applied about the exact final tangent. Roll integration is fourth-order on
smooth intervals. Tangent rotation plus a sampled absolute twist bound obeys the same
angular-step guard as the existing solver (using the maximum absolute sampled roll
rate, so opposite signs do not cancel the guard). Rapid unresolved roll oscillations
still require adequate spatial sampling; no universal adaptive frame-error bound
is claimed. The CLI's step-halving comparison measures accumulated effects,
including downstream sections inheriting an approximate transported frame.

## Section-local coordinates, roll, and continuity

A spline section has `domain=normalized`, `extent=1`, and the same inherited
right-handed frame as other sections: T=+X, N=+Y, L=+Z at identity. Controls are
section-local metres. Its start position must be zero and tangent +X to 1e-8.
Appending maps its controls through the incoming rider frame and anchor position.
No implicit alignment is performed. `CurveEdit::AlignStart` explicitly translates
the start to zero and rotates its tangent to +X using a deterministic shortest
rotation (an arbitrary roll choice in the antiparallel case is documented by this
operation). This preserves the curve's shape; it is an intentional edit.

Spline roll channels use `xi=s/L`, not native spline parameter, with output unit
`radians_per_normalized`. Their value is `dphi/dxi`, so spatial twist is
`channel(xi)/L`. Changing controls or knots recomputes L and retains all normalized
roll transition lengths/values. Total authored roll is unchanged; its rate per
metre changes with length. Existing time/distance sections retain rad/s or rad/m
and their exact Pass 1 rules. Authored twist, transported orientation, curvature,
and gravity-referenced banking are separate quantities. Banking remains undefined
near vertical even though the rider frame remains usable.

`PreparedCurve::continuity()` reports both one-sided evaluations at every internal
join: position gap, tangent angle (including reversed tangents), curvature-vector
gap, first-derivative gap, and second-derivative gap. G0/G1/G2 compare position,
tangent, and arc-domain curvature; C1/C2 compare derivatives in the native parameter.
They are not interchangeable when parameter speeds differ. Convenience predicates
take an absolute tolerance in each tested quantity's units. Use explicit thresholds
appropriate to the application's units rather than interpreting a predicate as
an engineering certification. Track spline sections require G1. G2 failures are
reported and allowed. Standalone curve evaluation can inspect tangent-discontinuous
knots; disconnected positions and tangent jumps are not supported as track sections.

At section boundaries, the pose is inherited by construction. `connections` reports
zero inherited position/tangent/orientation gaps and compares one-sided world
curvature vectors, spatial twist, and reference speed. `AllowRateJump` permits and
warns; `RequireContinuousRates` rejects changes above 1e-8 in the existing units.
Cross-method C1/C2 is not asserted: the comparable coordinate is physical distance,
not arbitrary native parameters. Internal spline joins remain a separate diagnostic.

`FitBezierJoin` supports explicit G1, C1, or C2 fitting at a shared cubic endpoint.
G1 aligns the outgoing handle while preserving its length. C1 mirrors the incoming
handle. C2 also sets the next outgoing control to match second derivatives for the
equal unit parameter spans. These operations change authored controls and undo as
normal edits; warnings never trigger fitting automatically. General endpoint-target
solving, G2 fitting across arbitrary representations, and loop-closure constraints
are not implemented.

## Editing and persistence

`qx_document::command::Editor` continues to provide transactional command batches,
bounded snapshot history, undo, and redo. `InsertSection` creates spline sections;
`EditSpline` accepts `qx_geometry::curve_edit::CurveEdit` for point insertion/removal,
precise point/handle movement, weights, knots, degree with explicit replacement
knots, Catmull-Rom parameterization, alignment, fitting, conversion, or curve
replacement. `EditChannel` edits normalized roll; `ReplaceSection` changes boundary
policy/reference speed. `ValidateConnections` explicitly runs the shared generator
to check generated boundary constraints. Document validation checks authored data;
generation-dependent constraints can still fail on an otherwise valid document.

Coordinated point-count/degree/knot/weight changes belong in one command batch.
The candidate is validated before publication, so invalid edits preserve document
and history. All numerical operations are public Rust APIs; no GUI is required.
Point ordering is representation-local and IDs survive reordering. Exact Boehm
knot insertion for B-spline/NURBS preserves the evaluated curve, updates blended
controls/weights, and retains existing IDs plus a caller-supplied new ID. It does
not promise unchanged positions for the existing control IDs. Arbitrary point
removal changes the curve and may require coordinated knot edits.

Exact conversion currently supports a single cubic Bezier or nonrational B-spline
to NURBS with unit weights. Unsupported conversions fail. Degree editing explicitly
reinterprets controls with new knots; it is not shape-preserving degree elevation.
No approximate conversion is presented as exact.

Schema version 2 persists the new enum variants, all control IDs/positions, degree,
knots, weights, parameterization, normalized roll, and existing section identities
and connection policies. Version-1 loading explicitly rejects version-2-only
constructs, then upgrades the version field without changing original section data.
Saving emits version 2; there is no downgrade path for Pass 1 readers. Unsupported
versions, unknown fields/variants, invalid metadata, and corrupt inputs remain
errors. No derived table, GPU object, selection, or camera enters the project.

## Native development inspector

This temporary engineering UI is authorized by the pass brief. It implements no
production editor layout or spline-control-point workflow. Human-authored UX in
`docs/ux/` remains authoritative. Only Coaster Properties was present in this
checkout; the referenced Inkscape workspace/welcome designs were not available as
repository files, so none were fabricated or replaced.

The app loads documents through `qx-document` and passes actual shared-solver
output to `qx-rendering`. Loading/reloading uses a worker thread after the initial
startup load, retaining the previous scene on failure. `R` explicitly reloads;
there is no filesystem watcher. Drop another project file onto the native window
to load it. Initial load errors print to the invoking terminal; later errors also
appear in the title. The title names the selected section and active diagnostics;
the invoking terminal and `H` expose the complete temporary control list.

| Controls | Action |
| --- | --- |
| Left drag / arrow keys | Orbit |
| Right or middle drag / WASD | Pan |
| Wheel / + and - | Zoom |
| F or Home / Enter | Fit all / fit selected section |
| Tab / 0 | Cycle selection / clear selection |
| 1 / 2 / 3 / 4 | Perspective / top / side (+Z) / front (+X) |
| O | Toggle orthographic |
| G / [ and ] | Toggle reference grid / halve or double spacing |
| N / K | Orientation triads / scaled curvature vectors |
| R / H / Escape | Reload / print help / close |

White boundary crosses mark joins independently of section color. Selection uses
white versus dimmed other sections and a title label. The grid is world XZ at Y=0;
axes at the world origin use X/red, Y/green, Z/blue, with their directions also
documented here. Orientation frames show T/N/L in the same colors. Curvature vector
length is a scaled/clamped diagnostic glyph, not a literal radius or physical force.

Rendering is a depth-tested wgpu line list with a WGSL shader, using winit's native
window/event loop. Positions are rebased in f64 about the scene bounds center
before f32 conversion. Camera transforms use f64 before conversion to GPU matrices.
This preserves local detail around large world offsets, but a single huge scene
still has f32 relative-span limits. Camera motion updates only a uniform buffer;
geometry uploads occur on load, selection, or diagnostic-display changes.
Rendering is event-driven and makes no FPS guarantee.

Minimized zero-sized windows skip configuration/rendering. Resize/DPI changes
reconfigure the surface/depth texture. Outdated/suboptimal surfaces reconfigure;
lost surfaces are recreated; missing adapters/device acquisition errors have
context. Timeout/occlusion skips the frame. Uncaptured GPU errors and device loss
are reported and close the inspector with a restart instruction. Automatic device
recovery is not implemented. Vulkan and DX12 are compiled; actual tested hardware
and unverified recovery cases are recorded in the pass report.

## Diagnostics and costs

The existing centerline, measurements, transitions, OBJ and SVG outputs remain.
`measurements.csv` identifies spline methods separately from force-vector inputs;
authored force columns are empty for splines. Finite-difference curvature, bank,
twist, direction and reference-speed load estimates use the same shared measurement
implementation as other sections. They are not simulated train telemetry.

Additional CSVs are `sections.csv` (IDs, methods, ranges), `connections.csv`
(section-boundary continuity), `curve-joins.csv` (native spline G/C diagnostics),
`numerical.csv` (arc lengths, estimated quadrature errors and work counts), and
`warnings.csv`. Centerline rows keep their old numeric columns; section sample
ranges identify their ownership, with the usual shared endpoint at joins.
OBJ remains a diagnostic centerline, not rails/contact surfaces.

Full deterministic regeneration remains the correctness baseline. No cross-edit
geometry cache is added: each new project/command result invalidates all generated
output. Prepared derivative nets and arc tables are reused within one generation;
de Boor work per evaluation depends on degree, not total control count, with binary
span lookup. Arc inversion does partial quadrature per query, trading work for
accuracy. Snapshot history copies authored controls, and regenerated sample/graph
storage grows with track length divided by spatial step. The benchmark prints an
explicit sample/graph payload estimate rather than claiming process peak memory.
Measured times, reference errors, and test results are in
[authoring-pass-2-report.md](authoring-pass-2-report.md).

Graphics-only dependencies are wgpu 30.0.1 (DX12/Vulkan and WGSL, MIT/Apache-2.0),
winit 0.30.13 (stable native event/window APIs, Apache-2.0), pollster 0.4.0
(startup async bridge, MIT/Apache-2.0), bytemuck 1.25.2 (safe POD buffer packing,
Zlib/Apache-2.0/MIT), and glam 0.30.10 (renderer-side matrices, MIT/Apache-2.0).
The prerelease winit 0.31 is deliberately not selected. Cargo.lock records exact
transitive versions. Primary API references: [wgpu](https://docs.rs/wgpu/30.0.1/wgpu/)
and [winit](https://docs.rs/winit/0.30.13/winit/). No new numerical-framework,
network-service, telemetry, or asset dependencies were introduced.
