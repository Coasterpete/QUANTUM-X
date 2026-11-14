# M4I — constraint residual classification and independent validation

Native Windows/MSVC, 2026-11-14. rustc/Cargo 1.99.1, LLVM 23.1.1,
x86_64-pc-windows-msvc, Intel Core i7-14700KF. Offline test infrastructure and
a separate initialization proposal only. No production behavior, UI, schema,
dependency manifest, timestep, B1 default or acceptance-policy change.

## Baseline and preserved research

Read AGENTS.md and M4F/M4G/M4H reports. Verified clean M4H at
`c39706b4af8bafd9bec30a51dd2e6f5c32af35d4`. Created the separate managed
`train-articulation-m4i/QUANTUM-X` worktree directly there and branch
`codex/train-articulation-m4i`. M4H and all earlier worktrees/evidence remain.
The original checkout's unrelated README/.vscode/tooling changes remain untouched.
The explicit milestone instruction overrides automatic publication: local commit
only; no push, merge, PR, release or desktop control.

Before implementation, native Release `source_guidance` and
`source_guidance_m4h` passed 10 tests, with eight opt-in experiments ignored.
The original solver, absolute gates and M4H tests are retained byte-for-byte.
All M4I implementation files are additions, behind the existing
`experimental-source-guidance` test feature.

Found Claude's scratch `harness` and `r8` investigations. Preserved 18 files with
SHA-256/size provenance: compact sources, manifests/locks, reports and summaries
in `docs/research/m4i-claude/`, and all four large JSONL outputs in ignored
`target/m4i/claude-evidence/`. Source scratch files were not edited. Archival
`.gitattributes` preserves original line endings and digests. The portable new
tests do not depend on temporary paths or execute the archived harnesses.
Also created and digest-verified the complete 18-file backup outside Cargo's
target tree: `C:/Users/coast/.codex/research/quantum-x/m4i-claude-c39706b.zip`
(13,724,403 bytes). No binary archive is added to Git and no research is uploaded.

Two archival discrepancies are explicit: its R8 README says 41 rows, whereas
the actual 11-body system has 65 rows/66 velocity variables; its classifier
summary drops the final substep. M4I includes the actual failing substep and
does not inherit either discrepancy.

## Independent analytical references

`m4i_analytic.rs` contains closed forms using only authored constants and Vec3
arithmetic. It calls no track evaluator, projection, Jacobian, impulse or solver.
The independent Python tool also constructs its references directly from
constants and raw body states. Matrix/reaction verification is a separate
category that deliberately consumes emitted matrices; it is not advertised as
an independent analytical geometry reference.

Fixtures reproduce the scratch investigation: 120 m straight with pitch -0.1
rad, or 110 m horizontal R20 circle, flat or initially banked 0.6 rad. Circle
plane is pi/2 minus bank. Start station is 5 m; speed is 5 m/s on the incline
and 15 m/s on circles. Mass is 100 kg, isotropic inertia 20 kg m², gravity
(0,-9.81,0), outer dt=1/240 s, 32 sweeps, four/sixteen internal steps.

For the incline, a=g sin(.1), s(t)=5+5t+a t²/2 and v²=25+2a(s-5).
Reaction is m(a tangent - gravity). For circles,
C(s)=(20 sin(s/20),0,20(1-cos(s/20))); reaction is the inward m v²/rho
plus upward mg. Bank changes the normal/lateral decomposition, not gravity or
the horizontal circular locus. A COM point guide leaves attitude free; it does
not imply body yaw. A two-guide carriage has chord 0.6 m, COM radius
rho=sqrt(20²-.3²) and omega_y=-v/rho. The five-support B1 carriage seats at a
slightly different radius: -0.371138 mm flat and -0.317793 mm banked relative
to R20. Its conditional orbit reference uses the measured initial COM radius,
reports that offset, and never substitutes it for authored centerline geometry.

Both production generated-track and B1 dense-source geometry match independent
positions/tangents/normals within 1e-9 at 15 non-node queries. Full native runs
export 24 independent six-second trajectories, 34,584 raw single-body states.
Reference checks include speed versus station, travel versus time, radial and
height error, angular speed, momentum-derived average reaction and SI energy.
The Python verifier unwraps angles explicitly; the Rust reference uses the
known less-than-one-revolution domain. No principal-atan2 jump is called drift.

Representative six-second results (absolute speed errors in m/s):

| Reference / engine | Steps | Max speed error | Final energy drift J | Steady relative reaction error |
| --- | ---: | ---: | ---: | ---: |
| Incline / production COM point | 4 | 2.75586e-4 | -0.299736689 | 1.25e-11 |
| Incline / production COM point | 16 | 6.88972e-5 | -0.074934171 | 5.34e-11 |
| R20 / production COM point | 4 | 2.17436e-5 | -0.032615417 | 2.94436e-4 |
| R20 / production zero-omega carriage | 4 | 7.51963e-3 | -5.656011128 | 8.12335e-4 |
| R20 / production compatible-omega carriage | 4 | 2.17264e-5 | -0.032622758 | 2.94484e-4 |
| R20 / B1 single carrier | 4 | 2.40148e-5 | +0.036102440 | 2.94440e-4 |
| R20 / B1 single carrier | 16 | 1.50092e-6 | +0.002256387 | 7.36052e-5 |
| Banked R20 / B1 single carrier | 16 | 1.50256e-6 | +0.002258788 | 7.36049e-5 |

“Steady” omits the first 24 ticks **only for that reaction statistic**. Every
initial state/step remains exported and checked. At four steps, the R20 point
and compatible carriage have about 212.6 N initial average-reaction error;
the zero-omega carriage has 279.0 N. The latter loses 5.63977 J on the first
outer step, versus 0.01635 J for a compatible carriage. The incline production
energy loss independently matches -m a² h T/2 within 1e-6 J, the exact constant
acceleration kick/advance composition's discrete error, not physical friction.
At four/sixteen steps, R20 path deviations are about 30.5/1.91 micrometres.

Banked B1's final sixteen-step reaction components are 1444.886169 N normal
and 374.601673 N lateral; independent midpoint references give 1444.886339 N
and 374.601921 N. Banked/flat single-body differences agree with the authored
frame rather than a different gravity or arbitrary force multiplier.

These fifteen-m/s B1 four-step trajectories are **ungated reference observations**:
1439/1440 outer steps on each circle exceed an original quality gate. They are
not accepted playback qualifications. Sixteen-step single-carrier runs pass the
unchanged quality gates. No gate is replaced by an analytical comparison bound.

Additional four-second articulated controls export 5,766 states: the actual
three-body production development consist and seven-body B1 one-car assembly
on all three references. Python reconstructs world/body inertia energy from
positions, quaternions and velocities (max discrepancy 1.456e-11 J). No
single-body centripetal formula is imposed on either coupled assembly.

| Case | Production first-step / 4 s drift J | B1 first-step / 4 s drift J |
| --- | ---: | ---: |
| Incline | -0.001710 / -1.598672 | -0.000784 / -1.397633 |
| Flat R20 | -162.328240 / -162.913148 | +0.142925 / +0.248763 |
| Banked R20 | -170.218707 / -170.441936 | +0.144448 / +0.251992 |

The models have different topology/mass/inertia: those absolute energies are
not direct model-equivalence comparisons. B1 circle observations violate its
quality gate at 959/960 steps; production does not use the B1 policy. These
observations expose initialization/integration effects, not qualified trains.

## Diagnostic definitions and the exact tick-563 candidate

Keep beta=.2 and internal h=1/(240 n). Let c0/J0/gamma0 be the actual frozen
start-of-substep gap, Jacobian and solver Jdot*v; v1 is the solved velocity;
c1/J1 are evaluated after the unchanged exponential pose advance. Define:

```text
sweep residual r = J0 v1 + h gamma0 + beta c0/h
stabilization B = -beta c0/h
frozen curvature H = d²/dt² c(exp(t v1) q0) at t=0
curvature change K = h (H - gamma0)
R = J1 v1 = B + r + K + unexplained_R
c1 = (1-beta)c0 + h r - h² gamma0 + h² H/2 + unexplained_c
```

H is measured using the inherited directional-J diagnostic and independently
checked by **gap-only** centered second differences at two widths, 2e-4/1e-4 s.
The independent 2e-5 curvature comparison gate is retained. These Taylor
remainders describe smooth local truncation; they are diagnostic observations,
not substitute acceptance criteria or a proof that every residual is harmless.

For slowly varying curvature and small sweep error, the gap recurrence gives
c approximately -h² gamma/(2 beta), and hence R approximately h gamma/2. Thus
the large stabilization term inherits finite-step curvature gap. Dropping that
term or renaming it a solver failure would miss the coupled recurrence. With
finite sweep r, the steady gap also contains h r/beta; its leading contribution
can cancel r in R. A small gate value does not prove converged impulses.

The exact inherited R8 fixture is unchanged: 12 m R8 arc, .04 rad/m twist,
initial bank .6 rad, shared middle bogie, lead 8.025 m, 11 bodies, 6 m/s.
Every candidate at every refinement is paired by exact state bits, reports and
hints against the original M4D tracking/integrator. Four-step tick 563 rejects;
tick 562 is the last accepted state. The report's maximum gap is
1.648939572663e-5; maximum velocity row is 0.0030034861710434135.
The unchanged gates remain gap <2e-5 and velocity row <.003, including mixed
metre/angular rows as in M4H. Candidate capture is restored transactionally;
rejected diagnostic state is never promoted into the accepted trajectory.

The failing row is support 25, body 8, running/left, internal substep 3:

| Signed contribution | Value |
| --- | ---: |
| Start gap c0, m | +1.584920881490e-5 |
| Solver gamma0, m/s² | -5.793796707773 |
| Stabilization B, m/s | -3.043048092461e-3 |
| Remaining finite-sweep r, m/s | +4.650756242761e-5 |
| Curvature change K, m/s | -6.905786432878e-6 |
| Unexplained velocity remainder, m/s | -3.985457762602e-8 |
| Measured R, m/s | -3.003486171043e-3 |

The four candidate substeps and all rows are exported, not just the failing
row. Max remaining sweep error is 7.80534e-4, on bearing rows; it is real and
substantial even though it does not dominate the row-25 gate crossing.
Max unexplained candidate velocity remainder is 1.69897e-6 across all rows.
The independently solved local system is full row rank; its mass-matrix
condition number is about 191.35. Projection/bearing diagnostics retain the
original |F|<2e-13, D>1e-8, local-window/domain, source/chart identity and .7-rad
bearing-swing checks. No chart loss, endpoint escape or singular projection
explains this candidate; all supports are in chart 0. These are local
regularity checks, not a global projection uniqueness certificate.

## Continuity, injected disturbance and whole-trajectory refinement

Between every pair of internal steps, compare the complete state by bits and
the start gap to the preceding end gap. The clean maximum handoff is
7.950e-17 m; foundation is exactly zero in these runs. This diagnostic is
separate from the within-step Taylor remainder and absolute quality gates.

Reproduced the exact negative in a separate trajectory: after tick 300, displace
body 8 by +1e-5 m in world Y before tick 301. The gap handoff detects the 10 µm
jump and complete-state comparison detects the position change. Continue that
disturbed branch through tick 330 under the original gates. Its sampled local
remainders remain small (gap 9.966e-12, velocity 2.870e-8), demonstrating why a
within-step truncation explanation alone would fail to identify corruption.
The clean trajectory and its source/model remain intact. NaN continuity input
and corrupted decomposition/projection evidence are explicitly rejected.

All refinements start from freshly seated/initialized state at tick zero and
run to the **same** tick 563, using 4, 16 or 64 internal steps throughout.
The four-step R8 endpoint below is the rejected candidate solely for numerical
comparison. No convergence claim is inferred from replaying one refined step
on a coarse accumulated gap (which would magnify beta*c0/h).

| Fixture / steps | Max gap row | Max velocity row | Tick-563 energy drift J | Sampled max unexplained c / R |
| --- | ---: | ---: | ---: | ---: |
| R8 / 4 | 1.64894e-5 | 3.003486e-3 | -29.169941 | 5.899e-10 / 1.699e-6 |
| R8 / 16 | 1.03595e-6 | 7.544600e-4 | -7.323254 | 9.223e-12 / 1.062e-7 |
| R8 / 64 | 6.48305e-8 | 1.888384e-4 | -1.832742 | 1.445e-13 / 6.641e-9 |
| Foundation / 4 | 2.28631e-6 | 3.348022e-4 | +0.582693 | 4.420e-11 / 1.273e-7 |
| Foundation / 16 | 1.42956e-7 | 8.370055e-5 | +0.144079 | 6.907e-13 / 7.957e-9 |
| Foundation / 64 | 8.93570e-9 | 2.092515e-5 | +0.035920 | 1.095e-14 / 4.973e-10 |

Gap/velocity maxima cover the whole attempted trajectory. Unexplained maxima
are explicitly sampled at ticks 1/24/300/480/562/563: all four substeps at n=4,
last substep at n=16/64. They are not asserted to be all-substep maxima at fine
refinement. Their approximate h³/h² behavior is distinct from global error.

Maximum body-position differences between complete 4/16 and 16/64 trajectories
are .003172384754/.000792720540 m on R8 (ratio 4.00190, observed first order)
and 2.362338e-6/1.477374e-7 m on foundation (ratio 15.9901). Foundation is not
forced into the R8 order; cancellation and fixture-dependent leading errors
prevent extrapolating one universal order from these two runs. Neither result
is an exact-trajectory error bound. The complete validation loops, including
paired controls, diagnostics and export, take roughly 3.83/15.39 times the
four-step cost at n=16/64 on R8 and 3.90/16.21 on foundation. These are validation
costs, not isolated solver-latency benchmarks.

## Impulse convergence, reaction estimates, energy and costs

At 13 checkpoints, independently solve A lambda = target - J u with
A=J M^-1 J^T, both a pivoted Rust square solve and NumPy. Check rank, condition,
linear residual, impulse application and the actual original 32-sweep result.
Local 32/128/512/2048-sweep experiments hold J/c/gamma/u fixed, avoiding the
mistake of comparing different nonlinear trajectories as one linear system.

At the failing internal step:

| Sweeps | Max local target error | Relative impulse error against direct solve | Max support force-estimate error N |
| --- | ---: | ---: | ---: |
| 32 | 7.80534e-4 | .109980 | 137.43 |
| 128 | 1.53294e-5 | .00243552 | 1.28751 |
| 512 | 7.30e-12 | 1.17e-9 | 5.97e-7 |
| 2048 | 2.82e-15 | 2.85e-11 | 4.14e-8 |

Impulse error above divides by the direct solution norm; relative to the
32-sweep impulse norm it is about .124, matching the scratch investigation.
Row norms use the inherited one-metre characteristic-length convention for
angular variables. Support force estimates use the translational gradient
times lambda/h. Other multiplier rates have N **or Nm** units and are reported
separately; they are not all called forces. An average impulse rate is not an
instantaneous continuous reaction. Instantaneous reactions are separately
solved on each trajectory at explicit checkpoints.

Eight isolated 563-tick attempts use 32/128/512 cold sweeps or 32 warm sweeps.
Warm starting reuses the previous internal-step lambda only on this immutable,
same-topology, same-h fixture; its first step is cold. It is not a supported
editing/cache lifecycle policy. The cold 32 experiment matches the original
tracked states, reports and hints exactly. Every experiment retains the original
quality gates and rolls back on failure. All R8 experiments reject tick 563;
all foundation experiments finish 563. No higher-sweep or warm default is installed.
Warm history is discarded with the stopped experiment; retrying a rejected
warm-start attempt is not a qualified lifecycle in this milestone.

| Fixture / sweeps | Final accepted energy drift J | Max accepted bearing velocity row | Median / P95 / max complete experiment step ms |
| --- | ---: | ---: | ---: |
| R8 / 32 cold | -29.214501 | 5.72392e-4 | 2.3675 / 2.8029 / 5.5528 |
| R8 / 128 cold | -29.202915 | 5.73121e-4 | 2.6915 / 3.7510 / 7.7447 |
| R8 / 512 cold | -29.202835 | 5.73122e-4 | 3.9986 / 5.0150 / 9.1565 |
| R8 / 32 warm | -29.202840 | 5.73121e-4 | 2.3639 / 2.8673 / 9.5378 |
| Foundation / 32 cold | +0.582693 | 2.18311e-4 | 2.7508 / 3.6837 / 7.9164 |
| Foundation / 128 cold | +0.579065 | 1.43816e-6 | 3.0716 / 3.7034 / 12.3563 |
| Foundation / 512 cold | +0.579058 | 1.43819e-6 | 4.3781 / 5.4577 / 11.4510 |
| Foundation / 32 warm | +0.579058 | 8.47419e-5 | 2.7637 / 3.8934 / 12.3928 |

Post-advance bearing velocity includes curvature/stabilization; it need not
decrease when the frozen linear solve converges. This explains why R8's
post-advance bearing maximum barely changes while local target error collapses.
512 sweeps is an additional-sweep trajectory observation; only the emitted
local systems have been compared to a direct solution, not every unexported
trajectory impulse. No unconditional all-state convergence certificate is claimed.

Integrated row impulses over the identical accepted horizon differ between
32/512 by relative norm 2.754e-6 on R8 and 2.722e-7 on foundation, despite the
large single-step local errors. Cancellation in an integrated observation is
not evidence that each step converged. Sampled max instantaneous support
multiplier changes are 1942.941327 to 1942.935878 N on R8, and 1066.668126 to
1066.667816 N on foundation. Energy changes of about .012/.004 J between
sweep settings are small compared with the drift remaining on those fixtures.
More sweeps alone do not remove the integrator's finite-step energy behavior.

Timings are a serial Release exploratory pass on a shared desktop, 563 attempted
steps per mode, compilation/preparation/control comparisons/printing outside
the per-step timers. Timers include constraint/J/Jdot queries, sweeps, advance
and continuation/hint update with exact geometry reuse. Separate reaction
diagnostics are outside. P95 uses interpolated percentiles. There is no repeated
order-alternated performance qualification, CPU isolation, production speedup,
deadline or real-time guarantee. Around 512 sweeps costs 1.69x/1.59x the observed
32-sweep median. Warm timing differences are within the scope of shared-machine
noise and are not claimed as a speed improvement.

## Production initialization proposal and next work

The separate [initialization proposal](train-articulation-m4i-initialization-proposal.md)
evaluates velocity-only mass projection using the existing production guide/
rail-roll constraints, validates it with public production residual diagnostics,
and explicitly addresses straight/curved/banked and articulated starts, limits,
motors, affine prescribed velocities, speed semantics, devices, determinism and
transactional failure. It remains unimplemented pending owner authorization.

Actual production R20 starts have about 1.05041 m/s maximum connected-anchor
velocity disagreement and .299985 m/s transverse guide speed; incline starts
are compatible. The single-carriage projection gives 14.992502061933 m/s from
a nominal 15 m/s zero-omega seed. Owner direction is needed on whether initial
speed describes the seed or an identified running-gear speed after construction.
The first-step train energy drop includes the coupled integration/stabilization
response, so it is not assigned wholly to one missing angular-velocity term.

Recommended next actions: separately authorize production initialization with
explicit speed semantics; investigate residual-based convergence/warm-start
lifecycle with all-group checks and bounded references; compare an alternative
discrete constraint/integration treatment against these controls before proposing
any acceptance-policy replacement. Keep continuity/projection/bearing checks
separate from truncation accounting. Wheel contact, friction, suspension,
collision, all-source qualification and release train dynamics remain outside
this milestone; no ride-safety or certification claim follows.

## Verification, reproduction and UX/Git status

Native formatting, workspace check and strict all-target Clippy (default and
experimental feature) pass. Completed native suites:

| Suite | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| Default full debug workspace | 502 | 0 | 13 |
| Experimental full debug workspace | 532 | 0 | 27 |
| Release experimental geometry | 112 | 0 | 2 |
| Release experimental physics | 149 | 0 | 24 |

M4I's explicit serial Release run passes all eight
tests (four ordinarily ignored experiments explicitly included) in 140.73 s.
The independent verifier passes 24 analytical trajectories, six articulated
controls, 75 decompositions and 13 directly solved local systems, plus deliberate
corrupt-evidence negatives. Python compilation and staged/unstaged whitespace
checks pass. All 27 added files are isolated additions; every pre-existing
tracked file remains unchanged, including original tests, source, manifests,
reports, viewers and UX documents. All 18 preserved research digests match,
including byte-preserving staged archival inputs and the separate ZIP backup.

Fresh execution of the baseline-built M4H Release test binary repeats its
480-tick energy/refinement experiment and bounded continuation twice. Unchanged
verifiers pass 3,868 raw energy reconstructions, 42 sparse-J/impulse/reaction
checkpoints, 1,282 accepted long-run states, five late reaction systems and exact
replay. R8 retains its 562-tick accepted prefix and tick-563 rejection;
foundation reaches 720 ticks with drift +1.389018775 J. These checks use the
unchanged binary built from this worktree's verified M4H baseline, not an
edited scratch solver. Other inherited opt-in profiling/GPU/exporter experiments
are not newly claimed as executed.

Fresh Release geometry/physics logs also pass the unchanged independent M4F
Decimal checks (585 queries in 117 cells, 48 adjacent-limit pairs, both stencil
spacings and 70/90-digit rolling controls), 42 physical datum/normal cases and
12 source reaction systems. The unchanged M4A/M4B SVD checks pass 38/29 matrices;
M4C passes six reaction systems; M4D passes 216 geometry cases and 18 reaction
systems. No derivative, rank, reaction or residual threshold is widened.

Final evidence: `validation.log`/`.summary.json`, `independent.log`,
`check-results.txt`, `workspace-*.log`, `release-*.log`, `clippy-*.log`,
`retained-m4h-{energy,long,replay}.log`, their independent logs/summary JSON,
`retained-dense-fixture.log`, and `independent-{dense,source,m4a,m4b,m4c,m4d}.log`.
Initial failed new-harness runs remain separate evidence. Their fixes correct
angle branch handling, exclude free point-body yaw from carriage references,
and avoid imposing R8's observed order on foundation. No inherited test or
physics threshold was changed to make those harness assumptions pass.

Run from this worktree; ordinary fresh Cargo targets also work. This session
uses a command-local CARGO_TARGET_DIR at the existing M4E-B build cache, while
new evidence is retained under this worktree's ignored `target/m4i/`.

```powershell
tools/run-constraint-validation-m4i.ps1
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features qx-physics/experimental-source-guidance -- -D warnings
cargo test --workspace
cargo test --workspace --features qx-physics/experimental-source-guidance
cargo test --release -p qx-geometry --features experimental-graph-source
cargo test --release -p qx-physics --features experimental-source-guidance -- --nocapture --test-threads=1
git diff --check
```

For the retained independent tools, capture the Release geometry/physics
commands above with `-- --nocapture --test-threads=1` into the corresponding
`target/m4i/release-geometry.log` / `release-physics.log`, then run:

```powershell
tools/verify-graph-dense-fixtures-m4f.ps1 > target/m4i/retained-dense-fixture.log 2>&1
python tools/verify-graph-dense-m4f.py target/m4i/release-geometry.log target/m4i/retained-dense-fixture.log
python tools/verify-source-guidance-m4f.py target/m4i/release-physics.log
foreach ($stage in @('a','b','c','d')) {
    python "tools/verify-train-guidance-m4$stage.py" target/m4i/release-physics.log
}
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance_m4h energy::energy_refinement_evidence -- --ignored --nocapture --test-threads=1 > target/m4i/retained-m4h-energy.log 2>&1
python tools/verify-energy-m4h.py target/m4i/retained-m4h-energy.log
cargo test --release -p qx-physics --features experimental-source-guidance --test source_guidance_m4h long::bounded_duration_and_independent_checkpoints -- --ignored --nocapture --test-threads=1 > target/m4i/retained-m4h-long.log 2>&1
# Repeat the preceding command into retained-m4h-replay.log, then:
python tools/verify-long-m4h.py target/m4i/retained-m4h-long.log target/m4i/retained-m4h-replay.log
```

New modules: analytical closed forms, production/B1 reference harness,
articulated raw-state controls, residual classification, isolated convergence
experiments and an independent NumPy verifier. Eight native tests include four
ordinary regressions and four explicit long/measurement runs. New tools have
no authority over production acceptance. No new UI or temporary developer UI
is introduced. Existing M4G developer viewer remains unchanged; no new
owner-facing UX design is implied. Manual native app/GPU/desktop checks, live
frame rates and packaging are not performed. If desired, the owner can run
`cargo run -p quantum-x --release -- edit` and check initialize/play/reset on an
existing project; no changed UI requires a new manual workflow here.

Local branch `codex/train-articulation-m4i`; the final local commit contains
this report and verified additions. No push, PR or merge is performed.
