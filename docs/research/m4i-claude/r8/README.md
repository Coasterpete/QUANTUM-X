# M4H R8 tick-563 investigation (scratch, outside the repository)

Includes the unchanged M4H module tree read-only by #[path] (same modules as
crates/qx-physics/tests/source_guidance_m4h.rs): helix fixture
(fixtures()[2]: initial roll 0.6, Arc R8 plane 0, roll rate 0.04 rad/m, 12 m),
shared-bogie mechanism lead 8.025 m (11 bodies, 41 rows), 6 m/s, 1/240 s,
4 internal steps, 32 sweeps, beta 0.2, gates p < 2e-5, v < 0.003.
Cargo.lock copied from the worktree.

    cd <this directory>
    cargo build --release
    ./target/release/qx-r8-replay.exe out/r8.jsonl            # ~90 s
    ./target/release/qx-r8-replay.exe out/perturb.jsonl perturb
    python -I analyze_r8.py out/r8.jsonl
    python -I classify.py out/r8.jsonl out/perturb.jsonl

r8.jsonl: per-tick per-substep per-row c, gamma, sweep residual, R=J(x1)v1;
candidate decomposition (Taylor, converged dense solve); projection/bearing
regularity; local replay of tick 563 at 16/64; global runs at 4/16/64 to 600.
perturb.jsonl: negative control, body 8 displaced 1e-5 m (+Y) before tick 301.
