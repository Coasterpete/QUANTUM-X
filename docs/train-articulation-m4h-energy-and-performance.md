# M4H — energy consistency and source-guidance performance

Native Windows/MSVC, 2026-11-13; rustc/Cargo 1.99.1, LLVM 23.1.1,
x86_64-pc-windows-msvc, Intel Core i7-14700KF.

Experimental offline research only. No production integration, editor changes,
physical wheel contact, push, merge or PR.

## Stage 0 — preserved M4G

Read AGENTS.md and the complete M4F/M4G reports. Verified clean M4G checkpoint
`9056e78d91d6df99f41440d3f2e1ef14ea722674` and its stage chain:
`8069a5e`, `9dcffa4`, `ce03070`, `a60e03e`, `9056e78`.
Created a separate managed worktree `train-articulation-m4h/QUANTUM-X` at that
checkpoint and branch `codex/train-articulation-m4h`. Earlier worktrees and the
original checkout's README/.vscode/tooling changes are preserved. Fetch succeeded
using Windows Schannel after the default OpenSSL trust-store lookup failed;
certificate verification was never disabled.

Before implementation, native Release focused physics controls passed 47 tests
(eight opt-in tests ignored), including both unchanged 480-tick fixtures, exact
replay, late rollback, derivatives and source-rail audit. Dense-source geometry
passed four tests. The actual foundation fixture probe and independent M4F
high-precision dense-geometry, datum/normal and reaction checks passed.
Evidence is ignored under `target/m4h/baseline-*`. Builds use a command-local
`CARGO_TARGET_DIR` at the existing M4E-B build cache; source and earlier evidence
are unchanged. A fresh ordinary Cargo target directory also works.

## Scope and gates

Keep the unchanged R8/.04 twist and foundation-prefix fixtures, physical source,
100 kg bodies, isotropic 20 kg m² inertia, 32 impulse sweeps, stabilization .2,
outer timestep 1/240 s, and all inherited geometry/mechanical gates. Compare
4/16/64 internal steps as experiments, not a production policy change.
Stage A must explain the measured drift before any integration replacement.
Stage B must preserve the uncached control and exact mechanics before any
duration extension. The M4G viewer and original source/dynamics remain controls.

## UX and Git

No production UX is changed or approved. No new developer UI is needed at this
checkpoint. Future production workflows require human design. No active desktop
control or manual verification is performed. Stage commits remain local only.

## Stage A — discrete energy/work definitions

SI convention: world Y is up, gravity is (0,-9.81,0) m/s², positive work adds
mechanical energy. For these unchanged diagnostic bodies, m=100 kg and
I=20 kg m² times identity, so rotational energy is independent of orientation
and there is no gyroscopic torque. Kt=Σ m|v|²/2, Kr=Σ I|ω|²/2,
U=-Σ m g·x. Seating/initial velocity construction precedes the ledger and is
not claimed to be an applied physical impulse.

For each internal step h, record v0, v1, x0, x1, J, c, gamma=Jdot v and all
accumulated row impulses lambda. The existing method first sets u=v0+h g,
then applies p=Jᵀlambda, v1=u+M⁻¹p and advances x1=x0+h v1. Define:

- Wg,kick = h Σ m g·(v0+u)/2, exactly the kinetic change of the gravity kick.
- Wg,path = Σ m g·(x1-x0) = -ΔU, actual gravitational work along translation.
- Wc = p·(u+v1)/2, exactly the kinetic change of the discrete constraint impulse.
- Δ(K+U) = Wc + Wg,kick - Wg,path. The last difference is the discrete
  gravity/position integration contribution, not another external energy source.

This identity does not assume constraint satisfaction. An independent Python
implementation reconstructs energy/work from raw state, impulses and mass; at
checkpoints it reconstructs p=Jᵀlambda from sparse rows, independently replays
32 PGS sweeps, solves the instantaneous reaction equations with NumPy, and
compares a beta=0 fixed-state counterfactual. Corrupted impulse/work/velocity
and nonfinite evidence are explicitly rejected.

The finite-sweep target is Jv1=-h gamma-beta c/h+r, beta=.2. Therefore
Wc=-h lambda·gamma-beta lambda·c/h+lambda·r-p·M⁻¹p/2. These four additive
terms identify curvature target, positional-stabilization target, finite-sweep
residual and the negative impulse quadratic contribution. The stabilization
term is an exact algebraic target contribution, not a unique causal allocation:
lambda itself depends on beta, and the quadratic term contains interactions.
The fixed-state beta=0 comparison isolates a different local counterfactual;
it is not an unstabilized trajectory or proof that removing stabilization is
valid. No post-position projection occurs, so there is no hidden pose-correction
potential-energy term.

Continuous instantaneous power lambda_force·Jv is also integrated by outer-step
trapezoids for comparison. On the imperfectly constrained recorded trajectory it
need not vanish. This integral is a sampled continuous diagnostic, not the
applied discrete impulse work and not an exact quadrature of it. Every energy
trajectory is paired with the inherited integrator at the same refinement,
including exact states, reports, hints and continuation envelopes. Capture
failure preserves the previously accepted samples as well as mechanical state.

### Stage A results and gate

Native Release: all six 480-tick runs and paired exact controls passed.
Independent Rust state/work assertions cover all 80640 internal substeps, with
capture bounded to one outer step at a time. Python consumes every baseline
internal step plus selected refinement steps: 3840 baseline + 28 refinement
samples. All 2880 outer ledgers are checked; only the 42 declared checkpoints
emit full sparse Jacobians. A complete raw archive of every 16/64-step state
is not claimed.
Independent reconstruction checked 3868 raw internal steps and 42 sparse-J/PGS/reaction checkpoints.
Maximum independently observed per-step energy closure discrepancy is 1.56248278204707e-11 J.

| Fixture / internal steps | Signed drift J | Discrete Wc J | Gravity integration J | Continuous reaction integral J |
| --- | ---: | ---: | ---: | ---: |
| helix-4 | -29.749220966 | -80.342113757 | 50.592892791 | -8.932965685 |
| helix-16 | -7.455227190 | -20.101161493 | 12.645934303 | -2.246161165 |
| helix-64 | -1.864931842 | -5.026272914 | 3.161341072 | -0.561438174 |
| foundation-4 | 0.312287664 | -110.568708343 | 110.880996008 | -0.304199546 |
| foundation-16 | 0.076810497 | -27.643558654 | 27.720369151 | -0.077316869 |
| foundation-64 | 0.019123492 | -6.910976360 | 6.930099853 | -0.019377915 |

The R8 baseline Wc decomposes into +18.056486 J curvature target,
-5.693684 J stabilization target, -3.259401 J finite-sweep residual and
-89.445515 J impulse quadratic. Together with +50.592893 J gravity integration
this explains -29.749221 J. The dominant negative additive term is the impulse
quadratic, substantially offset by gravity and curvature terms; attributing the
whole loss to stabilization or ideal reaction power would be incorrect.
Foundation instead gains energy: its signed drift has the opposite sign.
Both signs persist at 16/64 steps and magnitudes decrease approximately with h
(roughly fourfold for each fourfold refinement), consistent with a first-order
discrete splitting/advance error for these fixtures. This is finite evidence,
not an asymptotic proof or a recommendation to adopt expensive refinement.

R8/4: largest front station at tick 258; accumulated drift there -15.103753675 J, subsequent drift through tick 480 -14.645467291 J.

R8/16: largest front station at tick 258; accumulated drift there -3.784605749 J, subsequent drift through tick 480 -3.670621441 J.

R8/64: largest front station at tick 258; accumulated drift there -0.946693280 J, subsequent drift through tick 480 -0.918238562 J.

The ledger remains closed across reversal; no instantaneous turnaround energy
reset, source change or prescribed velocity is present. Complete per-tick
curves, all additive contributions and fixed-state beta counterfactuals are in
`stage-a-energy.summary.json`. Evidence: `stage-a-energy.log` and
`stage-a-independent.log`. Ordinary capture/rollback smoke and Python corruption
negatives passed. Code review against M4D shows observation/capture additions
only; all integration operations and gates are unchanged.

**Stage A gate: PASS within this scope.** The discrepancy is explained by the
stabilized discrete method, rather than missing external work or a state-energy
accounting error. No integration method replacement or production policy change
is attempted. Stage B may investigate exact geometry reuse independently.


### Baseline state energies and full ledger

All values below are joules on the unchanged four-internal-step, two-second runs.
Potential energy has an arbitrary fixed zero; only its changes are meaningful.

| Component | R8 initial | R8 final | Foundation initial | Foundation final |
| --- | ---: | ---: | ---: | ---: |
| Translational K | 18943.009510322 | 14555.084930338 | 19799.999999611 | 19574.947523196 |
| Rotational K | 68.220061225 | 52.417909495 | 0.000000000 | 0.092451856 |
| Gravitational U | 18030.924138499 | 22404.901649247 | 1805.040000000 | 2030.312312223 |

| Work/contribution | R8 | Foundation |
| --- | ---: | ---: |
| Gravity kick Wg,kick | -4323.384617957 | -114.391316216 |
| Gravity path Wg,path | -4373.977510748 | -225.272312223 |
| Discrete constraint Wc | -80.342113757 | -110.568708343 |
| Curvature target | 18.056486156 | 0.619892842 |
| Stabilization target | -5.693684103 | -1.622983909 |
| Finite-sweep residual | -3.259401191 | 1.320137507 |
| Impulse quadratic | -89.445514620 | -110.885754782 |

For foundation, +110.880996 J gravity integration and -110.568708 J constraint work
leave +.312288 J. It is a small residual of larger cancelling terms. Continuous
reaction work is -.304200 J, which even has the opposite sign from net drift.

## Stage B — exact immutable-snapshot reuse

`m4h_surface.rs` wraps the byte-unchanged `m4f_surface.rs`. With reuse and
profiling off, calls delegate to that original implementation. With either
experiment enabled, the original differential algebra and root search are
retained. No tolerance, bin, Newton iteration, root competition, source jet,
J/Jdot, support or bearing equation changes. Original M4G tests remain separate
and unmodified. The source snapshot is owned privately and exposed read-only
(no DerefMut). A source replacement constructs a new Rail with empty caches;
caches never move across snapshots, even if a caller incorrectly repeats a token.

Exact keys include document/generation, original chart, station f64 bits and
endpoint side; physical datum keys add rail/face names, role and all offset f64
bits. Projection keys also include point, hint and half-window f64 bits and
circuit mode. Chart ownership follows the original sided partition rule. No
rounding, proximity, old-hint substitution or cross-offset reuse is permitted.
Every cache hit has passed query validation; projection hits also revalidate
the source interval. Identity validity is a construction/immutability invariant.
No source edit lifecycle or caller token retirement requirement is waived.
Each complete timed step starts by clearing caches (included in its timer),
with an additional hard 8192-entry cap per map. At capacity, misses simply
recompute. Diagnostic phases start fresh caches and are timed separately.
Failures are not cached; caches are derived state independent of rollback.

Keys/negatives cover source generations, distinct snapshots with an erroneously
repeated token, actual source joins/sides, changed physical offsets, normal roles,
face identity, points, windows/hints, stale windows, nonfinite inputs and invalid
offsets. All successful geometry results and measured mechanical outputs are
compared by f64 bits, including derivatives, poses, velocities, reports, station
hints, envelopes and restored windows. Reaction/acceleration diagnostics also
match by bits. No numerical-equivalence tolerance replaces these comparisons.

### Complete-step timing

Native serial Release: each fixture has one complete 480-tick warm-up followed
by five measured 480-tick runs. Each tick pairs cached and original executions;
order alternates to reduce ordering bias. Preparation, compilation, assertions,
printing and independent diagnostics are outside step timers. Cache clearing and
normal mechanical validation/tracking are inside the cached timer. Timers are
off in both measured adapters; counts remain cheap Cell observations. Shared
Windows desktop observations, no affinity or isolated scheduling; no real-time
claim or hard maximum bound. Percentiles use linear interpolation over sorted
samples. Deadline is the unchanged 4.166667 ms outer interval.

| Fixture / evaluator | Steps | Median ms | P95 ms | P99 ms | Maximum ms | Observed misses |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| helix / original | 2400 | 4.7154 | 7.7713 | 9.4620 | 16.1332 | 1948 |
| helix / cached | 2400 | 2.3936 | 4.1810 | 6.3600 | 10.6395 | 122 |
| foundation / original | 2400 | 6.1210 | 8.6758 | 11.6436 | 24.6478 | 2399 |
| foundation / cached | 2400 | 2.7917 | 4.3419 | 6.8897 | 16.1602 | 128 |

These are same-session paired comparisons against the original evaluator, not
comparisons to historical M4G timings or a viewer FPS result. Individual run
statistics and raw samples are retained in `stage-b-cost.summary.json` and
`stage-b-cost.log`. All 5760 paired steps including warm-up passed bitwise
mechanical comparisons; the measured distribution contains 4800 steps per
engine across the two fixtures.

### Separate attribution and observed bottlenecks

An additional 480-tick run per fixture/mode enables nested timers, separately
pairs with the original, and separately times instantaneous reaction/J/Jdot
extraction. Categories are dense-source evaluation including lookup, physical
datum/normal algebra including lookup but excluding source, root search excluding
nested geometry/interval validation, and interval validation. Other constraint
assembly, PGS, pose advancement and bookkeeping are the remainder of complete
step time, not charged to geometry. Instrumentation overhead is present; these
runs do not replace the uninstrumented complete-step distributions above.

| Fixture / mode | Source calls / hits | Datum calls / hits | Projection calls / hits | Root iterations | Source ms | Datum ms | Root ms | Validation ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| helix / uncached | 2247593 / 0 | 2247593 / 0 | 302400 / 0 | 810023 | 1347.804 | 558.593 | 163.901 | 11.140 |
| helix / cached | 370198 / 0 | 1392983 / 1022785 | 302400 / 115200 | 497573 | 392.472 | 411.397 | 247.102 | 13.036 |
| foundation / uncached | 2614083 / 0 | 2614083 / 0 | 302400 / 0 | 785283 | 2290.538 | 840.064 | 232.676 | 36.952 |
| foundation / cached | 370180 / 71 | 1626306 / 1256126 | 302400 / 115200 | 488706 | 426.893 | 464.012 | 272.229 | 44.127 |

A source miss executes the unchanged fine and coarse cell poses, physical-station
inverses, normalized quaternion jets through order three and refinement diagnostics.
Physical datum/normal evaluation then derives offset P through order three and
normal derivatives through order two, even for root queries needing fewer outputs.
The timers group these complete operations; no claim is made about the individual
instruction-level cost of fine versus coarse inversion or each derivative order.
No lower-order query path is substituted to reduce work.

The observed useful reuse is exact physical differentials and identical bounded
projections, particularly repeated base constraint evaluation for directional
Jdot and substep boundary validation. The standalone dense-source cache has
zero hits in the R8 mechanical run and only 71/370180 queries in foundation
(about .019%): almost all remaining station/offset roots are distinct after
physical differential hits are removed. Its bookkeeping is included in
cached cost; no benefit is claimed for that layer. A future implementation
should measure a simpler cache without it rather than assuming all layers help.
No reactions or Jdot results themselves are cached; only their exact geometry
queries reuse data. Cold diagnostic phases have different root/finite-difference
query patterns and cannot inherit the mechanical cache's hit ratio.

| Fixture / mode | Separate reaction diagnostic median ms | P95 ms | P99 ms | Maximum ms |
| --- | ---: | ---: | ---: | ---: |
| helix / uncached | 8.6641 | 11.6957 | 17.2039 | 26.6579 |
| helix / cached | 4.6055 | 6.7791 | 8.2324 | 11.9022 |
| foundation / uncached | 10.4616 | 14.4848 | 19.5412 | 23.8299 |
| foundation / cached | 5.0120 | 6.0010 | 8.7630 | 12.5954 |

The reaction diagnostic uses the original restored .8 m support windows,
whereas mechanical integration uses the speed-driven continuation windows.
Those larger diagnostic root searches help explain why its query footprint and
cost differ. These are reaction/J/Jdot measurements, not full scene alignment,
serialization, browser rendering or M4G's complete offline exporter costs.

**Stage B gate: PASS for this exact-key prototype and these two fixtures.**
Mechanical and diagnostic outputs remain bitwise identical. Geometry reuse
reduces observed complete-step median cost while observed misses/outliers remain;
this is not production integration or a promise of sustained 240 Hz behavior.

## Stage C — bounded continuation, not unrestricted qualification

Both A/B gates passed before this experiment. Retained the exact source domains,
6 m/s initial speed, four internal steps, 32 sweeps and all acceptance gates.
Attempted at most 720 outer ticks (3 s), comparing every candidate to the original
by bits and retaining the discrete ledger. A gate failure restores poses,
velocities, support hints/windows, envelopes, tick count and the prior recorded
internal-step evidence. A fresh cache begins before each attempt; cached query
results have no authority over accepted state. Populated-cache invalid-step and
doubled-speed negatives also reject transactionally, including repeated failures
and a successful subsequent valid step.

| Fixture | Accepted ticks / duration | Final drift J | Max support gap m | Max bearing anchor gap m | Max bearing swing rad | Max roll residual rad | Max reaction N |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| helix | 562 / 2.341667 s | -29.214501166 | 1.64178258e-05 | 5.19815465e-06 | 0.186451641 | 1.94375957e-06 | 1942.941327 |
| foundation | 720 / 3.000000 s | 1.389018775 | 1.44686609e-06 | 2.92301691e-06 | 0.0224095018 | 1.167013e-07 | 1090.270605 |

R8 candidate tick 563 rejects because maximum velocity residual .003003486171
exceeds the unchanged .003 gate (position residual .000016489396 remains below
.00002). It is **not qualified for three seconds**. The accepted prefix ends at
tick 562; there is no endpoint extension, changed speed or relaxed gate. Both
original and cached engines reject identically. Foundation passes all 720 ticks.

helix accepted support station range [1.535532573353915, 11.836265316360908] m; charts [0]. Maximum accumulated ledger discrepancy 1.37172e-10 J and independent state-energy reconstruction discrepancy 1.45519e-11 J.

foundation accepted support station range [6.430001067765272, 29.92433897455032] m; charts [0, 1]. Maximum accumulated ledger discrepancy 2.85837e-10 J and independent state-energy reconstruction discrepancy 1.09139e-11 J.

All 1282 accepted states pass independent Python body-to-support/bearing
transforms, quaternion, normal/gap, projection F/D/domain, source chart ownership,
velocity-driven station continuation, bearing swing/roll, momentum and energy
checks. Five late reaction systems (R8 ticks 480/562; foundation 480/600/720)
pass independent full-rank NumPy reaction/acceleration and support-force checks.
Finite-difference J and directional Jdot refinements at those same states retain
3e-7 / 1e-6 gates. Two complete exports compare exactly, excluding test harness
boilerplate and unrelated negative messages. No timing data enters this replay.
Evidence: `stage-c-final.log`, `stage-c-replay.log`, `stage-c-independent.log`
and `stage-c-final.summary.json`. Original rolling seating, changing-bank corner,
restart-derivative and other expected negatives remain untouched.

**Stage C result:** PASS for the accepted bounded evidence, with the explicit
R8 continuation rejection above. No full-three-second R8 or general-layout claim.

## Stage D — final verification and kernel phase supplement

A further serial Release experiment reconstructs each actual tracking window,
repeats the dynamics kernel from the same pre-step state, and compares complete
states/reports by bits to both the tracked step and inherited control. All 1920
kernel repetitions pass. Timed components include their nested geometry work;
they exclude outer tracking and diagnostic extraction and must not be substituted
for complete-step distributions. Each number is a phase's individual median,
so column medians are not an additive complete-step median. `stage-d-phases.log`
and the augmented `stage-b-cost.summary.json` retain the full distributions.

| Fixture / mode | Constraint residual/J assembly ms | Directional Jdot v ms | Impulse solve/application ms | Advance/validation ms |
| --- | ---: | ---: | ---: | ---: |
| helix / uncached | 0.592500 | 1.809600 | 0.094500 | 0.631450 |
| helix / cached | 0.338750 | 1.481750 | 0.164050 | 0.724150 |
| foundation / uncached | 1.339950 | 4.176250 | 0.158000 | 1.422000 |
| foundation / cached | 0.381200 | 1.694850 | 0.158550 | 0.806550 |

These supplementary phase-mode runs are single serial passes on a shared
machine. Scheduling/frequency variation also changes observations of the unchanged
PGS phase. Use the repeated, order-alternated complete-step distributions for the
overall speed comparison, rather than inferring a speedup from any one phase.

The directional-curvature phase includes base, plus and minus constraint/J
assemblies; it is a principal consumer of source projection. Cache hits eliminate
repeated base and boundary queries while distinct displaced queries still execute
the original solver. PGS itself is unchanged. No J, curvature, multiplier or
reaction solve is cached. The extra test was present in the feature workspace and
Release builds (ignored there) and explicitly executed afterwards. Strict final
Clippy and formatting are rerun with it present.

| Native check | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| Full default debug workspace | 502 | 0 | 13 |
| Full experimental debug workspace | 528 | 0 | 23 |
| Release experimental geometry | 112 | 0 | 2 |
| Release experimental physics | 145 | 0 | 20 |


Final independent checks pass on the final Release evidence: 585 high-precision
pose queries in 117 cells, 48 adjacent-limit pairs, both derivative spacings and
70/90-digit rolling controls; 42 physical datum/normal cases and 12 M4F reaction
systems; 38 M4A matrices, 29 M4B matrices, six M4C and 18 M4D systems (including
the inherited independent geometry checks). M4H adds its own 3868 raw energy
reconstructions, 42 sparse-J/PGS/reaction checkpoints and five late reaction
systems. No derivative or reaction tolerance is widened.

Fresh execution of the unchanged M4G Release test binary exports both 481-state
clips twice. All 962 states, 12 reaction systems and replay compare correctly.
The unchanged Python exporter packages them and the unchanged Node/Edge headless
QA passes real playback controls, both clips, timestamps, scrubbing, cameras,
filter/isolation, overlays, histories and independent 66-face pose transforms.
One regenerated R8 PNG at 1 s was also visually inspected. The viewer deliberately
uses the preserved M4G `stage-b-cost-final.summary.json` as its original timing
metadata; those values are historical M4G controls, not new M4H measurements.
M4H timings and ledger fields remain separate offline logs/JSON/report tables.
No new viewer field or redesign was needed.

All five M4H opt-in experiments were explicitly executed in Release. Other
inherited ignored experiments remain opt-in except the explicitly executed M4G
export. Formatting, workspace check and strict all-target Clippy (default and
experimental feature) pass again after the final profiling test. Python compile,
Node syntax, staged/unstaged whitespace and preservation checks pass. The final
base comparison contains exactly 11 additions: **every pre-existing tracked file
is byte-unchanged**, including all production sources, original tests, viewers,
reports and dependency manifests. A normalized audit also verifies unchanged
physical differential algebra and bounded-root bodies beneath the hooks.

Final logs under ignored `target/m4h/`: `workspace-default.log`,
`workspace-feature.log`, `release-geometry.log`, `release-physics.log`,
`fmt-final.log`, `check-final.log`, `clippy-*-final.log`,
`stage-d-independent-geometry.log`, `stage-d-verify-*.log`,
`stage-d-phases.log`, `stage-d-performance-summary.log`,
`m4g-trace.log`, `m4g-replay.log`, `m4g-independent.log`, `m4g-export.log`,
`m4g-headless.log` and `preservation-final.log`, alongside the stage evidence.


## Energy interpretation limits

A systematic accumulated sign over two seconds does not mean every step has
that sign. R8 has 476/478/479 negative outer-step energy increments and 4/2/1
positive increments at 4/16/64 internal steps. Foundation has 480 positive
increments at every refinement. In the longer R8 accepted prefix, accumulated
loss partly reverses (-29.214501 J at tick 562 versus -29.749221 J at tick 480).
The discrete ledger explains this too. It is not a model of physical friction
or a universally dissipative method. The large cancelling gravity/impulse terms
must not individually be described as a unique causal defect. The measured net
error belongs to this kick/stabilized-impulse/advance composition; additional
controlled experiments are needed before selecting a replacement integrator.

## Remaining production blockers and recommended M4I

The experimental ideal bilateral datums still allow pulling forces and omit
unilateral wheel/rail contact, suspension, wheel spin, friction, impact, collision
and realistic vehicle masses/inertias. Projection remains a bounded local search,
not global uniqueness certification. Only the original continuous-source subset
is qualified; native curves, rider-reference inversion, periodic seams, arbitrary
sources and live editing/cache retirement are not production-qualified. R8's
velocity-residual continuation limit remains. The integrator still drifts and
refinement is expensive; geometry reuse changes none of these mechanics.

Recommend M4I retain this ledger and original engine as controls, investigate a
bounded alternative discrete constraint/integration treatment on the same
fixtures, and compare its net energy balance, finite-sweep error, stabilization
and turnaround behavior before choosing any policy. Separately measure a smaller
physical-differential/projection cache without the nearly unused source layer;
qualify cache budgets and source-generation replacement under explicit lifecycle
tests. Keep real-time and longer-duration gates independent. No wheel contact,
production promotion or final editor UX should be inferred from these results.

## Reproduction

Run in the M4H worktree. Ordinary fresh Cargo target directories also work.
Generated logs/JSON/HTML/profiles are ignored evidence, not committed assets.
Run repeated timing alone after other build/test processes finish.

```powershell
New-Item -ItemType Directory -Force target/m4h
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance_m4h energy::energy_refinement_evidence -- --ignored --nocapture --test-threads=1 > target/m4h/stage-a-energy.log 2>&1
python tools/verify-energy-m4h.py target/m4h/stage-a-energy.log
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance_m4h reuse::repeated_complete_steps -- --ignored --nocapture --test-threads=1 > target/m4h/stage-b-cost.log 2>&1
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance_m4h reuse::geometry_and_diagnostic_attribution -- --ignored --nocapture --test-threads=1 > target/m4h/stage-b-profile.log 2>&1
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance_m4h reuse::jacobian_curvature_phase_attribution -- --ignored --nocapture --test-threads=1 > target/m4h/stage-d-phases.log 2>&1
python tools/summarize-performance-m4h.py target/m4h/stage-b-cost.log target/m4h/stage-b-profile.log target/m4h/stage-d-phases.log
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance_m4h long:: -- --include-ignored --nocapture --test-threads=1 > target/m4h/stage-c-final.log 2>&1
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance_m4h long::bounded_duration -- --ignored --nocapture --test-threads=1 > target/m4h/stage-c-replay.log 2>&1
python tools/verify-long-m4h.py target/m4h/stage-c-final.log target/m4h/stage-c-replay.log
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features qx-physics/experimental-source-guidance -- -D warnings
cargo test --workspace
cargo test --workspace --features qx-physics/experimental-source-guidance
cargo test --release -p qx-geometry --features experimental-graph-source
cargo test --release -p qx-physics --features experimental-source-guidance -- --nocapture --test-threads=1

git diff --check
git diff --cached --check
```

New modules are integration-test support only, behind the existing experimental
feature. No new dependency, application feature, document schema, physical model,
production source, asset or UI is introduced. `m4h_energy` defines/reconstructs
the ledger; `m4h_dynamics`/`m4h_tracking` add transactional observation;
`m4h_surface` is the immutable exact-key adapter; `m4h_reuse` and `m4h_long`
exercise measurements, controls and rejection. Python verifiers independently
reconstruct energy, geometry transforms and matrix/reaction results.

## UX, manual checks and local checkpoints

Owner-authorized behavior is the offline energy/performance investigation and
preservation of M4G. No owner-facing UX or new temporary developer UI is added.
The existing M4G viewer remains provisional developer UI; its implementation is
unchanged. Production workflows/presentation still require human design.

No active desktop was controlled. Native app interaction, high-DPI behavior,
live FPS/240 Hz guarantees, release packaging and production B1/contact integration
are **not verified or claimed**. Owner-executable manual checks: open
`target/m4h/source-guidance-m4g.html`; scrub both original two-second clips,
inspect turnaround/shared bogie, play/pause/reset, vary speed and camera, select/
isolate bodies and toggle overlays. The unchanged native regression command is
`cargo run -p quantum-x --release -- edit`; initialize/play/pause/reset an existing
project and save/reopen. These manual checks were not performed here.

Local reviewed stage commits: Stage 0 `e74ba08`, A `f3102b2`, B `8b40e84`,
C `6f515d7`; the final validation commit contains this completed report and the
kernel-phase supplement. Branch `codex/train-articulation-m4h`, in the separate
managed worktree created at M4G. Existing M4G/earlier worktrees and original
README/.vscode/tooling edits are preserved. No push, PR, merge, release or
production integration is performed. Generated evidence is intentionally ignored.
