# Pass 3 implementation and verification report

## Implemented and integrated

- Human-controlled ordered section/timeline editing, all existing section methods,
  selectable authored force/angular-rate graphs, pointer endpoint editing and
  precise numeric transition/function/length/amplitude/derivative controls.
- Explicit transition insertion/removal/reordering, time/distance conversion,
  section insertion/removal/reordering and complementary native spline parameters.
  Draft previews, Apply/Cancel, real document transactions and undo/redo are wired
  into live solver output rather than decorative charts.
- A dedicated f64 heartline/reference-point module with signed T/N/L offsets,
  constant and variable profiles, distinct tangent/curvature/arc length/speed,
  frame angular velocity/acceleration and the complete rotating-point kinematic
  equation including relative-motion terms. Constrained rider-to-track inversion
  validates compatibility and rejects underdetermined/incompatible cases.
- Schema 3 settings, nominal train configuration association, strict migration
  from versions 1/2, preserved old fixtures, unknown-field rejection, recoverable
  save/reopen and undo/redo of reference settings. Visual style remains independent.
- Existing wgpu renderer retained with egui composition, live background generation,
  revision/error feedback, section highlights, projection/camera controls, and
  independent reference/offset/frame/point/curvature/load overlays. Geometry buffers
  reuse storage when possible; device and pipelines persist through edits.
- Headless editor interaction tests, offscreen rendering of the actual workspace,
  reference CSV export, a radius-20/18 m analytic fixture, an independent Python
  verifier and technical/manual-testing documentation.

Main implementation modules are `qx-geometry::heartline`, shared smooth ranges
in `solver`/`spline_track`, `qx-document::editing`/commands/persistence, the app's
`authoring/{session,panels,graph,spline}` and rendering `workspace`/`offscreen`.
No physics or audio behavior was changed. No reference application's source was
copied. Run `cargo run -p quantum-x -- edit` or follow
[the complete guide](authoring-pass-3.md).

Cargo metadata license expressions were inspected for the direct UI/PNG additions
and their font/text dependencies. egui/egui-winit/egui-wgpu and png offer
MIT/Apache-2.0 code terms. Bundled fonts also require OFL-1.1 and Ubuntu-font-1.0;
their exact upstream notices are preserved under `docs/licenses/egui-default-fonts/`.

## Checks executed

Native Windows, Rust/Cargo 1.99.1:

```text
cargo fmt --all -- --check                            PASS
cargo check --workspace                              PASS
cargo clippy --workspace --all-targets -- -D warnings  PASS
cargo test --workspace                               PASS (68 tests)
cargo build --workspace --release                     PASS
git diff --check                                     PASS
```

The final count includes the two unchanged physics/audio placeholder tests; these
are not evidence of implemented physics. New coverage includes analytic offset
circles and rolling helices, variable-offset derivatives, independent time/spatial
kinematic agreement, angular acceleration/Coriolis terms, refinement, inversions,
one-sided discontinuities, singularities, constrained inverse circles, mixed-method
reference geometry, strict old/new persistence, transactional editing and actual
CLI save/reopen/reference exports. Headless egui input tests verify graph dragging
as one undo step and keyboard Apply. Worker tests verify preview versus authoritative
state, rapid-edit coalescing, stale-result rejection and error recovery. Failed-open
and unrelated-draft preservation are tested.

One expanded concurrent CLI test run stalled during Windows subprocess startup
and was explicitly aborted; it was not counted as passing. All three CLI workflows
then passed with one test thread. The harness now serializes those shared-executable
workflows with a mutex; the subsequent ordinary `cargo test --workspace` executes
all assertions. No test was removed or skipped. Native concurrent subprocess startup
was not established as reliable on this host.

Additional Release checks executed:

```powershell
.\target\release\quantum-x.exe heartline-demo target/pass3-final-heartline
python tools/verify-heartline-exports.py target/pass3-final-heartline
.\target\release\quantum-x.exe examples target/pass3-final-regression
python tools/verify-authoring-exports.py target/pass3-final-regression
.\target\release\quantum-x.exe render-editor examples/heartline-reference.qx.json target/pass3-editor-release.png
.\target\release\quantum-x.exe render-editor examples/mixed-authoring.qx.json target/pass3-mixed-editor-release.png
.\target\release\quantum-x.exe benchmark
```

All completed successfully. Existing spline/mixed/analytic-reference source lengths,
step-halving results and frame errors reproduce Pass 2's reported numerical outputs.
Every exported example saved/reopened/regenerated identically. The unchanged v1
foundation regression still protects its original endpoint and 1521 samples.

## Measured numerical and rendering results

The analytic vertical circle uses 0.1 m maximum source spacing, 1258 stations,
track radius 20 m and +2 m rider-normal offset. Source speed is 20 m/s; the offset
point speed is 18 m/s. Independent exported-data measurements:

| Quantity | Observed result |
| --- | ---: |
| Source path length | 125.6637061436 m |
| Rider path length | 113.09733552934375 m |
| Rider length error against 18*2*pi | 1.1119993814645568e-10 m |
| Maximum radius error | 8.107292615022743e-12 m |
| Maximum curvature error against 1/18 | 7.714370808820092e-12 /m |
| Maximum speed error | 2.4994406544465164e-10 m/s |
| Maximum normal-load error against 18/g + cos(s/20) | 2.0389911981055775e-10 g |

These measurements apply to this resolved constant-radius case, not every allowed
track. The verifier tolerances are respectively 1e-6 m length, 1e-8 m radius,
2e-7 /m curvature, 1e-7 m/s speed and 1e-5 g load. Helix tests separately require
error reduction by more than 5 on spatial-step halving and fine curvature error
below 3e-5 /m. General derivative estimates, spline transport and arc-length
integration have the documented numerical limitations; no certified error bound
or arbitrary correction multiplier is used.

Actual offscreen Release rendering used NVIDIA GeForce RTX 4070, Vulkan, driver
617.42, at 1440x960. The circle workspace rendered 5286 line vertices; the mixed
workspace rendered 14902. The PNGs were opened as local image files for visual
inspection: readable panels, graphs and solver centerlines were present, including
the distinct smaller rider circle. An initial offscreen render exposed missing
font bundling and unconsumed egui texture deltas; both integration defects were
fixed and the successful renders repeated. This is offscreen visual QA, not manual
native-desktop interaction or an FPS benchmark.

The existing source-generation Release benchmark measured 10.414 ms for 64 cubic
B-spline controls / 4031 samples, and 106.023 ms for 1024 controls / 40917 samples.
Reported sample+graph payloads were 1,450,872 and 14,729,832 bytes. These single runs
exclude heartline generation, GPU resources, preparation allocations, spare capacity,
history and total process memory. They establish no interactive frame-rate guarantee.

## Limitations, UX and Git

This is a functioning development authoring environment. Final production editor
UX, welcome flow, track-family assets, physical rail/spine/contact surfaces and
rigid-body trains remain outside the implemented scope. General inverse heartline
orientation solving, loop closure/endpoint fitting, adaptive heartline error control,
3D control-point picking, interactive spline point-count changes and cross-edit
incremental solver caching remain unavailable. Explicit split/remove operations
can reshape transition functions as labeled; they are not universal exact fitting.

Geometry/ref-point generation runs in a worker, but explicit JSON Open/Save and
authored validation remain synchronous development commands. Source speed is constant
within each section; speed/rate jumps have one-sided diagnostics and no finite
impulse model. Calculated-path loads are design-time kinematic estimates. The future
independent vehicle solver still targets 1/240 second.

Owner-approved direction followed: graph/timeline-first human shaping, all prior
representations, the five stated workspace regions, independent visual properties,
no automatic layout redesign and no active-desktop control. All newly chosen widget
interactions, panel presentation and dialogs are labeled development-only. The
referenced editor/welcome sketches were absent; those final design choices remain
human-directed. No silence is treated as approval.

Native mouse/keyboard routing, resizing/minimization/DPI, file drops, surface/device
failure and sustained performance were **not manually verified**. The guide provides
an owner-executable checklist. No Computer Use or native input automation occurred.

Git history separates preserved owner instructions, Pass 2 implementation and Pass 3.
Pass 2 was validated, committed and merged through
[PR #2](https://github.com/Coasterpete/QUANTUM-X/pull/2), then the clean Pass 3 branch
was fast-forwarded to that main baseline. A safety copy of every original modified/
untracked source plus tracked patch and Git bundle was saved outside the repository
at `C:\Dev8\QUANTUM-X-safety-20261024-pass3`. The owner's ongoing verified-pass Git
policy is recorded additively in AGENTS.md. Final Pass 3 publication and merge
identities are available in its GitHub PR and the completion message. No reset,
force push, unmerged-branch deletion, or discarded source work was used.
