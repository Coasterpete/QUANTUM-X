# QUANTUM-X

QUANTUM-X is an independent, native roller coaster design and simulation project
in Rust 2024. Its **graph/timeline-first authoring engine** now combines force-vector
and geometric sections with cubic Bezier, B-spline, NURBS, and Catmull-Rom sections.
It includes f64 evaluation/derivatives, arc-length sampling, transported frames,
transactional editing/undo/redo, versioned projects, numerical exports, and a native
wgpu 3D inspector and an **interactive development authoring workspace**. The
workspace edits actual section/transition graphs, previews human edits in 3D,
and calculates distinct rider-reference geometry and design-speed loads.
Final production UX and fully validated vehicle/contact dynamics remain future work.

The M4 workbench adds Track/Train profiles on one document, saved resizable/tabbed
layouts, internal floating authoring panes, configurable logical window sizing and
UI scale, and adaptive dotted transition plots. Support Settings and Support
Appearance stay in the right Support Workspace; support construction and train
geometry/dynamics were unavailable in M4; M8.1/M8.2 add the experimental train
capabilities described below. Ordinary section creation presents coaster
concepts, with spline creation in an explicit advanced disclosure.

M5 builds on that workbench with a desktop command shell and Windows Open/Save As
dialogs, synchronized section cards and timeline, explicit before/after insertion,
contextual properties, direct transition value/boundary gestures, graph zoom/pan,
curated Lucide/Carbon icons and bundled Overpass Mono throughout the Editor.
The [M5 correction report](docs/editor-m5-correction.md) records graph interaction
fixes, view rules, regression coverage and offscreen comparisons.
See [the M5 implementation report and manual Windows checklist](docs/editor-m5.md).

M6 adds predecessor-aware insertion, explicit draft resolution at blocked section
actions, stable numeric-control identities, contextual transition editing, linked
graph/viewport inspection, and camera fitting against the actual visible bounds.
The straight-to-force-vector-hill workflow has a reproducible opt-in validation
fixture. See [the M6 report and exact manual Windows scenario](docs/editor-m6.md)
for automated results, captures, assumptions, and native checks still required.

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
cargo run -p quantum-x --release -- authoring-scenario target/m6/scenario
python tools/verify-m6-exports.py target/m6/scenario
```

The original five-section foundation and new spline-comparison, mixed-authoring,
and analytic-reference examples all save/reopen with identical regeneration.
Version-1 through version-4 projects migrate explicitly to Schema 5; their fixtures
are preserved. Heartline configuration, train configuration association and authored
reference intent persist without changing the existing section equations.
Exports include centerline/measurement/transition/reference-point CSV, spline and
section continuity/error diagnostics, OBJ centerlines, SVG plots, and a summary.
Reference design speed does not represent a physically simulated train.

In `edit`, select/insert/reorder sections in the Track Workspace or bottom timeline.
Select a force/rate channel, drag square endpoints, or enter precise numeric
parameters. A clean graph drag commits one undo step; a gesture alongside existing
numeric drafts stays staged. Numeric drafts preview automatically; Apply publishes
them and Cancel restores document values. Diamonds adjust neighboring transition
durations. Ctrl+wheel zooms the graph, middle drag pans, and F fits the hovered plot
or viewport. Ctrl+S saves (prompting for untitled projects), Ctrl+Shift+S opens Save
As, Ctrl+O opens a native picker, Ctrl+Z/Y undo/redo, and Ctrl+Enter applies a section.
Rider settings and complementary spline controls are in collapsible development
panels. Generation errors retain the previous view and identify its revision.
`render-editor` produces an actual offscreen wgpu/egui PNG without desktop control.

The temporary inspector supports orbit/pan/zoom, perspective and orthographic
views, grid/axes, section selection, orientation/curvature overlays, and reload.
`H` prints controls to the invoking terminal; `F` fits, `Tab` selects, `N`/`K`
toggle diagnostics, `1`..`4` change views, and `R` reloads. Drop another project
onto the window to open it. These debug controls are not the final editor UX.

See [the interactive authoring and heartline guide](docs/authoring-pass-3.md),
[the M4 workbench guide and exact Windows checklist](docs/workbench-m4.md),
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


### M7 procedural track construction

The editor now builds solid procedural track downstream of the mathematical
solver. Track Workspace → Coaster offers clearly labeled generic dual-rail and
single-rail validation configurations with profile, hardware and material
settings. View → Track display switches solid, centerline, technical and mesh-edge
presentations. Valid graph edits regenerate meshes on the background worker;
Apply/Cancel, undo/redo and save/reopen retain the authored geometry contract.
The named Hybrid/Monorail catalog concepts remain unfinished production styles.

```powershell
cargo run --release -p quantum-x -- track-scenario target/m7-review/final
python tools/verify-m7-exports.py target/m7-review/final
```

See [the M7 architecture, evidence, limitations and Windows checklist](docs/m7-procedural-track-engine.md).

### M8.2 vehicle architecture and playback

Train Workspace now shares persisted multi-car definitions, static GLB assets,
kinematic inspection and experimental rigid-body playback. Components have
independent mass centers and mounting frames; passenger assemblies can rotate
through physical hinges, limits and bounded torque motors. Fixed and ball joints
use the same solver. Car/bogie/rail counts and passenger placement are not fixed
to a conventional train. Schema 5 extensions preserve existing definitions.

Summary offers Initialize, Play/Pause, single-step and Reset with 240 Hz physics
on a worker thread. Preview renders the resulting body poses. Configuration's
explicit development fixtures support variable car counts with conventional,
below-track or rotating-seat arrangements. Visual model changes stay separate
from mechanics. Mechanical 4D profiles have distinct stored definitions; their
transmission dynamics are future work and playback rejects them explicitly.

```powershell
cargo run --release -p quantum-x -- train-playback-scenario target/m82-verification
python tools/verify-m82-exports.py target/m82-verification
cargo run --release -p quantum-x -- edit target/m82-verification/rotating-seats/playback.qx.json
```

See [M8.2 architecture, configuration support, verification and manual checks](docs/m82-universal-vehicle-playback.md)
and [M8/M8.1 foundation and asset pipeline](docs/m8-rigid-body-foundation.md).
The controls/assets remain development presentation. Centerline guides are an
idealized captive constraint, not wheel/rail contact or family-specific validation.
