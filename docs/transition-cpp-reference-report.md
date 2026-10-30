# Transition Editor: local C++ study and focused Rust improvements

This is a historical implementation report. The later
[source-publication audit](publication-2026-10-30.md) records current verification
and publication boundaries; the local-only Git statements below describe this
earlier pass, not the repository's current publication status.

2026-10-30. The reference was the existing **C:/Dev8/QUANTUM** checkout at
`6efcf29`, including its working tree. It was read only: no builds, edits, Git
mutations, or copied implementation code. Rust work was layered onto the
preexisting dirty **C:/Dev8/QUANTUM-X** tree. The current request overrides the
repository's standing commit/push/merge workflow; this pass remains local.

The investigation read implementations rather than inferring behavior from
screenshots. This pass did not re-watch the original recording; it retained the
existing explicitly synthetic reconstruction in `transition_scenario.rs`.

The following comparison distinguishes useful C++ behavior from behavior that
should remain specific to the newer Rust engine.

| Area | Local C++ implementation and finding | Rust comparison and result |
| --- | --- | --- |
| Creation / insertion / splitting | [ChannelProfileEditing.cpp](C:/Dev8/QUANTUM/core/src/ChannelProfileEditing.cpp:73), `splitChannelSegment`, evaluates the shared value and slope, retains the left ID, allocates a right ID, and inserts in order. [EditorUi.cpp](C:/Dev8/QUANTUM/editor/src/EditorUi.cpp:1721), `drawProfileSegmentMenu`, captures a physical split station for “Split here.” | Existing [editing.rs](../crates/qx-document/src/editing.rs), `split_transition`, already performs exact NURBS subdivision. Retained it. Added `region_edit::split_at` and a graph context menu; both graph and fraction commands select the new right region. Merely inserting a region does not invent a new force or layout. |
| Exactness of a split | C++ reuses the original easing preset on shortened domains. With custom cubic Hermite tangents, the evaluated split slope gives exact cubic restriction; general easing presets are re-evaluated over each new interval and need not preserve the original shape. | Rust remains stronger: homogeneous knot insertion, exact polynomial basis conversions, and exact analytic cosine restrictions are preserved. No Hermite replacement or curve fitting was introduced. |
| Shared values / boundaries | `setChannelSegmentValue` writes both neighboring values. `moveChannelSegmentBoundary` updates two adjoining domains and pins outer boundaries. Removal extends the previous region (or next for the first region) and inherits the removed far value. | Existing [region_edit.rs](../crates/qx-document/src/region_edit.rs), `endpoint` and `boundary`, retain physical-domain endpoint jets under explicit C0/C1/C2 contracts. Graphical constrained seams now have one shared square, owned by the left end. Free endpoints still show separate handles. NURBS removal now retains both surviving outer jets, preventing a disconnected following join. |
| Graph selection / navigation | [TransitionEditorModel.cpp](C:/Dev8/QUANTUM/editor/src/TransitionEditorModel.cpp:634), `extractSemanticProfileMarkers`, creates N+1 actual boundary markers; `chooseCurveHit` / `chooseMarkerHit` use deterministic hit priorities. `showTransitionEditor` draws several channels together, remembers selected segment IDs per channel, and highlights the selected curve span. | [graph_canvas.rs](../apps/quantum-x/src/authoring/graph_canvas.rs), `show`, retains the existing single active-channel arrangement, colors and span highlight. Added region number and station bounds directly in the selected span; exact graph-boundary clicks resolve left. Right-click chooses the region at the requested station rather than acting on an unrelated old selection. |
| Shape manipulation | The unified C++ graph primarily manipulates semantic endpoint markers. [PrecisionAuthoring.cpp](C:/Dev8/QUANTUM/editor/src/PrecisionAuthoring.cpp:390) separately exposes easing/Hermite slope modes and optional symmetric slopes. Positional Hermite/Bezier-style handles in that file are a separate geometric authoring system, not scalar NURBS regions. | The previous Rust routine NURBS shape controls were numeric only. Added circular control-ordinate handles and their control polygon for the selected region, using its existing exact refinement. First/last three controls stay protected, retaining both endpoint values and first/second derivatives. Knots and positive weights remain optional advanced controls. |
| Dragging | C++ `showTransitionEditor` locks horizontal/vertical intent after a threshold and uses rolling input deltas; `accumulateMarkerValueDrag` retains an unsnapped anchor. Shift uses the configurable fine gain, normally 0.1. | [graph_gesture.rs](../apps/quantum-x/src/authoring/graph_gesture.rs), `capture` / `update` / `finish`, retains captured target identity, press-time scale, cancel, and at-most-one commit. Added rolling displacement with live Shift gain 0.1 without modifier jumps. Kept the existing separate timing diamonds and value squares rather than replacing the established gesture model. |
| Zoom / graph navigation | C++ offers `fitSymmetricGraphRange`, `scaleGraphRange`, and `expandGraphRangeToInclude`; the unified graph uses the complete section domain. Camera wheel/pan handling elsewhere in `EditorUi.cpp` is viewport navigation, not proof of equivalent graph navigation. | Existing Rust pointer-anchored Ctrl+wheel zoom, middle pan, F-to-fit, exact range entry, per-channel view memory, clipping and scroll ownership remain. No automatic view rescaling during a drag changes the captured value scale. |
| Interpolation / units | [ScalarTransition.cpp](C:/Dev8/QUANTUM/core/src/ScalarTransition.cpp), `evaluateScalarTransition`, derivative and integral functions, uses explicit domains and optional Hermite tangents. [TransitionFunctions.cpp](C:/Dev8/QUANTUM/core/src/TransitionFunctions.cpp) implements the easing families. `ProfileRowStyle` adapters convert displayed angular units and physical distance. | [profile.rs](../crates/qx-geometry/src/profile.rs) keeps f64 scalar rational NURBS with derivatives divided by length and length squared. Roll/Pitch/Yaw remain rad/m or rad/s; normal/lateral targets remain g. Graph control abscissae are Greville positions mapped to the authored station domain, not uniform control indices or coaster arc-length estimates. The horizontal coordinate is fixed during ordinate edits. |
| Adjacent transitions / continuity | C++ shared values enforce C0; merely sharing a value does not ensure matching slopes or curvature. Moving a boundary can change physical derivatives. | Existing Rust C0/C1/C2/Free contracts remain explicit. Routine local shape edits keep all boundary jets fixed; a shared endpoint edits the two constrained neighbors; independent Free endpoints stay independent. `join_incoming` remains an explicit opt-in two-region C2 reshape. No automatic smoothing, hidden constraint relaxation, or migration repair was added. |
| Preview / commit / history | [ApplicationService.cpp](C:/Dev8/QUANTUM/editor/src/ApplicationService.cpp:474), `previewProfileDrag`, stages from an original profile with a revision token. `commitProfileDrag` validates/prepares and publishes one transaction; cancellation discards it. `commitPreparedEdit` / `restoreHistory` manage document history. The unified UI also queues typed profile/value/distance commands through its frontend. | [session.rs](../apps/quantum-x/src/authoring/session.rs), `preview_project`, `queue_generation`, `poll`, `execute`, and [panels.rs](../apps/quantum-x/src/authoring/panels.rs), `workspace`, retain the coalescing worker and revision checks. New control gestures use that same path. Clean gestures commit once on release; gestures over an existing draft remain staged for Apply/Cancel. Context-menu structural edits are staged. |

New numerical corrections are deliberately narrow. `NurbsProfile::split` no
longer assigns a false C2 contract when directly splitting a repeated knot with
an intentional C0/C1 derivative break. The higher-level `Transition::split`
already classified physical derivative gaps; that existing behavior is retained.
`editing::remove_transition` now handles NURBS removal by extending the survivor
and restoring the original outer jets, including the removed far jet. Other
channels and section extent remain unchanged. This is explicitly a reshape,
whereas `region_edit::merge` is exact concatenation.

The focused additions are in [graph_regions.rs](../apps/quantum-x/src/authoring/graph_regions.rs)
(`split`, `context_menu`, `controls`) and [region_edit.rs](../crates/qx-document/src/region_edit.rs)
(`split_at`, `shape_control`). The graphical control net is a temporary refined
copy until a real edit succeeds. Merely selecting or drawing NURBS does not
change saved knots, IDs, values, weights, or undo history. A control ordinate is
not a point that the curve must interpolate; the tooltip states this explicitly.

Unexpected geometry can still come from an intentional Free value step, a
C0/C1 slope/second-derivative break, a very short edited duration, extreme control
values or rational weights, an explicit removal/reshape, or an inter-section
rate/reference-speed jump. A mathematically C2 scalar profile does not by itself
bound curvature, jerk, loading, or ride quality. The authoring solver's design
speed is distinct from physical train motion. The existing rigid-body solver
and its 1/240 s timestep were not changed.

Verification covers exact split/merge, rational nonuniform knots and weights,
shape and physical derivatives, repeated edits, C0/C1 repeated-knot splitting,
all four removal positions in a C2 chain, untouched neighboring regions and
channels, invalid controls and stations, Apply/Cancel, preview synchronization,
undo/redo, save/reopen, and prior schemas. New egui event tests exercise actual
context-menu clicks and control drags without sending Windows input. Existing
five-scale/three-layout gesture tests remain. Two older workflow tests now
explicitly select the left region after splitting to retain their original
geometry fixtures and unchanged assertions.

Reproduce from the Rust repository root:

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
target/release/quantum-x.exe transition-scenario target/transition-reference-pass/fresh
python tools/verify-transition-reference.py target/transition-reference-pass/fresh
target/release/quantum-x.exe physics-diagnostic
```

Executed successfully in this pass: `cargo fmt --all -- --check`, workspace
check, Clippy with warnings denied, full workspace tests (**265 passed, zero
failed**, including headless integration tests), native Windows workspace
Release build, Release `transition-scenario` before/after offscreen runs,
`verify-transition-reference.py` with the pre-change directory, Release
`physics-diagnostic`, and `git diff --check`. Release `examples` exports also
passed the existing `tools/verify-authoring-exports.py` numerical verifier.
Checks use `--locked` where supported. No tests were disabled or weakened.

The output directory for `transition-scenario` must be new. The Python verifier
independently evaluates the saved homogeneous NURBS and its first two physical
derivatives at all 10,001 CSV stations; it also checks shared C2 behavior,
nonzero interior edits, numerical geometry evidence and capture dimensions.
Passing a second argument naming the pre-change output checks identical saved
fixtures across the actual implementation change.

Measured Release evidence on the NVIDIA GeForce RTX 4070 / Vulkan:

| Measurement | Result |
| --- | --- |
| Split profile value / first / second derivative differences | `1.388e-17` / `2.168e-18` / `9.216e-19` in their physical units |
| Split generated position / curvature differences | `1.388e-17 m` / `8.153e-16 /m` |
| Python independent saved-NURBS value / slope / second differences | `1.735e-17` / `2.168e-18` / `9.758e-19` |
| Local shape edit: generated endpoint versus independent 20,000-step planar quadrature | `2.417e-10 m` |
| Local shape edit: integrated pitch difference | `3.152e-11 rad` |
| Generated authored-pitch graph versus edited scalar profile | `0 rad/m` |
| Generated finite-interval curvature versus midpoint rate magnitude | `2.924e-6 /m` (threshold `2e-5 /m`) |

The curvature comparison is an independent finite-interval measurement, not an
assertion of bit equality with a pointwise input. No timing/FPS claim is made.

Actual before/after captures contain both graph and viewport, using identical
synthetic saved projects:

- [Before this pass](../target/transition-reference-pass/before/profile-after-split.png)
- [After: one shared seam and direct NURBS controls](../target/transition-reference-pass/after/profile-after-split.png)
- [After a local shape edit, with generated viewport](../target/transition-reference-pass/after/profile-after-local-shape-edit.png)
- [Numerical evidence](../target/transition-reference-pass/after/numerical-evidence.json)
- [Full final workspace test log](../target/transition-reference-pass/workspace-tests-final.log)

These are real offscreen wgpu/egui captures, not retouched screenshots or a
proposed coaster layout. They were visually inspected. No active desktop was
controlled, and native mouse feel is **not manually verified**.

UX status: the changes follow this request's approval for direct region editing,
predictable graph control points, shared-boundary feedback and clearer selection,
within the existing colorful Editor arrangement. No new product screen, layout,
theme, or developer-only interactive UI was introduced. The capture scenario is
development-only. Final interaction feel remains subject to owner review.

Remaining differences: C++ multi-channel overlays and per-channel region-selection
memory, snapping preferences, integrated-rotation targets, and broader precision
transform commands were studied but not ported. Rust retains its existing channel
selector, which resets region selection on a channel change. Exact merge still
requires compatible degrees, shared values, and the 256-control limit. Editing a
merged profile's repeated internal knot may require advanced constrained control
work; an edit that violates its interior contract is rejected instead of silently
fitting it. Very short spans and extreme weights still require numerical care.
No wheel/rail-contact physics or ride-safety certification is implied.

Owner manual checklist: run `cargo run --release -p quantum-x -- edit`; open a
project and right-click within a region; split at the shown station; confirm the
new right region and unchanged geometry. Drag a circular shape control and then
the shared square; inspect both local shape and join diagnostics. Toggle Shift
during a drag, use Esc, and verify timing diamonds, Apply/Cancel, undo/redo and
save/reopen. Deliberately choose Free under Advanced to verify separate values.
Review the same operations in a floating pane and at high DPI.

Git status: branch `codex/public-identity-library`, HEAD `8691591`. Existing local
work was preserved; this pass adds uncommitted changes only. No commit, push,
merge, branch creation, or PR occurred. The C++ working-tree status remains the
same as at the beginning of the inspection.
