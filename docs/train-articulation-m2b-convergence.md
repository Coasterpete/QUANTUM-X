# Train articulation M2B — assembly convergence and conditioning

Date: 2026-11-06. Native Windows / Rust 1.99.1 / MSVC, on
`codex/train-articulation-m1`, based on `bcc5add`. M1 and M2A remain uncommitted
and preserved. This is a bounded initial-pose projection improvement, not a new
vehicle dynamics solver.

## 1. Result and scope

The original curved and changing-bank 8-car fixtures now converge in **153
sweeps**, versus 825 / 817 in M1 and M2A. The original 16-car fixtures now
converge in **521 / 513 sweeps**, and the original 32-car fixtures in **1881 /
1873 sweeps**, within the unchanged **2400-sweep** limit. Every connection,
guide and angular constraint must still meet the existing 1e-5 m / 1e-5 rad
limits. No average or percentile is used for acceptance.

The retained change is **64 original, unrelaxed sweeps followed by relaxation
factor 1.85 on the same projection rows**. Translation and rotation clamps,
numerical radius weights, pinned guide, authored order, graph edges and all
rigid dimensions remain unchanged. A final convergence check now measures sweep
2400 itself; exhausted errors no longer describe sweep 2393. Explicit nonfinite
residual checks prevent overflow from being ignored by a maximum comparison.

This is a measured improvement, not a general assembly guarantee. Independently
feasible **32-car asymmetric-hitch** fixtures still exhaust the budget. Their
failures remain visible and are not classified as mechanically impossible.

Changed M2B implementation:

- [`assembly.rs`](../crates/qx-physics/src/assembly.rs): bounded numerical
  relaxation and terminal/finite residual checks.
- [`assembly_diagnostics.rs`](../crates/qx-physics/src/assembly_diagnostics.rs):
  per-connection signed world gap and angular error, per-guide residual/station/
  local point/pin flag, and component-origin pose snapshots at checkpoints.
- [`assembly_convergence.rs`](../crates/qx-physics/tests/assembly_convergence.rs)
  and [`support/feasible.rs`](../crates/qx-physics/tests/support/feasible.rs):
  independent witnesses, propagation/order characterization, asymmetric/shared
  graphs, incompatibility checks and opt-in Release timing.
- M1's assembly characterization retains its fixtures and residual assertions;
  expected long-chain outcomes and the terminal check count are updated for the
  actual improved behavior. Its printed reports are abbreviated to avoid
  dumping every new pose snapshot.

M2A chord implementation, dynamics, playback, COM conversion, fixed 1/240 s
timestep, impulse iterations/substeps, document/schema, dependencies and UI are
unchanged by M2B. No wheel fit/contact, velocity projection or whole-consist
circuit seam initialization was added.

## 2. Architecture and diagnosis

Read alongside the [M1 baseline](train-articulation-m1-baseline.md) and
[M2A chord report](train-articulation-m2a-chord-robustness.md).

The persisted component/connection graph is authoritative. Inspection first
resolves placements/mounts, then optionally seeds independent two-pivot car
chords. Shared or asymmetric gear can bypass that optimization. Mounts are
initialization guesses, not extra physical welds. Projection operates on rigid
component-origin poses, and initialization later converts those to mass-center
poses for independent rigid-body integration.

For a connection's world-axis scalar anchor error `e`, the translation update is

```text
d = clamp(omega * e / (2 + |ra cross n|² / Ra² + |rb cross n|² / Rb²), -.2, .2)
Ra = max(.5 * length(component_dimensions), .25 metres)
```

Both bodies receive opposite translations and weighted rigid rotations.
Single-body guides use the corresponding denominator with translation weight
1. These weights are numerical conditioning choices, not physical inverse
masses. Every individual rotation increment still clamps to 0.15 rad.
Roll-lock projection rotates about its anchor; rail twist rotates about the
guide centroid. Fixed/hinge angular projection retains its existing axes and
limit definitions. Relaxation scales both sides of each angular correction.

Each sweep walks authored mechanical connections, then all guides, then rail
attitudes. Guide stations relax tangentially through the existing bounded local
projection. The first guide alone also constrains its longitudinal coordinate
to its seed station. Other guides constrain transverse displacement. There is
no added longitudinal lock per car. Existing station-neighborhood and open-end
rejections remain in force.

The conventional graphs have respectively 7/31/63/127 bodies and
6/30/62/126 connections for 2/8/16/32 cars. They contain independent two-ended
drawbars, rather than coincident car hitch arms. Local chord accuracy does not
set their inter-car distances. Coupler corrections displace adjacent rigid
assemblies; guides and angular rows subsequently reconcile those changes.

Evidence for poor conditioning rather than local chord failure:

- The unchanged M2A seeds start near 6.2216e-3 m worst anchor gap and
  4.4444e-4 m worst guide error. Changing bank additionally starts at
  1.9205e-3 rad angular error. All three versions share these seeds to reported
  precision.
- In the curved 32-car M2A case, the worst gap drops to 1.5165e-4 m at sweep 9,
  then grows slowly to 1.3430e-4 m at 513 and 1.3474e-4 m at 1025 before
  falling to 1.0751e-4 m at 2400. This long transient is not proof of an
  incompatible configuration or permanent stagnation.
- Near the M2A terminal state its per-sweep gap ratio is approximately
  **0.999796**. M2B's late ratio is approximately **0.997280**. These are
  observed local contractions of the maximum, not spectral bounds or a global
  convergence theorem.
- At sweep 1025 the 32-car M2A checkpoint-to-checkpoint maximum component
  translation is 7.862e-6 m and rotation 2.488e-7 rad. M2B gives 6.792e-6 m
  and 1.902e-6 rad, while reducing the anchor gap much faster. These are net
  rigid-pose changes over eight sweeps, not summed correction impulses. The
  transient snapshots permit separate per-component analysis without changing
  simulation behavior.
- Error is concentrated toward the pinned end and decreases toward the free
  tail. Reversing constraints or component order changes propagation; moving
  the pin toward the middle changes it substantially. This supports a slow
  coupled chain mode. It does not establish the pin as the sole cause.

Diagnostics remain compile-time opt-in (`TRACE`), read-only observations. They
do not feed acceptance, change ordering, supply seeds, choose relaxation, or
run during playback. Histories contain at most 301 checks plus seed/terminal
samples; pose/edge arrays increase developer-only memory use. They should not
be collected per tick. Early aborts can still leave no terminal sample.

## 3. Independent mechanical feasibility

Fixtures are numerical development data, not owner-designed coaster layouts.
The original radius-45 m, bank-0.25 rad, 500 m generated track and front station
200 m are reused. Changing bank uses M1's unchanged spatial roll-rate profile
from 0 to .004 rad/m. Actual generated/Hermite positions are queried; no ideal
circle is substituted for the generated changing-bank geometry.

Before modifying the solver, separate test construction established witnesses
for 2/8/16/32 cars on both tracks and with symmetric/asymmetric hitches. Its
geometry was rechecked after optimizing the test-only scalar root search:

1. Independently bracket a .4 m chord for each rigid gear's two longitudinal
   guide points. Set its X axis along the endpoint chord and its origin at the
   midpoint. Remove rail twist about that same X axis, preserving both guides.
2. Independently bracket the rear gear station to obtain the original 2.4 m
   separation between a car's suspension pivots. Orient the car along those
   pivots, with zero swing/twist roll relative to the front gear. Locate its
   origin from the original `(-1.2,-.8,0)` attachment.
3. March rearward by a scalar hitch-distance root, so successive car hitch
   points are exactly 1 m apart. Place an independent rigid drawbar between
   them, using its unchanged +/- .5 m anchors.
4. Independently reconstruct the original first guide's seed station and use
   it to fix the witness's gauge. A test compares the witness guide directly
   with the production seed's pinned target; this is not just a witness at a
   different longitudinal station.

The fixture search uses bracketed scalar interpolation with periodic bisection,
bounded iteration counts and final world-geometry assertions. It never invokes
production assembly, M2A chord solving or production station projection. It is
specific test construction, not a new production graph algorithm. Verification
checks each mechanical anchor pair, each guide point and independent
`2*atan2(q.x,q.w)` roll/rail twist. Current witness maxima are below 2e-10 m /
rad (typically 1e-14 to 1e-13 m and 1e-16 rad), orders of magnitude below
assembly acceptance. This is numerical constructive evidence, not a formal
interval proof.

Asymmetric fixtures retain every original suspension and rigid drawbar. Before
construction, upstream car hitch anchors gain `(0,.13,.09)` m and downstream
anchors `(0,-.07,-.04)` m. These are deliberate fixture definitions; no solver
alters them. They test unequal vertical/lateral offsets rather than merely
renaming symmetric anchors.

A separate five-body, four-connection two-car fixture shares one gear body
between cars. Its independent witness closes on both curved tracks; production
assembly converges in 41 / 33 sweeps and preserves exactly one shared body.
The original M1 redundant shared-gear fixture is also retained: straight
success and curved unresolved failure are not treated as equivalent mechanisms.

Two kinds of incompatibility remain explicit:

- The original extra straight-track guide on one gear cannot fit its rigid
  guide pattern; assembly reaches the limit with approximately .294 m guide
  residual under M2B (M2A: .2 m).
- A new parallel edge uses the same B anchor as an existing edge but an A
  anchor .1 m away on the same rigid component. By the triangle inequality at
  least one of the two anchor gaps must be >= .05 m in every pose. An 8-car
  fixture verifies this individual-edge bound and rejects the assembly.
  A second case separates the A anchors by just 22 micrometres: its terminal
  mean connection gap is **9.2326e-6 m**, below tolerance, while its worst gap
  is **7.9411e-5 m** and guide error only **7.6208e-10 m**. It still fails.
  This directly guards against hiding a bad edge in an aggregate mean.

The impossible local 40 m chord on a radius-8 m track still aborts in the chord
phase. These cases distinguish constructive feasibility, proven incompatibility
and unresolved numerical assembly.

## 4. Alternatives measured before selection

All candidates kept the 2400-sweep cap, rigid transforms, authored constraints
and tolerances. Ordinary authored order and the original pin were retained in
the selected implementation.

| Candidate | Curved 8 / 16 / 32 sweeps | 32-car terminal gap | Decision |
| --- | --- | --- | --- |
| M2A original projection | 825 / limit / limit | 1.0751e-4 m | Baseline |
| Alternate forward/reverse connection direction each sweep | 1033 / limit / limit | 1.5916e-4 m | Worse |
| Rotation weights x .25, guides included | 697 / limit / limit | 1.1023e-4 m | Does not resolve chain |
| Rotation weights x 4, guides included | 1425 / limit / limit | 1.3915e-4 m | Worse |
| Relaxation 1.5 from first sweep | 345 / 1425 / limit | 7.7743e-5 m | Partial benefit |
| Relaxation 1.8 from first sweep | 137 / 601 / 2345 | 9.8878e-6 m | Resolves chain; 2-car case slows 41 -> 57 |
| 64 original sweeps, then 1.85 | 153 / 521 / 1881 | 9.9698e-6 m | Retained |
| 64 original sweeps, then 1.9 | 161 / 369 / 1337 | 9.8631e-6 m | Faster longest case, slower 8-car; not retained |

The fixed 64-sweep warm-up retains exact existing results for solves completed
before that point, including all original 2-car radius-45 m cases and straight
chains. It settles large local guide/attitude transients before acceleration.
1.85 resolves the requested original long chains with useful budget margin and
slightly fewer 8-car sweeps than 1.9; it is not claimed to be an optimal or
universally stable relaxation factor. The nonlinear solver can oscillate or
fail on other graphs. No tuning parameter is exposed to users.

The first relaxed sweep can increase the worst gap: curved 32-car checkpoints
57 -> 65 have a per-sweep ratio about 1.190. Therefore no monotonicity-based
success rule, small-update success rule, or premature stagnation rejection was
introduced. The final maximum physical residual remains the acceptance test.
Directed production seeds and block/sparse solvers were not added after the
smaller measured change met the original-chain objective. The independent
directed witness is evidence of feasibility, not a replacement for physical
joints or for the general mechanical graph.

### Ordering and pin sensitivity

The following uses changing bank. "Middle pin" swaps a middle front gear to
the first component position, deliberately changing which guide is pinned;
it also changes guide/attitude iteration order. Thus it measures those combined
effects, not an isolated gauge-position experiment. Reverse-components likewise
changes both pin and guide order. Reverse-edges alone isolates connection order.

| Cars | Variant | M2A sweeps | joint (m) | M2B sweeps | joint (m) |
| --- | --- | --- | --- | --- | --- |
| 8 | authored | 817 | 9.743443e-06 | 153 | 7.270070e-06 |
| 8 | reverse-edges | 697 | 9.828727e-06 | 145 | 7.919712e-06 |
| 8 | reverse-components | 857 | 9.998910e-06 | 153 | 9.059352e-06 |
| 8 | middle-pin | 281 | 9.680291e-06 | 137 | 9.178563e-06 |
| 8 | reverse-all | 913 | 9.757154e-06 | 169 | 6.369640e-06 |
| 16 | authored | limit (2400) | 2.271775e-05 | 513 | 9.800515e-06 |
| 16 | reverse-edges | limit (2400) | 1.633673e-05 | 497 | 9.340352e-06 |
| 16 | reverse-components | limit (2400) | 1.999612e-05 | 457 | 9.520135e-06 |
| 16 | middle-pin | 833 | 9.959881e-06 | 249 | 7.584483e-06 |
| 16 | reverse-all | limit (2400) | 2.091044e-05 | 481 | 9.769870e-06 |
| 32 | authored | limit (2400) | 1.082765e-04 | 1873 | 9.904219e-06 |
| 32 | reverse-edges | limit (2400) | 8.290296e-05 | 1825 | 9.906598e-06 |
| 32 | reverse-components | limit (2400) | 1.047604e-04 | 1809 | 9.812211e-06 |
| 32 | middle-pin | limit (2400) | 1.498352e-05 | 601 | 9.478250e-06 |
| 32 | reverse-all | limit (2400) | 1.196604e-04 | 1865 | 9.929999e-06 |

These tests establish repeatability for each input order, not bit-identical
solutions across different gauges/orders. All five tested M2B permutations
converge for 8/16/32 cars. Production does not sort, reorder or rename anything.

## 5. Consistent M1 / M2A / M2B characterization

M1 source was preserved by M2A under ignored `target/m2a-evidence/m1-preserved`;
M2A source was copied before M2B edits under `target/m2b-evidence/preserved`.
Isolated non-published crates under `target/m2b-evidence/` reran the original
M1 assembly/chord and M2A assembly against the same current fixture/test harness,
with common read-only diagnostics. The new diagnostic-only fields do not
participate in either solve. Dependencies match the workspace versions.
No baseline was reconstructed from the M2B solution, and no production source
was replaced during these final comparison runs.

| Fixture / cars | M1 sweeps | M2A sweeps | M2B sweeps |
| --- | --- | --- | --- |
| Curved / 2 | 41 | 41 | 41 |
| Curved / 8 | 825 | 825 | 153 |
| Curved / 16 | limit (2400) | limit (2400) | 521 |
| Curved / 32 | limit (2400) | limit (2400) | 1881 |
| Changing bank / 2 | 41 | 41 | 41 |
| Changing bank / 8 | 817 | 817 | 153 |
| Changing bank / 16 | limit (2400) | limit (2400) | 513 |
| Changing bank / 32 | limit (2400) | limit (2400) | 1873 |

Straight 2/8/16/32-car cases remain one sweep in all three versions. M1/M2A
residuals agree at the precision below; f64-bit identity across changed chord
algorithms is not asserted. Failure rows measure actual sweep-2400 poses, even
though the old result string used sweep 2393.

| Fixture | M1/M2A joint (m) | guide (m) | angular (rad) | M2B joint (m) | guide (m) | angular (rad) |
| --- | --- | --- | --- | --- | --- | --- |
| Curved / 2 | 9.143505e-06 | 1.805354e-07 | 1.688048e-08 | 9.143505e-06 | 1.805354e-07 | 1.688048e-08 |
| Curved / 8 | 9.744024e-06 | 9.805030e-08 | 5.143433e-08 | 9.127445e-06 | 3.324338e-06 | 3.559806e-08 |
| Curved / 16 | 2.292145e-05 | 2.270511e-07 | 1.267352e-07 | 9.514169e-06 | 3.454726e-06 | 8.639358e-08 |
| Curved / 32 | 1.075116e-04 | 1.060551e-06 | 6.014618e-07 | 9.969790e-06 | 3.617073e-06 | 9.591031e-08 |
| Changing bank / 2 | 9.627143e-06 | 1.789618e-07 | 2.086923e-08 | 9.627143e-06 | 1.789618e-07 | 2.086923e-08 |
| Changing bank / 8 | 9.743443e-06 | 9.781160e-08 | 5.143187e-08 | 7.270070e-06 | 2.645901e-06 | 2.294766e-08 |
| Changing bank / 16 | 2.271775e-05 | 2.243314e-07 | 1.253030e-07 | 9.800515e-06 | 3.558239e-06 | 8.906847e-08 |
| Changing bank / 32 | 1.082765e-04 | 1.064587e-06 | 6.039096e-07 | 9.904219e-06 | 3.592841e-06 | 9.530551e-08 |

The new diagnostics retain every connection's signed gap, not only the worst.
All successful fixtures also meet rigid-attachment and rail-attitude limits.
Across the newly measured successful curved/asymmetric matrix, maximum rigid
attachment error is 1.544e-14 m and maximum rail-attitude residual is
8.671e-8 rad. Relaxation leaves somewhat larger guide/rail residuals than exact
end-of-sweep projection in M2A, still inside every original acceptance gate.
Ordinary/observed results match exactly; repeated reports/results are
deterministic; cloned train/track inputs remain unchanged. Independent
world-geometry checks additionally verify the solved conventional/asymmetric
and shared-gear poses.

### Initial and terminal error distribution

For symmetric fixtures, every car initially has approximately 6.2216e-3 m worst
incident-connection gap. The following edge distributions include every edge,
not just couplers; all values are metres. "Bad" counts edges above 1e-5 m.

| Fixture | Seed max | M2A min / median / max | Bad | M2B min / median / max | Bad |
| --- | --- | --- | --- | --- | --- |
| Curved / 2 | 6.222e-03 | 1.445e-06 / 4.111e-06 / 9.144e-06 | 0 | 1.445e-06 / 4.111e-06 / 9.144e-06 | 0 |
| Curved / 8 | 6.222e-03 | 3.732e-07 / 9.756e-07 / 9.744e-06 | 0 | 1.762e-07 / 8.107e-07 / 9.127e-06 | 0 |
| Curved / 16 | 6.222e-03 | 5.123e-10 / 1.512e-06 / 2.292e-05 | 18 | 1.416e-07 / 6.215e-07 / 9.514e-06 | 0 |
| Curved / 32 | 6.222e-03 | 1.325e-06 / 7.121e-06 / 1.075e-04 | 58 | 1.108e-07 / 6.107e-07 / 9.970e-06 | 0 |
| Changing bank / 2 | 6.222e-03 | 1.551e-06 / 4.197e-06 / 9.627e-06 | 0 | 1.551e-06 / 4.197e-06 / 9.627e-06 | 0 |
| Changing bank / 8 | 6.222e-03 | 3.815e-07 / 9.646e-07 / 9.743e-06 | 0 | 1.260e-07 / 6.100e-07 / 7.270e-06 | 0 |
| Changing bank / 16 | 6.222e-03 | 1.829e-08 / 1.488e-06 / 2.272e-05 | 18 | 1.474e-07 / 6.397e-07 / 9.801e-06 | 0 |
| Changing bank / 32 | 6.222e-03 | 1.344e-06 / 7.129e-06 / 1.083e-04 | 58 | 1.103e-07 / 6.074e-07 / 9.904e-06 | 0 |

For curved 32 cars, the complete per-car distribution is below. Each row is the
maximum among that car's incident suspension/coupler edges; adjacent cars can
share a coupler edge in this summary. Per-edge histories remain available in
the characterization output and report API.

| Car | Initial max (m) | M2A terminal (m) | M2B terminal (m) |
| --- | --- | --- | --- |
| 1 | 6.221555e-03 | 1.075116e-04 | 9.969790e-06 |
| 2 | 6.221555e-03 | 7.455763e-05 | 4.470426e-06 |
| 3 | 6.221555e-03 | 7.847110e-05 | 4.349709e-06 |
| 4 | 6.221555e-03 | 8.141022e-05 | 4.442377e-06 |
| 5 | 6.221555e-03 | 8.298133e-05 | 4.605797e-06 |
| 6 | 6.221555e-03 | 8.299553e-05 | 4.662552e-06 |
| 7 | 6.221555e-03 | 8.144965e-05 | 4.615163e-06 |
| 8 | 6.221555e-03 | 7.849765e-05 | 4.473793e-06 |
| 9 | 6.221555e-03 | 7.441526e-05 | 4.254600e-06 |
| 10 | 6.221555e-03 | 6.955985e-05 | 3.977892e-06 |
| 11 | 6.221555e-03 | 6.432797e-05 | 3.666137e-06 |
| 12 | 6.221555e-03 | 5.911316e-05 | 3.341916e-06 |
| 13 | 6.221555e-03 | 5.426620e-05 | 3.025903e-06 |
| 14 | 6.221555e-03 | 5.005966e-05 | 2.734913e-06 |
| 15 | 6.221555e-03 | 4.665936e-05 | 2.480170e-06 |
| 16 | 6.221555e-03 | 4.410769e-05 | 2.266110e-06 |
| 17 | 6.221555e-03 | 4.232525e-05 | 2.090249e-06 |
| 18 | 6.221555e-03 | 4.113283e-05 | 1.944478e-06 |
| 19 | 6.221555e-03 | 4.028791e-05 | 1.817542e-06 |
| 20 | 6.221555e-03 | 3.952424e-05 | 1.697809e-06 |
| 21 | 6.221555e-03 | 3.858612e-05 | 1.575426e-06 |
| 22 | 6.221555e-03 | 3.725515e-05 | 1.443531e-06 |
| 23 | 6.221555e-03 | 3.537053e-05 | 1.320061e-06 |
| 24 | 6.221555e-03 | 3.284357e-05 | 1.225729e-06 |
| 25 | 6.221555e-03 | 2.966564e-05 | 1.087163e-06 |
| 26 | 6.221555e-03 | 2.590794e-05 | 9.169588e-07 |
| 27 | 6.221555e-03 | 2.171162e-05 | 7.297269e-07 |
| 28 | 6.221555e-03 | 1.726807e-05 | 5.410820e-07 |
| 29 | 6.221555e-03 | 1.279027e-05 | 3.665695e-07 |
| 30 | 6.221555e-03 | 8.479009e-06 | 2.785004e-07 |
| 31 | 6.221555e-03 | 4.501426e-06 | 2.570568e-07 |
| 32 | 6.221555e-03 | 2.281504e-06 | 2.758788e-07 |

### Asymmetric fixtures and remaining feasible failures

| Fixture | M1/M2A sweeps | joint (m) | M2B sweeps | joint (m) | guide (m) | angular (rad) |
| --- | --- | --- | --- | --- | --- | --- |
| Curved / 2 | 145 | 8.683139e-06 | 113 | 6.870986e-06 | 3.578597e-06 | 6.135569e-07 |
| Curved / 8 | limit (2400) | 2.403022e-05 | 273 | 9.542369e-06 | 3.864824e-06 | 5.475766e-07 |
| Curved / 16 | limit (2400) | 3.904747e-03 | 1185 | 9.859173e-06 | 3.888682e-06 | 5.879600e-07 |
| Curved / 32 | limit (2400) | 1.368656e-02 | limit (2400) | 1.088458e-03 | 4.275478e-04 | 6.534642e-05 |
| Changing bank / 2 | 145 | 7.798706e-06 | 113 | 7.108686e-06 | 3.575764e-06 | 5.781466e-07 |
| Changing bank / 8 | limit (2400) | 2.078525e-05 | 265 | 8.200261e-06 | 3.331113e-06 | 4.722846e-07 |
| Changing bank / 16 | limit (2400) | 3.873784e-03 | 1169 | 9.364238e-06 | 3.694087e-06 | 5.607144e-07 |
| Changing bank / 32 | limit (2400) | 1.386729e-02 | limit (2400) | 1.024721e-03 | 4.025277e-04 | 6.174711e-05 |

Asymmetric edge distributions (metres; min / median / max):

| Fixture | Seed | M2A terminal | Bad | M2B terminal | Bad |
| --- | --- | --- | --- | --- | --- |
| Curved / 2 | 0.000e+00 / 5.102e-15 / 1.547e-01 | 1.125e-06 / 4.255e-06 / 8.683e-06 | 0 | 4.147e-06 / 5.229e-06 / 6.871e-06 | 0 |
| Curved / 8 | 0.000e+00 / 2.350e-14 / 1.547e-01 | 9.786e-07 / 8.304e-06 / 2.403e-05 | 14 | 4.383e-07 / 2.431e-06 / 9.542e-06 | 0 |
| Curved / 16 | 0.000e+00 / 2.350e-14 / 1.547e-01 | 5.863e-06 / 1.270e-03 / 3.905e-03 | 61 | 1.536e-07 / 2.684e-06 / 9.859e-06 | 0 |
| Curved / 32 | 0.000e+00 / 2.350e-14 / 1.547e-01 | 1.539e-04 / 4.243e-03 / 1.369e-02 | 126 | 1.251e-05 / 2.190e-04 / 1.088e-03 | 126 |
| Changing bank / 2 | 0.000e+00 / 1.873e-14 / 1.547e-01 | 9.570e-07 / 3.756e-06 / 7.799e-06 | 0 | 3.965e-06 / 5.540e-06 / 7.109e-06 | 0 |
| Changing bank / 8 | 0.000e+00 / 2.769e-14 / 1.547e-01 | 8.438e-07 / 7.152e-06 / 2.079e-05 | 10 | 3.532e-07 / 1.995e-06 / 8.200e-06 | 0 |
| Changing bank / 16 | 0.000e+00 / 2.769e-14 / 1.547e-01 | 5.579e-06 / 1.264e-03 / 3.874e-03 | 61 | 1.376e-07 / 2.554e-06 / 9.364e-06 | 0 |
| Changing bank / 32 | 0.000e+00 / 2.843e-14 / 1.547e-01 | 1.573e-04 / 4.283e-03 / 1.387e-02 | 126 | 1.173e-05 / 2.030e-04 / 1.025e-03 | 126 |

M1 and M2A give the same asymmetric classifications/sweeps and agree at the
reported gap precision. M2B resolves both 8- and 16-car asymmetric cases, but
32-car asymmetric cases remain **SweepLimit**, with every terminal gap retained.
Their independently constructed witnesses prevent misreporting these as
incompatible geometry. The result is never returned as an assembled train.

## 6. Release performance

The same timing test generated tracks outside timers, inspected once for
diagnostics/warm-up, then timed seven batches of three ordinary solves using
`black_box`. Values are microseconds per assembly: median [min,max] of batch
means. M1, M2A and M2B ran sequentially on the same native machine; compilation,
track generation, witness construction and diagnostics are outside these
timers. Failure rows time the complete bounded failed solve.

| Fixture | M1 (us) | M2A (us) | M2B (us) |
| --- | --- | --- | --- |
| Curved / 2 | 489.0 [487.5, 494.5] | 454.0 [453.2, 467.8] | 772.8 [731.6, 2007.0] |
| Curved / 8 | 38197.0 [37941.7, 38261.6] | 37725.5 [37605.2, 37825.5] | 12124.0 [11985.0, 12333.7] |
| Curved / 16 | 376317.5 [225245.3, 407637.1] | 370349.2 [223820.0, 401218.0] | 82432.7 [81492.2, 97939.7] |
| Curved / 32 | 773728.8 [734050.8, 819562.7] | 772644.0 [737054.7, 800037.4] | 605215.3 [591396.1, 635159.6] |
| Changing bank / 2 | 800.2 [781.3, 849.1] | 792.5 [734.7, 1501.8] | 780.8 [739.3, 944.9] |
| Changing bank / 8 | 63506.3 [62583.2, 76706.2] | 62327.4 [61032.7, 63800.4] | 12022.5 [11642.9, 12100.3] |
| Changing bank / 16 | 387137.0 [377893.8, 398253.5] | 372753.1 [369424.0, 397567.3] | 84697.8 [78832.7, 94362.7] |
| Changing bank / 32 | 792335.9 [764948.8, 813012.6] | 771102.4 [733273.0, 817038.2] | 611066.5 [585707.5, 626111.3] |

Shared-machine variation is substantial, including within some baseline batches.
These are observations, not CI thresholds or controlled CPU-frequency benchmarks.
The deterministic sweep/residual improvement is stronger evidence than any
precise timing ratio. A faster failed baseline does not equal successful assembly.
Initialization durations are not per-tick physics timings, rendering FPS or proof
that 32-car playback performs acceptably.
The two-car M2B sample is slower than the curved M2A sample despite the same
41 sweeps and identical early numerical path. This run does not isolate the
cause or prove zero ordinary-path overhead. Controlled profiling is required
before attributing that difference to code generation or machine load.

The existing ordinary and instrumented M1 timing tests were also rerun unchanged.
Their opt-in traces now retain more per-edge/pose data, so diagnostic timings
are not directly comparable to the old aggregate-only instrumentation. No trace
work was added to the ordinary path or playback. Large graphs/high attachment
counts can use much more diagnostic memory and pairwise rigid-check work.

## 7. Verification and reproduction

Executed from the existing worktree root:

```powershell
cargo fmt --all -- --check
cargo test -p qx-physics -p qx-document --tests
cargo test --release -p qx-physics --test assembly_convergence --test assembly_diagnostics --test chord_characterization --test initialization_diagnostics -- --nocapture --test-threads=1
cargo test --release -p qx-physics --test assembly_convergence -- --ignored --nocapture --test-threads=1
cargo test --release -p qx-physics --test articulation_baseline -- --ignored --nocapture --test-threads=1
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
```

Results: **114 physics/document tests passed**, zero failed, five ignored timing/
characterization tests in the ordinary run. Native Release characterization:
**21 passed**, zero failed, three ignored. All **five** ignored Release tests
were explicitly executed and passed (three convergence characterization/timing
tests and the two unchanged M1 assembly timers). Baseline M1/M2A summary,
propagation and Release timing comparison runs also passed.

Workspace check, strict all-target workspace Clippy, format check and tracked
diff check passed. Untracked source/report whitespace and preservation hashes
were checked separately. No tests or physical tolerances were weakened. The
new near-tolerance contradictory-edge fixture additionally rejects an assembly
whose **mean** connection gap is below tolerance while individual gaps exceed
it.

Byte comparisons confirm 23 pre-existing physics files and both prior reports
are unchanged. The three intentional extensions to existing files are
`assembly.rs`, `assembly_diagnostics.rs` and `tests/assembly_diagnostics.rs`;
M2B adds its convergence test, independent fixture helper and this report.
In particular, M2A `chord.rs`, `consist.rs`, `solver.rs`, all earlier timing/
chord/initialization tests and the original fixture helper are preserved.

Local raw logs, candidate transformations and preserved baselines are under
ignored `target/m2b-evidence/`. `m1-summary.txt`, `m2a-summary.txt`, the propagation
logs, `m2b-ignored.txt`, `release-final.txt`, `distributions.txt` and the timing
logs supply these tables. They are local evidence, not required runtime assets
or a new repository dependency. The final test sources reproduce M2B numerical
and timing characterization without those caches. Earlier version comparison
requires the preserved earlier source, since those passes are intentionally
uncommitted.

Not manually verified: desktop interaction, rendering, cameras, DPI, subjective
playback or UI performance. No GUI was launched or controlled. Full workspace
tests and an application Release package were not run; targeted physics/document
and native Release tests plus workspace compilation cover this pass. Optional
owner checklist: `cargo run -p quantum-x --release -- edit`, open an existing
interior train project, inspect/initialize, exercise Play/Pause/Reset, save and
reopen. This checklist has not been performed by the agent.

## 8. Remaining limitations and M3 recommendations

1. Retain the feasible asymmetric 32-car failures as explicit regression evidence.
   If longer/more offset chains become required, investigate general grouped
   initialization or sparse/block corrections against these witnesses and
   ordering variations; do not increase the sweep budget or accept a mean error.
2. The existing radius floor, scalar world-axis projections and nonlinear
   relaxation are not scale-invariant/global solvers. Tests here do not establish
   behavior for all 2048-body graphs, near-singular mounts, very different scales,
   mechanical-profile drives, manufacturers or articulated vehicle designs.
3. The pin/order experiment is coupled as described above. An isolated gauge
   experiment would require an explicit test-only pin selector while holding all
   row order fixed. No claim that a middle production pin is universally better
   follows from these results.
4. Wheel-level fitting, wheel/contact semantics, physical contact, nonzero-speed
   velocity consistency and topology-aware whole-consist seam initialization
   remain separate milestones. M1's nonzero-speed `J*v` mismatch remains;
   improved positional closure does not fix it. Do not use M3 wheel fitting to
   hide unresolved inter-car constraints or present guides as contact physics.
5. Carry the independent witness, incompatible-edge and per-edge maximum tests
   into M3. Preserve rigid dimensions, source identities and transactional
   failures, and measure position, attitude, velocity and dynamic reaction
   behavior separately. No ride-safety or full-train realism claim is made here.

UX status: no owner-approved UI/interaction behavior changed, no developer UI
introduced and no new UX decision required. Existing controls retain their prior
approval/provisional status. Improved assembly is reached through the existing
inspection/initialization API; ongoing dynamics and playback receive rigid
initial conditions through their existing contracts.

Git status: local, uncommitted M2B changes on `codex/train-articulation-m1`, with
M1/M2A and unrelated work preserved. Nothing committed, pushed, published,
merged or submitted as a PR. The separate `C:/Dev8/QUANTUM-X` tooling checkout
was not edited.
