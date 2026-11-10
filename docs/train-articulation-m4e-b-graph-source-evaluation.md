# Train articulation M4E-B — prepared graph-source evaluation

Date: 2026-11-10. Native Windows x86_64 MSVC, Rust 1.99.1.

**Result:** a feature-gated, immutable numerical graph-source evaluator in
`qx-geometry`, independently checked at arbitrary physical stations. It reuses
production generation, its RK4 step, scalar evaluators and angular-rate equation.
Queries restart from retained checkpoints and perform at most **two local RK4
steps**, independent of distance from the origin. There is no train integration.

The returned spatial jets differentiate the source equations at the numerically
integrated pose. Their provenance is explicit. They are **not exact derivatives
of the local RK4 query extension**. This pass supplies convergent numerical
source evaluation and source-boundary diagnostics; it does not certify a globally
C3 numerical interpolant, error bounds for arbitrary authored projects, or B1
dynamic behavior. The distinction remains a qualification requirement before
using these jets in a test-only constraint adapter.

## 1. Preserved checkpoint and scope

Read `AGENTS.md`, [M4D](train-articulation-m4d-continuity-and-tracking.md) and
[M4E-A](train-articulation-m4e-a-source-rail-adapter.md). M4E-A was clean at
`97950668650ca7850919835f7a88fd1713143813`, containing exactly the two audit
additions described in its report. The existing commit was the checkpoint;
no duplicate checkpoint commit was created. Its retained Release audit and
M4C/M4D regressions passed before implementation: 18 passed, four opt-in tests
ignored, across `source_rail_audit`, `guidance_robustness`, `guidance_continuity`.

Created a managed worktree directly from that commit, then switched to
`codex/train-articulation-m4e-b`:

`C:/Users/coast/.codex/worktrees/train-articulation-m4e-b/QUANTUM-X`.

The original tooling checkout's README, `.vscode/` and development-tooling edits,
and the M1/M4C/M4D/M4E-A worktrees are preserved. A command-local Schannel fetch
succeeded. No Git configuration or certificate policy was changed. No push,
merge, PR, desktop control, or application launch was performed.

The implementation changes neither `generate()`'s options/output semantics nor
the native schema, editor, production physics, train definitions, M2B settings,
wheel assumptions, fixed timestep, or earlier regression tests/reports.

## 2. Reused source interfaces

| Interface | Reuse |
| --- | --- |
| `Context::omega` | Its existing expression is factored once into private `omega_axes`, generic over ordinary vectors or second-order station jets. Production still uses ordinary `Vec3`/`f64`. No second copy of the track equations exists. |
| `Context::derivative` | Unchanged: `C_s=R X`, `q_s=(0,k)q/2`; normalized source frame, world angular rate. |
| `Context::step` | Unchanged RK4, normalization, finite-position, lost-precision and rotation-step guards. Preparation and local queries both use it. No alternative ODE integrator was added. |
| Source knots | The existing knot union/sort/dedup/endpoint policy is extracted into private `Context::knots`; both production and preparation use it. |
| Scalar channels | Existing `Channel::evaluate_side` supplies value, first and second derivatives, including explicit native scalar-profile knot limits. Existing validation, sided input-boundary tolerance and integration knots are retained. |
| `generate` / `CenterlineSample` | Two ordinary generations provide numerical checkpoints and the exact source-to-`smooth_ranges` ownership map. Measurements and graphs are discarded after preparation. No derivative is obtained by differentiating sampled poses. |

A temporary five-section generation control covered straight, arc, rider-local,
force and world-geometric sources with initial bank and roll. Before and after
factoring, the complete `GeneratedTrack` debug-output hash was
`bb19a2023b13cb01` (601 poses, 600 measurements) on this toolchain. The temporary
example was removed. This is a local bitwise regression observation, not a
portable hash/file-format promise. Existing production tests are the persistent
regression controls.

## 3. Prepared solution and supported subset

The opt-in Cargo feature is `qx-geometry/experimental-graph-source`; it is absent
from default application dependencies. The public experimental entry point is
`solver::prepared::PreparedGraph::prepare`. Solver state, equations, RK4 and jet
arithmetic remain private. No other crate exposes a new API.

Preparation clones the exact initial conditions and sections, retains caller
document-instance/generation identity and options, and validates a declared
`AuthoredReference::TrackCenterline`. It invokes `generate` at requested maximum
step h and h/2, with the same per-generation sample budget and all original
guards. It builds charts from source integration knots and sections. Checkpoint
poses are mapped back to the exact input subdivisions used by generation.

For each query, binary search selects the sided source chart and the preceding
checkpoint at each resolution. An exact node returns its saved pose; another
station performs one production RK4 step from that checkpoint. This is reusable
local integration, not a new dense interpolating polynomial. The maximum local
integration distance is one retained cell. No query integrates from station zero
except a query inside the first cell.

Station uses the authored physical centerline, in metres. A section's time input
maps by its **design** speed, never a train's simulated velocity. Section lengths
are accumulated with the existing compensated `InputSum`; shared boundaries use
one identical station value. The station map and generated accumulated distances
can differ at floating-point roundoff. Endpoint source input is set explicitly.

| Section/control type | Status |
| --- | --- |
| Straight, including variable roll | Supported; tangent is constant while the full frame may rotate. |
| Arc, including bank, twist and inversion | Supported using the existing rider-frame curvature equation. Constant curvature plus twist has an independent helical exponential reference. |
| Rider-local pitch/yaw/roll | Supported, including varying quintic/cosine/quadratic profiles. |
| Force normal/lateral/roll | Supported with prescribed section design speed, gravity projection and existing channel derivatives. This is authoring geometry, not train dynamics. |
| Legacy and NURBS **scalar** channel profiles | Existing value/first/second APIs suffice for this jet order. Repeated-knot C0/C1 scalar fixtures are tested and incompatible joins are rejected for continuous intervals. No third scalar derivative is invented or required. |
| Multiple compatible sections | Supported with inherited production pose and sided source jets. |
| Incompatible source joins | Retained as separate sided charts for inspection. `validate_interval` rejects intervals touching/crossing a rejected join, in either direction. Individual explicit one-sided evaluations remain available. |
| World-geometric sections | Conservatively rejected in this prototype; differentiation of its gravity-referenced normalized basis and vertical singularity needs separate qualification. Production generation is unchanged. |
| Native Bézier/B-spline/NURBS/Catmull–Rom track sections | Rejected. Native higher derivatives, station inversion jets and continuous bank transport are outside this milestone. Scalar NURBS profiles are a different subsystem. |
| Rider-reference inverse, variable offsets, periodic wrapping | Rejected/not offered. A full loop can be evaluated on its open `[0,L]` domain; no periodic seam contract is implied. |

Preparation may retain disconnected derivative charts; it does not accept the
whole source as one smooth guide. Any future traversal consumer must call
`validate_interval` and respect sided chart ownership. An isolated boundary query
does not authorize crossing it. No production `AllowRateJump` or
`RequireContinuousRates` behavior is strengthened or relaxed.

## 4. Derivative provenance and equations

Coordinates remain +X forward, +Y up, +Z lateral, SI/f64.
With full local-to-world frame R, T=R X and world angular rate k:

```text
C'   = T
C''  = k × T
C''' = k' × T + k × (k × T)
(R a)'  = k × (R a)
(R a)'' = k' × (R a) + k × (k × (R a))
```

Second-order forward differentiation of the **shared** `omega_axes` expression
propagates ordinary derivatives, not Taylor coefficients. First evaluate with
axis first derivatives k×axis to obtain k'. Then supply axis second derivatives
k'×axis+k×(k×axis) to obtain k''. Scalar derivatives are converted from section
input x by 1/scale and 1/scale², scale=metres/input. This includes the derivatives
of the gravity projections in force sections; gravity is not held fixed in the
rotating frame. All returned jets must be finite.

`Evaluation` reports source/section/chart identity, section input, physical
station, numerical C/R, C'/C''/C''', k/k'/k'', refinement differences and local
RK4 step count. Derivative provenance is
`SourceEquationAtIntegratedPose`. C and R are RK4 approximations. Spatial jets
are analytic differentiation of the source RHS **evaluated on those approximate
poses**. They have inherited integration error and are not exact analytic poses,
fitted derivatives, finite-difference production jets, or derivatives of Hermite
positions/quaternion slerp.

`constant_offset(a)` returns P through P''' using the M4E-A product-rule formula,
in centerline station. It rejects nonfinite offsets/results and datum tangent
length <=1e-12, including the inward circle-centre offset. It defines no rail
surface, wheel radius, running/side normal, contact law, or projection guarantee.

## 5. Numerical accuracy and continuity

Each evaluation reports raw fine/coarse differences for C, orientation, and all
three angular orders. These include accumulated checkpoint and local query
errors. They are **not certified upper bounds**. Difference/15 estimates fine
RK4 error only in an asymptotic, truly 2:1 refinement regime. Ceil-rounded cell
counts and generation's four-cell minimum can give another ratio, even identical
grids on short spans. Zero difference is not proof of zero error. The prototype
does not automatically select a tolerance or accept an arbitrary source for B1.

No pose interpolation occurs, so there is no separate Hermite/slerp interpolation
error. There is local RK4 extension error. Its literal derivatives need not equal
the returned source jets; artificial checkpoint limits can have small numerical
derivative defects. A query's source jets agree across the inherited checkpoint
pose to roundoff; that fact alone does not prove the numerical extension C3.
The independent pose stencils below measure approximation consistency, not a
global continuity theorem. No smoothing or derivative averaging hides this issue.

Independent rigid exponential/circle references at 72 stations per case,
requested h=.1 m, fine h=.05 m, initial bank .6 rad:

| Source | Maximum C error, m | Maximum orientation error, rad |
| --- | ---: | ---: |
| M1 600 m straight | 5.685e-14 | 0 |
| M1 R12, 80 m turn | 5.114e-12 | 3.908e-13 |
| R12 full vertical loop | 4.390e-12 | 3.687e-13 |
| R8, .04 rad/m twist helix | 2.544e-11 | 2.853e-12 |
| Existing constant rider-local controls | 3.927e-14 | 7.754e-16 |

These tests also check C'–C''' and k–k'' against the independent reference.
The retained R40 force-circle fixture matches its independent circle and constant
world angular rate. Integrated quadratic-roll orientation matches its explicit
scalar roll integral; k/k'/k'' match the closed form. Time/distance versions of
one profile agree in physical station, including derivative units.

For an R10 arc, h=2/1/.5 m gives fine endpoint errors
4.912294e-7 / 3.073100e-8 / 1.922112e-9 m. At the **non-node** station 8.7316 m,
errors are 4.085887e-7 / 2.650799e-8 / 1.689173e-9 m. Both sequences demonstrate
fourth-order convergence. Fine generation nodes agree with the prepared query
within 1e-11 m position / 1e-12 in frame-normal vector, including station
accumulation roundoff.

Eight independent degree-10, 17-point NumPy pose stencils cover varying
rider-local and force profiles at .2, 7.431, 15.0137 and 29.8 m. They fit positions
and all three rotated axes, without the Rust source RHS or jet algebra. Angular
jets are independently recovered from frame derivative identities. Observed max
errors against Rust source jets:

| C' | C'' | C''' | k | k' | k'' |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 6.044e-13 | 1.501e-11 | 1.618e-10 | 4.115e-14 | 1.077e-12 | 1.643e-11 |

Eight additional constant-offset position stencils at a=(.1,-.08,.6) m agree
through P''' with maximum errors 6.089e-13 / 1.528e-11 / 1.649e-10. Independent
check tolerances are 2e-9 / 2e-8 / 2e-6 for first/second/third spatial derivatives;
they are numerical verification tolerances, not changed B1 acceptance gates.
Rust centered-difference checks separately verify the derivative chain. These
observations do not bound every possible interval, step size or source profile.

Source joins compare explicit k/k'/k'' limits at the **same shared numerical
pose**, separating source jumps from integration differences. Absolute 1e-9
thresholds in rad/m, rad/m², rad/m³ classify compatibility numerically. With
inherited C/R and these angular limits, C'–C''' and the constant-offset jets have
the corresponding source compatibility. This classification is not symbolic
proof at the exact unknown boundary solution.

The retained straight-roll counterexamples reject each missing angular order:

| Source join | k gap | k' gap | k'' gap |
| --- | ---: | ---: | ---: |
| Zero into .02 constant | .02 | 0 | 0 |
| Linear into equal constant | 0 | .004 | 0 |
| Quadratic into tangent-matching linear | 0 | 1.735e-18 | .0016 |

Both section joins and internal transition joins are tested, with exact sided
section/chart ownership. The latter two section joins still pass production
`RequireContinuousRates`, as expected; that policy never promised higher jets.

The reproducible fixture probe loads **actual** `qx_document::sample` data:
foundation's first two sections, length 72 m, form three source charts. Joins at
12 and 42 m have zero measured angular gaps through k''. Adding section 3 makes
the 72 m join fail with gaps .01544785384 / .000139440271 / 7.54476322e-7.
All 997 off-grid queries in each supported prefix succeed. The full five-section
project rejects section 4's unsupported world-geometric type. It is never silently
converted, reshaped, or claimed to be one C3 guide.

## 6. Cost, invalidation and next milestone

Preparation is two production generations plus source-cell bookkeeping and sided
boundary jets. Retained memory is linear in the two cell counts; temporary peak
memory also includes the generated pose/graph arrays. Each retained cell stores
two poses and two input values (128 bytes on this target). The 600 m probe has
18,000 cells, about 2.30 MB of cell payload, plus containers/source metadata.
This is a size calculation, not a peak-memory measurement.

Queries binary-search charts/cells and perform at most two local RK4 steps plus
two source-jet evaluations. Channel lookup retains its existing bounded linear
scan (up to 4096 transitions/channel); cost is not independent of source complexity.
There is no length-proportional traversal from the initial station. Interval
validation scans the finite source-boundary list; it is separate from pose queries.

Serial Release observation, eleven batches of 1000 varying-profile rider-local
queries, compilation/logging excluded from query timing:

| Length | Preparation ms | Checkpoints | Query µs minimum / median / maximum |
| --- | ---: | ---: | --- |
| 30 m | .8285 | 902 | 1.4948 / 1.4964 / 1.5056 |
| 600 m | 14.2949 | 18002 | 1.5240 / 1.5283 / 1.5806 |

Every query uses at most two RK4 steps. Preparation includes its full two calls to
`generate`. Timings are shared-machine observations, not worst-case bounds,
physics-tick timings, deadline guarantees, or production FPS. The timing run was
separate from the workspace test run.

Snapshots own exact source clones. `matches_source` checks initial conditions,
all sections/IDs/channels/domain/speed and options. Every evaluation/interval
query checks the expected `(document-instance,generation)` token first. The
caller must retire that token on edits, undo/redo, ID replacement, reordered
sections, speed/domain/length changes or regeneration settings changes. The
evaluator cannot detect an edit if a caller deliberately supplies its old token.
Failed regeneration must not relabel the retained old snapshot as current.
The physical-role declaration must also come from the current source; arbitrary
section data cannot prove that the caller did not mislabel rider authoring.
Offset/face changes require separate invalidation of future derived datum/hint
caches. There are no app/projection caches or persisted identity fields here.

The next smallest milestone is a **test-only** constant-datum/normal adapter over
accepted physical source charts, with explicit accuracy/extension-defect gates,
source/face/chart tokens and sided interval rejection. Validate its P/P'/P''/P'''
and n/n'/n'', then use that same adapter for all B1 initialization, residual/J,
Jdot and reaction calls. First qualify the numerical differential mismatch on
the selected sources; do not quietly assert that local RK4 is an exact C3 dense
solution. If stricter guarantees are necessary, investigate a derivative-matched
dense extension around the same RHS upstream. Native higher jets, rider inverse,
periodic seams, contact, energy integration and production guidance remain separate.

## 7. Verification and files

Evidence is local under ignored `target/m4e-b/`. Cargo commands used a
command-local `CARGO_TARGET_DIR` pointing to the existing M4C build cache;
old source/evidence files were not edited. To reproduce with a new target cache,
omit that environment override. NumPy is required only for the independent tool.

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features qx-geometry/experimental-graph-source -- -D warnings
cargo test --workspace --features qx-geometry/experimental-graph-source
cargo test --release -p qx-geometry --features experimental-graph-source
cargo test --release -p qx-geometry --features experimental-graph-source --test graph_source -- --nocapture --test-threads=1 > target/m4e-b/graph-release.log 2>&1
python tools/verify-graph-source-m4e-b.py target/m4e-b/graph-release.log
& tools/verify-graph-source-fixtures-m4e-b.ps1
cargo test --release -p qx-geometry --features experimental-graph-source --test graph_source prepared_cost -- --ignored --nocapture --test-threads=1
cargo test --release -p qx-physics --test articulation_baseline --test assembly_convergence --test assembly_diagnostics --test chord_characterization --test initialization_diagnostics --test wheel_geometry --test bogie_fitting --test mechanical_guidance --test rail_normal_guidance --test guidance_robustness --test guidance_continuity --test source_rail_audit -- --nocapture --test-threads=1 > target/m4e-b/characterization.log 2>&1
python tools/verify-train-guidance-m4a.py target/m4e-b/characterization.log
python tools/verify-train-guidance-m4b.py target/m4e-b/characterization.log
python tools/verify-train-guidance-m4c.py target/m4e-b/characterization.log
python tools/verify-train-guidance-m4d.py target/m4e-b/characterization.log
git diff --check
git diff --cached --check
```

Final results on the finished Rust source:

- Formatting, default workspace check, and strict all-target workspace Clippy
  both with and without the experimental feature: passed.
- Full debug workspace with the feature: **511 passed, zero failed, 14 ignored**.
  This includes every inherited production geometry and M4C/M4D corner regression.
- All geometry Release tests with the feature: **105 passed, zero failed, one
  ignored**. The new ignored cost test was separately executed and passed.
- Retained native Release physics characterization: **83 passed, zero failed,
  12 ignored**, using unchanged physics test sources and default geometry features.
- New independent NumPy checks: **eight pose-jet and eight constant-offset cases
  passed**. The actual-document fixture probe passed its prefix/chart/rejection
  assertions and 1,994 arbitrary-station evaluations.
- Retained independent NumPy verifiers: **38 M4A matrices, 29 M4B matrices, six
  M4C reaction systems, 18 M4D reaction systems and 216 M4D geometry cases passed**.
- `Context::derivative` and `Context::step` are byte-identical to M4E-A after
  newline normalization. Git staged/unstaged whitespace checks passed.
- The twelve inherited opt-in physics long-run/profiling tests and GPU evidence
  exporter were not explicitly executed. No manual GUI result or new application
  Release package is claimed.

Primary logs: `workspace.log`, `geometry-release.log`, `characterization.log`,
`graph-release.log`, `fixture-probe.log`, `cost.log`, `check.log`, `clippy.log`,
`clippy-default.log` and `m4a`�`m4d-independent.log`. The generation-control probe
is recorded in `generation-control.log`. Logs and the fixture probe's generated
Cargo package remain ignored, outside the commit.

Changed files:

1. `crates/qx-geometry/Cargo.toml`: opt-in research feature; no dependencies added.
2. `crates/qx-geometry/src/solver.rs`: shared rate/knot seams; RK4 unchanged.
3. `crates/qx-geometry/src/solver/rate.rs`: single generic source-rate expression.
4. `crates/qx-geometry/src/solver/station_jet.rs`: private second-order differentiation.
5. `crates/qx-geometry/src/solver/prepared.rs`: immutable prepared evaluator.
6. `crates/qx-geometry/tests/graph_source.rs`: nine ordinary tests and opt-in cost probe.
7. `tools/verify-graph-source-m4e-b.py`: independent pose/offset derivative checks.
8. `tools/verify-graph-source-fixtures-m4e-b.ps1`: actual-document-fixture probe in an ignored standalone Cargo package; no crate dependency/feature wiring changes.
9. This report.

No owner-facing UX behavior is changed; no development-only UI is added; no new
UX decision is finalized. Future presentation of physical source diagnostics
requires human design. GUI rendering, interaction and playback are **not manually
verified**. No new editor command or application release package is delivered.
Existing owner regression command: `cargo run -p quantum-x --release -- edit`;
open an existing train project, Inspect/Initialize, Play/Pause/Reset, save/reopen.
This milestone adds no UI feature for that checklist to demonstrate.

Work is local on `codex/train-articulation-m4e-b`, based on the verified M4E-A
checkpoint. The implementation/report are committed locally after verification.
No push, merge or PR is performed or requested.
