# Train articulation M4C — guidance robustness

Date: 2026-11-08. Native Windows x86_64 MSVC, Rust 1.99.1.

M4C strengthens the **isolated experimental B1 bilateral guide**. It does not
install B1 in production or implement wheel contact. The changing-bank failure
is reproduced and independently identified as a nonsmooth distance minimum.
Piece-aware bounded projection, explicit transactional station continuation,
cached interpolation coefficients and sparse impulse rows are implemented in
test-only modules. The previous M4B implementation remains an executable control.

## Accepted integration baseline and housekeeping

Fetched `origin/main`: `59bcba882a31aea8f97613d7bd76945f20dbf6e8`, the merge of
[PR #12](https://github.com/Coasterpete/QUANTUM-X/pull/12). Its complete tree is
identical to `6ff56bfef7e369c4bcc654290f4cba7a0b12613e`. No inherited commit was
reset, reverted, rebased, cherry-picked or reapplied. A command-local
`http.sslBackend=schannel` resolved the initial OpenSSL issuer-certificate failure;
certificate verification was never disabled and repository Git settings were not
changed.

PR #12 now has a factual post-merge scope clarification preceding its original
description. The accepted baseline includes QUANTUM Dynamics M1.0–M1.3, including
editor/rendering changes, diagnostic import, assets and native schema 7, as well
as train-articulation M1–M4B. The earlier description's narrower no-schema/no-UX
claims are explicitly qualified as applying to the articulation subset.

[Draft PR #11](https://github.com/Coasterpete/QUANTUM-X/pull/11) was closed as
superseded with an explanatory comment. Its head
`bcc5adda8d08426c53135d5ea0a6039bd852107c` is an ancestor of fetched mainline;
`origin/main..bcc5adda` contains zero commits. Its M1.3 content is fully inherited.
It was not merged, and its remote branch remains at that SHA.

Created a separate managed worktree:
`C:/Users/coast/.codex/worktrees/train-articulation-m4c/QUANTUM-X`, branch
`codex/train-articulation-m4c`, directly from fetched mainline. Before editing,
the checkout was clean, `cargo check --workspace` passed, all physics/document
integration tests passed, and the unchanged M4B Release suite passed 12 tests
with one timing test ignored. That timing test was separately executed and
passed. The existing tooling checkout and M1 articulation worktree are retained.
All earlier milestone reports remain unchanged, including
[M2B](train-articulation-m2b-convergence.md),
[M4A](train-articulation-m4a-mechanical-guidance.md) and
[M4B](train-articulation-m4b-rail-normal-guidance.md).

## Implementation and unchanged mathematical contract

New files:

- [guidance_robustness.rs](../crates/qx-physics/tests/guidance_robustness.rs):
  independent geometry, derivative, dynamics, reaction, replay and timing checks.
- [m4c_surface.rs](../crates/qx-physics/tests/support/m4c_surface.rs): immutable
  cached Hermite/quaternion piece coefficients, exact one-sided knot jets,
  per-piece safeguarded Newton projection, explicit corner witnesses and counters.
- [m4c_tracking.rs](../crates/qx-physics/tests/support/m4c_tracking.rs): transient
  support station hints, local continuation envelope and transactional commit.
- [m4c_dynamics.rs](../crates/qx-physics/tests/support/m4c_dynamics.rs): sparse
  nonzero row columns and cached effective mass, plus optional phase profiling.
- [verify-train-guidance-m4c.py](../tools/verify-train-guidance-m4c.py): independent
  NumPy SVD and LAPACK reaction/acceleration verification; optional developer tool.

The exact M4B `Model`, residual/Jacobian equations, initializer, finite-difference
`Jdot*v`, dense reaction solver, state/report types and energy function are reused
by module import. A selectable reference rail path executes the unchanged M4B
projection and jets. The original dense impulse step is also executed directly
as a control. No old source, assertion or tolerance is changed.

Coordinates remain +X forward, +Y up, +Z lateral; SI/f64. The same synthetic
seven-body mechanism, 100 kg bodies, isotropic 20 kg m² inertia, five bilateral
normal supports per axle and authored bearings remain. Shared-bogie checks use
11 bodies and six axles. Source geometry and synthetic development fixtures
are never smoothed or reshaped. These are ideal guide charts, not tread contact,
no-slip steering or manufacturer-accurate mechanisms.

Production guidance, schema 7, editor UX, M2B convergence settings, the
`1/240 s` outer timestep, four experimental internal substeps, 32 impulse sweeps,
stabilization and all M4B dynamic acceptance gates are unchanged. No dependency
or runtime API is added. All executable additions are integration-test modules;
the editor and production `World` cannot select them.

## Changing-bank interpolation corner

The inherited bank fixture uses `bank(s)=.02s+.001s²`, with Hermite centerline
interpolation and piecewise shortest quaternion interpolation at 0.025 m sample
spacing. The analytic source is smooth, but its **sampled offset rail datum**
has different one-sided derivatives at interpolation knots. M4C uses that actual
interpolation, rather than substituting the source analytic curve.

Both the unchanged M4B control and M4C accept 23 outer ticks at 6 m/s and reject
the 24th transactionally with `Corner`. The failing datum is at **20.425 m**:

| Quantity | Measured value |
| --- | ---: |
| Left `F=(x-P) dot P'` | +1.144148147e-7 m |
| Right F | -9.907277964e-8 m |
| `|P'_left-P'_right|` | 9.891869736e-5 |
| `|n_left-n_right|` | 3.680735884e-6 |

Since the derivative of squared distance is `-2F`, distance decreases into the
knot from the left and increases to the right. There is a corner minimum with
no smooth stationary root between these one-sided values. Independent golden
minimization of position distances on the neighboring pieces selects the knot
within 1e-7 m. This is not a mechanical-incompatibility proof or a bisection
tolerance problem. A direct query at 20 m separately measures a 9.60463e-5
derivative jump and 2.77781e-5 normal jump.

The new projector never bisects across that jump. It preserves the corner as a
failing distance candidate and exposes station, support datum, point and
one-sided measurements. State, every hint and accepted tick count survive a
failed step unchanged, including repeated attempts. It adds no impact law,
averaged frame, hidden smoothing or automatic branch retry. Crossing this
genuinely nonsmooth chart still needs an explicitly defined geometry/model
policy before production integration.

## Bounded projection and branch continuation

Projection divides its bounded window at actual interpolation knots, including
explicit unwrapped circuit seams. Every bracket uses one piece's exact jets;
Newton is safeguarded by the bracket and capped at 48 iterations. Station
residual is below 2e-13 m and the regularity denominator must exceed 1e-8.
Endpoint/window-edge roots, degenerate normals, invalid input and missing local
targets remain failures. Work is capped at 2,048 pieces per query.

Unlike M4B's closest-spatial-minimum choice, competing distinct local minima
are rejected even when their distances differ. This deliberately conservative
policy prevents choosing a different nearby branch just because it is closer.
Tests cover repeated equal-distance laps and near-parallel unequal-distance
laps; a small neighborhood can select each explicit seed separately. Circuit
station queries remain unwrapped in both directions across more than one lap.
An index-driven piece walk avoids floating-point reclassification into the
previous span at a seam.

Initialization retains the existing +/-0.8 m seed neighborhoods. After a valid
initial assembly, the tracker records each actual projected support station and
uses a declared **+/-0.08 m** continuation neighborhood, with at most **0.04 m
station innovation per outer tick**. These research bounds are explicit, not a
universal speed guarantee. Stations select a local guide chart; rigid-body
forces and impulses still determine vehicle motion. Larger seeks/speeds fail
without falling back to a distant root. A 12 m/s straight case specifically
passes the core step but fails the post-step hint gate, proving late rollback.

The candidate rigid state, all new hints and the accepted tick count commit
together only after every support succeeds. Hints are transient and are not
saved to the project. Future seek/reset/track-edit adapters must explicitly
reinitialize them; the immutable test rail avoids stale coefficient caches.

**Limit:** one descending bracket is sampled per interpolation piece. Multiple
roots within a single piece, extremely narrow minima and singular charts are
not certified globally. Distinct roots within the existing 1e-7 m deduplication
scale are not separately resolved. This is improved bounded local enumeration,
not certified interval geometry or general self-crossing-track correctness.

## Numerical and dynamic verification

New ordinary tests cover straight, tight turn, changing bank, helix, vertical
loop and inverted fixtures:

- Cached P/P'/P''/n/n'/angular-rate jets agree with unchanged M4B evaluation
  within 1e-12. Independent position and normal differences at 1e-5/1e-6 m agree
  with derivatives within 2e-8. Position-only, span-by-span golden minimization
  agrees with projected station within 2e-7 m for displaced targets.
- All B1 Jacobian columns at closed and 0.002-scaled displaced configurations
  agree with residual differences within 3e-7. `Jdot*v` step refinement agrees
  within 1e-6; independent second differences of c agree within 2e-5. Difference
  steps are refined to stay in the same actual interpolation pieces.
- Sparse and original dense impulses on the same rail agree within 1e-10 in
  velocity and 1e-6 in each load. Against the completely unchanged projection
  control, eight successive ticks agree within 1e-7 in velocity, 1e-9 m in COM
  positions and 1e-3 in each reaction multiplier. These are tolerance comparisons,
  not claims of bit identity across different root algorithms.
- Independent NumPy SVD finds 41 independent rows / 42 body coordinates in all
  six reaction systems at thresholds 1e-5, 1e-7 and 1e-9. Independent LAPACK loads
  differ from Rust by at most 3.64e-12; acceleration residual is below 1.5e-13.
  The comparison gates are respectively 1e-7 and 1e-7. Ideal reaction power is
  below 1e-7 W. Straight static support sums to +6867 N and inverted support to
  -6867 N; tensile bilateral support remains visible and is not physical contact.
- Every COM's horizontal-turn acceleration is checked against analytic
  centripetal acceleration within 2e-5 m/s², independently of the dense solver.

Two separately initialized runs have exactly equal states, hints, tick counts
and complete reports at every accepted step. Ordinary runs cover +/-3 m/s for
120 ticks (0.5 s) on straight, horizontal turn and inverted guides. Opt-in native
Release runs extend a turn through a complete revolution and a shared-bogie
graph through two seconds:

| Run | Ticks / time | Travel | Max position row | Max velocity row | Max energy drift |
| --- | --- | ---: | ---: | ---: | ---: |
| Horizontal turn, 6 m/s, no gravity | 2100 / 8.75 s | 52.50655 m | 1.226996e-5 | 2.343758e-3 | 0.03924 J |
| Shared-bogie straight, 3 m/s, gravity | 480 / 2 s | 5.999998 m | 2.063520e-6 | 3.348022e-4 | 0.003421 J |

The turn travels farther than `2*pi*8 m` on an **open** sampled reference; this
does not establish full-consist circuit seam traversal. Linear rows use m/m/s;
angular rows use rad/rad/s. Each row has its own physical units even when a
compact log reports a maximum numerical magnitude. Existing 2e-5 position and
0.003 velocity research test envelopes are retained. Energy drift is gated at
1e-5 of initial kinetic energy, with a 5 mJ floor. Maximum external rail/gravity
linear momentum imbalance is below 8.1e-10 N, against a 1e-6 N gate.

## Profiling and scoped optimization

The unchanged baseline timing test reproduced median outer-tick costs of
6.055 ms straight, 11.192 ms changing bank and 11.099 ms helix. Optional phase
instrumentation shows approximately 59% in directional curvature evaluation,
with almost all remaining cost in initial/final contract evaluation. Dense
sequential sweeps take about 0.16 ms; optimizing only sweeps cannot fix the
projection-dominated cost.

Wide-window per-piece enumeration is **slower** than M4B's 64-bin scan: it checks
both sides of every knot and evaluates roughly 104,000 jets over the 400
projections in a tick. It remains the conservative initialization/control path.
Continuation reduces the same work to approximately 11,000–12,500 jets. Cached
piece coefficients avoid repeated Hermite/quaternion setup. Sparse impulse rows
also reduce the sweep phase to about 0.04–0.05 ms while retaining row order and
iteration count. No analytic Hessian or replacement dense reaction solver was
introduced; independent derivative checks remain active.

Final native serial timings on the Intel Core i7-14700KF are below, in ms:
median [minimum, maximum] of 35 individual first-tick samples after one warm-up.
Every sample starts from the same valid 3 m/s state. Track/model construction,
initialization and reset are outside timers; compilation and logging are outside
timers. The complete wrapper includes both transactional copies, final station
projection and hint commit. Core profiling includes four substeps, all contract
evaluations and impulse sweeps, but excludes the wrapper's final hint update.

| Fixture | M4B reference core | Wide piecewise core, dense | Tracked core, sparse | Complete tracked wrapper |
| --- | --- | --- | --- | --- |
| Straight | 6.014 [5.967,6.173] | 8.030 [7.941,8.250] | 1.046 [1.035,1.348] | 1.082 [1.074,1.195] |
| Changing bank | 11.273 [10.966,11.571] | 15.879 [15.525,16.330] | 2.084 [2.060,2.295] | 2.175 [2.148,2.372] |
| Helix | 11.211 [11.024,11.553] | 15.803 [15.456,16.227] | 2.050 [2.009,2.200] | 3.581 [2.104,3.873] |

With local tracking but dense sweeps, core medians are 1.174/2.198/2.154 ms;
thus most of the improvement comes from the narrower explicit search, not the
sparse sweep optimization. The local search supports smaller station changes
than initialization; this is a qualified research operating envelope, not an
equivalent unrestricted replacement for broad search.

The combined ignored-test run overlapped workspace regression testing; its
timing samples are excluded from this table. Final paired profiling was rerun
after all other test/compile jobs from this pass finished. These remain shared
machine observations. The helix wrapper varies from 2.104 to 3.873 ms and has a
larger median than its preceding core phase profile; this run does not isolate
that variation's cause. The measured local ticks fit within 4.167 ms, but no
hard real-time guarantee, long-chain cost, editor FPS or production performance
claim follows. Changing-bank timing measures a valid initial tick, not traversal
through its later corner.

## Commands and verification results

Run from the M4C worktree root:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --release -p qx-physics --test assembly_convergence --test assembly_diagnostics --test chord_characterization --test initialization_diagnostics --test wheel_geometry --test bogie_fitting --test mechanical_guidance --test rail_normal_guidance --test guidance_robustness -- --nocapture --test-threads=1
cargo test --release -p qx-geometry --test rail_geometry
cargo test --release -p qx-physics --test guidance_robustness -- --ignored --nocapture --test-threads=1
cargo test --release -p qx-physics --test guidance_robustness paired_phase_profile -- --ignored --nocapture --test-threads=1
python tools/verify-train-guidance-m4a.py target/m4c/characterization-release.log
python tools/verify-train-guidance-m4b.py target/m4c/characterization-release.log
python tools/verify-train-guidance-m4c.py target/m4c/characterization-release.log
git diff --check
```

Results:

- Formatting, workspace check, strict all-target workspace Clippy and whitespace
  checks pass.
- Full workspace: **491 passed, zero failed, 11 ignored**, including all seven
  new ordinary tests. The ignored cases are ten numerical characterization/timing
  tests and the existing GPU exporter.
- Native Release characterization: **81 passed, zero failed** (72 physics plus
  nine rail-geometry tests). Eight opt-in tests in those targets remain ignored
  by the ordinary characterization command.
- Both new ignored M4C tests were explicitly executed and passed: complete-turn/
  shared-graph replay and paired phase timing. Timing was separately rerun serially
  as described above. The original M4B timing test also passed before editing.
  Seven other inherited ignored numerical timers/characterizations and the GPU
  exporter were not explicitly rerun in this pass.
- Independent verification: **38/38 M4A matrices, 29/29 M4B matrices, 6/6 M4C
  reaction systems** pass their independent NumPy checks.
- All **474 baseline tracked files** remain unchanged. Exactly six files are
  added, with no modification of earlier tests, reports, production code or
  dependencies. The tooling checkout keeps its original modified README and
  untracked tooling files; the old M1 worktree remains clean at `6ff56bf`.
  Report links and all added files' whitespace were checked.

Raw local logs are under ignored `target/m4c/`, including
`baseline-check.log`, `baseline-tests.log`, `workspace-tests.log`,
`characterization-release.log`, `opt-in-final.log`, `profile-serial-final.log`
and the three independent-verification logs. They are not runtime assets.

## Limitations, UX and Git status

No new production-integration blocker was discovered. The reproduced bank
corner remains an explicit limitation of this actual interpolated geometry;
M4C does not claim traversal through it. Full dynamic loop/inversion traversal,
general inertia and mass ratios, active bearing bounds, unilateral contact,
impacts, clearance, steering/friction/wheel spin, suspension, certified root
isolation and production adapters remain outside this pass. Long-run evidence
is for the declared synthetic mechanisms and supported tracks only.

No interactive native application was launched or controlled. The existing
workspace headless CLI/export tests did run noninteractively. Rendering, cameras, DPI,
interactive playback and application frame rate are **not manually verified**.
There is no new editor command. Optional owner regression command:
`cargo run -p quantum-x --release -- edit`; open an existing interior train
project, Inspect/Initialize, Play/Pause/Reset and save/reopen. This is an
owner-executable checklist, not a reported test result.
A full application Release package was not built; native Release execution
covers the numerical test targets.

UX status: no owner-approved UI/interaction changed, no temporary developer UI
introduced, and no new UX decision finalized. Future production mechanical
metadata, contact behavior and diagnostic presentation still require owner
design. Existing approved/provisional controls retain their prior status.

Git status: M4C is a local pass on `codex/train-articulation-m4c`. No M4C push,
merge or PR is authorized or performed. PR #12 metadata and PR #11's superseded
closure are the only requested remote mutations. Previous worktrees, branches,
milestone reports and merged mainline history are preserved.
