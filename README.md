# QUANTUM-X

QUANTUM-X is an independent, native roller coaster design and simulation project
in Rust 2024. Its **graph/timeline-first authoring engine** now combines force-vector
and geometric sections with cubic Bezier, B-spline, NURBS, and Catmull-Rom sections.
It includes f64 evaluation/derivatives, arc-length sampling, transported frames,
transactional editing/undo/redo, versioned projects, numerical exports, and a native
wgpu **development-only 3D inspector**. The production editor and rigid-body train
simulation remain future work.

From the repository root on Windows with Rust installed:

```powershell
cargo run -p quantum-x -- demo target/foundation-demo
cargo run -p quantum-x -- generate examples/foundation.qx.json target/generated 0.1
cargo run -p quantum-x -- examples target/pass2-examples
cargo run -p quantum-x -- inspect examples/mixed-authoring.qx.json
cargo run -p quantum-x -- generate examples/numerical-reference.qx.json target/reference 0.1
cargo run -p quantum-x --release -- benchmark
```

The original five-section foundation and new spline-comparison, mixed-authoring,
and analytic-reference examples all save/reopen with identical regeneration.
Version-1 projects migrate explicitly to version 2; the original fixture is
preserved. Exports include centerline/measurement/transition CSV, spline and
section continuity/error diagnostics, OBJ centerlines, SVG plots, and a summary.
Reference design speed does not represent a physically simulated train.

The temporary inspector supports orbit/pan/zoom, perspective and orthographic
views, grid/axes, section selection, orientation/curvature overlays, and reload.
`H` prints controls to the invoking terminal; `F` fits, `Tab` selects, `N`/`K`
toggle diagnostics, `1`..`4` change views, and `R` reloads. Drop another project
onto the window to open it. These debug controls are not the final editor UX.

See [the multi-method mathematics and API guide](docs/authoring-pass-2.md),
[the Pass 2 verification report](docs/authoring-pass-2-report.md),
[the foundation mathematics and API guide](docs/core-foundation.md),
[reference research notes](docs/research/openfveid.md), and
[the pass report](docs/foundation-pass-1-report.md) for conventions, verification,
limitations, and next steps. Human-directed UX specifications remain in `docs/ux/`.

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```
