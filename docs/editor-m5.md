# M5 integrated editor implementation and verification

**Historical initial M5 report.** The owner's subsequent corrective brief replaces
the initial font choice and graph navigation/scrolling behavior. See
[M5 correction](editor-m5-correction.md) for the current implementation, causes,
checks, comparisons and remaining native acceptance work.

Implemented locally on 2026-10-25 over the uncommitted M4 workbench on
`codex/integrated-authoring-pass3` (starting HEAD `f01ba3d`). No branch switch,
commit, push, PR, merge or active-desktop control was performed. The explicit M5
instruction overrides the repository's general publish/merge workflow. Existing
M4 files and the owner's upstream icon trees were preserved.

## Actual implementation

The written approved spatial structure is retained through the existing
egui_tiles trees: top desktop shell, Track left, viewport center, Support right,
Transition Editor below. Docking, tab grouping, split sizes, internal floats,
independent profiles, display preferences and the separate preference store remain
the M4 implementation. The Inkscape attachment itself was absent; only its layout
diagram and written interpretation could be followed.

Track's section cards show order, name, method and domain extent, with an explicit
selected border and shared timeline selection. Long names truncate with full
details in tooltips; the list scrolls independently. Insert Section exposes the
existing coaster section types with separate Insert Before and Insert After.
Earlier/Later use MoveSection and preserve the stable selected ID. Remove uses
the existing command and history. Pending section drafts guard selection,
insertion and reordering. Spline insertion remains in the advanced disclosure.

Section, Coaster, Reference and Project contexts expose the existing selected
section properties, concept catalog, rider reference, metadata and initial
conditions without scrolling through every unrelated group. Mechanism properties
are identified as future work. Every existing numeric authoring capability remains
available. Apply, Cancel, Ctrl+Enter and history retain the shared Editor model.

The transition canvas now provides channel-specific curve colors, selected-region
shading, 20-point endpoint hit areas, hover/drag feedback, separated endpoint
targets at joins, boundary lines and top-edge diamond handles. Handles have
specific units/behavior tooltips. Cursor inspection reports evaluated curve values
at the pointer domain coordinate. Numeric rulers/dotted grids/zero references use
the existing f64 Plot mapping. Ctrl+wheel zooms about the pointer, middle drag pans,
F over the graph fits, and View range retains exact X/Y view controls. View changes
never write authored data or history. Invalid ranges recover display bounds.

Dragging a diamond transfers duration between two adjacent transitions, preserving
total channel extent, stable IDs, endpoint values, function data and all other
channels. The same operation has an exact numeric End boundary control. Values
outside the supported minimum duration are rejected with an explanation, rather
than clamped into a different authored result. The most recent valid draft remains
visible. Boundary timing can change shape/derivatives; no hidden continuity fitting
is performed. Existing mathematical validation still decides whether Apply works.

A pointer gesture snapshots the preceding section draft. Clean-section gestures
publish one ReplaceSection transaction on release, including release outside the
plot. Gestures starting with numeric drafts remain staged until Apply. Esc during
a gesture restores that snapshot; releasing afterward cannot commit it. Unchanged
gestures do not create a command. Transition selection exposes its ordinal, stable
ID/function and values. Precision Editor retains lengths, values, amplitude,
Hermite derivatives, function choice, split, removal and transition ordering.
Existing split/remove reshape explanations are retained. Derived measurements live
in a clearly named disclosure and remain design-speed diagnostics.

The top shell now has File, Edit, View, Preferences and Window/Layout menus, a
grouped toolbar, workspace switching and project/dirty status. The developer path
entry field is removed. Full path remains in File and project-name hover text.
New uses the existing unsaved guard. Open uses a native Windows picker. Save uses
the current path or prompts for an untitled file; Save As has Ctrl+Shift+S. Dialogs
run on a single-flight worker, with an editor modal while pending. Completion is
polled without blocking rendering; opening still invokes the unsaved guard.
Cancel/disconnection preserve the document. Save As changes the current path and
saved snapshot only after existing validated, recoverable persistence succeeds.
Project schemas, solver and saved-layout version remain unchanged.

The viewport retains the existing renderer/camera and selected-section display.
Fit All and Fit Selection use Carbon icons; perspective and navigation commands
remain discoverable with gesture help. Small panes use compact labeled/tooltipped
icons and a View menu so the toolbar cannot consume their full height. F is routed
only over its graph/viewport, avoiding competing fit commands. Reference paths,
ground grid and diagnostic vectors remain separate toggles in View.

Support Settings, Appearance, View and Diagnostics remain in the right dock group.
No support constructor was fabricated. Train retains Configuration, Preview and
Summary with the same heartline draft, nominal train ID, one document/history,
camera/layout state and consistent font/icon presentation. Summary distinguishes
committed settings from pending drafts; no train mesh or articulated dynamics was
introduced. Welcome, Library, Simulator, audio and environment systems are outside
this pass.

## Modules and assets

| File under `apps/quantum-x/src/authoring/` | Responsibility |
| --- | --- |
| `commands.rs` | Desktop menus/toolbar/status and existing command dispatch |
| `files.rs` | Single-flight native dialogs and completion/error/cancel state |
| `timeline.rs` | Shared section cards, sequence and authoritative ordering |
| `graph_canvas.rs` | Reversible value/boundary gestures and graph view navigation |
| `graph.rs` | Channel/transition selection, precision tools and derived plots |
| `theme.rs` | Font registration, scale-aware styles and icon loader installation |
| `icons.rs` | Semantic embedded SVG registry and accessible icon buttons |
| `panels.rs`, `session.rs`, `layout_ui.rs` | Contextual composition and shared draft/file/view integration |
| `plot.rs`, `m5_tests.rs` | f64 range operations and targeted regressions |
| `mod.rs` | Existing editor adapter; optional offscreen section/channel selection |

`apps/quantum-x/src/main.rs` extends render selection without changing previous
CLI forms. `crates/rendering/src/offscreen.rs` settles font/nested layout over four
passes before capture, preventing clipped command rows. GPU pipeline and window
lifecycle are unchanged. Cargo manifests/lock, README, UX direction and these
reports document integration. `workbench.rs`, `window.rs` and other M4 edits were
already present; they are not new M5 functionality.

Seventeen curated runtime icons are in `assets/editor-icons/`; the full source
trees in `assets/icons/` are untouched. Lucide uses its supplied ISC/inherited
notices; Carbon uses Apache-2.0. Both complete license files and a source/prepared
SHA-256 manifest are retained. White-mask preparation changes tint only, not path
geometry. Stable semantic registry entries use embedded bytes, cached SVG decode
and DPI-sized textures. No unrelated icon library or emoji command substitutes
were introduced. See that directory's README for distribution notices.

The initial pass incorrectly chose Red Hat Mono for engineering fields and left
ordinary text proportional. The owner's correction replaces that integration with
unmodified Overpass Mono Regular and its SIL OFL 1.1 under
`assets/fonts/OverpassMono/`, primary for both text families and all controls.
Existing Unicode fallbacks, logical sizing, UI zoom and native DPI remain.

Dependencies: egui_extras 0.37 (MIT OR Apache-2.0), with **only SVG** loading enabled,
and Windows-only rfd 0.17.2 (MIT), using Windows native dialogs without GTK/portal
dependencies. SVG support adds resvg/usvg and parsing/raster dependencies. Cargo
metadata license review found the 19 newly locked packages MIT, MIT OR Apache-2.0,
or Apache-2.0 OR MIT. No HTTP loader, network service, telemetry or proprietary
dependency was added. The font uses its separate OFL license.

## Automated verification

The following passed on native Windows:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

The workspace suite passed 87 tests. App tests now include synchronized actual
pointer selection in both navigators, before/after insertion and ordering history,
draft guards, value-drag staging/Escape, boundary-drag one-step undo, Ctrl+wheel and
middle-pan view-only behavior, range mathematics, invalid operations, file dialog
completion/cancel/disconnection with injected results, Unicode Save As roundtrip,
failed-save identity retention, icon registry uniqueness/SVG decode at 16/24/32
pixels, and 960x640 equivalent native sizes at UI scales 0.75/1/1.25/1.5/2.
Existing workspace switching, independent layout/floating/persistence, prior
spline/project fixtures and numerical/undo suites continue to pass. Tests inject
egui input and dialog results; they never open native dialogs or control Windows.

Existing independent numerical exports/verifiers also passed:

```powershell
target\release\quantum-x.exe examples target/m5-examples
python tools/verify-authoring-exports.py target/m5-examples
target\release\quantum-x.exe heartline-demo target/m5-heartline
python tools/verify-heartline-exports.py target/m5-heartline
```

The analytic-reference total-length error was approximately 1.23e-11 m. The
heartline analytic circle radius error was 8.11e-12 m and normal-load error
2.04e-10 g. These are fixture results, not general solver error bounds or physical
ride validation. Existing intentional fixture discontinuity diagnostics remain.

## Offscreen visual checks

```powershell
target\release\quantum-x.exe render-editor examples/mixed-authoring.qx.json target/m5-track-1440.png track 1440 960 2 normal
target\release\quantum-x.exe render-editor examples/mixed-authoring.qx.json target/m5-track-960.png track 960 640 2 normal
target\release\quantum-x.exe render-editor examples/heartline-reference.qx.json target/m5-train-1440.png train 1440 960
target\release\quantum-x.exe render-editor examples/heartline-reference.qx.json target/m5-train-960.png train 960 640
```

All four actual egui/wgpu editor captures were generated and inspected from files
on NVIDIA GeForce RTX 4070, Vulkan, driver 617.42. No native windows or desktop
input were used. They show the approved spatial structure, section sequence,
support location, shared styling and compact small-window camera toolbar.
Small panes intentionally scroll to precision/property controls. Optional
`section-id channel` chooses an existing supported authored channel and fits its
graph for verification; it never changes the project or layout. Supported names
are roll, normal, lateral, pitch and yaw. Unsupported IDs/channels fail explicitly.

Offscreen visual success is not proof of intuitive editing, native docking/pointer
correctness, dialog focus, accessibility, mixed-monitor DPI behavior or performance.
No 60 FPS measurement or polished/final UX claim is made.

## Owner-executable Windows acceptance (not manually verified)

```powershell
$env:QX_WORKBENCH_PREFS = Join-Path (Get-Location) 'target\m5-manual-workbench.json'
cargo run -p quantum-x --release -- edit examples/mixed-authoring.qx.json
```

1. Confirm Track left, viewport center, Support right and Transition Editor below.
   Resize splits; drag/group tabs; float and dock authoring panes using their
   context menus. Reset current layout. Confirm geometry/history do not change.
2. Scroll the Section List and horizontal timeline. Select from each, including a
   truncated name, and confirm the same section/3D highlight. Use Insert Before
   and After with known types, Earlier/Later and Remove; undo each action once.
3. Stage a numeric section change. Selection/order commands must guard the draft.
   Apply with Ctrl+Enter, or cancel with Esc. Visit each context tab; verify exact
   values, old spline controls and metadata/reference settings remain reachable.
4. Select a force-vector section and Normal force. Hover to inspect units/values;
   drag each square at a shared join, including outside the graph, and release.
   Clean drags must undo in one step. Stage a name/value first, drag, then verify
   it remains a draft. Esc during that drag restores the preceding draft.
5. Move an internal diamond; verify only its two neighboring durations change and
   total section extent is preserved. Enter the same boundary numerically.
   Try crossing its neighbor: check the explanation and retained valid data.
6. Open Precision Editor. Test endpoint values, amplitude, supported functions,
   Hermite slopes, transition ordering, split/remove warnings, Apply and Cancel.
   Derived diagnostics must stay distinct from authored force/rate channels.
7. Ctrl+wheel zooms about the graph pointer; middle drag pans. View range accepts
   exact bounds; invalid bounds show recovery. F over graph fits graph; F over 3D
   fits track. Plot gestures and splitters must never move the 3D camera.
8. Orbit/pan/wheel in the viewport; test Fit All, Fit Selection, perspective and
   orthographic views. Toggle overlays in View; confirm vectors are diagnostics.
9. Ctrl+2 opens Configuration/Preview/Summary. Change nominal ID/reference offset,
   Apply, Ctrl+Z/Y. Leave a draft, switch twice with Ctrl+1/2, and check document,
   draft, selection, independent layout and camera/overlay state preservation.
10. File → Open and Ctrl+O must show Windows file pickers. Cancel without changes.
    Open a second file with unsaved work and test Keep/Discard. Save As to a new
    Unicode-named path; Ctrl+S updates that file; reopen and compare values.
    Test existing-file replacement, an unavailable/read-only destination and an
    invalid project. Check document and prior file identity survive failures.
11. New on a dirty project must warn. Save an untitled project: it must prompt.
    Close with dirty data/drafts: verify the existing explicit discard warning.
    Verify native dialogs foreground correctly and do not permit concurrent edits.
12. Test 1440x960 and 960x640, UI scale 0.75/1/1.25/1.5/2 and native Windows DPI.
    Check text, handle targeting, compact camera tools and scrolling to all actions.
    Test Automatic/Custom/Remember, maximize/minimize/restore and mixed monitors;
    reopen after monitor removal and verify recovery and saved independent layouts.
13. Confirm Support Settings/Appearance remain right; View/Diagnostics tabs work;
    unavailable construction tools have no fake enabled operations. Compare the
    whole layout against the original Inkscape drawing and review visual choices.

Close the app, then `Remove-Item Env:QX_WORKBENCH_PREFS` in that PowerShell session
to return to the normal preference location. Use Save As during testing to avoid
overwriting bundled examples.

## Limits and UX status

Implemented owner-directed organization, numeric authoring, native file commands,
icon/font sources and real editing semantics are listed above. Final colors,
spacing, card/handle treatment and precision disclosure behavior remain subject
to owner visual/interaction approval, as recorded in `docs/ux/editor-m5.md`.
No new developer-only screen was added; the existing concept catalog, reference
renderer and nominal train tooling remain honestly limited development features.

Direct drag-to-reorder sections was deliberately not added; explicit undoable
Earlier/Later commands serve both navigators. Fit uses finite display sampling,
not a guarantee that every analytic extremum lies in view. Very small/high-scale
panes require scrolling/resizing; layout persistence does not save focus/scroll.
rfd does not distinguish native dialog cancellation from native initialization
failure, so both preserve the document and report no selected file. Native Windows
dialogs are unparented and must be manually checked for focus/z-order on this host.
Non-Windows builds show an explicit native-dialog availability message; native
Linux/macOS behavior was not verified. Existing synchronous project load/save I/O
and full generation/snapshot costs remain, without a new performance claim.

No support model, mechanism engine, train model/dynamics, welcome/library or
simulator functionality is advertised as implemented by M5. Production UX
acceptance remains the owner's judgment after the manual checks.
