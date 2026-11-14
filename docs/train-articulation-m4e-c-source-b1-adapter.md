# Train articulation M4E-C — source/B1 adapter qualification

Date: 2026-11-10. Native Windows x86_64 MSVC, Rust 1.99.1.

**Result: stop before datum/normal adapter construction or B1 coupling. A
derivative-matched numerical dense extension is required for the proposed
multi-section physical rail contract.** M4E-B's poses converge accurately, and
its source-equation jets remain useful. They are different derivatives from
those of its restart-at-checkpoint query extension. A resolved rolling-line
counterexample has artificial derivative jumps inside one compatible source
chart. Finer integration reduces these defects but does not supply a single
third-order differentiable reference across restarts.

This is a completed qualification finding, **not** a source-driven B1 dynamics
result. No new physical rail, normals API, projection backend, B1 installation,
parallel graph integrator, production generation change, or physics rewrite is
implemented. A constant-offset position diagnostic uses the existing M4E-B
algebra; it is never supplied to the mechanism.

## 1. Baseline, isolation and feature settings

Read repository `AGENTS.md`, [M4E-A](train-articulation-m4e-a-source-rail-adapter.md),
[M4E-B](train-articulation-m4e-b-graph-source-evaluation.md),
[M4C](train-articulation-m4c-guidance-robustness.md) and
[M4D](train-articulation-m4d-continuity-and-tracking.md), together with the
prepared evaluator, original RK4, scalar/rate implementation and B1 contract.

The requested M4E-B checkpoint is
`77c49c8baf9f88186628204333a59334c949f9aa`, with a clean M4E-B worktree. Created
a managed worktree **directly at that commit**, then switched to
`codex/train-articulation-m4e-c`:

`C:/Users/coast/.codex/worktrees/train-articulation-m4e-c/QUANTUM-X`.

The original checkout was **not clean**: it had modified `README.md` and untracked
`.vscode/` and `docs/development-tooling.md`. Those edits and all earlier
worktrees are preserved. They were not copied into this pass. A command-local
Schannel fetch succeeded; no Git configuration or certificate policy changed.
No push, merge, PR, GUI launch, desktop control or external message was
performed.

Before additions, native Release checkpoint checks passed: nine M4E-B graph
tests (one cost test ignored), and 18 M4C/M4D/M4E-A tests (four opt-in tests
ignored). The application feature tree contains only the default geometry
feature. `experimental-graph-source` remains opt-in; no Cargo manifest, default
feature, application dependency, document schema or production timestep changes.

## 2. Qualification gates and why they apply

Station is physical centerline distance in metres; axes remain +X forward,
+Y up, +Z lateral. The mathematical source ODE has `C'=RX` and
`R'a=k cross Ra`. M4E-B analytically differentiates that source equation at
numerically integrated poses. Its query pose is instead one normalized RK4
step with a variable endpoint, starting from the preceding saved checkpoint.
An exact checkpoint returns its saved pose.

The acceptance sequence was fixed before considering a new rail backend:

1. Accept only finite, declared physical-centerline source charts with valid
   identity, sided domains and compatible source limits. Reject unsupported
   kinds and invalid intervals before evaluating constraints.
2. Establish that query position/frame and reported jets describe **one
   differential reference**, including internal numerical checkpoints. The
   generic M4D eliminated-projection/Jdot algebra needs P through third order
   and n through second order in a neighborhood, not only at zero residual.
   Full C3 C/R is its useful sufficient condition. Small pose error or agreement
   between two evaluations of the source RHS does not establish this condition.
3. If that condition is established, check the candidate adapter and B1 against
   the **existing** numeric gates below. Do not replace them with larger
   independent-differencing tolerances. Require refinement/conditioning evidence
   that resolves error at the relevant gate; unresolved measurements grant no
   qualification.

| Inherited test/solver quantity | Unchanged gate | Relevance to the source adapter |
| --- | --- | --- |
| Projection F=(x-P) dot P' | absolute F < 2e-13 m | P' must differentiate the same P used to locate the stationary point. |
| Projection D | D > 1e-8 | Local regularity, in addition to nonzero datum tangent and normal cross product. |
| Position initialization | maximum row < 2e-11 | One reference for seating and subsequent residuals. |
| Velocity initialization | maximum Jv < 1e-9 | J must differentiate those residuals. |
| Independent residual/J column differences | discrepancy < 3e-7 | Includes closed and .002-scaled displaced poses. |
| Directional Jdot*v refinement | discrepancy < 1e-6 | Derivatives and chart membership must survive stencil refinement. |
| Independent second differences of c | discrepancy < 2e-5 | Independent acceleration-constraint check. |
| Acceleration equation / independent reactions / ideal power | 1e-7 in each equation's units / N or N m / W | Required only after geometry and constraints qualify. |
| Short-run quality | position rows < 2e-5, velocity rows < .003 | Not permission to relax the tighter differential checks. |

The structural second gate is not a newly tightened numeric threshold. At a
genuine derivative jump the relevant classical derivative is undefined; a
single value cannot be the two one-sided limits. The numeric gates concern
well-defined quantities. A separately justified approximate-RHS constraint model
with restricted stencil widths and interval defect bounds might be possible,
but M4E-B does not provide that contract, and adopting it is not silently
equivalent to differentiating its actual numerical query function.

The 1e-9 angular compatibility thresholds in `PreparedGraph::Boundary` classify
**source** k/k'/k'' limits at a shared numerical pose. They neither inspect
internal RK4 restart derivatives nor certify numerical-extension C3 continuity.
No new tolerance in this milestone accepts a failed chart.

In particular, the inherited formulas

```text
F=(x-P) dot P'
D=P' dot P'-(x-P) dot P''
ds/dx=P'/D
g=n+P' * ((x-P) dot n'-P' dot n)/D
```

use the chain rule for that **same** P and n. If P' is a source-RHS value rather
than the derivative of the queried P, then -F_s is not generally the supplied
D, and g need not differentiate the actual eliminated residual. Independent
position/frame accuracy does not remove those missing terms. Differentiating
g for Jdot also needs the consistent higher derivatives.

## 3. Reproducible measurement design

New executable code is confined to a feature-gated geometry integration test,
its measurement helper, and two developer tools:

- `graph_extension_qualification.rs`: measured first-derivative defect/refinement,
  diagnostic exports, identity/domain/unsupported-source negatives, and an opt-in
  upstream query timer.
- `support/m4e_c_extension.rs`: reuses `PreparedGraph` and ordinary `generate`;
  it implements no ODE or rail backend. It reconstructs fine checkpoint station
  locations from the existing chart/range subdivisions, independently of
  accumulated generated distances.
- `verify-graph-extension-fixtures-m4e-c.ps1`: runs **actual**
  `qx_document::sample::foundation_project()` data in an ignored standalone probe.
  It does not copy the foundation controls or add dependencies/features to crates.
- `verify-graph-extension-m4e-c.py`: independent NumPy differentiation of actual
  emitted query poses, rigid-exponential references, and a high precision
  fixed-axis RK4 control using only Python's standard `Decimal`/`Fraction`.

The matrix covers straight, banked R12 arc, R8/.04 twist helix, the retained
12 m/.4 rad/m rolling line, M4E-B varying rider-local and force sections, and
foundation's compatible two-section 72 m prefix. Requested maximum steps are
.2/.1/.05 m; retained fine steps are .1/.05/.025 m. Original rotation/sample
guards apply, with no override.

These are numerical development controls, not a proposed coaster layout. The
arc controls are shortened to 12 m and use initial bank .6 rad; rider/force use
the retained control profiles with that same initial bank. The rolling line has
identity initial orientation. The foundation prefix uses its actual, unchanged
document initial conditions and sections. None is asserted to have the same
geometry as M4D's separate analytic bank/helix references.

There are **621 independent 17-pose stencils** (10,557 emitted pose rows). Each
contains C and all three rotated axes, not sampled `GeneratedTrack` interpolation.
It also records the center's source C'/C''/C''' and k/k'/k'', and the derived
source frame derivatives through order three. Locations include:

- Off-grid ordinary stations within a cell.
- First, middle and penultimate internal fine checkpoints in every source chart.
- Separate left/right stencils of .8 and .4 cell widths, and stencils crossing
  the same checkpoint. Each one-sided stencil remains inside its actual cell.
- Sided source-chart endpoints and stations .1 cell inside each endpoint.
  Foundation includes the 12 m section join and 42 m internal transition join.

NumPy fits degree-eight polynomials to **positions and axes only**, with actual
rounded station differences and the center value removed. Angular derivatives
are recovered independently from orthogonal-axis identities:

```text
k   = sum(a cross a') / 2
k'  = sum(a cross a'') / 2
k'' = sum(a' cross a'' + a cross a''') / 2
```

The verifier records raw mismatches and f64 input-conditioning estimates. These
are measurements, **not adapter acceptance thresholds or certified bounds**.
In particular, shrinking f64 position stencils at world coordinates of tens of
metres can amplify roundoff above the derivative defect being investigated.
Even the exact straight control then has apparent nonzero higher differences.
No such noisy maximum is asserted to be a true source defect.

## 4. Independent references, refinement and the resolved defect

Independent rigid-exponential controls check position, frame axes and the source
spatial/angular jets. At requested step .1 m, maximum position errors are
1.782e-12 m for the R12 arc and 9.584e-12 m for the twisted R8 arc. Maximum frame
axis errors are 5.896e-14 and 5.710e-13 respectively. The rolling line's position
is exact in this fixture; its frame-axis error against the exact source is
1.500e-10. These demonstrate pose/source-solution accuracy, not derivative
matching of the numerical extension.

Raw fine/coarse position differences at four fixed stations per case:

| Fixture | requested .2 m | requested .1 m | requested .05 m |
| --- | ---: | ---: | ---: |
| Varying rider-local | 1.02716e-9 | 6.41761e-11 | 4.00670e-12 |
| Varying force | 1.62235e-9 | 1.01357e-10 | 6.27632e-12 |
| Actual foundation prefix | 8.18321e-10 | 5.11723e-11 | 3.09285e-12 |

The approximately sixteenfold reductions support fourth-order pose convergence
in this experiment. They are raw differences, not guaranteed error bounds.
Rider/force k/k'/k'' refinement differences are also recorded, with correct
physical-station units. No alternate general graph solver is used as an oracle.

A Rust sixth-order centered first difference at ordinary points inside the
rolling line's first fine cell gives frame-first-derivative mismatches
1.601435e-9 / 1.001076e-10 / 6.256541e-12 for requested .3/.15/.075 m.
The approximately sixteenfold reductions resolve an actual query/RHS difference
without touching any source join.

### High precision restart control

For the constant rolling line, the quaternion has only two nonzero components.
The independent Python control reduces the existing normalized RK4 method to a
closed two-component expression for **one step**. It normalizes every stage;
the usual linear RK4 stability polynomial would represent a different method.
It is not a second general geometry engine and cannot evaluate force, rider-local
or arbitrary graph sources. Its values agree with **all 69 rolling stencils**,
including distant checkpoints, within **1.638e-15 per pose component**.

Nine-point, exact-rational-weight one-sided differences of that expression use
70 and 90 decimal digits, two spacings (1e-5 and 5e-6 m), and limits from each
side of the first fine checkpoint. Spacing refinement agrees below 1e-25 in
the derivative vectors. At the restart, the pose is shared; the right derivative
jet equals the source jet, while the incoming RK4 extension has the following
resolved defects. Frame columns report the maximum axis-vector norm.

| Fine cell h, m | frame first, /m | frame second, /m² | frame third, /m³ | k' defect, rad/m² | k'' defect, rad/m³ |
| --- | ---: | ---: | ---: | ---: | ---: |
| .1 | 9.99817e-10 | 3.99970e-8 | 1.19993e-6 | 3.99890e-8 | 1.19945e-6 |
| **.05 (default)** | **6.24971e-11** | **4.99991e-9** | **2.99996e-7** | **4.99966e-9** | **2.99966e-7** |
| .025 | 3.90621e-12 | 6.24997e-10 | 7.49997e-8 | 6.24989e-10 | 7.49979e-8 |

The k defect equals the frame-first column in this fixed-axis control. The
mathematical source has constant k=.4 X and k'=k''=0 throughout. It has **one
source chart and no source boundaries**. Thus these are artificial restart
defects, not authored roll jumps. The first/second/third defects shrink roughly
as h^4/h^3/h^2, respectively. They remain nonzero at every finite tested h.

Decimal differentiates the real-arithmetic RK4 branch expression, isolating the
method defect from floating-point evaluation noise; it does not assert that a
literal f64 staircase is classically differentiable. The agreement with Rust
poses and the independently resolved Rust first difference connect this control
to the implemented query. No incoming limit is replaced by the source RHS.

### Physical-offset implication and numeric gate comparison

For the explicitly declared local offset a=(.1,-.08,.6) m, differentiate
P=C+Ra on each numerical branch. No mesh or wheel geometry is consulted.

| Fine h, m | P' jump | P'' jump, /m | P''' jump, /m² | false source F at an incoming-stationary 2 mm target, m |
| --- | ---: | ---: | ---: | ---: |
| .1 | 6.05199e-10 | 2.42106e-8 | 7.26330e-7 | **1.17641e-12** |
| .05 | 3.78301e-11 | 3.02649e-9 | 1.81590e-7 | 7.35355e-14 |
| .025 | 2.36446e-12 | 3.78317e-10 | 4.53981e-8 | 4.59613e-15 |

The 2 mm displacement is perpendicular to the actual incoming P' and maximizes
the source-P' stationarity error in that plane. Its incoming-sided F is zero to
high precision. At requested .2 m (fine h=.1) the erroneous source F already exceeds
the **unchanged 2e-13** projection gate. At default resolution this particular
F comparison passes; that fact alone does not qualify J/Jdot or restart crossing.

At default h=.05, centered second differences of the actual offset position
across the restart, multiplied by 6² for a **datum traversal diagnostic** at
6 station-m/s, disagree with the source acceleration as follows:

| Station half-width, m | acceleration discrepancy, m/s² |
| --- | ---: |
| 6e-5 | 2.26437e-5 |
| 3e-5 | 4.53417e-5 |
| 1.5e-5 | 9.07379e-5 |
| 1e-6 | 1.36183e-3 |

The first half-width corresponds to 6 times the B1 directional time stencil
1e-5 s. The discrepancy grows approximately as the reciprocal width because
the restart has a P' jump. At fine h=.025 the first discrepancy falls to
1.41189e-6, then still grows under width refinement. Refining RK4 is not the
same as establishing a differentiable numerical reference.

This diagnostic exceeds the scale of the 1e-6 refinement and 2e-5 independent
acceleration gates. It is **not a B1 constraint residual, failed train step or
reaction measurement**. Particular scalar constraints can have cancellations;
none can be assumed for arbitrary support offsets and finite residuals. The
resolved lack of the generic differential contract is the reason to stop.

### Actual authored prefix and unresolved f64 differences

Foundation's first two sections form three source charts. The 12 and 42 m
source boundaries have zero angular gaps through k''. At requested .1 m its
ordinary-station position-only fits differ from C'/C''/C''' by at most
4.584e-13 / 9.334e-11 / 1.018e-7. Frame third-derivative mismatch is at most
3.883e-9 there. Those are encouraging local measurements.

At one-sided checkpoint stencils, raw foundation C''' differences reach
4.978e-5, while the conservative f64 input-conditioning estimate reaches
5.137e-3. These numbers **do not resolve a true C''' defect**. The report does
not mislabel noisy finite differences as either proof of discontinuity or proof
of consistency. Complete per-location C/R/k mismatch, conditioning and
refinement records are in the independent JSON summary. Unlike the reduced
rolling control, these arbitrary-source results supply no interval certificate.

## 5. Supported/rejected charts and physical regularity

| Candidate | Source evaluation | M4E-C physical B1 qualification |
| --- | --- | --- |
| Nonrolling straight | Exact derivative control to arithmetic precision | Positive control only; no new straight-only backend built. |
| Straight with variable/constant roll | Supported by PreparedGraph | Rolling restart counterexample fails generic differential contract. |
| Banked arc, twisted arc | Supported; independent exponential accuracy checks pass | Numerical-extension derivatives not certified across restarts. |
| Rider-local and force sections | Supported; independent query differences/refinement measured | Not admitted as a physical B1 chart by these measurements. |
| Actual foundation prefix [0,72] m | Source-compatible, three charts | Source compatibility passes; numerical differential qualification withheld. |
| Foundation including section 3 | Evaluator retains sided charts | 72 m join rejected: k/k'/k'' gaps .01544785384 / .000139440271 / 7.54476322e-7. |
| Discontinuous roll/rate/higher-rate joins | Sided evaluations retained | Interval touching/crossing the join rejected in either direction. |
| World-geometric sections | Explicit preparation error | Unsupported, including foundation section 4. |
| Native track sections | Explicit preparation error | No higher native jet/transport implementation. |
| Rider-reference inverse geometry | Explicit physical-role error | No station inversion or rider-to-track derivative substitution. |
| Open endpoints / out-of-domain queries | Valid inward side only / explicit error | No wrapping, periodic seam or remote station fallback. |

The new negatives also preserve stale document/generation rejection and
equal-geometry section-ID replacement detection. A circle-center offset fails
the existing nonzero datum tangent check. A separate R8/.04 twist offset
a=8 Y gives a **nonzero** P'=.32 RZ but zero `RZ cross P'`; normalization fails.
This explicitly records the degenerate running-normal case without constructing
a usable normal adapter. A regular P' alone is insufficient.

Ambiguous local projections remain explicit failures in the **unchanged**
M4B–M4D controls. There is no new source projector whose branch behavior can be
claimed. No failed source/chart triggers smoothing, an enlarged tolerance,
unsupported world geometry, a widened window or a remote-root search.

## 6. Identity, mechanics and cost

Every existing prepared evaluation/interval query checks the caller's document
instance and generation. `matches_source` compares exact initial state, sections,
IDs, controls and generation options. New tests reject both wrong document and
wrong generation, and detect replacement section identity. The source arrays
remain exactly unchanged after measurement.

Caller-owned token retirement is still required on edits/undo/regeneration; a
caller reusing the old token cannot make the evaluator detect unseen edits.
Datum/face/offset identity and station-hint tracking are **not implemented** here,
because the datum/normal adapter was not qualified. They must be bound to the
source/chart generation when that work becomes justified. No fabricated
stale-datum API or persisted identity field is added.

Source-derived B1 initialization, projection, residual/J, velocity initialization,
directional Jdot*v, reactions, rigid-body dynamics and transactional steps are
**not reached**. No mixed source-initialization/sampled-simulation path exists.
The original M4B–M4D synthetic references, numerical acceptance gates, rigid
attachments, support-force/power checks, rollback and replay remain unmodified
controls. Their successful regressions do not certify this different geometry.
No new source reaction, contact, energy fidelity, 24 m/s or production claim
follows.

The retained M4D independent rerun reproduces 18 reaction systems and 216
geometry cases: maximum load discrepancy 1.188e-11 N/N m, acceleration residual
2.504e-13 in each row's units, and ideal reaction power 1.513e-11 W. These agree
with the original research observations on its **separate synthetic references**.
They are not measurements on the foundation source or the rolling restart
diagnostic.

The serial Release measurement on the Intel Core i7-14700KF ran after all other
test/build commands from this pass finished. A complete adapter query or B1
source step cannot be timed: neither exists after the qualification stop.
The opt-in upstream timer includes
exact source snapshot comparison, identity checks, interval validation, both
resolution queries/RK4 work, source jets, and existing P-through-P''' constant
offset algebra. It excludes normals, projections and tracking, so its timings
must not be presented as full adapter or physics-step costs.

Microseconds per upstream query, **batch average** minimum / median / maximum
over eleven 1,000-query batches after a 1,000-query warm-up:

| One-section fixture | Minimum | Median | Maximum |
| --- | ---: | ---: | ---: |
| Straight | .8439 | .8518 | 1.3433 |
| R12 arc | 1.2648 | 1.2785 | 1.5032 |
| Twisted R8 arc | 1.1631 | 1.1663 | 1.2084 |
| Rolling line | .8693 | .8763 | .8871 |
| Varying rider-local | 1.5724 | 1.5765 | 1.6086 |
| Varying force | 1.4510 | 1.4563 | 1.4708 |

Preparation, construction, warm-up, compilation and logging are outside timers.
Stations vary within each batch and are off-grid. These are throughput observations on a
shared machine, not individual-query worst cases, a multi-chart source benchmark,
physics tick times, deadline guarantees or editor FPS. No tolerance or numerical
guard is weakened to obtain them. Full adapter query and full source-backed B1
step costs remain **not available**.

## 7. Smallest next production-infrastructure prerequisite

Add an **upstream, derivative-matched dense position/full-frame extension around
the same shared Context RHS**, initially behind the existing experimental
feature. Retain the real source equations, physical station map, immutable
identity and sided chart validation. Match sufficient derivatives through order
three at every numerical checkpoint and accepted source boundary, with documented
pose/derivative accuracy and compatibility between C' and RX. Differentiating
the retained representation must produce the jets supplied to its consumer.

This need not change production `generate()` output in the next research pass.
Its acceptance tests should retain this counterexample, independent one-sided
limits, arbitrary-station derivatives, actual foundation joins, bounded query
work and refinement experiments. Do not replace this with independent fits of
C/R, another physics-side geometry solver, differentiated render samples,
source-jet substitution or per-checkpoint traversal rejection disguised as a
continuous whole guide. Merely exposing more checkpoint metadata does not repair
the missing derivatives.

After that contract passes, the smallest adapter is still the M4E-A constant
datum/normal algebra, with bounded regular projection and source/chart/offset/
face generation identity. Supply that **same implementation** to every B1
equation and only then attempt short runs at the unchanged 1/240 s outer step.
Native higher derivatives, rider inversion, periodic seams, contact, suspension,
friction, wheel spin and energy-integrator changes remain separate work.

## 8. Verification, files, UX and local Git

Commands run from this worktree (logs/build products are ignored). Cargo used a
command-local `CARGO_TARGET_DIR` pointing to the M4E-B worktree's target cache;
its source and earlier evidence were not removed. A separate default target
directory also works. Serial timing is performed only after the other checks
finish.

```powershell
New-Item -ItemType Directory -Force target/m4e-c
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features qx-geometry/experimental-graph-source -- -D warnings
cargo test --workspace > target/m4e-c/workspace-default.log 2>&1
cargo test --workspace --features qx-geometry/experimental-graph-source > target/m4e-c/workspace-feature.log 2>&1
cargo test --release -p qx-geometry --features experimental-graph-source > target/m4e-c/geometry-release.log 2>&1
cargo test --release -p qx-geometry --features experimental-graph-source --test graph_extension_qualification -- --nocapture --test-threads=1 > target/m4e-c/extension-release.log 2>&1
cargo test --release -p qx-geometry --features experimental-graph-source --test graph_source -- --nocapture --test-threads=1 > target/m4e-c/graph-release.log 2>&1
& tools/verify-graph-extension-fixtures-m4e-c.ps1 > target/m4e-c/fixture-release.log 2>&1
python tools/verify-graph-extension-m4e-c.py target/m4e-c/extension-release.log target/m4e-c/fixture-release.log
python tools/verify-graph-source-m4e-b.py target/m4e-c/graph-release.log
cargo test --release -p qx-physics --test articulation_baseline --test assembly_convergence --test assembly_diagnostics --test chord_characterization --test initialization_diagnostics --test wheel_geometry --test bogie_fitting --test mechanical_guidance --test rail_normal_guidance --test guidance_robustness --test guidance_continuity --test source_rail_audit -- --nocapture --test-threads=1 > target/m4e-c/characterization.log 2>&1
python tools/verify-train-guidance-m4a.py target/m4e-c/characterization.log
python tools/verify-train-guidance-m4b.py target/m4e-c/characterization.log
python tools/verify-train-guidance-m4c.py target/m4e-c/characterization.log
python tools/verify-train-guidance-m4d.py target/m4e-c/characterization.log
cargo test --release -p qx-geometry --features experimental-graph-source --test graph_extension_qualification upstream_query_cost -- --ignored --nocapture --test-threads=1 > target/m4e-c/cost-serial.log 2>&1
python -m py_compile tools/verify-graph-extension-m4e-c.py
git diff --check
git diff --cached --check
```

Results on the finished Rust implementation:

- Formatting, default workspace check, and strict all-target Clippy with and
  without the experimental feature pass. Formatting and both Clippy configurations
  were rerun after the final degenerate-normal negative was added.
- Default debug workspace: **502 passed, zero failed, 13 ignored**.
- Feature-enabled debug workspace: **514 passed, zero failed, 15 ignored**.
- Native Release geometry with the feature: **108 passed, zero failed, two
  ignored**, including all three new ordinary tests. The new cost test is also
  executed separately. Final ordinary qualification logs use the finished
  Rust helper and all negatives.
- Native Release physics: **83 passed, zero failed, 12 ignored**, across all
  twelve inherited numerical test targets listed above.
- Independent new evidence verification: **621 actual-query pose stencils**,
  69 rolling stencil comparisons to Decimal, independent rigid-exponential
  pose/source-jet controls, and two-spacing/70-and-90-digit restart controls pass
  their evidence/known-defect assertions. This is an executable **failed
  qualification**, not a green adapter acceptance test.
- Retained independent verifiers: eight M4E-B pose and eight offset stencils;
  **38 M4A matrices, 29 M4B matrices, six M4C reaction systems, 18 M4D reaction
  systems and 216 M4D geometry cases** pass.
- Python syntax and all four report links checked. Git staged/unstaged
  whitespace and addition-only scope checks are performed before commit.

The twelve inherited opt-in physics long-run/profiling cases, M4E-B's old query
cost test, and the GPU evidence exporter are **not explicitly executed** in this
milestone. No dynamic run is extended on the unqualified graph source. Workspace
headless CLI/export regressions do run noninteractively; no native GUI or new
application Release package is launched/built for manual verification.

Primary evidence is under this worktree's ignored `target/m4e-c/`: checkpoint
logs, `workspace-default.log`, `workspace-feature.log`, `geometry-release.log`,
`extension-release.log`, `fixture-release.log`, `independent.log`,
`independent-summary.json`, `graph-release.log`, `characterization.log`, the
`m4a`/`m4b`/`m4c`/`m4d`/`m4eb-independent.log` files, final format/Clippy logs and
`cost-serial.log`. The actual-fixture probe's generated manifest/source are
ignored too. Logs, JSON evidence and build products are not committed.

Changed files relative to `77c49c8` are exactly this report and the four test/tool
files listed in section 3:

1. `crates/qx-geometry/tests/graph_extension_qualification.rs`
2. `crates/qx-geometry/tests/support/m4e_c_extension.rs`
3. `tools/verify-graph-extension-fixtures-m4e-c.ps1`
4. `tools/verify-graph-extension-m4e-c.py`
5. `docs/train-articulation-m4e-c-source-b1-adapter.md`

All **497 inherited tracked files** are unchanged. No prior assertion/report,
production implementation or dependency is edited.

No owner-approved UX behavior changes, no development-only UI is added, and no
new UX decision is finalized. Manual rendering, playback and interaction are
**not verified**; no GUI was launched. There is no new UI command to exercise.
An optional owner-only regression remains `cargo run -p quantum-x --release --
edit`, open an existing train project, Inspect/Initialize, Play/Pause/Reset, then
save/reopen. It does not demonstrate a new source B1 adapter.

The verified qualification milestone is committed locally on
`codex/train-articulation-m4e-c`, directly above the requested checkpoint. Nothing
is pushed, merged or submitted as a PR; earlier worktrees and unrelated edits
remain in place.
