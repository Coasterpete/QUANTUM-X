# M4H — energy consistency and source-guidance performance

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
