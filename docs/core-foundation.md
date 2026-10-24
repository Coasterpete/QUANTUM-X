# Integrated core foundation

## Scope and execution

The executable in `apps/quantum-x` is a development-only command-line diagnostic.
It creates or loads an authored project, generates geometry, saves/reopens the
project, requires identical regeneration, and exports inspectable results:

```powershell
cargo run -p quantum-x -- demo target/foundation-demo
cargo run -p quantum-x -- generate examples/foundation.qx.json target/generated 0.1
```

The optional final number for `generate` is the maximum integration step in
metres. The demo uses 0.1 m. Output directories are created if needed; named
diagnostic files in an existing output directory are replaced. A failed generation
does not create the output directory. Export I/O failures may leave incomplete
diagnostic files, but authored project saving uses recoverable replacement.

| File | Meaning |
| --- | --- |
| `project.qx.json` | Validated version-1 authored document; no derived meshes, frames, UI pointers, or device resources. |
| `centerline.csv` | Endpoint positions, rider/contact frames, quaternions, integrated distance, and reference-design time. |
| `measurements.csv` | Interval-midpoint derived measurements and separate authored channels; empty cells mean not applicable/undefined. |
| `transitions.csv` | Each individual transition's values and analytic derivatives, including both one-sided endpoints. |
| `centerline.obj` | Vertices and line segments in metres, Y up; a diagnostic centerline, not physical rails. |
| `trajectory.svg` | Side/plan projections. Axes scale independently; these are diagnostic plots, not a CAD viewport. |
| `measurements.svg` | Generated measurements with authored force overlays where applicable. |
| `transitions.svg` | Individual authored section channels with explicit domain/output units. |
| `summary.txt` | Section ranges, endpoint, warning details, regeneration check, and step-halving comparison. |

PNG previews are not a runtime dependency or automatic executable output.

## Domain and crate boundaries

`qx-geometry` owns `math`, `transition`, `authoring`, and `solver`. It depends only
on Serde for passive data serialization. `qx-style` reads a data-only catalog in
`assets/catalog/styles.json`; it does not depend on geometry. `qx-document` owns
the versioned project, shared edit commands, history, persistence, and original
example. The app composes those services and exports their results. No new crates
were needed. Audio, rendering, and physics placeholders remain untouched.

There was no existing spline implementation to preserve in this checkout.
Additional spline representations can later produce comparable sampled geometry
without replacing the graph/timeline authoring model.

## Coordinates and units

World coordinates are right-handed. The identity rider frame has tangent
`T = +X`, normal `N = +Y`, and lateral `L = +Z`, with `T × N = L`.
`N` means rider/track normal, not necessarily a Frenet normal. A Hamilton unit
quaternion maps this local frame into world coordinates. Positions and distance
use metres, time seconds, angles radians, speed m/s, and gravity m/s².
Gravity is `G = (0, -g, 0)`; default `g = 9.80665`.

Positive normal load is toward `N`, positive lateral load toward `L`. A level
unbanked straight at constant speed therefore measures `(normal, lateral) =
(1, 0)` g. Positive roll is a right-handed twist about `T`, rotating `N` toward
`L`. Angular channels are **rates**, not angles: rad/s in a time section or rad/m
in a distance section. Force channels are dimensionless g multiples in either
domain. Their analytic derivatives use g/s or g/m respectively.

Each section covers the explicit local input interval `[0, extent]`. Ordering
determines connections. The initial pose is the first starting boundary; every
subsequent section inherits its predecessor's endpoint pose. Generated section
ranges expose both boundary sample indices and global distance bounds. Adjacent
sections share a single endpoint sample. Section IDs are nonzero stable integers;
transition IDs are nonzero and unique within their channel.

## Governing equations

All geometry is integrated by spatial arc coordinate `s`, with `|dP/ds| = 1`.
For a time-domain section, `s = v_ref x`, where `x` is local authored time.
For a distance-domain section, `s = x`. Reference speed is constant within each
section; absent an override it uses the initial design speed. It is never a
simulated vehicle state. Design time accumulates `ds / v_ref` across sections.

For rider curvature components `kN`, `kL` and twist `tau`:

```text
dP/ds = T
dT/ds = kN N + kL L
omega_world = T × (dT/ds) + tau T = kN L - kL N + tau T
dq/ds = 0.5 * (0, omega_world) * q
```

This is a transported orthonormal rider frame with explicit authored twist.
It stays defined on straight track, inflections, and loops. The optional Frenet
normal is `(dT/ds)/|dT/ds|`, and the Frenet binormal is its cross product with `T`
in the order `T × FrenetNormal`; both are absent below curvature `1e-10 /m`.

### Force-vector sections

Given authored normal/lateral specific-force multiples `n`, `l`:

```text
kN = (g*n + G·N) / v_ref²
kL = (g*l + G·L) / v_ref²
```

This follows by projecting `v_ref² dT/ds = G + F_specific` onto `N` and `L`.
The tangent component of gravity is excluded from curvature; maintaining constant
design speed implicitly requires longitudinal specific force `-G·T`. This is a
geometry-design constraint, not an energy-conserving trajectory or dynamics model.
A banked straight may require nonzero lateral authored force to balance gravity.
No heartline offset, rolling resistance, drag, or vehicle mass is modeled here.

### Geometric sections

World-geometric controls describe pitch angle change toward gravity-referenced
up and heading angle change toward +Z from +X (rotation about world -Y):

```text
U = normalize(Y - (Y·T) T)
dT/ds = pitch_rate_spatial * U + yaw_rate_spatial * (T × Y)
```

Convert time rates to spatial rates by dividing by reference speed. This definition
properly reduces heading's directional effect near vertical. World heading/pitch
coordinates are singular when the horizontal tangent magnitude is at most `1e-6`;
generation rejects world-geometric sections there with a rider-local alternative
in the error. It does not silently select an arbitrary axis.

Rider-local geometric controls instead set `kN = pitch_rate_spatial` and
`kL = yaw_rate_spatial`. Banking therefore changes their bending plane. They
remain usable through vertical tangents and inversions.

Straight sections set `kN = kL = 0` while allowing twist. An arc sets
`kN = cos(plane_angle)/radius`, `kL = sin(plane_angle)/radius`. With zero twist
this is a planar circular arc. Nonzero twist rotates the curvature plane and
therefore generally generates a spatial curve instead of a planar circle.

### Roll, banking, and graph measurements

`tau` is rider-frame twist about `T`. It is not generally the derivative of
gravity-referenced banking. Bank uses `atan2(N·(T × U), N·U)`, wrapped to
`[-pi, pi]` and absent near vertical. Pitch is `asin(T.y)`; heading is
`atan2(T.z, T.x)`. Wrapped angles may jump by 2pi without a physical frame flip.

Measurement rows sit at step midpoints, never across a section/transition boundary.
With adjacent generated tangents and midpoint quaternion:

```text
Tprime_est = (T_end - T_start) / ds
specific_force_est = v_ref² * Tprime_est - G
derived_normal_g = specific_force_est · N_mid / g
derived_lateral_g = specific_force_est · L_mid / g
curvature_est = |Tprime_est|
twist_est = rotation_vector(q_end * conjugate(q_start)) · T_mid / ds
```

Derived world pitch is `Tprime_est·U`. Derived heading is
`Tprime_est·(T×Y)/|T×Y|²`; both are absent near vertical. Authored rider-local rates
and these world-coordinate measurements describe different quantities, as the CSV
column names indicate. Multiply spatial rates by reference speed to get rad/s.
These independent finite differences are second-order diagnostics; they are not
the fourth-order internal integrator derivatives. Midpoint elevation uses endpoint
averaging, also second order. Authored force values are never substituted for
derived measurements.

## Transitions and editing

Every channel is an ordered list of transitions with positive lengths. Let
`u = local_input / length`, `a = start`, `b = end`, and `d = b-a`:

| Function | Value before amplitude |
| --- | --- |
| Constant | `a`, requires `a=b` and zero amplitude |
| Linear | `a + d*u` |
| Quadratic | `a + d*u²` (blend-in convention) |
| Cubic smoothstep | `a + d*(3u²-2u³)` |
| Quintic smoothstep | `a + d*(10u³-15u⁴+6u⁵)` |
| Cosine | `a + d*(1-cos(pi*u))/2` |
| Cubic Hermite | Standard endpoint-value/endpoint-slope cubic; slopes are in output/input units, not normalized-u slopes |

All nonconstant functions optionally add `amplitude * 16u²(1-u)²`, an endpoint-flat
bump with unit peak at `u=0.5`. Amplitude does not modify endpoint values or slopes.
The engine evaluates analytic first derivatives, validates C0/C1 joins, and permits
intentional discontinuities with diagnostics. Internal evaluation is right-continuous;
the last endpoint is left-sided. Public evaluation rejects out-of-domain inputs.

`Channel::insert`, `replace`, and `remove` are transactional. Changing a transition
length shifts subsequent input boundaries through prefix sums. Replacing a transition
can edit values, amplitude, function, slopes, and length while preserving its ID.
Section channel coverage must match its extent. Nothing silently stretches other
channels or adjusts their values. Use a document command batch to coordinate edits.

`qx_document::command::Editor` exposes immutable authored state plus
`execute([Command, ...])`, `undo()`, and `redo()`. Commands support insertion after
a stable section ID (or at the beginning), removal, replacement, channel edits,
initial conditions, metadata, and style selection. Commands validate the final
candidate before publishing it. Failed batches preserve both document and history.
A successful new edit clears redo. History uses bounded snapshots (1..256 entries),
so memory grows with document size; an editor should choose a conservative capacity.
Generated geometry is not stored in history: regenerate from the current document.

## Integration quality and boundary behavior

The coupled position/quaternion ODE uses classical fourth-order Runge-Kutta, with
normalized stage orientations and normalized accepted orientations. Each smooth
interval is split into uniform steps no longer than `max_step_m`, with at least
four steps per interval. The solver ends steps exactly at every transition knot
across all channels and at each section boundary. Initial RK stages use right-sided
channel values and final stages left-sided values, avoiding contamination from a
neighboring discontinuity.

Global integration error is O(h⁴) on sufficiently smooth, resolved profiles; it is
not an unconditional bound. Normalization, roundoff, extreme derivatives, long
tracks, and singular coordinates affect practical accuracy. Steps reject stage
rotation above the configured angular limit and return a contextual error asking
for a smaller spatial step. This pass has no adaptive local-error controller.
The executable also compares positions with a second run at half the step size;
that difference is evidence for one example, not a universal error estimate.
If refinement exceeds the sample budget or minimum step restriction, the summary
states why the comparison is unavailable and still exports the successful solve.

Channel prefix sums use compensated summation. Input boundaries within
`8 * f64::EPSILON * (1 + max(|a|, |b|))` are treated as the same boundary,
including one-sided evaluation and section-coverage validation. This tolerance
remains smaller than the minimum transition length at the maximum input extent.
It prevents roundoff-equivalent channel knots from creating nearly zero steps.

Position and orientation always carry continuously into the next section.
`allow_rate_jump` warns if incoming curvature/twist/reference speed changes by more
than `1e-8` in their respective units. `require_continuous_rates` rejects that join.
Channel C0/C1 checks use absolute and relative `1e-10` tolerance. Abrupt rates are
never silently smoothed. Curvature warnings default to `0.5 /m` and are engineering
diagnostics, not ride-safety criteria. This is an open centerline; loop closure and
endpoint-target solving are not implemented.

Arc distance accumulates from the independent spatial variable, not chord lengths
or uniform spline parameters. `sample_at_distance` binary-searches those stations,
then uses cubic Hermite positions with endpoint tangents and shortest quaternion
interpolation for frames. Queries are approximate between integration samples;
resampling does not make a coarse solve more accurate. Frame queries do not
constitute a wheel/rail constraint or a motion integrator.

## Persistence, catalog, and input limits

Version 1 JSON persists metadata, initial conditions, style IDs, sections, channel
units, and complete transitions. Serde/serde_json provide standard serialization;
the `float_roundtrip` parser feature preserves saved f64 values on reopening.
Unknown fields and variants and unsupported schema versions are rejected rather
than discarded. There is no earlier real document format or migration to support.
`examples/foundation.qx.json` is the version-1 compatibility fixture.

Saving validates before touching the target, writes a unique sibling file using
`create_new`, syncs/closes it, then renames it over the destination. Native Windows
overwrite and invalid-save preservation are integration-tested. This does not
promise crash durability of directory metadata or atomic behavior on every remote
filesystem. Concurrent saves are last-successful-rename-wins, with no collaboration
or file-locking protocol. Never use untrusted asset paths without an asset-loader
containment check; this pass does not load catalog assets.

Input limits include 16 MiB/project, 4 MiB/catalog, 4096 sections, 4096 transitions
per channel, 65536 total transitions, bounded text, nonzero unique IDs, finite
numeric values, section/channel input length at most 1e6, design speed 0.1..500 m/s,
gravity 0.01..100 m/s², and world position norm at most 1e9 m. Default generation
budget is 200000 samples; the hard configurable ceiling is 1000000. Too little
position/distance precision or unresolved rotation returns errors rather than NaNs.
These limits bound common malformed inputs; performance at the ceilings has not
been benchmarked. Channel lookup and full regeneration are deliberately simple.

The catalog has its own version, family/style IDs and numbers, descriptions,
compatible train IDs, relative asset paths, thumbnail/larger preview paths, extra
reference images, and development status. Released entries must reference an asset
and both preview sizes; file existence/provenance require a future asset pipeline.
The three included styles are **concepts**, have no assets/previews, and promise
no manufacturer accuracy. Changing style cannot enter the geometry solver.

Dependencies added are Serde and serde_json, both dual MIT/Apache-2.0 upstream,
plus their Cargo-locked transitive dependencies. No graphics, networking, cloud,
telemetry, or numerical framework dependency was introduced.
Cargo metadata also identifies MIT/Apache-2.0 for the derive/procedural macro
dependencies, Unlicense-or-MIT for memchr, MIT for zmij, and an additional
Unicode-3.0 data license for unicode-ident. The previously missing `qx-style`
package declaration now states the repository's existing MIT-or-Apache-2.0 terms.

## Verification and future work

Tests cover analytic force and geometric circles, level/banked force balance,
fourth-order convergence, independent finite-difference loads, distinct geometric
frames, time/distance equivalence, shared boundary poses, one-sided transitions,
rotation invariants, derivatives, invalid inputs/budgets, deterministic regeneration,
version/unknown-field rejection, filesystem save/overwrite, commands/history,
style independence, and both CLI commands with byte-identical exported numerical
data. See the pass report for the actual commands and results.

Full train physics, contact surfaces, bogies, collision, mechanisms, environmental
forces, and constraints remain separate future work in `qx-physics`, whose intended
baseline simulation step remains 1/240 s. That future fixed dynamics timestep does
not control geometry-generation sampling. There is no articulated physics claim.

Next priorities: owner-directed graph/timeline interactions on these commands;
adaptive integration error control and convergence diagnostics for wider cases;
incremental regeneration; loop/endpoint constraints; physical rail/contact-surface
generation; then a separately validated rigid-body dynamics pass. Final editor UX,
track assets, material/rendering work, and performance benchmarks remain pending.
