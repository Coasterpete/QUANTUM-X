# Integrated Workbench M4

Historical M4 report. [M5](editor-m5.md) supersedes the path-field file workflow,
graph gesture presentation, typography and command shell while retaining this
workbench's layout/preferences architecture. Use M5's current acceptance checklist.

Implemented locally on 2026-10-25 on `codex/integrated-authoring-pass3`, starting
from a clean `f01ba3d` checkout. No branch changes, commits, pushes or PRs were
performed, following the current request's override of the repository Git policy.
An unrelated untracked `assets/icons/` directory appeared during the pass and was
left untouched; it is not part of this implementation.

## Workspaces and real capabilities

Run `cargo run -p quantum-x --release -- edit [project.qx.json]`. The default Track
profile has top menus/commands, left Track Workspace, center 3D Viewport, right
Support Workspace and bottom Transition Editor. Use the top profile controls or
Ctrl+1 / Ctrl+2 to switch Track / Train. One `Session`, document, generation
worker and undo/redo history remain authoritative through both profiles. Section,
transition, graph range and all staged edits remain intact. Renderer camera and
overlay settings are retained separately for each profile during the session.

Each profile owns an independent `egui_tiles::Tree`, including split proportions,
tab ordering, selected tabs and floating authoring panes. Drag visible tabs to
split or group panes; seven-point gutters provide splitter targets. Minimum tile
size is 160 UI points, subject to the available host area. Right-click an authoring
tab or use Window / Layout to float it. Floating windows are egui windows **inside
the application's native window**. Dock pane (or closing the floating window)
returns it to its retained dock location. The GPU viewport remains docked because
the existing renderer draws one shared surface viewport; native multiwindow GPU
rendering was not added. A viewport grouped under an inactive tab stops rendering
and accepting camera input. Reset current workspace layout recovers that profile;
Reset Window/Layout restores both profiles and default window preferences.

Track retains selection, insertion, reordering, numeric edits, graph dragging,
domain conversion, regeneration, draft cancellation, undo/redo and save/open.
Ordinary insertion contains Force-vector, Rider-local geometric, World-geometric,
Straight and Circular arc. Bézier, B-spline, NURBS and Catmull–Rom insertion lives
under **Advanced mathematical representations**. Existing spline sections display
their actual representation and retain their complementary control editor.
There are no mathematical conversions, schema changes or solver modifications.
Transition functions (smoothstep, cosine, Hermite etc.) remain ordinary editable
profiles, with the existing validation and transaction semantics.

Support Workspace opens on the right and contains **Support Settings** and its
**Support Appearance** disclosure. Unavailable construction tools identifies
prefabs, foundation/rail connector generators, support definition copy/paste and
structure/component/attachment editing as unavailable. QUANTUM-X has no support
document model, so there are no generator buttons, fabricated settings or support
objects. Viewport controls and real generation diagnostics are separate View and
Diagnostics tabs in the same right-hand dock group. Support presentation state
has no connection to section drafts or document history. Branched route authoring
is also absent; Route currently means the existing ordered section chain.

Train has Configuration on the left, Preview in the center and Summary on the
right. Configuration edits the **existing** nominal train configuration ID,
authored-reference intent and constant/quintic rider offsets through SetHeartline
commands. These are the same draft and committed fields exposed from Track.
Apply is undoable; Summary reads committed data and indicates a pending draft.
Preview uses the existing track/rider reference geometry and optional overlays.
There are no train meshes, consists, mass/bogie editing, articulated dynamics or
ride systems. Design speed and derived loads are kinematics, not train telemetry.

## Window preferences and persistence

Window / Layout → Display preferences provides:

- Automatic: 90% of usable width and 88% of usable height, after allowing for native
  decorations, with the application's minimum size where the monitor permits it.
- Custom: configured logical client width/height, normally at least 960 × 640.
- Remember (default): last normal logical size, or Automatic on first launch.
- Maximized: preserved independently of the last normal rectangle.
- UI scale: 0.75–2.0 egui zoom, independent of native DPI and client logical size.

Normal dimensions and physical outer-window position are captured on native
move/resize/DPI events and shutdown. The startup resolver chooses the saved
monitor when the title-bar position still belongs to an available work area;
otherwise it recovers onto the primary monitor. Sizes/positions are bounded to
that area, including negative-coordinate and small-monitor cases. Windows uses
GetMonitorInfoW's work rectangle so the taskbar is excluded; other platforms use
winit's monitor bounds as a fallback, not a claim of tested native support.
The current decoration allowance is 16 logical pixels horizontally and 48
vertically. Native border/taskbar behavior and mixed-DPI movement still need
manual Windows acceptance. No monitor resolution or display mode is changed.

Preferences live at `%LOCALAPPDATA%\QUANTUM-X\workbench-v1.json`, separately from
coaster files. `QX_WORKBENCH_PREFS` can override the path for isolated verification.
The saved active profile and both layouts are restored on restart. Camera pose,
overlay toggles, document selection and scroll/text focus are session state, not
saved project or layout data. If LOCALAPPDATA and the override are both missing,
the current host has session-only preferences.

Loading caps input at 256 KiB, checks the preference version, validates finite
dimensions/zoom and checks layout reachability, depth, pane uniqueness, references,
focused tabs, positive shares and floating-pane records. Invalid layout/version
data recovers defaults, shows an actionable message and retains the original file
until the user selects Reset Window/Layout. Recoverable window numbers are
sanitized. Floating rectangles are constrained to the current host content area.
Normal layout changes are compared at most once a second; unchanged state causes
no write. A bounded background writer creates/syncs a unique sibling file and
renames it into place. Shutdown waits for the final save; filesystem errors are
shown in the workbench. This is not a cross-process configuration merge protocol.

Native surface/depth-target resizing remains in qx-rendering. Camera hit testing
now uses egui pixels-per-point (native DPI multiplied by UI zoom), keeping input
and the GPU viewport in the same coordinates. DPI changes cancel camera drags.
Only scene/overlay changes upload line data; moving splitters, changing layouts
or resizing windows does not submit track generation jobs.

## Precision plot presentation

`authoring/plot.rs` provides f64 axes/mapping, bounded 1/2/5-decade lattice steps,
adaptive subdued dots, clipped zero references, numeric rulers and unit labels.
Both the authored transition plot and derived diagnostic plot use it. The surface
is near-black; selected curves/handles are brighter and selected square handles
are larger. Hover readouts include units. Y range / Fit values remain available;
Domain range adds explicit numeric X view bounds without changing authored inputs.
Curve display samples resolve the visible domain. Invalid Y view ranges are
identified and recover to display defaults. Invalid domain ranges display the
whole section. Dots and reference lines never participate in snapping or commands.
Dragging beyond the visible Y range still commits one undo transaction.

The section timeline scrolls horizontally, keeping one row even in large documents.
Graph height responds to available visible pane height. Numeric controls wrap and
remain scrollable at smaller sizes. Derived plots still use design-speed results
from the last generated scene; they are not a physics rewrite. No 60 FPS claim or
new lighting/material/rail functionality is made.

## Components and reference evaluation

| Component | Responsibility |
| --- | --- |
| `apps/quantum-x/src/authoring/workbench.rs` | Profile/pane layout data, validation and separate preference storage |
| `apps/quantum-x/src/authoring/layout_ui.rs` | Top commands, docking/floating integration, Support and Train compositions, display controls |
| `authoring/panels.rs`, `session.rs`, `mod.rs` | Existing commands/drafts, ordinary/advanced insertion and native preference adapter |
| `authoring/plot.rs`, `graph.rs` | Reusable dotted plots and existing transition interactions |
| `crates/rendering/src/window.rs`, `workspace.rs` | Monitor work-area resolution, window lifecycle, profile navigation and DPI input |
| `crates/rendering/src/gpu.rs`, `camera.rs` | Hidden viewport handling and retained camera snapshots |
| `apps/quantum-x/src/main.rs` | Offscreen workspace/size verification options |

Read repository AGENTS.md, core foundation, Pass 2/3 architecture and the available
human-authored `docs/ux/coaster-properties.md` before implementation. Original
Inkscape sketches were not found in these materials. The owner's current M4 brief
is the authority for structure; [the captured UX direction](ux/workbench-m4.md)
does not finalize colors, typography, detailed interactions or future tools.

Original QUANTUM was inspected read-only at local revision `6efcf29`. References
included EditorUi.cpp's `drawScalarDotGrid()` and separate workspace dock builders,
EditorUiTrain.cpp and recovery-m1b-train-workspace.md, SupportAppearance,
SupportPicking/SupportNodeManipulation, Supports.hpp, support application services,
WoodenSupportGenerator.hpp, the managed support tools and engine Application.cpp's
usable-monitor sizing. The transferable behavior is independent profiles, honest
capability presentation, separate support appearance, and engineering-unit dot
spacing. Its support/physics/asset implementations were not ported or copied.
The newer QUANTUM Avalonia workbench also documents a separate Supports profile;
the current owner's M4 right-hand Support Workspace requirement takes precedence.

Evaluated [egui_dock 0.21.1](https://docs.rs/egui_dock/0.21.1/egui_dock/), which
targets egui 0.36, and [egui_tiles 0.18.0](https://docs.rs/egui_tiles/0.18.0/egui_tiles/),
which matches this workspace's egui 0.37 and provides split/tab drag layouts. Used
egui_tiles without downgrading the renderer/UI stack. It is MIT OR Apache-2.0;
its direct dependencies were already present. Enabling egui serde adds enumn
0.1.14 (MIT OR Apache-2.0). Direct windows-sys 0.61 (already transitive,
MIT OR Apache-2.0) supplies the work-area query. No new font/assets were added.

## Verification completed

On native Windows, all of these completed successfully:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
target\release\quantum-x.exe render-editor examples/heartline-reference.qx.json target/m4-track.png track 1440 960
target\release\quantum-x.exe render-editor examples/mixed-authoring.qx.json target/m4-small.png track 960 640
target\release\quantum-x.exe render-editor examples/heartline-reference.qx.json target/m4-train.png train 1440 960
target\release\quantum-x.exe render-editor examples/heartline-reference.qx.json target/m4-train-small.png train 960 640
target\release\quantum-x.exe examples target/m4-examples
python tools/verify-authoring-exports.py target/m4-examples
target\release\quantum-x.exe heartline-demo target/m4-heartline
python tools/verify-heartline-exports.py target/m4-heartline
```

Workspace tests cover switching via actual egui key events, document/draft/history
preservation, resizing without generation requests, floating/docking, persistence,
focused tabs, corrupted layouts and spline fixture compatibility. Mapping tests
cover finite/extreme scales, tick bounds/density and degenerate inputs. Window
tests exercise invalid/offscreen/mixed-DPI placement and small-monitor constraints.
Existing transition drag checks also exercise release outside the plot bounds.

Offscreen images were generated on NVIDIA GeForce RTX 4070, Vulkan, driver 617.42,
and inspected as files. This created no native desktop windows and injected no
OS input. Independent example export checks passed. The analytic heartline circle
had radius error below 1e-8 m and normal-load error below 1e-5 g, satisfying the
existing verifier. These checks do not establish native interactive behavior,
real train physics, structural support functionality or rendering performance.

## Exact owner-executable Windows checklist (not manually verified)

Use isolated preferences and save edits to a new project path:

```powershell
$env:QX_WORKBENCH_PREFS = Join-Path (Get-Location) 'target\m4-manual-workbench.json'
cargo run -p quantum-x --release -- edit examples/mixed-authoring.qx.json
```

1. Confirm the default top/left/center/right/bottom structure. Support Settings and
   Support Appearance must be in the right Support tab. Open View and Diagnostics;
   confirm actual overlays/camera commands and sampling regeneration work.
2. Resize all three split boundaries. Drag View/Diagnostics tabs into another dock
   group, then back. Confirm the 3D viewport retains correct aspect and camera
   input does not trigger on splitters, graph handles or floating windows.
3. Right-click Transition Editor → Float pane. Move/resize its internal window,
   edit an endpoint, then Dock pane. Repeat with Track and Support; closing a float
   must return its pane. Reset current workspace layout restores the default.
4. Select a force section and Normal channel. Change a numeric draft without
   applying. Ctrl+2 must show Configuration/Preview/Summary; Ctrl+1 must restore
   the selected section/channel/draft and graph range. Undo history must survive.
   Test independent camera angles/overlays and tab placement in both profiles.
5. In Train, change the nominal ID or rider offset and Apply. Summary must show
   the committed value, reference preview must update and Ctrl+Z/Y must undo/redo.
   Stage another offset without applying, switch twice and confirm it remains.
   Cancel the draft before using history if the draft guard requests it.
6. In Track, select Normal/Roll and edit Y range / Fit values / Domain range. Dots,
   rulers and zero lines must adapt without changing the document. Drag a square
   endpoint within and beyond the visible plot, release and verify one-step undo.
   Exercise split/remove/reorder transitions, Hermite slopes, amplitude, Apply,
   Cancel and Ctrl+Enter. Confirm curves never paint into adjacent panes.
7. Open Display preferences. Test Custom 1280 × 800, Maximized off, Apply. Test
   UI scale 1.25 independently. Confirm numeric text input, graph handles and
   camera pointer targets still align. Test Automatic and Remember as well.
8. Resize to 960 × 640 and a large window; maximize/unmaximize, minimize/restore
   repeatedly. Confirm the surface recovers without stretching or validation
   errors. Small panes must scroll to all numeric actions. No generation message
   should be issued merely by moving/resizing panels or the native window.
9. Move to a differently scaled Windows monitor (where available), including one
   with negative desktop coordinates. Check text, viewport and graph input at
   100%, 150% and 200% Windows scaling. Close/reopen with Remember, then with
   Maximized; verify the normal rectangle and each profile's layout return.
10. Close on the secondary monitor, disconnect that monitor, and reopen. The title
    bar/window must recover onto the available work area. Alternatively, with the
    app closed, set the isolated preference JSON's `window.position` to
    `[50000,50000]`, reopen and check recovery. Never change the display resolution.
11. Set Project path to `target/m4-manual-project.qx.json`, Ctrl+S, reopen that path
    and check section data, nominal configuration and reference geometry. Existing
    spline sections must still show their controls. Ordinary insertion must offer
    coaster concepts, with spline creation only under the advanced disclosure.
12. Close the app, back up the isolated preference file and replace its contents
    with invalid JSON. Reopen: defaults and a recovery message must appear, and
    the invalid file must remain intact. Reset Window/Layout must recover normal
    saving. Document data/history must be unaffected by either layout reset.

When done, close the app and run `Remove-Item Env:QX_WORKBENCH_PREFS` in that
PowerShell session to return to the normal application preference location.
