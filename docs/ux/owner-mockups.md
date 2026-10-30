# Owner-designed Welcome and Editor UI

The owner's original Inkscape drawings (`bitmap.png` and `drawing.png`, also
supplied as photographs) are the primary composition and identity references.
The 2026-10-29 implementation request approves translating those references into
the native Rust/egui application. It supersedes restrained-accent defaults in
older documents and AGENTS.md. It does not authorize invented coaster layouts,
production assets, support generators, or ride operations.

## Implemented direction

- Welcome uses black, a centered lime **QUANTUM** / blue **DYNAMICS** wordmark,
  large violet Library, blue Editor, green Info, red News and orange Setup
  navigation. Hover and keyboard focus add a white inner border. Navigation
  reflows to two columns at small effective widths; short windows scroll.
- Editor without a project offers New/Open and Recent Projects. No document is
  created by navigation. New Project still asks only for its name. Description,
  family and numbered style remain inside Editor → Coaster Properties.
- Setup is application configuration, with staged window sizing, maximized state
  and UI scale. Apply uses the existing window command/preferences service;
  Cancel/back leaves unapplied configuration unchanged. Existing project setup,
  file dialogs, recent recovery, templates, draft guards and save behavior remain.
- Info describes actual capabilities. News explicitly reports its unavailable
  state without a network connection or fabricated articles. Library retains
  installed samples/templates and catalog inspection, including honest empty
  states and development-fixture labels.
- Track stays left, the viewport central, Support right and Transition Editor
  below. Train retains Configuration / Preview / Summary. Red workspace headings,
  color bars, distinct command groups and larger controls follow the references.
  Overpass Mono and Lucide/Carbon remain embedded; the executable icon is untouched.
- Graph colors follow the drawing: normal force blue, roll rate red, lateral
  force green, pitch rate violet and yaw rate yellow. Existing mathematical names,
  units, functions, exact inputs and gesture semantics are unchanged. Blue and
  violet use brighter variants of their reference hues for readable small text.
- Default body/button text grows from 13 to 15 logical points; small text from
  11 to 13; standard controls from 26 to 32 points high. Engineering graph tools
  retain a compact 24-point row with full-size text. Numeric field labels and
  inputs stay together in grids; scientific axis labels fit the reserved gutter.
- The initial Track split gives 56% of dock height to the upper workspaces and
  44% to transition authoring. This provides room for real graph controls beyond
  the conceptual drawing. Existing saved layout trees are neither migrated nor
  reset. Docking, in-window floating panes, window sizing and Ctrl+1/Ctrl+2 remain.

## Normal authoring versus optional development tools

`View → Development tools` is a session-only opt-in, outside the project format
and saved layout schema. It exposes train fixtures/mechanics, experimental
rigid-body playback, engineering markers and detailed import controls. Toggling
it does not alter the document, train poses, assets or undo history. An already
active playback remains clearly indicated with a way to reopen its controls.

Constraint residuals, reference details, generation timings and library internals
remain available through optional disclosures/Diagnostics. Normal Train shows
model assignment, visual transforms, camera tools and inspection station entry.
Missing-asset errors, invalid inputs, draft state and save failures remain visible.
Generation status uses ordinary progress text; timing/sample counts remain in
Diagnostics. Navigation resets survive egui's multiple layout passes.

Support explicitly reports that construction is unavailable. No fake support,
prefab, clearance, station, launch, brake or test-operation button was introduced.
The existing simplified physics foundation remains accurately identified as
experimental rigid-body playback, never a finished train simulator.

## Responsive limits and remaining owner design

Short Transition docks use a colored channel selector and Graph tools menus that
retain precise values, transition selection, exact view bounds, measurements and
section commands. At extremely small effective sizes (notably 960×640 at 200%
UI zoom), a dock too short for a useful graph explicitly asks to resize or float
it. Normal 960×640 and 1440×960 layouts show the graph. No unreadable miniature
plot or clipped editing controls are substituted for usable authoring space.

Exact palette shades, proportions and spacing are conservative implementation
choices within the approved direction, and remain reviewable. Future News
content, production library previews, support-authoring interactions and final
experimental-train workflows still need owner design and functional development.
No multi-coaster manager or new project schema is introduced.

## Reproducible verification

The original screenshots in `target/editor-workflow-final` provide the before
comparison. Fresh captures must come from the updated executable, never from
those older files or the drawings. `ui-reference-scenario` renders the real App,
workspaces, document geometry and loaded GLBs through wgpu/egui, without creating
a native window or touching the active desktop. Its evidence checks that the
input document and file bytes remain unchanged. Personal recents/preferences
are excluded from diagnostic captures; native startup retains them.

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --workspace --locked
./tools/package-windows.ps1 -OutputDirectory 'C:/Temp/QUANTUM Dynamics-owner-ui'
./tools/verify-owner-ui.ps1 -Executable 'C:/Temp/QUANTUM Dynamics-owner-ui/quantum-x.exe' -OutputDirectory 'C:/Temp/qx-owner-ui-verification'
```

The verification script runs from outside the checkout. It includes existing
M6 authoring/numerical, M7 construction, M8.1 GLB/transform and M8.2 playback
verifiers; startup/Coaster Properties round trips; the physics diagnostic; and
fresh Welcome, Editor and Train captures at 1440×960 and 960×640 through
100/150/200% UI scaling. Packaging compares the original ICO payload with the
source PE and both packaged executables. UI verification checks image pixels,
palette presence, image dimensions and bounded viewports; visual inspection
is still required to assess presentation.

Native interaction is **not manually verified**. Owner acceptance checklist:

1. Launch the packaged public executable. Check Welcome composition, Tab focus,
   navigation feedback, resizing and Windows monitor DPI.
2. Visit all five destinations. Cancel Setup changes; apply a different UI scale.
   Choose Editor, cancel New and Open, then create a name-only project.
3. Edit sections/graphs and Coaster Properties, apply/cancel, undo/redo, save and
   reopen. Return to Welcome with unapplied drafts and exercise the save guard.
4. Drag dock dividers/tabs, float/redock the graph and restart to check preferences.
   At large UI scales enlarge a short graph pane using these existing controls.
5. Open a project with train GLBs. Assign a model, edit/apply/cancel its transform,
   inspect stations, and switch optional Development tools on and off. Verify
   missing-model feedback. Experimental playback remains explicitly opt-in.

Results and exact deliverable paths are recorded in `docs/owner-ui-report.md`.
