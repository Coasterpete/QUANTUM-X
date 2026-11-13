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
a separate local commit. No project schema, production train selector, production
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

## UX and desktop status

No owner-facing UX is changed; no temporary UI is introduced in Stage A.
Production playback, authoring and persistence are untouched. No GUI was
controlled, no manual visual result or FPS measurement is claimed. The owner
regression command remains `cargo run -p quantum-x --release -- edit`: open an
existing train project, Inspect/Initialize, Play/Pause/Reset, save/reopen. Those
desktop checks are not manually verified by this initiative.
