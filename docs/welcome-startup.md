# QUANTUM Dynamics welcome and project startup

The owner-approved Inkscape composition now replaces the initial provisional
Welcome presentation described below. See [the current UI mapping](ux/owner-mockups.md)
and [implementation evidence](owner-ui-report.md). The existing startup, document,
file and cancellation services described here remain in use.

The public Windows executable now starts at a dedicated Welcome Screen without
constructing an Untitled document or opening either editor workspace. The shell
is functional provisional development UI. The owner has approved the startup
requirements in this pass, not its final layout, proportions, thumbnails, palette
or branding composition. Existing Overpass Mono, egui/wgpu and the original
embedded icon are retained. No icon source, Blender file or generated icon
artwork was changed.

## Entry points

| Invocation | Behavior |
| --- | --- |
| `QUANTUM Dynamics.exe` without arguments | Welcome; no document, generation worker or editor session |
| `quantum-x.exe` without arguments | Existing development CLI help |
| Either executable with `edit` | Existing Untitled development editor |
| Either executable with `edit <project.qx.json>` | Validate and open that project directly |
| Either executable with `<project.qx.json>` | Validate and open that project directly |
| Existing CLI subcommands | Preserved, including diagnostics, exports and scenarios |

Invalid explicit projects recover to Welcome with an actionable error; no
replacement document is invented. A valid JSON parse alone does not enter the
editor: schema/catalog validation, numerical generation, reference geometry and
renderable track construction must succeed first. Existing schemas 1–5 retain
their existing migration/validation paths; no project format changed.

`authoring/startup.rs` owns lifecycle and optional `Session`. `startup_ui.rs`
composes the replaceable welcome/setup/library presentation. `project_library.rs`
discovers installed projects; `recent.rs` maintains separate application history.
The renderer accepts an empty `InspectionScene`, with no authored geometry, and
suppresses the 3D viewport on the welcome/setup pages. Editor Track/Train layout,
commands, physics and document ownership remain in their existing modules.

## Project actions

- **New Project / Ctrl+N:** asks only for the project name before allocating a
  document. A new project has an editable 10 m straight, a 20 m/s design reference
  speed and an explicitly unassigned style. Coaster name, description and catalog
  choices belong to **Edit → Coaster Properties** inside the Editor. Geometry and
  reference speed remain editable in the Track Workspace.
  Create validates the candidate and its scene, then enters the Editor as an
  unsaved project. Save/Save As chooses its file location. Cancel creates no file
  or document and returns to the previous project if invoked from the editor.
- **Open Project / Ctrl+O:** uses the same native Windows project picker as the
  editor. Cancel, a disconnected dialog worker, missing files, malformed data or
  numerical/construction failures retain the current application/document state.
  Only one native picker can be pending. A dropped project file uses the same
  validation and replacement guards; setup must finish/cancel before a dropped
  file can replace another project.
- **Library:** separates existing numerical sample projects, installed templates,
  and Track/Train catalog inspection. Missing folders show empty states. Nothing
  generates a sample ride or installs content automatically. Track/Train library
  inspection preserves the existing planned statuses; style application remains
  in the existing editor panels.
- **Recent Projects:** successful open/save promotes a path and project name;
  new unsaved documents do not create entries. Missing files are identified with
  Locate and Remove actions. Locate uses the native picker, validates the
  replacement first, and updates history only after success. Refresh rechecks
  files that were restored or moved while Welcome was visible.
- **File → Return to Welcome Screen:** clean documents return immediately.
  Unsaved changes and all existing draft categories offer Keep current project,
  Save & continue, or explicit Discard and continue. Save reuses the existing
  atomic Apply All & Save workflow. Invalid drafts, failed writes and canceled
  Save As never authorize leaving. Canceling draft resolution returns to editing.
  Returning to Welcome releases the editor document and derived worker state.

Editor New uses the same unsaved guard, but the previous session remains alive
through setup. Even after choosing Discard and continue, canceling setup or
failing validation retains that session, drafts and undo history. Only successful
creation replaces it. Closing the application while such setup is open still
guards the retained project's unsaved edits.

The initial straight is neutral editable geometry, not an authored coaster layout.
New projects do not select a planned catalog style. Generic development construction
is independent of catalog assignment. Template creation preserves existing catalog
assignment, author, description,
sections and mechanics, gives the copy a new project identity, leaves it unsaved,
and remembers source-relative GLBs for loading and Save As packaging. Template
source files are retained. Samples open as regular files; use Save As to keep a
separate copy.

## Content and preferences

Portable packages copy the five pre-existing `examples/*.qx.json` files into
`assets/projects/samples`. They are explicitly labeled numerical development
fixtures. `assets/projects/templates` is initially empty. Future template content
uses ordinary supported `.qx.json` projects and project-relative assets in that
directory. No template marketplace, installer, preview artwork or network service
is introduced. Scanning is nonrecursive, bounded to 256 projects per collection
and at most 1025 directory entries. Projects are validated when used.

Source checkouts also discover their existing `examples` directory. Portable
builds resolve content beside their executable and do not fall back to checkout
examples when package content is missing. `QX_ASSETS_ROOT` retains its existing
override semantics.

Workbench preferences remain at their existing path and version. Recent paths
use `%LOCALAPPDATA%/QUANTUM-X/recent-v1.json`, independently versioned and bounded
to 24 entries / 64 KiB. Writes use the existing atomic temporary-file replacement
helper on a serial worker and finish on normal application exit. A corrupt or
unsupported history file is retained, a warning is shown, and history becomes
session-only. `QX_RECENT_PROJECTS` overrides the history path. When
`QX_WORKBENCH_PREFS` is set for an isolated run, its sibling `.recent-v1.json` is
used unless explicitly overridden. No saved application preferences were migrated
or reset.

## Automated verification

The `--startup-check [arguments]` diagnostic uses the production launch resolver
and session initialization without creating a native window. It reports the
selected page and whether a document exists, and never writes history/preferences.
Packaging checks the public no-argument route, direct development editor, both
explicit-file routes, project-library count, embedded icon and existing catalog.
This diagnostic is evidence of routing/initial state, not a claimed physical
Explorer double-click or native file-dialog interaction.

Additional automated tests cover welcome controls, cancellation,
invalid files, draft guards, save failures, moved-file recovery, history restart,
template copy and source-relative GLB packaging. Pointer events in tests are sent
only to an offscreen egui context, never the owner's active desktop.

Final native Windows results, 2026-10-29:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --workspace` | 239 passed, zero failed; includes 12 startup/lifecycle tests |
| `cargo build --release --workspace` | Passed, native Windows/MSVC |
| `./tools/package-windows.ps1 -OutputDirectory 'target/distribution/QUANTUM Dynamics-welcome-final-20261029'` | Passed; public Welcome/no-document, direct edit/file routes, 5 samples, 0 templates, catalog, version and original embedded icon verified |
| Packaged `render-welcome target/welcome-verification/welcome.png` from outside the checkout | Passed; actual wgpu/egui controls inspected offscreen on NVIDIA RTX 4070 / Vulkan |
| Packaged `physics-diagnostic` | Passed, existing 240 Hz fixture |
| Packaged `authoring-scenario`, `track-scenario`, `train-asset-scenario`, `train-playback-scenario` | Passed from outside the checkout |
| `python tools/verify-m6-exports.py target/welcome-verification/authoring` | Passed |
| `python tools/verify-m7-exports.py target/welcome-verification/track` | Passed; 14 fixtures and 40 actual PNG captures |
| `python tools/verify-m81-exports.py target/welcome-verification/train-assets-m6` | Passed with the documented M6 acceptance input |
| `python tools/verify-m82-exports.py target/welcome-verification/train-playback` | Passed |

Artifacts are under `target/welcome-verification`, with the full test log at
`target/welcome-workspace-tests.log`. The final portable folder is
`target/distribution/QUANTUM Dynamics-welcome-final-20261029`; its
`startup-check.json` reports `page: welcome`, `document_open: false` and no project
name. The captured welcome image shows no Track/Train controls or viewport.

An additional train-asset scenario using `examples/foundation.qx.json` completed,
but its independent image verifier failed the existing minimum projected-width
assertion. That verifier's documented input is the M6 acceptance track. Repeating
with that input passed unchanged; no framing/physics tests were weakened. This
pass makes no claim of universal train-framing acceptance on every authored track.

Opening and creation currently validate synchronously at the explicit project
action boundary, preserving the existing bounded loader/solver and transactional
replacement behavior. Large valid projects can pause the UI during that action;
asynchronous loading/progress and performance measurements remain future work.
No project-generation work runs on ordinary welcome redraws.

No commit, push, PR or merge was performed. Work remains local on
`codex/public-identity-library`, preserving its pre-existing uncommitted identity,
library, icon, editor and physics changes.

## Owner-executable native checklist — not manually verified

No active-desktop Computer Use was performed. Run these against the portable
folder, preferably copied to a location outside the checkout:

```powershell
./tools/package-windows.ps1 -OutputDirectory 'C:/Temp/QUANTUM Dynamics-welcome-check'
& 'C:/Temp/QUANTUM Dynamics-welcome-check/QUANTUM Dynamics.exe' --startup-check
& 'C:/Temp/QUANTUM Dynamics-welcome-check/quantum-x.exe' edit
& 'C:/Temp/QUANTUM Dynamics-welcome-check/quantum-x.exe' edit 'C:/path/owner-project.qx.json'
```

1. Double-click **QUANTUM Dynamics.exe** in Explorer. Expect the Welcome title,
   original native icon and no Untitled/editor viewport. Check resizing, high DPI
   and keyboard focus on the shell.
2. Choose New Project, type a name, then Cancel. Expect Welcome and no file.
   Repeat and Create. Expect the named project in the Editor with an editable
   10 m straight and no assigned style. Edit → Coaster Properties must expose name,
   description, family and numbered style. Apply, undo/redo, Save As and reopen.
   Planned selections must not change the geometry or imply available assets.
3. Return to Welcome, Open Project, cancel the Windows picker. Expect no document.
   Try corrupt and nonexistent projects, then a valid saved project. Only the valid
   project should enter the editor.
4. In the editor, stage graph/numeric, metadata, construction and train visual
   drafts. File → Return to Welcome → Keep current project must retain all drafts.
   Repeat Save & continue → cancel draft resolution. Repeat with an invalid draft
   and with a write-denied save destination. Each must keep the project. Resolve
   valid drafts and Save; verify file contents and Welcome. Explicit Discard must
   also return to Welcome without writing.
5. From an unsaved editor choose New, then cancel setup after the guard. Expect
   the original project, drafts and history. Repeat while canceling Save As and
   while closing the application/setup window; no other project may be lost.
6. Reopen the app to inspect Recent Projects. Move a saved project externally,
   refresh, cancel Locate, then select the new valid path. Test a bad replacement
   and Remove. Expect no unwanted editor/document and preserved history on failure.
7. Browse Library. Expect the five existing development samples and no templates.
   Open a sample. In a separate package copy remove its sample files and verify the
   empty state. Install an owner-supplied template `.qx.json` in the templates
   directory, refresh and create a copy; verify the source file is unchanged.
8. Check direct `edit`, `edit <path>` and `<path>` launches under both executable
   names; check existing diagnostics/scenarios. No native file association or
   installer is part of this portable package.

Final welcome composition, sample/template thumbnails, richer creation controls
and library interaction refinements remain owner-directed design decisions.
