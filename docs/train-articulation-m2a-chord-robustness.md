# Train articulation M2A — chord robustness and topology

Date: 2026-11-06. Native Windows x86_64 MSVC, Rust 1.99.1 / LLVM 23.1.1,
Intel Core i7-14700KF. Work continues in the existing
`codex/train-articulation-m1` worktree at `bcc5add`, with its uncommitted M1
changes retained. This is an initial-condition geometry change, not a change
to rigid-body integration, impulse dynamics or the mechanical graph solver.

## 1. Root causes and scope

The [M1 report](train-articulation-m1-baseline.md) remains unchanged as a
historical baseline. Its diagnostics, assembly/initialization regressions,
timing tests and fixture helper remain intact. The chord characterization
fixtures remain, with corrected failures replaced by positive accuracy checks.
An ignored local snapshot of the M1 physics sources/tests and tracked patch is
also retained under `target/m2a-evidence/m1-preserved/`.

M1 evaluated `distance(P(c-h/2), P(c+h/2)) - L` at 96 uniform spans, then
performed 48 bisections after the first nonnegative probe. Consequently:

- A positive interval narrower than a probe gap could be missed entirely.
- A tangent root has no sign change; uniformly sampled points need not hit it.
- Missing an early interval could produce a later, numerically accurate root
  on the wrong branch. The folded fixture returned 1.05 m instead of 1.02 m.
- A strictly nonnegative endpoint residual was required, so roundoff of only
  9.1e-15 m rejected the valid 599.995 m / 0.01 m straight-track fixture.
- Initial placement/assembly use open stations. Geometric closure alone does
  not carry the document's authored Circuit intent into those APIs.

The implementation changes only chord seeding and transient chord diagnostics.
Authored dimensions, anchors, connection lengths, train semantics, schema **7**,
editor behavior, assembly weights/order/limits, **2400 sweeps**, **1e-5 m/rad**
closure limits, **1/240 s** physics steps and playback's **four substeps** remain
unchanged. No wheel fitting, contact, velocity projection or propagation
optimization was implemented.

## 2. Algorithm and mathematical basis

[`src/chord.rs`](../crates/qx-physics/src/chord.rs) implements the same search
for ordinary assembly and opt-in diagnostics. `assembly::chord` delegates its
open-track solve to this module. No dependency or geometry-format change is
needed.

`GeneratedTrack::sample_at_distance` interpolates positions with cubic Hermite
polynomials using endpoint positions, station differences and frame tangents.
Orientation is independently interpolated by slerp. The new search uses the
position polynomial, rather than assuming the interpolated frame tangent is
the exact derivative of that polynomial.

1. Traverse symmetric spans `h >= 0` in increasing order. Split at every sample
   boundary crossed by either `c-h/2` or `c+h/2`; a pair of cursors merges those
   boundaries without allocating a list or scanning the whole track first.
2. On each resulting interval, parameterize span by `x` in `[0,1]`. Compose
   each Hermite position with its affine station coordinate. Their difference
   `D(x)` is a cubic vector polynomial.
3. Normalize by rigid length `L`. The cubic Bezier control hull bounds the
   vector norm from above: if all four control vectors have norm below
   `1 - tolerance/L`, that entire interval is too short and can be skipped.
   This bound uses cubic basis conversion only; it does not convert authored
   track representations or replace them with Bezier curves.
4. Otherwise form `F(x) = dot(D(x)/L, D(x)/L) - 1`, of degree at most six.
   Recursively isolate roots of its derivative, down to degree one. Consecutive
   derivative roots partition `F` into monotone intervals. Test these in order,
   bisect sign crossings and also test extrema/endpoints for a tangent root.
5. Refine each sign bracket with at most 64 bisections. Stop on an exact
   polynomial zero, a representationally exhausted bracket, or normalized
   interval width at most `8 * f64::EPSILON`. There is no Newton step or
   division by a small nonlinear slope. Linear derivative roots outside
   `[0,1]`, including overflowed ratios, are excluded.
6. Re-query both returned stations through the existing track sampler. Success
   requires a **finite metre residual within the reported roundoff allowance**.
   An inaccurate candidate is an explicit `ResidualNotMet` failure; the search
   does not silently skip it and return a later branch.

In exact arithmetic this derivative partition finds all crossings and repeated
roots of each finite polynomial. Multiple roots of `F'` are detected at roots
of its own derivative. Constant polynomials and exactly zero leading terms are
handled explicitly. In floating point, derivative multiplicity checks use
`64 * EPSILON * sum(abs(coefficients))`; near-coincident normalized roots are
merged within `16 * EPSILON`. No arbitrary coefficient cutoff silently lowers
the degree.

The independent residual allowance is

```text
8 * f64::EPSILON * max(1 metre, abs(center) + L + norm(P(center)))
```

It accounts for station/coordinate rounding, and is unrelated to the mechanical
assembly tolerance. Searches requiring an allowance larger than **1e-9 m** fail
explicitly rather than accepting increasing physical slack. Residual alone is
not a station-accuracy bound near tangency; the derivative partition and bracket
refinement are needed because the root becomes ill-conditioned there.

The Hermite composition is intentionally tied to the current sampler formula.
Future changes to position interpolation must update this helper and retain
the independent sampler-residual and analytic geometry regressions.

## 3. Search and topology guarantees

Open search retains `h <= min(4*L, 2*min(c, track_length-c))`. Rear/front
stations stay symmetric about the supplied center. The intended direction is
**increasing symmetric span**, not increasing world coordinate, signed vehicle
speed or a globally nearest point on a folded track. There is no extrapolation.

For a valid, ordered generated centerline, the search visits every polynomial
piece in that bounded prefix and tests monotone subintervals in order. It
therefore improves first-branch selection without trusting `h ~= L` as a seed.
This is qualified by f64 coefficient/critical-point accuracy, the residual
allowance, root merging and fixed work limits. It is **not an interval-arithmetic
certificate of complete detection for arbitrary folded geometry**, nor a
global feasibility/uniqueness guarantee. Distinct roots or an infeasible gap
below numerical resolution cannot always be distinguished.

There are at most **4096 visited interval pairs**, **131072 polynomial
evaluations**, five recursive derivative levels and 64 iterations per bracket.
Budget exhaustion returns `SearchBudgetExceeded`, without skipping an unresolved
prefix or accepting a later branch. Very dense sampling or very large search
distances can hit this limit even when a mathematical root exists. The new
interval count can exceed M1's 96 probes; those work units have different costs.

At an open endpoint, only station rounding within the same floating-point
allowance may snap to `[0, track_length]`, followed by the real residual check.
The formerly failing endpoint returns `(599.99, 600)` m. A **20 nm** shortage
is explicitly rejected in the regression; user centers outside the domain are
rejected even if only 1e-12 m outside. No physical dimension is clamped.

### Explicit circuit chord query

`assembly_diagnostics::diagnose_chord_on_layout` accepts the existing
`topology::LayoutMode`. Wrapping requires **Circuit intent and a valid
`topology::diagnose` result**, including endpoint position, tangent, full frame
and section-join tolerances. Open and Shuttle retain open endpoints even on a
geometrically complete circle. Proximity alone is insufficient.

Circuit search is limited to `min(4*L, track_length)` total span, with at most a
half circuit on either side. Pieces split at seams. It returns ordered,
**unwrapped** rear/front stations about a canonical center (`track_length`
normalizes to zero); only sampling uses modulo. Thus at center zero the circle
fixture returns approximately `(-1.202898823894, 1.202898823894)` m. Rigid chord
length is verified against the wrapped samples.

The existing closure contract permits 1 mm / 1 mrad gaps; it does not promise
exact mathematical periodicity. The search does not smooth or rewrite a seam.
A candidate exactly on a discontinuous tolerated seam can fail the independent
sample residual. Complete detection across such discontinuities is not claimed.

### Initial assembly dependency, deliberately not bypassed

**Full consist initialization across the seam remains unsupported.** The chord
query now demonstrates a valid seam-crossing seed, but `consist::seed`,
`inspect`, `initialize`, assembly guide/attitude projection and inspection
station ordering still have an open-track contract. `TrackSimulation` validates
Circuit intent for runtime guidance, but its initializer currently has no
topology parameter. Merely wrapping the two chord stations would leave earlier
placement failing and later station sorting/projection inconsistent.

A compatible follow-up needs explicit topology passed through those stages,
unwrapped train-relative station ordering/neighborhood tests, wrapped sampling
at adapters, topology-aware residual diagnostics and playback/presentation
station handling. Existing open APIs should retain their behavior. This pass
documents that dependency instead of inferring circuit intent from closure or
introducing partially wrapped assembly.

## 4. Fixture outcomes and numerical accuracy

All corrected-success cases independently query the returned stations and
measure Euclidean separation. Analytic span checks separately bound geometry
generation/interpolation error. Fixtures are diagnostic geometry, not designed
coaster layouts.

| Fixture | M1 | M2A / independent check |
| --- | --- | --- |
| Nine circles: R=2/8/30 m, L/R=0.05/0.6/1.9 | Success | Success; first analytic branch, maximum span error 4.253286e-8 m; chord residuals below 1e-12 m |
| R=10 m, L=19.9999 m, center 40 m | Missed positive interval | Success, span 31.352681062958 m; sampled residual zero in this build, analytic span error below 2e-7 m |
| Same approximate circle, L=20 m | No bracket | Remains no bracket; generated/Hermite geometry is slightly inside the ideal diameter near its extremum; no geometry inflation |
| Same circle, L=20.1 m | No bracket | No bracket, explicitly infeasible within this search |
| M1 narrow folded interval, L=1 m | Returned later root at 1.05 m | First root at 1.02 m, independently measured residual 4.45e-16 m in magnitude |
| Straight, center 599.995 m, L=0.01 m | No bracket | Success at endpoint, residual -9.095156e-15 m |
| Complete R=10 circle, four seam centers | Open assembly failure | Explicit Circuit chord query succeeds at 0, 0.2, length-0.2 and length; sampled residual magnitude at most 2.67e-15 m; full assembly dependency remains |
| Exact quadratic fold: D(h)=h-h²/4, L=1 m | Not previously covered | Tangency at h=2 m detected; sampled residual zero; span error below 3e-9 m |
| Same fold, L=1-1e-8 and 1-1e-12 m | Not previously covered | Both first roots agree with `2-2*sqrt(1-L)` within 3e-9 m; residual below 2e-14 m |
| Same fold, L=1+1e-10 / 1.0001 m | Not previously covered | No bracket, preserves the physical shortage |
| Cubic fold: D(h)=h-3h²/4+h³/8 | Not previously covered | Two roots inside one Hermite interval; L=3/8 first root `(5-sqrt(13))/2` within 1e-12 m |
| Same cubic, 1e-10 m below its maximum | Not previously covered | Narrow interval inside one polynomial piece found; independent monotone cubic reference within 1e-9 m, residual below 1e-14 m |
| Straight lengths 1e-8, 1e-6, 0.01, 100, 500 m | M1 covered 0.01 through 20 m | All succeed; independent span/separation error below 2e-12 m |
| Invalid lengths/centers, nonfinite geometry, empty/zero-length track | Explicit errors where covered | Explicit errors; no NaN success or panic; reused reports clear previous observations |
| Dense unresolved prefix / excessive coordinate scale | Not previously covered | Explicit work-budget / precision failure, no later-root fallback |

Exactly tangential mathematical geometry is tested with the exact quadratic
fixture rather than asserting that an approximate integrated circle must have
exactly the ideal diameter. A small residual at tangency alone does not prove
accurate station location; both are independently asserted where known.

## 5. Chord work and Release duration

M1 counters are preserved but their documented units change: `bracket_probes`
now counts visited Hermite interval pairs; `bisection_iterations` includes
derivative-root and chord-root refinement. New `polynomial_evaluations`,
`extrema_tested`, `residual_tolerance_m` and `circuit` fields expose the new work
and acceptance contract. `bracket_span_m` is the first monotone candidate
interval, potentially ending in tangency. New terminations distinguish invalid
topology, numerical residual failure and budget exhaustion. All are transient.

| Fixture | M1 probes / bisections | M2A intervals / bisections |
| --- | ---: | ---: |
| Curved 2 cars | 50 / 96 | 34 / 98 |
| Curved 8 cars | 200 / 384 | 138 / 392 |
| Curved 16 cars | 400 / 768 | 279 / 784 |
| Curved 32 cars | 800 / 1536 | 559 / 1568 |
| Straight 32 cars | 787 / 1536 | 516 / 441 |
| Near-tangent circular chord | 96 / 0, failed | 230 / 49, correct |
| Folded first-root fixture | 26 / 48, wrong branch | 765 / 0, correct |

Each successful new search performs three full sampler queries: center validation
and two independent endpoint checks. Polynomial construction reads sample
positions/tangents directly. A conventional curved chord typically needs 60
polynomial evaluations; it no longer performs approximately 146 expensive
position/frame sampler queries. Tangency/extrema and dense folded cases can
cost more polynomial/interval work. No unbounded search was introduced.

The existing unchanged ignored assembly timing tests were run **before editing
M1 and after M2A**, on the same machine. Tracks are generated outside timers;
one warm solve precedes seven batches of five solves. Values are microseconds
per solve, median [minimum, maximum] of batch means. Compilation, desktop/GPU
work and track generation are excluded. Timed runs were separate from numerical
characterization. These are shared-machine observations, not CI thresholds.

| Fixture | M1 ordinary (us) | M2A ordinary (us) |
| --- | ---: | ---: |
| Straight / 2 | 34.620 [34.260, 40.500] | 17.440 [17.320, 25.880] |
| Straight / 8 | 143.280 [141.120, 170.360] | 74.320 [74.040, 78.440] |
| Straight / 16 | 304.740 [286.500, 325.060] | 149.740 [148.000, 151.580] |
| Straight / 32 | 591.340 [582.060, 605.000] | 322.620 [314.240, 357.220] |
| Curved / 2 | 483.940 [482.920, 488.540] | 457.760 [453.540, 467.860] |
| Curved / 8 | 38268.560 [37996.500, 39469.760] | 38026.640 [37554.060, 38215.000] |
| Changing bank / 8 | 37542.600 [37503.740, 38182.380] | 37787.800 [37344.720, 38124.520] |

Straight ordinary assembly is about half the measured M1 duration; long curved
assembly is dominated by projection and changes little. Exact fingerprints
remain identical for straight 2/8/16. Straight 32 and curved fingerprints change
at f64 precision; existing physical-residual, replay and rigid-dimension checks
pass. Bit-identical M1 poses are not the compatibility contract for a changed
numeric seed.

| Fixture | M1 instrumented (us) | M2A instrumented (us) |
| --- | ---: | ---: |
| Straight / 2 | 43.820 [43.700, 57.300] | 26.800 [26.500, 28.380] |
| Straight / 8 | 205.020 [197.180, 343.760] | 116.340 [114.900, 124.580] |
| Straight / 16 | 396.080 [379.940, 421.560] | 241.860 [233.900, 270.360] |
| Straight / 32 | 817.300 [800.800, 830.660] | 519.820 [508.300, 573.860] |
| Curved / 2 | 535.960 [531.240, 543.880] | 507.500 [495.620, 539.060] |
| Curved / 8 | 40357.060 [39897.960, 40842.440] | 66665.180 [52685.460, 84162.220] |
| Changing bank / 8 | 40240.540 [39539.140, 41115.560] | 67515.580 [64957.040, 75173.360] |

Instrumented curved timing varies substantially, including within these samples
and in M1's historical runs. This run does not establish a stable instrumentation
overhead or its cause. Ordinary timings and unchanged sweep counts do not show
a disproportionate ordinary-path regression on the measured train fixtures.
Dense/worst-budget chord searches, failed 16/32-car duration, and huge attachment
distance scaling were not separately benchmarked; their bounded work is not a
promise of constant-time search or unchanged performance for every input.

## 6. Effect on long chains and recommendations for M2B

The unchanged M1 assembly characterization repeats ordinary and observed solves,
compares results/reports exactly, and verifies immutable train/track inputs.
The new chords do not change the measured convergence classification or sweep
counts:

| Fixture | M1 -> M2A sweeps | M2A terminal joint gap (m) | M2A guide residual (m) |
| --- | ---: | ---: | ---: |
| Curved / 8 | 825 -> 825, converged | 9.7440243e-6 | 9.8050295e-8 |
| Curved / 16 | 2400 -> 2400, limit | 2.2921448e-5 | 2.2705110e-7 |
| Curved / 32 | 2400 -> 2400, limit | 1.0751160e-4 | 1.0605507e-6 |
| Changing bank / 8 | 817 -> 817, converged | 9.7434429e-6 | 9.7811595e-8 |
| Changing bank / 16 | 2400 -> 2400, limit | 2.2717747e-5 | 2.2433138e-7 |
| Changing bank / 32 | 2400 -> 2400, limit | 1.0827651e-4 | 1.0645873e-6 |

These joint/guide residuals agree with M1 at its reported precision. Curved
seeds still start near 6.2216e-3 m joint gap and 4.4444e-4 m guide residual;
changing bank also starts near 1.9205e-3 rad angular error. Small accurate local
car chords do not close the independently authored inter-car drawbars and guide
graph without projection. Straight chains still converge in one sweep.

This separates the corrected chord-detection failures from the unchanged
long-chain closure problem. Slow error propagation/conditioning through the
sequential graph remains a supported hypothesis, not a proven sole cause or
proof that all failed graphs are feasible. No sweep limit was increased to hide
the remaining failures. The stale failure message still describes the last
scheduled check at sweep 2393; M1 diagnostics continue to measure sweep 2400.

Recommended M2B work:

1. Use independently feasible long-chain fixtures and per-edge residual histories
   to measure propagation from the pinned guide, ordering sensitivity and
   conditioning. Keep positional, angular, guide and rigid-dimension errors
   separate; accurate chords are not a global closure certificate.
2. Evaluate seed/ordering/conditioning changes with explicit acceptance criteria,
   comparing convergence work and residual history before changing budgets.
3. Treat topology-aware initialization as explicit API/data-flow work through
   placement, assembly, diagnostics and playback, preserving authored intent and
   open/shuttle behavior. Add whole-consist seam/reversal tests at that stage.
4. Retain nonzero-speed velocity consistency as a separate task. M1's initial
   `J*v` mismatch on curvature/changing bank remains; this pass does not seat or
   project velocities. Wheel fitting/contact likewise remain separate.

## 7. Verification, limitations, UX and Git

Executed from the existing worktree root:

```powershell
cargo test -p qx-physics -p qx-document --tests
cargo test --release -p qx-physics --test assembly_diagnostics --test chord_characterization --test initialization_diagnostics -- --nocapture --test-threads=1
cargo test --release -p qx-physics --test articulation_baseline -- --ignored --nocapture --test-threads=1
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Results:

- Physics/document regression tests: **109 passed, 0 failed; 2 timing tests
  ignored** in the ordinary run (M1: 105 passed). The ten chord tests preserve
  all six M1 cases and add four tests covering the additional numerical cases.
- Native Release characterization: **16 passed, 0 failed** (three assembly,
  ten chord and three initialization tests).
- Native Release assembly timing tests: **2 passed**, explicitly running the
  ignored tests on both the preserved M1 and M2A implementations.
- Workspace build check, strict all-target workspace Clippy, formatting and
  `git diff --check`: **passed**. Untracked source/report files were also
  checked for trailing whitespace. Git's existing LF/CRLF conversion notices
  were informational, not whitespace-check failures.
- Hash comparisons confirm unchanged M1 report, fixture helper, assembly and
  initialization diagnostics tests, timing tests, `consist.rs` and `solver.rs`.

Local raw logs are under ignored `target/m2a-evidence/`; measured tables above
come from `m1-timing.txt`, `m2a-timing-final.txt` and `release-final.txt`.
`regression-final.txt` records the complete targeted regression run. No external
service or asset is required; diagnostic tracks are generated by the fixtures.

Remaining limits: bounded symmetric local search, floating-point root isolation,
the explicit coordinate-precision and work budgets, approximate generated
geometry, tolerance-based circuit closure, open-track full consist initialization
and unchanged long-chain projection/velocity limitations described above.

**Not manually verified:** desktop interaction, rendering, cameras, DPI or
subjective playback. No GUI was controlled or launched. Full workspace tests
and a new application Release package were not run for this physics-only pass;
targeted native Release tests and workspace compilation cover the changed code.
For optional owner verification, run `cargo run -p quantum-x --release -- edit`,
open an existing train project, inspect and initialize an interior train,
exercise Play/Pause/Reset, then save/reopen. This checklist has not been performed
by the agent and does not imply seam-aware initialization is available.

UX status: no owner-approved interaction or visual behavior changed; no temporary
developer UI added; no new UX design approval is required. The added diagnostic
API is not a shipping editor workflow. Existing provisional controls retain
their status.

Git status: local, uncommitted changes on `codex/train-articulation-m1`, including
the preserved M1 work. Nothing was committed, pushed, published, merged or
submitted as a PR. The separate `C:/Dev8/QUANTUM-X` checkout and its unrelated
tooling changes were not edited.
