# QUANTUM-X

QUANTUM-X is an independent, native roller coaster design and simulation project
in Rust 2024. Its **graph/timeline-first authoring engine** now combines force-vector
and geometric sections with cubic Bezier, B-spline, NURBS, and Catmull-Rom sections.
It includes f64 evaluation/derivatives, arc-length sampling, transported frames,
transactional editing/undo/redo, versioned projects, numerical exports, and a native
wgpu 3D inspector and an **interactive development authoring workspace**. The
workspace edits actual section/transition graphs, previews human edits in 3D,
and calculates distinct rider-reference geometry and design-speed loads.
Final production UX and rigid-body train simulation remain future work.

From the repository root on Windows with Rust installed:

```powershell
cargo run -p quantum-x -- demo target/foundation-demo
cargo run -p quantum-x -- generate examples/foundation.qx.json target/generated 0.1
cargo run -p quantum-x -- examples target/pass2-examples
cargo run -p quantum-x -- inspect examples/mixed-authoring.qx.json
cargo run -p quantum-x -- edit
cargo run -p quantum-x -- edit examples/mixed-authoring.qx.json
cargo run -p quantum-x -- heartline-demo target/heartline
cargo run -p quantum-x -- render-editor examples/heartline-reference.qx.json target/editor.png
cargo run -p quantum-x -- generate examples/numerical-reference.qx.json target/reference 0.1
cargo run -p quantum-x --release -- benchmark
```

The original five-section foundation and new spline-comparison, mixed-authoring,
and analytic-reference examples all save/reopen with identical regeneration.
Version-1 and version-2 projects migrate explicitly to version 3; their fixtures
are preserved. Heartline configuration, train configuration association and authored
reference intent persist without changing the existing section equations.
Exports include centerline/measurement/transition/reference-point CSV, spline and
section continuity/error diagnostics, OBJ centerlines, SVG plots, and a summary.
Reference design speed does not represent a physically simulated train.

In `edit`, select/insert/reorder sections in the Track Workspace or bottom timeline.
Select a force/rate channel, drag square endpoints, or enter precise numeric
parameters. Graph release commits one undo step. Numeric drafts preview automatically;
Apply publishes them and Cancel restores document values. Ctrl+S saves the entered
path, Ctrl+O opens it, Ctrl+Z/Y undo/redo, and Ctrl+Enter applies a section.
Rider settings and complementary spline controls are in collapsible development
panels. Generation errors retain the previous view and identify its revision.
`render-editor` produces an actual offscreen wgpu/egui PNG without desktop control.

The temporary inspector supports orbit/pan/zoom, perspective and orthographic
views, grid/axes, section selection, orientation/curvature overlays, and reload.
`H` prints controls to the invoking terminal; `F` fits, `Tab` selects, `N`/`K`
toggle diagnostics, `1`..`4` change views, and `R` reloads. Drop another project
onto the window to open it. These debug controls are not the final editor UX.

See [the interactive authoring and heartline guide](docs/authoring-pass-3.md),
[the Pass 3 verification report](docs/authoring-pass-3-report.md),
[the multi-method mathematics and API guide](docs/authoring-pass-2.md),
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
