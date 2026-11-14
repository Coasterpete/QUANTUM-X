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
