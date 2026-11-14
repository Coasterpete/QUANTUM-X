# QUANTUM-X analytic reference harness (scratch, outside the repository)

Links the worktree crates by path and includes the unchanged B1 research
modules read-only via `#[path]`. Writes no tracked file. Cargo.lock copied from
the worktree so dependency versions match.

Rust emits raw states only; `analyze.py` computes every reference from the
authored fixture parameters (theta, R, bank, s0, v0, m, I, g).

Fixtures (one authored source feeds both engines: `generate` -> production
GeneratedTrack, `ContinuousGraph::prepare` -> B1 source):
- incline: Straight 120 m, initial pitch -0.1 rad, s0=5 m, v0=5 m/s, 6 s
- circle:  Arc R=20 m, plane pi/2 (horizontal), s0=5, v0=15, 6 s
- banked:  initial roll 0.6 rad, Arc R=20 m, plane pi/2-0.6 (horizontal), v0=15, 6 s
Bodies: 100 kg, isotropic 20 kg m^2 (matches B1's hard-coded values), g=9.81,
1/240 s outer step, 32 iterations, 4 or 16 substeps.

Reproduce (Git Bash):

    H=<this directory>
    cd "$H"
    cargo build --release
    ./target/release/qx-reference-harness.exe out/run1.jsonl
    ./target/release/qx-reference-harness.exe out/sweep.jsonl sweep
    python -I analyze.py out/run1.jsonl > out/summary1.json
