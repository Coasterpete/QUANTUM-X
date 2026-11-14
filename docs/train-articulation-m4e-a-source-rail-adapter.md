# Train articulation M4E-A — source-derived physical rail audit

Date: 2026-11-08. Native Windows x86_64 MSVC, Rust 1.99.1 / LLVM 23.1.1.

**Result: stop the continuous-adapter implementation path at the upstream
geometry boundary.** The real source retains scalar transition derivatives and
native curve position derivatives through order two, but does not expose a
continuous, banked, physical-station solution with the higher derivatives and
join guarantees B1 needs. Reconstructing one from generated poses would repeat
M4D's sampled problem; implementing another ODE/frame integrator in physics tests
would create a parallel geometry engine. Neither is done.

This milestone delivers an executable audit of actual `generate()` output and
the smallest upstream contract needed next. It does **not** demonstrate a working
continuous source rail adapter, source-driven B1 traversal, or production guidance
integration. All new executable code is in one integration test. Production
generation, mechanics, schema, dependencies, fixtures, tolerances and editor are
unchanged.

## 1. M4D checkpoint and preservation

Read repository `AGENTS.md` and the M1, M2A, M2B, M3A, M3B, M4A, M4B, M4C and
[M4D reports](train-articulation-m4d-continuity-and-tracking.md). The requested
M4D worktree was already clean and locally committed at
**`ca26ce668380d4b91d1ab9d0ace3b6691088a241`**:

`C:/Users/coast/.codex/worktrees/train-articulation-m4d-local/QUANTUM-X`.

Its commit contains exactly these eight additions, 2,344 lines, with no modified
inherited files:

1. `crates/qx-physics/tests/guidance_continuity.rs`
2. `crates/qx-physics/tests/support/m4d_characterization.rs`
3. `crates/qx-physics/tests/support/m4d_dynamics.rs`
4. `crates/qx-physics/tests/support/m4d_reference.rs`
5. `crates/qx-physics/tests/support/m4d_surface.rs`
6. `crates/qx-physics/tests/support/m4d_tracking.rs`
7. `docs/train-articulation-m4d-continuity-and-tracking.md`
8. `tools/verify-train-guidance-m4d.py`

Before creating M4E-A, focused native Release verification passed: 26 ordinary
M4B/M4C/M4D tests (five opt-in tests ignored) and nine physical rail tests.
`git diff --exit-code HEAD`, whitespace and clean-status checks passed. The
existing completed commit is the verified checkpoint; no empty duplicate commit
was manufactured. An additional unchanged M4D evidence run passed all seven
ordinary tests, 216 independent NumPy geometry cases and 18 reaction systems.

Created a managed worktree directly from that SHA, then the local branch
`codex/train-articulation-m4e-a`:

`C:/Users/coast/.codex/worktrees/train-articulation-m4e-a/QUANTUM-X`.

A command-local Schannel fetch succeeded, without changing Git configuration or
disabling certificate validation. No push, merge or PR was performed. The
unrelated tooling checkout at `C:/Dev8/QUANTUM-X` retains its README / `.vscode` /
development-tooling edits. M1, M4C and M4D worktrees remain intact.

## 2. Actual production geometry data flow

| Stage | Actual implementation and retained information | Relevant loss / limitation |
| --- | --- | --- |
| Graph/timeline edits | App graph/region commands operate on `qx-document::Project.initial` and ordered `sections`; stable `SectionId`, channel transition IDs, domains, boundary policies, native curve controls/knots/weights persist. `command::Editor` provides transactional history. | Source is richer than generated poses; do not substitute meshes or graph measurements for it. |
| Scalar authoring functions | `transition::{Transition,Channel}` and profile evaluators return value, first and second input derivatives. Internal `Channel::evaluate_side` and `integration_knots` preserve explicit transition/native-profile boundaries. | Channel side evaluation is crate-private; scalar derivatives are not spatial position/orientation jets. Continuity reports do not prohibit jumps. |
| Graph section solution | `solver::Context::{omega,derivative,step}` implements spatial RK4 for `State {p,q}`. `C_s=R X`, `q_s=(0,k)q/2`. Distance input scales by 1, time by section design speed. Force sections use design speed and gravity, not simulated train motion. World-geometric controls explicitly fail near vertical; rider-local controls remain usable. | `Context` is private. Intermediate RK4 derivatives are discarded when `step` returns only the next pose. There is no retained dense solution or source-jet query at arbitrary station. |
| Graph generated output | `solver::generate` stores endpoint poses in `CenterlineSample`, section ranges, one endpoint angular rate per section, midpoint diagnostic graphs, connections and `smooth_ranges`. | The precise graph loss occurs in `Context::step` returning `State`, followed by `generate` storing only `CenterlineSample`. Neither RK stages nor position/frame derivative fields survive. Graph measurements are differences of adjacent frames, not exact source jets. |
| Native curve source | `curve::{Curve,PreparedCurve,CurveEvaluation}` retain the actual Bézier, B-spline, NURBS or Catmull–Rom representation and evaluate position, first and second native-parameter derivatives, including one-sided knots. | No third/fourth derivative API. `CurveJoin` measures only through C2/G2. `Section::validate` requires G1, not C3/C4, and permits curvature jumps. |
| Native station conversion | `arc_length::ArcLengthTable` integrates speed on native spans using adaptive Simpson, then bounded Newton/bisection for `u(s)`. | Quadrature errors are estimates, not certified bounds. `sample_at_distance` returns derivatives in native `u`, not station `s`; no inversion derivative jet is returned. |
| Spline orientation/bank | `spline_track::append` evaluates exact source positions/tangents at inverted stations, applies two minimal tangent rotations through each midpoint, and Simpson-integrates roll per normalized physical section distance. | Full orientation is a sampled approximation to Bishop transport. Only resulting quaternions are stored. `omega_at` computes endpoint curvature/twist, not a continuous transported frame or k'/k''. Loss occurs at these discrete rotations and final `CenterlineSample` push. |
| Rider / heartline | `Project::reference_geometry` calls `heartline::generate_reference`. Supplied-jet `offset_path` / `track_from_rider` algebra is exact; inverse checks tangent/curvature compatibility and rejects incompatible cases. | `generate_reference` supplies estimated jets from a five-frame, degree-four stencil restricted to `smooth_ranges`. It returns discrete `ReferencePoint`s, not a differentiable source evaluator. `OffsetProfile` exposes only through second derivative. |
| Physical rider inverse | App `session::scene_cached` calls `supports::physical_attachment_track` when authoring a rider reference. That adapter uses derived physical positions/distances and frames for supports, construction and train consumers. | It clones generated metadata, clears graphs/smooth ranges, deduplicates equal physical stations and remaps section ranges. It does not rederive section angular rates per physical station or retain both derivative limits. A raw `GeneratedTrack` has no physical/rider role or revision tag. |
| Procedural construction | `construction::sweep_with_caps` maps local profile points plus T/N/L offsets through sampled poses. Repeated hardware samples the generated path. Segmented construction validates identical running members across variants. | Triangle/profile geometry does not recover source continuity or define a running/side contact law. Profile-origin offsets are geometric datums. |
| Arbitrary distance query | `GeneratedTrack::sample_at_distance` independently evaluates cubic Hermite position from sample positions/frame tangents and shortest quaternion interpolation (small-angle nlerp). | This creates a new piecewise interpolant. C is ordinarily C1 at pose-shared knots, but C''/C''' and frame rates need not match. Frame T need not equal the derivative of the position interpolant between nodes. |
| Physical rail query | `rail_geometry::PhysicalRailGeometry` / `RailReference` / `WheelCenterPath` differentiate that actual interpolant, including nlerp rate, explicit rail and center offsets, endpoints and incoming tangents. | This is a valid sampled-geometry adapter, not a continuous authoring-source adapter. It exposes a first derivative; its pose/topology validation does not certify B1's C3 physical jets. `from_map` checks length, not full source identity. |
| Physics references | `consist`, production `solver::{TrackGuide,TrackAttitude}`, M3A `wheel_geometry` and M3B fitting consume generated physical geometry. `supports::Attachment` resolves stable section/fraction/offset through the same sampler. | Structural support attachments and mechanical B1 support datums are distinct. Production guidance remains its historical model. M4B–M4D B1 uses explicit synthetic datum faces in integration tests. |

`smooth_ranges` mark authoring/channel/native boundaries where diagnostic
stencils must stop. Their name does not certify every generated interpolation
knot, derivatives through order three, or a periodic seam. A finer sampling step
does not supply that missing guarantee.

Precise loss anchors at the preserved checkpoint: [RK4 pose-only return in
solver.rs:267](../crates/qx-geometry/src/solver.rs#L267), [independent generated
interpolation at solver.rs:150](../crates/qx-geometry/src/solver.rs#L150),
[discrete spline transport at spline_track.rs:131](../crates/qx-geometry/src/spline_track.rs#L131),
[pose-only spline output at spline_track.rs:152](../crates/qx-geometry/src/spline_track.rs#L152),
[sample-based frame differentiation at heartline.rs:225](../crates/qx-geometry/src/heartline.rs#L225),
and [physical metadata conversion at supports.rs:9](../crates/qx-geometry/src/supports.rs#L9).

## 3. Physical-geometry contract required by B1

Retain +X forward / +Y up / +Z lateral, SI/f64. Station `s` is **physical
centerline distance**, never offset-rail arc length, normalized input or design
time. A continuous-source evaluator must supply, from one immutable snapshot:

- Source/generation identity; physical-reference role; stable section and native
  span/channel identity; exact finite station domain; sided boundary ownership.
- `C,C',C'',C'''` in world coordinates and full orientation `R`, including bank.
- World angular rate per station `k,k',k''`, with `R'a=k cross Ra`.
- Explicit rail/face identity and local support datum offset `a`. Source running
  profile origin plus explicitly declared guide offset is sufficient for this
  **synthetic B1 chart**, not a claim of actual tread/contact geometry.
- `P,P',P'',P'''` and running/side normals `n,n',n''` from that same geometry.
- Reported derivative provenance (analytic source, integrated approximation with
  declared error, numerical estimate, unavailable), continuity/degeneracy results,
  sided joins and explicit periodic-seam status. Never silently cross a rejected
  boundary or smooth it away.

For constant local offset `r=Ra`, the unchanged M4D algebra gives:

```text
P    = C + r
P'   = C' + k × r
P''  = C'' + k' × r + k × (k × r)
P''' = C''' + k'' × r + 2 k' × (k × r)
       + k × (k' × r) + k × (k × (k × r))

running: w=R Z, b=w × P'       side: w=R Y, b=P' × w
w'=k × w; w''=k' × w + k × (k × w)
running b'=w' × P' + w × P''
running b''=w'' × P' + 2 w' × P'' + w × P'''
(side uses P' × w in the same product rule)
ell=|b|; ell'=b·b'/ell
ell''=(b'·b'+b·b'')/ell - (ell')²/ell
n=b/ell
n'=b'/ell - b ell'/ell²
n''=b''/ell - 2 b' ell'/ell² - b ell''/ell² + 2 b (ell')²/ell³
```

For actual rigid support point `x`, the inherited projection and equations stay:

```text
F=(x-P)·P'=0
D=P'·P'-(x-P)·P'' > 1e-8
ds/dx=P'/D
c=(x-P)·n
g=n+P' ((x-P)·n'-P'·n)/D
J=[g, (Q local_point) × g]
J a + Jdot v = 0
```

Require finite jets, nonzero P'/b, a regular D and bounded, unambiguous local
chart selection. Positive D at isolated samples does not certify an interval or
global projection uniqueness. C3 position **and** full C3 orientation are useful
sufficient local conditions for this chart; C2 centerline alone is insufficient.
Where an arc-length native curve supplies `R X=C'`, C3 orientation generally
needs native position derivatives through **order four** (T''' enters k'').
Independent fits to C and R cannot evade that compatibility requirement.

Native conversion must differentiate `u(s)` consistently: with `v=|C_u|`,
`u_s=1/v`, `u_ss=-v_u/v³`, `u_sss=(3v_u²-v v_uu)/v⁵`. Higher frame derivatives
also need the corresponding higher native derivatives. Merely returning
`CurveEvaluation.first` from an arc-length query does not make it C_s.

For rider authoring, first derive the compatible physical path and full frame
from the source jets. If `f=|dC_physical/ds_authored|`, every physical derivative
uses `d/ds_physical=(1/f)d/ds_authored`, including `k_physical=k_authored/f`.
Variable offsets additionally need a''' and product-rule terms. The current
sample-derived inverse and second-order offset API cannot certify these terms.
No fallback to rider stations or guessed normals is permissible.

## 4. Bounded implementation decision and candidate architecture

No new `Rail`, geometry engine, source integrator or selectable B1 backend is
implemented. The new [source audit](../crates/qx-physics/tests/source_rail_audit.rs)
imports the unchanged M4C sampled evaluator solely to measure actual generated
pieces. Two-sample restrictions preserve exact polynomial/quaternion
coefficients and sided limits, rather than taking epsilon steps across knots.
They do not reconstruct or claim to be the authoring source.

The narrow future design belongs in `qx-geometry`: an immutable prepared-source
snapshot, shared graph RHS/transport implementation, station mapping, chart
registry and sided jets. Its rail-datum adapter should be a small offset/normal
algebra layer. The test B1 surface seam can then consume it for **all**
initialization, residual/J, velocity initialization, reactions, Jdot and dynamics.
Never use source rails for initialization and sampled rails for later forces.
The existing M4B test contract also records branches from the adapter's track
sample boundaries to keep directional Jdot stencils within one chart. A source
adapter must expose its **actual source chart boundaries** there (or adapt this
test-only branch seam), rather than carrying unrelated mesh samples or pretending
that multiple source regions form one global smooth chart.

For graph sections the available scalar value/first/second derivatives can in
principle generate k/k'/k'' by differentiating the existing RHS along its
solution. The missing part is a retained, error-described arbitrary-station
solution with matching derivatives, not an impossibility of the ODE itself.
That work must reuse `Context`, not copy it into a physics test. Native sections
add genuinely missing higher derivative interfaces and continuous bank transport.

## 5. Authored fixtures and measurements

All eleven cases call the real production `generate` at unchanged default options
(maximum 0.1 m step). Source tuples are cloned and verified unchanged; full
generation repeats exactly; output section IDs/order, shared boundary indices
and exact station joins are checked. Cases are numerical development data, not
owner-designed coasters. M1 straight/arc definitions and existing document
foundation/mixed/numerical/spline/heartline fixtures are reused. The new
constant-curvature/twist helix is a bounded authored `Arc` with roll rate, not
M4D's separate analytic `Path::Helix`; an independent axial-advance check verifies
the generated points lie on its expected axial progression.

Maximum one-sided jumps over **all** generated interior knots, both running/side
roles, offsets `(0,0,0)`, `(0,-.08,.6)` and `(.1,-.08,-.6)` m:

| Actual generated case | Samples / sections | P' jump | P'' jump, 1/m | k jump, rad/m | n' jump, 1/m | Minimum measured D |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| M1 straight, 600 m | 6001 / 1 | 6.66e-16 | 3.33e-14 | 0 | 0 | 1 |
| Foundation force hill, first two regions | 721 / 2 | 7.526e-5 | 2.414e-6 | 1.491e-4 | 7.575e-5 | .98440 |
| M1 tight R=12 turn, initial bank .6 | 801 / 1 | 2.413e-15 | 1.760e-5 | 4.093e-15 | 2.624e-6 | .90234 |
| Tight horizontal R=8 turn, zero bank | 601 / 1 | 2.776e-15 | 5.938e-5 | 4.496e-15 | 8.449e-6 | .85539 |
| Same, roll rate 0 to .004 rad/m | 801 / 1 | 3.027e-6 | 1.767e-5 | 5.000e-6 | 5.647e-6 | .90234 |
| R=8, constant .04 rad/m twist helix | 601 / 1 | 2.051e-15 | 3.270e-5 | 6.126e-15 | 3.078e-5 | .99975 |
| Existing .4 rad/m rolling line / offset rail helix | 121 / 1 | 2.917e-15 | .001937 | 4.829e-15 | .001815 | 1 |
| R=12 complete vertical loop / inversion | 755 / 1 | 6.662e-16 | 3.703e-6 | 3.317e-15 | 2.855e-6 | .99983 |
| Foundation, five connected regions | 1521 / 5 | .005273 | .01555 | .01544 | .01399 | .98440 |
| Mixed graph/native, ten connected regions | 3644 / 10 | .06909 | .1587 | .1488 | .1106 | .87577 |
| Native numerical project, four regions | 1117 / 4 | .006403 | .05020 | .05000 | .05000 | .99990 |

Columns are independent maxima, not simultaneous at one knot. Local regularity
is sampled at 37% of each piece with a 2 mm normal residual. Exact joins have
separate emitted maxima in `M4EA_CASE section_jumps`; the full/mixed projects
attain the displayed maxima at section joins. Small position/frame gaps do not
make these derivative jumps acceptable to a generic smooth B1 chart. These
tests observe them; they do not install a new production rejection policy.

At selected interior pieces in every fixture, independent centered position,
first-derivative and normal differences agree with the preserved sampled jets
within 2e-7. Bounded projection returns the known local station within 2e-7 m.
Every sampler/offset position comparison passes 2e-10 m; largest measured error
is 1.365e-13 m. These compare two readings of the **existing sampled geometry**,
not a nonexistent continuous source adapter.

The actual four-family spline project is separately queried through its native
`PreparedCurve` and `ArcLengthTable`, including inherited section anchors:
maximum source/generated node position difference is **3.333e-14 m**; maximum
mid-piece difference is **1.440e-8 m**. The expected between-node difference is
from Hermite interpolation instead of native evaluation (plus numerical arc
inversion). These positional observations certify no frame or higher jet.

Its original Bézier join and three Catmull–Rom joins are G1 but not G2;
curvature-vector gaps are .09973, .08788, .04654 and .06543 /m. Cubic B-spline
joins at native u=1/3 and 2/3 pass existing C2/G2 checks but independent one-sided
differences of native second derivatives estimate third-derivative gaps
**382.08** and **1433.61** in native-parameter units. They are numerical estimates,
not analytic station-derivative certificates. The quadratic rational quarter
circle has no internal native knot here; that does not supply missing derivatives
or guarantee future rational-curve joins.

Two additional actual generated, two-region straight-roll cases are accepted by
`RequireContinuousRates`:

| Source rate join at 5 m | P' gap | P'' gap, 1/m | P''' gap, 1/m² |
| --- | ---: | ---: | ---: |
| Linear .004s into constant .02 | 0 | .002421239352 | .0001452743611 |
| Quadratic .0008s² into linear .02+.008(s-5) | 0 | 8.674e-19 | .0009684957408 |

These limits use exact public scalar transition derivatives and the shared
boundary frame, with the unchanged M4D offset algebra. Independent position-only
one-sided differences of integrated scalar roll polynomials verify P'' within
2e-7 and P''' within 2e-6. They are **boundary counterexamples**, not a general
source evaluator. The original abrupt internal roll transition warns, and a
strict incompatible next-region rate is rejected without source changes.

Open endpoints accept s=0/L, reject outside/nonfinite stations and provide only
their valid one-sided geometry. The existing R=20 heartline loop passes pose
closure; its actual sampled seam has P'' gap **4.554e-10 /m** at a .55 m rail
offset. This is a finite sampled seam observation, not proof of periodic third
jets. A deliberately injected .5 mm endpoint gap still passes existing topology
closure but is rejected as `Corner` by the preserved stricter experimental rail.
Neither seam is repaired or used to claim source-driven circuit dynamics.

M4D's **20.425 m sampled changing-bank corner remains unchanged**. Both the
focused checkpoint tests and the final regression run retain acceptance of 23
ticks followed by transactional `Corner`, independent position minimization,
and the separate smooth-reference control. This milestone does not claim that
the actual M1 changing-bank arc has that same source bank law or failure station.

## 6. Physical conversion and source revision/invalidation

Running profile origins are selected by actual construction ID: default
`rail-right` has offset `(0,0,.55)` m. A separate explicit `(0,-.08,0)` guide offset
is retained. Neither profile radius nor mesh normals determine B1's support face.

The existing heartline loop authored as a rider reference converts source length
**125.663706143592 m** to physical length **138.230076757839 m** (radius 20 to
approximately 22 m). Section identity is preserved. However,
`physical_attachment_track` clears `smooth_ranges` and keeps the original
section endpoint rate of .05 rad/source-m rather than the physical .04545...
rad/m. The audit explicitly detects that cloned-rate mismatch; it must not be
used as a physical jet. The physical sampler/rail offset position is verified
against the derived physical track. This narrows the blocker to station jets and
their provenance rather than claiming no physical conversion exists.

Existing app protections are useful but are not embedded in a rail reference:

- `Session::queue_generation` increments a session revision for changed source
  jobs; `poll` rejects old worker results. `scene_cached` compares initial state,
  sections and options for centerline reuse and heartline/construction separately.
- Playback's private `Source` snapshots train mechanics, initial state, sections,
  options, operations/topology and heartline. `sync` discards changed sources;
  `discard` advances a worker epoch; stale worker snapshots/actions are rejected.
  It deliberately excludes visual asset changes. It does not key B1 face/rail
  caches to construction geometry, because that guidance is not implemented.
- `GeneratedTrack`, rail samples and M4C/M4D station hints contain no source
  generation identity. Equal length, sample count or pointer value is insufficient.
  The audit replaces a section ID and regenerates **identical pose samples** with
  different section identities. Geometry-only equality would miss replacement.

Proposed transient identity (no project schema change): a document-instance
identity plus monotonic generation token bound to immutable initial/section/
channel/native geometry, physical reference/offset settings, generation options,
layout mode/seam policy, explicit rail/face offsets and catalog/construction
revision. Separate mechanical/support IDs and a seek/reset epoch key station
hints. Use exact snapshot comparison or a defined content digest plus generation
identity; a naked process-local numeric revision can collide across documents.

| Change | Required invalidation |
| --- | --- |
| Graph/channel or banking edit, undo/redo | Retire affected source generation and its jets/charts/hints; regenerate from the exact accepted source, including draft identity if previews are ever supported. |
| Region length/domain/reference speed or native control/knot edit | Invalidate station mapping and downstream anchors/frames as well as the changed region. |
| Section deletion/replacement/reordering | Reject missing/stale section/span IDs even if geometry or length happens to match. |
| Regeneration/options change | Bind the new derived evaluator to a new generation; do not reuse coefficients or hints from a preceding output just because station extents match. |
| Rider offset/reference-role change | Recompute the physical map and all station derivatives; reject rider-station caches. |
| Running datum/face offset change | Retire physical datum jets and support projections; rendering-only material changes need not change physical geometry identity. |
| Circuit/Open/Shuttle change or seam change | Invalidate wrapping, lap/chart ownership, valid station domains and all continuation hints. |
| Seek/reset/new initialization | Clear projection state and velocity initialization together; bounded fresh initialization, never remote-root fallback. |

Every query/step should compare its caller token with the snapshot and reject
`StaleSource` **before** applying state, hints or reactions. A failed regeneration
must leave the previous snapshot identifiable as old, even if retained visually.
M4E-A implements no pretend stale-reference API: without a source evaluator,
there is no new adapter cache to invalidate. Current app guards and the
equal-samples/different-ID counterexample are audited; token enforcement remains
an explicit upstream requirement.

## 7. Numerical verification and performance implications

The mechanical equations and research code are imported unchanged. Independent
M4D NumPy checks reproduce 216 geometric cases and 18 reaction systems; maximum
P''' discrepancy is 1.537e-9, n'' 1.122e-12, and reaction discrepancy 1.189e-11
N/N m. These certify **M4D's separate smooth development reference**, not this
source pipeline. Retained matrix/rank, acceleration, residual/J, replay and corner
tests continue to verify the mechanical controls.

The audit's sided sampled derivatives, native comparison, source-boundary
counterexamples and position-only differences give independent evidence of the
specific upstream gaps. They do not estimate missing source jets and feed them
to B1. No new source-adapter matrix or dynamic result is claimed.

A future source snapshot adds preparation/memory cost for source spans, integration
state, transport and station inversion. Queries must reuse prepared curves and
arc tables, and use bounded dense solution access rather than integrating from
station zero on every support query. Projection still needs chart-aware local
root handling and regularity checks. Native jet evaluation/arc inversion and
continuous frame transport require measurement before any real-time claim.
No M4E-A performance benchmark, production FPS result or new application Release
package is claimed. Test durations are not physics-tick timings.

## 8. Smallest upstream changes and recommended M4E-B

1. Add a nonpersisted `qx-geometry` prepared graph-source evaluation interface
   around the **existing** `Context` equations, with sided C/R/k jets through the
   required orders and a retained arbitrary-station solution. State its numerical
   error/continuity behavior, including RK/dense-output boundaries. An endpoint
   derivative query or new export of RK4 samples alone is insufficient.
2. Define explicit source-chart boundaries and continuity validation for k/k'/k''
   and physical P/P'/P''/P''' and n/n'/n''. Keep existing generation behavior and
   authored `AllowRateJump` semantics; classify unsupported intervals for B1.
   Do not redefine `RequireContinuousRates` as a stronger promise without design.
3. For native curves, extend the existing evaluator to sided higher derivatives
   (generally through fourth order for the compatible frame), station-derivative
   conversion and continuously evaluated bank/transport. Validate actual degree,
   knot multiplicities, rational weights and section joins. A cubic simple knot
   being C2 is not enough; do not require all supported native families to be
   falsely certified before a first bounded graph-only evaluator can work.
4. Add typed physical-reference/station provenance and higher offset/inverse
   jets; explicitly restrict initial scope to authored physical centerlines and
   constant declared offsets if that is the defensible subset. Do not silently
   accept raw rider output or cloned source rates as physical jets.
5. Bind immutable snapshots and projection hints to explicit generation, chart,
   physical datum and topology identity; reject stale/singular/discontinuous
   references transactionally. This can remain transient without schema/UI work.

**M4E-B recommendation:** authorize the smallest graph-only upstream source
evaluator/jet and continuity-contract pass first, retaining existing generation
outputs as regression controls. Start with actual straight/arc/rider-local/force
sections and explicit accepted/rejected joins, independent position/frame
derivatives and edit invalidation. Then bind the unchanged test B1 contract to
that evaluator as one coherent research path. Native higher jets/transport and
rider inversion can remain explicitly unsupported until separately verified.
Energy-integration improvements, production B1 installation and wheel contact
remain outside that geometry pass. Do not begin a train physics rewrite to work
around this blocker.

## 9. Verification record

Exact commands and results are recorded below. Evidence is local
under ignored `target/m4e-a/`; checkpoint evidence is under M4D's ignored
`target/m4e-a-checkpoint/`. Command-local Cargo caches reuse the M4D Release and
M4C development target directories without changing their source or old logs.

Before the new worktree, from M4D:

```powershell
cargo test --release -p qx-physics --test guidance_continuity --test guidance_robustness --test rail_normal_guidance
cargo test --release -p qx-geometry --test rail_geometry
cargo test --release -p qx-physics --test guidance_continuity -- --nocapture --test-threads=1 *> target/m4e-a-checkpoint/m4d.log
python tools/verify-train-guidance-m4d.py target/m4e-a-checkpoint/m4d.log
```

From M4E-A (directories created with `New-Item -ItemType Directory -Force`):

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --release -p qx-physics --test source_rail_audit -- --nocapture --test-threads=1 *> target/m4e-a/source-audit.log
cargo test -p qx-physics --test source_rail_audit -- --nocapture --test-threads=1 *> target/m4e-a/source-audit-debug-final.log
cargo test --release -p qx-physics --test articulation_baseline --test assembly_convergence --test assembly_diagnostics --test chord_characterization --test initialization_diagnostics --test wheel_geometry --test bogie_fitting --test mechanical_guidance --test rail_normal_guidance --test guidance_robustness --test guidance_continuity --test source_rail_audit -- --nocapture --test-threads=1 *> target/m4e-a/characterization.log
cargo test --release -p qx-geometry *> target/m4e-a/geometry-release.log
python tools/verify-train-guidance-m4a.py target/m4e-a/characterization.log
python tools/verify-train-guidance-m4b.py target/m4e-a/characterization.log
python tools/verify-train-guidance-m4c.py target/m4e-a/characterization.log
python tools/verify-train-guidance-m4d.py target/m4e-a/characterization.log
git diff --check
git diff --cached --check
```

Final results:

- Format, workspace check and strict all-target workspace Clippy passed.
- Full debug workspace: **502 passed, zero failed, 13 ignored**. All inherited
  M1–M4D regressions passed, including the original sampled corner. The full
  workspace run preceded the audit's final horizontal-turn fixture addition;
  the final four-test audit was separately rerun as described below.
- Native Release physics characterization: **83 passed, zero failed, 12 ignored**,
  including four new audits. All geometry Release tests: **96 passed, zero failed**.
- The four-test audit was additionally rerun in **debug and Release, four passed
  in each**, after adding the zero-bank R=8 horizontal turn to the fixture matrix.
  Strict all-target workspace Clippy and formatting passed again after that
  addition. Full mechanical characterization used unchanged source.
- Independent verification: **38 M4A matrices, 29 M4B matrices, six M4C reaction
  systems, 18 M4D reaction systems and 216 M4D geometric cases passed**.
- All **488** checkpoint tracked files remain unchanged; only the two named
  additions are staged/committed. Whitespace and seven local report links checked.

The twelve opt-in numerical long-run/timing cases and inherited GPU evidence
exporter are **not explicitly executed in this milestone**. Their tests remain
unchanged and ignored by ordinary workspace execution. No timing observation is
borrowed as a new M4E-A result. During checkpoint verification an M4C verifier was
also mistakenly given the M4D-only log and rejected its missing M4C records;
the final combined characterization log supplies the correct records and all
four verifiers pass. This was an evidence-input error, not a mechanical failure.

The new audits initially needed compile corrections for debug array formatting
and use of the public per-transition endpoint evaluator rather than the private
channel sided API. No production visibility or behavior was changed to make
the tests compile. Final commands above use the finished source.

## 10. Scope, UX and Git status

Changed files relative to the M4D checkpoint are this report and
`crates/qx-physics/tests/source_rail_audit.rs`. No earlier report, test, fixture or
production source changes. The M4D eight-file implementation is preserved exactly.
M2B settings, body/bogie schema, 1/240 s timestep, experimental substep policies,
wheel assumptions and prior tolerances are unchanged.

No owner-approved UX behavior is changed, no temporary developer UI is added,
and no new UX/workflow decision is finalized. Future production physical metadata
and diagnostics still need human design. Desktop rendering, interaction, cameras,
DPI and playback are **not manually verified**; no GUI was launched or controlled.
There is no new editor command. An optional owner regression is
`cargo run -p quantum-x --release -- edit`, open an existing interior train
project, Inspect/Initialize, Play/Pause/Reset, save and reopen.

Work is local on `codex/train-articulation-m4e-a`, descended directly from the
verified M4D checkpoint, with the two-file audit committed locally after checks.
Its commit is available with `git rev-parse codex/train-articulation-m4e-a`.
Nothing is pushed,
merged or submitted as a PR. The audit conclusion is complete; the continuous
source-derived rail adapter remains blocked by the documented upstream contract.
