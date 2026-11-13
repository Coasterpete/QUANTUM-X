# M4F — source geometry to experimental train guidance

2026-11-13, native Windows/MSVC. Local research, opt-in
`qx-geometry/experimental-graph-source`; no default feature changes.

## Checkpoints and scope

M4E-C is preserved at `fbe5c6613920a2a1592c79dc79c713125ee66d13`, also named
`checkpoint/m4e-c-source-qualification`. Its clean worktree and counterexamples
are unchanged. M4F starts directly there in the managed worktree
`C:/Users/coast/.codex/worktrees/train-articulation-m4f/QUANTUM-X`, branch
`codex/train-articulation-m4f`. The unrelated README/tooling edits in the original
checkout are preserved. Command-local Schannel fetch succeeded after the default
OpenSSL fetch failed certificate verification. No certificate policy/configuration
was changed. No push, merge or PR is authorized or performed.

Stages are gated: A requires one continuous differential reference and independent
derivative/source-convergence evidence; B requires unchanged mechanical gates;
C requires B and an isolated development visualization. Each completed stage is
a separate local commit (A: `751865a`, B: `6a541ca`; C is the final checkpoint).
No project schema, production train selector, production
playback or editor UX change belongs to this initiative.

## Stage A — numerical representation

Implementation: `solver/prepared/dense.rs`, alongside the unchanged restart
`PreparedGraph`. `ContinuousGraph` first obtains the same two RK4 checkpoint
grids through `PreparedGraph`; it uses the existing `Context::omega_jet` and
shared source rate equations. It introduces no graph ODE in physics and changes
neither `Context::derivative` nor `Context::step` nor production generation.

An independent position/frame Hermite fit was considered but not adopted: its
interior position tangent would not necessarily equal the orientation tangent.
Instead, each cell has a septic quaternion polynomial Q(t), t in [0,1], matching
the shared source quaternion and its first three nominal-station derivatives at
both endpoints. For nominal cell length h, Hamilton conjugate Q*, and pure X:

```text
C(t) = C(0) + h integral_0^t vec(Q(v) X Q*(v)) dv
s(t) = s(0) + h integral_0^t |Q(v)|² dv
R(t) = rotation(Q(t) / |Q(t)|)
```

Both primitives have degree at most 15, so their evaluation uses polynomial
antiderivatives, without variable-endpoint RK4 or quadrature. A conservative
power-coefficient ball bounds |Q| away from zero throughout every retained cell;
preparation fails if the bound cannot certify |Q| > .5. Thus s_t > 0, and a
48-iteration safeguarded Newton inverse evaluates at physical station. The
inverse residual is scaled to local h, not total world distance. Existing source
validation, sample and rotation budgets still apply.

The identity `C_s = vec(Q X Q*) / |Q|² = R X` holds throughout this mathematical
representation, rather than only at its endpoints. Orientation derivatives
include quaternion normalization **and** the inverse station chain rule.
`Evaluation` reports derivatives of this representation, marked
`NumericalDenseExtension`, including k/k'/k''. It performs no query-time RK4.
Position jets follow that same hodograph; constant-offset jets reuse the existing
algebra. They are not source RHS values substituted for dense derivatives.

At a shared node, the quaternion Hermite data through order three are shared.
The speed and its derivatives are algebraic functions of those same data, so
inverse reparameterization preserves C3 orientation there. Integrating its
tangent gives C4 position in real arithmetic. Source joins require equal sided
source angular jets through k'' at the common integrated pose; genuine source
jumps remain sided. The older `Boundary.compatible` 1e-9 classification is not
used as a continuity guarantee: even a 5e-10 roll-rate jump is rejected by dense
interval validation. Internal RK4 checkpoints are never rejected or used as
stencil barriers.

Physical positions and stations are accumulated from the new hodograph. They
are deliberately **not pinned** to independent RK4 positions or nominal authored
station lengths. The original chart, section ID and input remain queryable, with
an explicit numerical nominal-input/physical-station mapping. On the default
fixtures the observed total length differences are at most 1.528e-13 m for the
coarsest rolling control and 1.422e-14 m on the foundation prefix. These are
measurements, not certified global approximation bounds or document edits.

### Independent evidence and limits

`graph_dense_extension.rs` exports actual polynomial coefficients, physical
domains, start positions, poses and supplied derivatives. The new Python tool
reconstructs polynomial primitives with Decimal, solves its own scalar inverse,
and differentiates **positions and rotated axes only**, using exact-rational
nine-point weights. It reuses no Rust angular/inverse derivative formula and no
source ODE. Two station spacings (1e-5, 5e-6 m), 70 digits and separate 90-digit
rolling controls distinguish stencil error from implementation error.

The matrix covers straight, banked R12 turn, twisted R8 arc, retained .4 rad/m
rolling line, varying rider-local and force controls, and the **actual unchanged**
foundation [0,72] m prefix including its 12/42 m source joins. Steps .2/.1/.05 m
are requested; fine grids are .1/.05/.025 m. These are development fixtures,
not user-authored coaster layouts or ride-quality claims.

585 query poses in 117 emitted cells and 48 independently reconstructed adjacent
limit pairs pass. Maximum observed spacing-refinement discrepancy is 4.862e-27.
Maximum adjacent pose gap is 5.303e-15; first/second/third derivative vector
component gaps are 1.577e-16 / 4.302e-17 / 3.423e-17. These finite coefficient/
prefix arithmetic residuals are reported, not asserted to be identically zero.
Continuity is the endpoint-Hermite method's real-arithmetic property, evaluated
to finite precision; a literal rounded f64 query is not a differentiable real
function. No interval rounding certificate is claimed.

The former rolling restart is traversable without a boundary guard. Its
high-precision, actual-coefficient offset acceleration diagnostic at 6 m/s has
errors 4.154e-8 / 1.662e-7 / 6.647e-7 m/s² for widths 6e-5 / 3e-5 / 1.5e-5 m,
below the inherited 2e-5 acceleration comparison gate. The growth at very small
widths reveals coefficient/prefix roundoff; it is **not** claimed to converge to
zero indefinitely. Raw f64 second differences reach 2.653e-5 at the smallest
width. They cannot resolve that limit; the independent Decimal reconstruction
is the resolved measurement. No physics reaction or constraint is inferred from
this datum-only traversal diagnostic.

The retained M4E-C tool still reproduces its original h^4/h^3/h^2 restart defect
and emits `FAIL differential restart contract`. That is the expected old-method
counterexample, not the result of the new extension. No tolerances or old tests
were edited to make it disappear.

Constant-helix position errors for requested steps 1/.5/.25 m are
3.0404e-8 / 1.9006e-9 / 1.1880e-10 m (approximately fourth order); orientation
errors are 5.7025e-9 / 3.5648e-10 / 2.2282e-11 rad. Varying rider/force cases
also show approximately sixteenfold position refinement and decreasing
first-through-third pose derivative differences. Third orientation derivatives
reach a roughly 2e-9 roundoff/reference floor in that experiment; no higher
asymptotic order or interval error bound is asserted.

The representation still approximates the graph source solution: interpolated
k in a cell need not equal its source RHS exactly. It preserves `C_s=R X` within
its own reference and converges toward the source, which are separate checks.
Native splines, world-geometric controls, rider inverse, periodic seams, arbitrary
datum offsets varying with station, global projection uniqueness and all-track
qualification remain unsupported. Extreme refinement/large coordinates can
amplify coefficient and inverse conditioning errors. All tokens must be retired
on source edits. Retention is linear in both grid sizes, with substantially more
memory per cell than M4E-B; no interactive-performance claim follows.

### Reproduction and regression

Evidence logs are ignored under `target/m4f/`. Builds used a command-local
`CARGO_TARGET_DIR` pointing at the existing M4E-B build cache. Omit that override
for a new cache. The fixture script generates only an ignored standalone package;
it does not enable experimental geometry in application or document manifests.

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features qx-geometry/experimental-graph-source -- -D warnings
cargo test --workspace --features qx-geometry/experimental-graph-source
cargo test --release -p qx-geometry --features experimental-graph-source --test graph_dense_extension -- --nocapture --test-threads=1 > target/m4f/stage-a-targeted.log 2>&1
& tools/verify-graph-dense-fixtures-m4f.ps1 > target/m4f/stage-a-fixture.log 2>&1
python tools/verify-graph-dense-m4f.py target/m4f/stage-a-targeted.log target/m4f/stage-a-fixture.log
cargo test --release -p qx-geometry --features experimental-graph-source --test graph_extension_qualification -- --nocapture --test-threads=1 > target/m4f/retained-m4ec-extension.log 2>&1
& tools/verify-graph-extension-fixtures-m4e-c.ps1 > target/m4f/retained-m4ec-fixture.log 2>&1
python tools/verify-graph-extension-m4e-c.py target/m4f/retained-m4ec-extension.log target/m4f/retained-m4ec-fixture.log
git diff --check
```

Stage A gate: **PASS for the tested graph-source scope**, with the numerical
limits above. Formatting, default workspace check, strict all-target Clippy
with and without the experimental feature, full feature-enabled debug workspace
(518 passed, zero failed, 15 ignored), and all feature-enabled Release geometry
tests (112 passed, zero failed, two ignored) passed. Final source-boundary
lookup/negative additions were also checked by the Release geometry suite and
strict feature Clippy. The retained opt-in long runs/cost tests were not executed.
Stage A is checkpointed in the first M4F commit; no Stage B qualification follows
from geometry tests alone.

## Stage B — narrowly scoped B1 coupling

The opt-in physics feature `experimental-source-guidance` enables only research
integration tests; it installs no production backend. `source_guidance.rs`
imports M4B's contract, initialization, mass/inertia and reaction equations and
M4D's refined dynamics/transactional tracking **unchanged**. The new
`support/m4f_surface.rs` supplies a physical-centerline snapshot, explicit constant
local rail/face datum, and the running/side normal algebra from M4E-A. It uses
P through order three and normal through order two from **the same** dense
reference. Normals are constructed perpendicular to the actual offset tangent,
never guessed from a mesh, manufacturing profile or nominal frame alone.

Initial body seeds are mapped from the declared mechanical topology/local
dimensions onto the source for every body. The shared nonlinear seating solve,
velocity projection, residuals, Jacobians, directional Jdot, acceleration,
reactions, continuation and dynamics all receive that same `Rail`. No synthetic
path pose is left as a physics reference. The adapter's two `GeneratedTrack`
samples are endpoint metadata for the inherited branch guard only: the whole
snapshot must first pass equal-sided-source-jet interval validation. No sample
interpolator is queried. Original source chart IDs remain in the dense source;
equal-sided-jet source joins inside this connected differential interval
are traversable, as are all internal numerical checkpoints.

Projection retains the M4D bounded 0.025 m-bin search, 48 Newton iterations per
bracket, at most 128 bins, rejection of competing local minima, `|F| < 2e-13`
and `D > 1e-8`. It is not global projection/root certification. New physical
normal and offset degeneracies fail; stale source identities fail. This adapter
requires one qualified open connected source interval. It offers no periodic
seam, rider inverse, unsupported source kind or contact fallback.

### Qualification and explicit rejection

Required positive cases are the retained straight, banked arc, twisted arc,
varying rider-local and force fixtures, plus the actual foundation prefix. Each
uses both one-car/two-bogie and two-car/shared-middle-bogie assemblies (7 or 11
rigid bodies). Speed is 6 m/s, fixed outer timestep 1/240 s, four internal
substeps, existing 32 impulse sweeps. There are 24 ticks / .1 simulated second
per run, with exact deterministic replay. This is short-run qualification of
ideal bilateral datums, not a general vehicle/contact simulation.

The high-twist .4 rad/m rolling line is a geometry-positive/mechanism-negative
stress case, excluded from the positive seating scope before testing. Its source
and all numerical checkpoints remain admitted by Stage A. With the unchanged
mechanism dimensions, seeds, .2 correction bound and .7 rad bearing envelope,
the bounded seating solve returns **NoConvergence**. It creates no simulation
state and leaves the model untouched. Roughly .96 rad of source roll across a
2.4 m car explains why this is a severe mechanism test, but the bounded failure
is not a proof of global infeasibility. No layout, twist, bearing limit or solver
tolerance was changed to admit it. This configuration is **not mechanically
qualified** and is not a visualization candidate.

The positive mechanical matrix passes unchanged gates at both seated and
.002-scaled displaced poses. Observed maxima:

| Independent quantity | Maximum | Unchanged gate |
| --- | ---: | ---: |
| Residual finite-difference J columns | 1.770e-9 | 3e-7 |
| Directional Jdot*v stencil refinement | 4.094e-10 | 1e-6 |
| Independent second differences of c | 2.557e-6 | 2e-5 |
| Independent reaction solve discrepancy | 1.052e-11 | 1e-7 |
| Acceleration equation residual | 1.324e-13 | 1e-7 |
| Ideal constraint power | 6.105e-12 W | 1e-7 W |

Initialization preserves position rows <2e-11 and Jv <1e-9. NumPy independently
checks all 12 reaction matrices, full row rank at 1e-5/1e-7/1e-9, mass-weighted
acceleration, and reaction power. Decimal independently differentiates 42
datum/normal cases from source polynomial **poses**: an inner position-only
stencil obtains P', then normal normalization and outer stencils obtain n'/n''.
No Rust normal/jet formula is reused. All observed geometry errors are below
2.447e-15 (position) and 3.620e-16 (normal and derivative components).

Across the twelve .1 s runs, maximum position and velocity rows are 1.272e-5 and
.002261, below the inherited 2e-5 / .003 quality envelope. Maximum absolute
energy drift is 1.267 J; the largest drift/initial-kinetic ratio is 7.002e-5.
Energy is measured, **not conserved exactly** by this stabilized impulse method.
Invalid internal-step configuration triggers a late transactional failure after
candidate windows are prepared: poses, velocities, hints, windows, envelopes and
tick count all remain unchanged. Existing M4B–M4D rejection/replay regressions
also remain executable. Separate runs carry a support from 11.805 m through the
actual 12 m foundation source join to 12.405 m, for both topologies.

Paired replay timings in one serial Release observation were 69–163 ms for 24
ticks (both replays together). These are shared-machine observations, exclude
rendering and preparation, and do not establish a worst-case 240 Hz or 60 FPS
budget. Query evaluation still includes coarse/fine comparison and higher jets;
no performance optimization is inferred from correctness.

Reproduction:

```powershell
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance -- --nocapture --test-threads=1 > target/m4f/stage-b-targeted.log 2>&1
python tools/verify-source-guidance-m4f.py target/m4f/stage-b-targeted.log
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets --features qx-physics/experimental-source-guidance -- -D warnings
cargo test --workspace --features qx-physics/experimental-source-guidance
```

Retained native Release M4A-M4E-A regression: 40 passed, zero failed, five
ignored. The unchanged independent verifiers pass 38 M4A matrices, 29 M4B
matrices, six M4C systems, 18 M4D systems and 216 M4D geometry cases.

Stage B gate: **PASS within the positive scope above**. Full feature-enabled
debug workspace: 523 passed, zero failed, 15 ignored. Formatting, default
workspace check, and strict all-target Clippy both with and without the research
feature passed. The five new Release tests and both independent tools passed.
Default application feature-tree inspection confirms experimental geometry and
source guidance are not enabled. Stage B is checkpointed in the second M4F
commit. No failed positive mechanical gate has been waived; the rolling seating
negative remains an explicit limitation.

## Stage C — isolated development visualization

The integration can remain entirely outside the app: an ignored opt-in test
exports actual `Tracked.state.poses` and physical alignment measurements, then
a standard-library Python tool validates the log and builds a self-contained
offline HTML/SVG viewer. No server, third-party JavaScript, new runtime dependency,
production render path, document field or B1 selector is added. This is a
development trace format, not a project format or persisted authoring command.

Files: `support/m4f_visualization.rs`, `render-source-guidance-m4f.py`,
`source-guidance-m4f-viewer.html` and `verify-source-viewer-m4f.mjs`. The source
adapter/mechanical equations from Stage B remain unchanged. Export uses its
qualified R8/.04-twist shared-bogie fixture, gravity, 6 m/s initialization and
24 fixed 1/240 s outer steps. No source-following pose animation is substituted
for the dynamics. A second export reproduces the scene and all 25 state records
**exactly**, excluding the intentionally variable timing observations.

Each frame includes all 11 actual body positions/orientations, 30 support points
and their projected physical datums/normals, bearing anchor pairs, row residuals,
velocity residuals, energy drift, support-tangent angle and body angle relative
to its nearest physical centerline frame. Rendering transforms the primitive
boxes with those **actual** quaternions. Two cars share one middle bogie identity;
the exporter does not duplicate the carrier. Source datum lines are sampled for
display only. They never feed the physical reference, initialization or dynamics.

The viewer offers frame selection, previous/next, discrete slow replay, camera
yaw/elevation/reset, body IDs, normal visibility and keyboard commands. Running
and side normal segments also differ by solid/dashed pattern. It reports the
different units of position/angular constraint rows. Body/source and support/
datum angles are observations, not extra constraints, contact-fit results or
certification. Magnitudes are displayed numerically; micrometre residuals are
not silently exaggerated in the geometry.

### Actual usability and performance observations

The first view fit the full source and foreshortened the assembly too strongly
to inspect articulation. That was observed in a headless screenshot and fixed:
the camera now fits the entire recorded assembly, uses a clearer initial angle,
and gives cars transparent primitive geometry so the bogies/anchors remain
readable. The inspected 1500x1200 screenshot shows both cars, all three bogies
including the shared middle one, carriers and bearing connections. Camera fit
uses all recorded states, so frame selection does not resize the assembly.

This remains a small offline inspector. The clip spans **only .100 s**, replayed
24 times slower with no pose interpolation. It demonstrates source coupling and
local articulation, not a complete ride traversal. The camera is orthographic;
there is no live authoring, picking, arbitrary project import, train asset
renderer, contact visualization, perspective camera or editor integration. Body
dimensions are explicitly provisional stand-ins. Source lines can leave the
camera view; extreme camera angles can overlap labels. General display sizes,
high-DPI behavior and owner interaction have not been manually qualified.

In one native Release export observation, the slowest physics step took 3.156 ms,
the slowest alignment/record-emission pass 2.561 ms, and the slowest combined
step/diagnostic export 5.642 ms. The last exceeds the 4.167 ms timestep budget
for this observation. This is why no live 240 Hz diagnostic or interactive
performance claim is made. These are a short fixture on a shared machine, not
worst-case bounds; rendering/preparation are excluded. HTML is about 309 KB.
Display FPS, GPU throughput and integrated application latency are **unmeasured**.

### Reproduction and verification

```powershell
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance visualization::export_actual_shared_bogie_poses -- --ignored --nocapture --test-threads=1 > target/m4f/stage-c-trace.log 2>&1
python tools/render-source-guidance-m4f.py target/m4f/stage-c-trace.log
node tools/verify-source-viewer-m4f.mjs target/m4f/source-guidance-m4f.html
```

The Python renderer uses only the standard library and opens no window. The
optional Node QA tool uses an installed Edge/Chromium executable (a second
argument can provide its path), two isolated profiles under ignored `target/`,
`--headless`, and hidden process creation. It never attaches to an active browser
or the owner's desktop. It checks the real rendered controls, including timed
replay advancement, keyboard commands, final-state replay reset, timeline,
camera/reset and normal visibility. It verifies 11 body rows and 66 body faces,
then exports the PNG. The original QA face count accidentally included a string
inside the script as well as SVG elements; restricting that check to the actual
SVG fixed the verifier. No renderer/physics gate was waived.

Artifacts, all ignored: `target/m4f/source-guidance-m4f.html` (open manually),
`source-guidance-m4f.json`, `source-guidance-m4f.png`, `stage-c-viewer-dom.html`,
trace/replay and QA logs. Static screenshot inspection and real headless-browser
interaction checks passed. Node syntax checking passed before headless execution;
the final HTML is additionally parsed/executed by Edge. No active desktop or
native application interaction was performed.

Stage C gate: **PASS as an isolated development trace viewer**. Final full
feature-enabled debug workspace: 523 passed, zero failed, 16 ignored (the new
export is ignored by the ordinary suite and was explicitly executed twice in
Release). Formatting, default workspace check, strict all-target Clippy with
and without the research feature, trace validation, exact replay comparison,
Python/Node syntax checks and final real headless-browser QA all passed.
The third M4F commit checkpoints this stage and report. No native application
Release package, manual desktop check, long-run train qualification, live frame
rate or additional ignored profiling run is claimed.

## UX and desktop status

No owner-facing UX is changed. Stages A/B introduce no UI. Stage C implements the
owner-authorized development visualization scope in a clearly provisional,
separate trace viewer; its layout is not an approved production UX design.
Production playback, authoring and persistence are untouched. Future production
guidance presentation/workflows still require human design. No active GUI was
controlled, no manual native-app result or FPS measurement is claimed. The owner
regression command remains `cargo run -p quantum-x --release -- edit`: open an
existing train project, Inspect/Initialize, Play/Pause/Reset, save/reopen. Those
desktop checks are not manually verified by this initiative.
