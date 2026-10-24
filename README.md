# QUANTUM-X

QUANTUM-X is an independent, native roller coaster design and simulation project
in Rust 2024. Its first integrated foundation is a **headless graph/timeline
authoring engine**, with force-vector and geometric track generation, persistent
projects, undo/redo commands, and numerical exports. The editor and rigid-body
train simulation remain future work.

From the repository root on Windows with Rust installed:

```powershell
cargo run -p quantum-x -- demo target/foundation-demo
cargo run -p quantum-x -- generate examples/foundation.qx.json target/generated 0.1
```

The five-section example generates a changing 3D trajectory and checks that its
authored JSON survives save/reopen with identical regeneration. Outputs include
centerline, measurement, and transition CSV; a centerline OBJ polyline; SVG
diagnostic plots; and a text summary. These are development diagnostics. The
reference design speed does not represent a physically simulated train.

See [the foundation mathematics and API guide](docs/core-foundation.md),
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
