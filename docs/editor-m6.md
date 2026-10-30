# M6 integrated track authoring: implementation and verification

The subsequent [M6 graph-drag correction](editor-m6-graph-drag-correction.md)
documents the native pointer lifecycle defect, regression coverage and updated
Windows reproduction checklist. Native acceptance remains with the owner.

This pass extends the existing uncommitted M4/M5 work in place on
`codex/integrated-authoring-pass3`, starting at HEAD `f01ba3d`. It does not declare
native usability accepted merely because automated checks pass. No desktop
control, branch change, commit, push, PR, or merge was performed. The owner's
task-specific Git restriction supersedes the standing repository Git workflow.

## Implemented workflow

New projects retain their neutral 10 m straight and default 20 m/s design speed.
Insert section presents the five ordinary coaster construction methods and
explicit advanced spline methods. Before/after targets the authoritative selected
section; the new section becomes selected with Normal active for force sections,
Pitch for geometric sections, and Roll for other constructions. Existing sections
are not fitted, smoothed, converted, or reauthored by insertion.

Section properties expose name, Apply/Cancel, length/duration, applicable channels,
and selected-transition values. Starting pose/design speed are accessible in the
same Track context; the full initial quaternion remains in Project. Domain
conversion and advanced speed/strict-boundary settings remain available after
the essentials. The graph toolbar adds Edit values and Transitions; narrower
panes consolidate secondary actions under More and, when necessary, Graph view.
Optional precision tools still scroll independently of the pinned graph header.

Pending section navigation, insertion, ordering, and removal use a shared
nonmodal resolver: **Apply & continue**, **Cancel draft & continue**, or **Keep
editing**. Apply and the subsequent operation are separate logical undo steps.
Invalid Apply retains the pending action and exact draft for correction. View
navigation, channel inspection, workspace switching, and hover do not commit or
discard drafts. Cancel restores the section while retaining separately staged
initial/reference settings. Applying unrelated metadata also retains their
preview. Save still requires explicit resolution of drafts; New/Open retain
the existing unsaved-document protection.

## Stable IDs and keyboard correctness

The full widget-driven scenario reproduced the reported regression: entering a
new section length added the draft notice in the section navigator. Its automatic
ID allocation shifted the following properties, allowing the retained numeric
focus to identify Cancel on the next pass. Enter then canceled the authored edit.

Section navigation and section properties now have independent stable ID scopes;
properties include the stable section ID. Transition value/precision scopes also
include section, channel, and transition IDs so a focused edit cannot migrate to
another authored target. Dynamic notices no longer change their numeric IDs.

Keyboard Apply runs after the widgets parse the current input batch. Typing and
Ctrl+Enter together therefore commit the newly parsed value. Plain Enter finishes
the field without accepting/discarding the document draft. Escape ownership is
retained across passes because egui may dismiss a popup or release text focus
before application shortcuts run. Closing that local editor cannot also cancel
the section with the same Escape. Text-editor undo/redo retains keyboard priority.

Investigation also verified that egui 0.37 has one memory-backed popup. A nested
ComboBox replaced its parameter-menu parent, so its choices disappeared on the
next pass. Parameter menus now stay open during input; nested choices use the
menu's submenu stack. Outside a menu, the normal ComboBox remains. This applies
to section type and transition function selection, not just the validation test.

Regression tests exercise precise entry, appearance of the real draft notice,
retained focus ID, plain Enter, button Apply, button Cancel, Escape from numeric
focus and menus, explicit section Escape, same-batch Ctrl+Enter, and one logical
undo transaction. A complete test constructs the hill using actual workbench
widgets rather than assigning the final project directly.

## Geometry, defaults, transactions, and precision

`qx-document::insertion::initialize` consumes the current committed generated
track and an explicit template. `SectionRange` now exposes its exact one-sided
endpoint world angular rate and design speed as derived data; these fields do
not change the project schema. Both integrated and analytic spline paths supply
those values.

For incoming unit tangent T and world angular rate omega, the endpoint curvature
vector is `omega × T`, and spatial twist is `omega · T`. A new force section uses
`(v² curvature - gravity) · N / g` and the equivalent lateral projection. Rider
geometry projects curvature onto the inherited rider N/L axes; world geometry
uses the existing gravity-referenced pitch/yaw basis. Rates are re-expressed in
the selected domain. Thus a tilted/banked predecessor is not incorrectly treated
as level 1 g / 0 g. World geometry at a vertical start is rejected with an explicit
Rider-local alternative.

Defaults span 10 physical metres, or its equivalent time when the predecessor is
time-based. A predecessor speed override is carried explicitly when needed.
Force/geometric/roll defaults use flat quintic transitions, permitting deliberate
endpoint editing without first replacing a Constant function. Straight, radius-40
arc, and straight mathematical spline templates keep their stated construction;
they may have a documented curvature jump. Pose/frame continuity is inherited by
the existing solver in every supported representation. Strict downstream rate
constraints are validated rather than relaxed. Diagnostics now identify measured
curvature, twist, and speed gaps and the section that rejects them.

Geometry-changing acceptance validates the complete candidate before replacing
the document/history. Numerical failures cannot enter history through Apply,
insertion, removal, or ordering. Open also preflights generation before replacing
the existing project. Preview generation remains coalesced on the background
worker, publishes only its latest revision, and retains the last valid view on
failure. Duplicate identical requests are suppressed. Rejected edits invalidate
earlier pending publications so an old success cannot erase their error.

All original f64 geometry, RK4 integration, transition evaluators, amplitude and
Hermite slope semantics, domain conversion, spline representations, and schema-3
persistence remain. The new section IDs increase monotonically within a session,
including across removal/undo, avoiding collisions with cached section views.
Opening a document resets those transient caches. Existing schema migrations and
curve compatibility tests remain applicable.

Historical splitting below is superseded by [shape-preserving NURBS regions](transition-nurbs-regions.md).
Split into two Hermite regions was an explicit **reshape**: it preserved
endpoint values/slopes and extent, but a nonlinear source need not retain its
interior shape. It is not a silent linearization or a new exact subdivision
algorithm. Removal explicitly extends a neighbor. Individual lengths, amplitude,
Hermite slopes, transition ordering, function changes, and numeric shared-boundary
entry remain accessible in the precision tools.
Reselecting the active Hermite function preserves its authored derivatives;
only an actual function-type change initializes new function parameters.

## Viewport and synchronized inspection

Camera fitting projects all eight actual geometry-bounds corners into the current
camera basis, using a 15% margin and the real viewport aspect. This avoids wasted
bounding-sphere space on thin/short tracks. Hidden rider-reference geometry no
longer expands Track framing. Startup and successful New/Open request framing;
each workspace resolves its first fit at its actual viewport size. Regeneration
and insertion do not reset manual camera positioning. Fit Selection uses the
selected range. Both projection modes retain double-precision camera/origin math.

Visual review also exposed coplanar depth fighting: grid lines at Y=0 could
obscure a straight track on the same plane, despite correct camera fitting.
The grid prefix now uses a non-depth-writing line pipeline; authored geometry
retains its normal depth test/writes. Native and offscreen rendering share this
batching rule. No track coordinates are displaced to make the line visible.
Offscreen verification checks actual pixels around projected samples of an
isolated selected straight, requiring at least 90% to remain visibly white.

Clicking the projected centerline selects a section through the same Session
selection service used by the list/timeline. A left drag beyond four logical
pixels orbits; middle/right drag pans; the wheel zooms. Existing pane/layer hit
testing prevents graph, popup, dock-splitter, and floating-window input from being
routed as viewport navigation. Native input still requires the manual checks below.

Selection highlights, transient section hover, start/end crosses, tangent arrows,
normal indicators, and precise endpoint coordinate readouts are derived overlays.
Authored graph hover places a corresponding station marker in the viewport;
viewport station inspection links back to the selected graph. Hover never changes
persistent selection. Stale/failed drafts are labeled and do not project a new
graph cursor onto unrelated old geometry. Derived plots remain read-only.

## Reference comparison and independently chosen scope

The focused comparison was read-only against local reference checkouts, not an
assertion about their latest remote versions. No reference source was copied.

| Reference | Inspected material and transferable concept | Deliberate boundary |
| --- | --- | --- |
| Original QUANTUM, `6efcf29` | ForceDrivenRegion, AuthoredSections/ChannelProfile validation, TransitionFunctions, TransitionEditorModel, insertion UI, ViewportCamera, and ForceDrivenAcceptanceTests. Sequential endpoint/frame inheritance, original-local-coordinate profile evaluation, semantic section anchors, explicit failure/transaction handling, and bounds-based inspection informed tests and composition. | Its construction-energy/speed and rider-offset equations differ from QUANTUM-X's constant design-speed model; those equations and its broader transition catalog were not ported. |
| QuantumCoasterWorksLib, `a4ad466` | TrackAuthoringGraphCompiler, CandidateEvaluator, SectionDefaultFactory, EngineeringSnapshot, SelectionService, SnapshotNavigation, Math Plot projection, CameraFrameBuilder, TrainFollowerState, and train-transform architecture review. Neighbor-aware defaults, immutable candidate validation, stable section/station projections, and separation of geometry/configuration/pose are useful concepts. | The inspected graph compiler explicitly rejects unsupported nongeometric families; it is not a ready-made force-vector compiler. Its follower/camera/transform APIs were research only and do not satisfy QUANTUM-X rigid-body dynamics requirements. |
| openFVeiD, `74c02bb` | track::newSection, sectionHandler, section integration context, and TransitionView's applicable channel lists/transition properties. Ordered section continuation and graph-first normal/lateral/roll versus pitch/yaw/roll authoring informed the workflow. | GPL reference code, assets, smoothing behavior, extra transition families, and implementation-specific numerical assumptions were not copied or imported. |

Verified reusable *test concepts* include tilted/banked starts, one-sided endpoint
rates, strict incompatible downstream joins, vertical world-axis singularity,
split/reorder/invalid-candidate atomicity, station/selection correspondence, and
save/reopen regeneration. QUANTUM-X implements these through its own Rust services.
No new dependency, graphics backend, train simulation, audio, terrain, or support
construction system was added by M6.

## Key implementation locations

| Location | Responsibility |
| --- | --- |
| `crates/qx-document/src/insertion.rs` | Pure predecessor-aware template initialization |
| `crates/qx-geometry/src/solver.rs`, `spline_track.rs` | Exact derived endpoint rates and specific incoming-constraint diagnostics; original generation equations retained |
| `apps/quantum-x/src/authoring/session.rs`, `workflow.rs` | Shared selection, candidate preflight, draft resolution, revision publication, independent preview state |
| `authoring/panels.rs`, `graph.rs` | Stable widget scopes, keyboard ownership, persistent parameter menus/submenus, contextual section/transition controls |
| `authoring/timeline.rs`, `graph_canvas.rs`, `layout_ui.rs` | Shared navigation, hover/station linkage, viewport context/feedback |
| `crates/rendering/src/camera.rs`, `scene.rs`, `gpu.rs`, `workspace.rs`, `inspector.rs`, `offscreen.rs` | Bounds fitting, centerline picking, overlays, depth-independent grid background, separate profile navigation, noninteractive capture/pixel validation |
| `authoring/scenario.rs`, `m6_tests.rs`, `tools/verify-m6-exports.py` | Opt-in scenario, real-widget regressions, independent exported numerical checks |

Paths beginning `authoring/` above are under `apps/quantum-x/src/`.

## Reproducible scenario and automated evidence

Run from the repository root:

```powershell
target\release\quantum-x.exe authoring-scenario target/m6/scenario
python tools/verify-m6-exports.py target/m6/scenario
target\release\quantum-x.exe examples target/m6/examples
python tools/verify-authoring-exports.py target/m6/examples
target\release\quantum-x.exe heartline-demo target/m6/heartline
python tools/verify-heartline-exports.py target/m6/heartline
```

The opt-in CLI fixture uses the same section insertion, resize, explicit split,
value/function, boundary, Apply, undo/redo, save, and reopen services. It is never
hard-coded into the editor startup. The widget-level test independently performs
the authoring through real egui controls, compares the resulting generated track
with that fixture, and saves/reopens it. Native Windows file-dialog interaction
is separate from these synthetic UI events and document-service checks.

The authored scenario is a 10 m straight followed by an 80 m force section at
20 m/s. Normal inputs are quintic 1→2, 2→0, 0→0, 0→1 g. After the boundary edit,
their lengths are 22/18/20/20 m. Lateral uses 0→0.1→0 g in two 40 m regions; roll
uses 0→0.006→0 rad/m in two 40 m regions. This is a deliberately synthetic open
layout for tool validation, not an owner-authored production coaster.

## Verification results

The final native Windows verification executes all five required Cargo commands:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

All pass. **125 tests pass**, including 58 application unit tests, 3 application
CLI integration tests, and the existing geometry/document/physics/style/rendering
checks. M6 adds 21 application regressions and 3 rendering regressions. Existing
M4/M5 tests remain enabled. The numerical-failure test was strengthened to expect
atomic rejection before history publication, instead of accepting an unbuildable
section and testing only its later worker failure.

The full workbench test uses actual egui pointer/key/text events to insert the
force section, type its extent, split/select/retype transitions, choose functions,
author lateral/roll profiles, edit a shared boundary, undo/redo, and save/reopen.
It reproduces exactly the generated numerical fixture. Additional widget tests
cover all five insertion-type choices at 960×640, draft resolution at both sizes,
amplitude, named Hermite derivatives, function reselection, transition ordering,
removal, invalid removal, and focused numeric/keyboard lifecycle. Synthetic egui
input never injects mouse or keyboard events into Windows.

| Scenario measurement | Observed result | Check |
| --- | --- | --- |
| Total length / samples | 89.9999999999996 m / 901 | 90 m within 1e-9 m |
| Peak elevation | 10.232834342674371 m | Rise and descending tangent both present |
| Save/reopen and regeneration | Exact equality on this machine/build | Authored parameters and generated track |
| Step-halving position difference | 4.751558071835e-10 m | Comparison, **not** a rigorous error bound |
| Frame squared-norm / orthogonality errors | 2.220446049250313e-16 / 1.249000902703301e-16 | Below 1e-12 |
| Exported authored-profile evaluation error | 6.439293542825908e-15 | Independent quintic evaluation, below 1e-12 |
| Derived normal / lateral load residuals | 1.5897799327396767e-5 / 1.29921508285058e-6 g | Below 2e-5 / 2e-6 g at 0.1 m sampling |
| Incoming pose/rate join | Shared endpoint/frame; curvature/twist gaps below 1e-12 in continuation tests | Strict continuation accepted without fitting |

The existing authoring export verifier passes for spline-comparison,
mixed-authoring, and numerical-reference projects. The independent analytic
reference reports total-length error 1.2278178473934531e-11 m and circle-position
error 4.9338866199459144e-9 m. The heartline verifier passes with radius error
8.107292615022743e-12 m and normal-load error 2.0389911981055775e-10 g. Diagnostic
warnings in the existing mixed/spline examples remain visible; they were not
silenced to obtain a successful result. All three Python verifiers pass.

Full commands/results: [verification.txt](../target/m6/verification.txt).
Independent M6 metrics: [verification.json](../target/m6/scenario/verification.json).
The saved project is [project.qx.json](../target/m6/scenario/project.qx.json).

## Offscreen captures

Actual egui/wgpu output was generated without native windows on NVIDIA GeForce
RTX 4070, Vulkan, driver 617.42. Each PNG has the exact requested dimensions.
The eight files were visually inspected, including startup straight visibility,
Normal/Lateral/Roll profiles, selected context, and the compact graph controls.
The straight's coplanar visibility also passes the GPU pixel check described above.

| Case | 1440 × 960 | 960 × 640 |
| --- | --- | --- |
| New project / straight | [Capture](../target/m6/scenario/new-project-1440.png) | [Capture](../target/m6/scenario/new-project-960.png) |
| Hill / Normal | [Capture](../target/m6/scenario/hill-normal-1440.png) | [Capture](../target/m6/scenario/hill-normal-960.png) |
| Hill / Lateral | [Capture](../target/m6/scenario/hill-lateral-1440.png) | [Capture](../target/m6/scenario/hill-lateral-960.png) |
| Hill / Roll | [Capture](../target/m6/scenario/hill-roll-1440.png) | [Capture](../target/m6/scenario/hill-roll-960.png) |

These captures do not verify native desktop interaction, frame rate, or final UX
approval. All numerical outputs and images are ignored local development artifacts.

## Owner-executable Windows checklist — not manually verified

Use isolated preferences and save to a new file:

```powershell
$env:QX_WORKBENCH_PREFS = Join-Path (Get-Location) 'target\m6\manual-workbench.json'
target\release\quantum-x.exe edit
```

1. Confirm the approved workbench, Overpass Mono, icons, and the visible neutral
   10 m straight. Inspect the starting coordinates and Starting pose/design speed;
   keep defaults for this scenario. Fit All should make the straight easy to see.
2. Select the straight. Open Insert section, choose Force-vector, then Insert
   after. Confirm section 2 is selected in the list and timeline and Normal is
   active in the Transition Editor. No JSON or CLI editing is needed.
3. Enter **80** in Length (m), then press plain Enter. Confirm the draft notice,
   retained value, and preview. Enter must not activate Cancel or Apply. If the
   sidebar is short, scroll it to its properties. Fit Graph/F over the graph
   shows the enlarged domain; extent edits preserve existing graph view ranges.
4. Open Transitions (More in compact panes). With Insert fraction 0.5, split the
   Normal transition. Split the first region again, select the third region in
   the Transition dropdown, and split it. You now have four 20 m regions.
5. Select each region. In Edit values choose Quintic smoothstep and enter this
   table; amplitude remains zero. Closing menus must preserve the draft.

   | Region | Start g | End g |
   | --- | --- | --- |
   | 1 | 1 | 2 |
   | 2 | 2 | 0 |
   | 3 | 0 | 0 |
   | 4 | 0 | 1 |

6. Select Lateral force, split its 80 m region halfway, and choose Quintic
   smoothstep in each. Enter 0→0.1 and 0.1→0 g. Select Roll rate, split halfway,
   and similarly enter 0→0.006 and 0.006→0 rad/m. Roll is an angular rate, not an
   absolute bank-angle target. Apply/Ctrl+Enter accepts this section once.
7. Select Normal region 1. Open Precision editor and set End boundary to **22**.
   Apply. Confirm the adjacent lengths are now 22/18/20/20, total 80 m. Ctrl+Z
   should restore 20/20/20/20 in one step; Ctrl+Y should restore 22/18/20/20.
8. Fit All and Fit Selection. Inspect continuity, endpoint arrows/readouts, orbit,
   pan, zoom, and Perspective/Top/Side/Front/orthographic views. Edit a normal
   endpoint: a clean graph drag commits once; over an existing draft it stays
   staged. Hover the graph and the track and check their corresponding station.
   Click a section to select it; hovering must not select it. Subsequent edits
   and insertion must not reset a manually positioned camera.
9. Stage a name/number change, then request another section or insertion. Try
   Keep editing, Apply & continue, and Cancel draft & continue. Verify no values
   are silently applied/discarded, and accepted actions undo separately. Try an
   invalid negative transition length: Apply must reject it, retain the draft,
   and keep the prior valid viewport. Correct it or explicitly Cancel.
10. With a draft pending, Escape from an open parameter menu must close that
    menu without canceling the section. Escape leaving a focused numeric editor
    must not also cancel the section. After leaving local focus, a separate
    Escape cancels the section. Confirm Apply and Cancel remain reachable.
11. Ctrl+S/Save As to `target\m6\manual-hill.qx.json`. Reopen it and check all
    values, boundaries, roll/lateral inputs, length, and geometry. Continue by
    inserting another ordinary section. Separately try before/after insertion,
    ordering/removal, and a strict incompatible incoming-rate policy; rejection
    must identify the constraint without reauthoring the neighboring sections.
12. Ctrl+2 and Ctrl+1 must preserve shared document/history and independent layout
    and camera state. Support Settings/Appearance stay right. Float/dock the
    Transition pane; repeat numeric/graph edits, wheel ownership, and keyboard
    checks at 1440×960, 960×640, and the UI scales/native DPI settings you use.
    Check pointer capture, releasing drags outside, native dialogs, focus, and
    mixed-monitor scaling. No performance benchmark is implied by these checks.

Close the application, then run `Remove-Item Env:QX_WORKBENCH_PREFS` in that shell
to restore the normal preference location.

## Limitations and UX/Git status

Native pointer/keyboard drivers, physical focus/capture, docking comfort, mixed-DPI
movement, file dialogs, sustained authoring performance, and the owner's final
usability judgment remain manually unverified. Small/high-scale panes require
scrolling properties and may consolidate graph commands; arbitrarily tiny panes
cannot display all precision tools at once.

Geometry acceptance and insertion/Open preflight currently perform a bounded
solver validation synchronously at explicit transaction boundaries. This is a
deliberate integrity check before publishing history; live preview work remains
asynchronous/coalesced. Large documents may pause on acceptance. CPU centerline
picking is linear in sampled segments with one precomputed projection per query.
No 60 FPS or large-scene responsiveness claim is made. Moving preflight/picking
to a scheduled/cache-backed path is a future performance improvement if measured
scene costs justify it.

The viewport still renders centerline/reference lines, not rail surfaces or a
finished train asset. Profiles use constant design-speed kinematics; measured
loads are geometry diagnostics, not train simulation or safety/operational
certification. This pass does not add articulated train dynamics, ride devices,
closed-circuit solving, automatic layout generation, or hidden smoothing.

Owner-directed workflows and the workbench were followed. Exact contextual-menu
presentation, compact treatment, notice placement, fitting margins, picking
tolerance, and overlay styling remain reviewable implementation choices. No new
shipping developer-only UI was added; the existing development rendering and
opt-in diagnostic fixture remain labeled. See [UX status](ux/editor-m6.md).

All changes remain local and uncommitted on the original branch. Existing M4/M5
changes and unrelated assets/files were retained; `target/m6/baseline/` contains
the pre-M6 tracked patch and relevant source copies. Artifacts/logs under ignored
`target/m6/` are local verification data. Nothing was published.
