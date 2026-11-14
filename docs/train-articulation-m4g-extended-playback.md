# M4G — extended source-guided dynamics and diagnostic playback

Native Windows/MSVC, 2026-11-13. Experimental integration tests and standalone
developer evidence only. No production simulator, schema, editor or train assets
are changed. No push, merge or PR is authorized.

## Stage 0 — preserved baseline

Read repository `AGENTS.md`, the M4F report, and relevant M4D/M4E-A/B/C research.
The clean M4F checkpoint is `87d561b38ece1e1e2ee8603417640501d2d9f0ef`;
its three stages are `751865a` (continuous source geometry), `6a541ca`
(experimental B1 coupling), and `87d561b` (actual-pose offline viewer).
Those commits, report, original viewer and evidence remain intact.

Created the managed worktree
`C:/Users/coast/.codex/worktrees/train-articulation-m4g/QUANTUM-X` directly at
that checkpoint, then branch `codex/train-articulation-m4g`. All previous
worktrees and the original checkout's README/.vscode/development-tooling edits
are preserved. Explicit task instructions supersede the repository's automatic
publication workflow: local stage commits only.

Before implementation, native Release `graph_dense_extension` passed all four
tests. The focused physics baseline (`source_guidance`, `mechanical_guidance`,
`rail_normal_guidance`, `guidance_robustness`, `guidance_continuity`,
`source_rail_audit`) passed 45 tests, with six opt-in cases ignored.
The unchanged M4F independent dense fixture/derivative and source reaction tools,
and M4A/B/C/D independent matrix/reaction verifiers passed. Baseline logs are
under ignored `target/m4g/baseline-*`; no inherited gate or equation was edited.
Cargo uses a command-local `CARGO_TARGET_DIR` at the existing M4E-B build cache;
the older source/evidence is untouched. Omit that variable for an independent
new build cache.

## Stage A — qualification in progress

Qualification is required before extending the viewer. Initial duration probes
use the exact original 12 m R8/.04 rad/m twist source, initial bank .6 rad,
lead 8.025 m, shared middle bogie, 6 m/s and gravity (0,-9.81,0) m/s².
A second independent case uses the unchanged foundation project's first two
sections, physical interval [0,72] m, lead 11.625 m, traversing the compatible
12 m source join. No source length, control, banking, mass, bearing or support
equation is changed. Outer timestep remains 1/240 s, four internal substeps and
32 impulse sweeps. Full per-tick evidence and independent checking are pending.
