# Editor workflow and Coaster Properties refinement

Authority: the owner's 2026-10-29 “QUANTUM Dynamics — Editor Workflow, Coaster
Properties & Professional UI Refinement” request. It supersedes older startup
setup requirements and restrained-accent defaults. Existing human-directed
workbench composition and Coaster Properties requirements remain in force.

Before: Welcome → New Project asked for author, straight length, design speed
and a planned style (defaulting to the first catalog entry) → authoring session.
After: **Welcome → New Project (name only) → Editor → Coaster Properties**.
Creation opens an unsaved project with one editable 10 m straight, the existing
20 m/s design reference speed, no train and no assigned catalog style. Save As
chooses location. Normal public-executable launch retains Welcome with no document;
developer `edit`, explicit-file launches and other CLI commands retain their routes.
Template copies preserve their authored geometry, configuration, description and
author while receiving a new identity and name. Cancel/failure retains the old
session, including drafts and history, until successful creation.

The complete application environment is **Editor**. Track Workspace and Train
Workspace remain its authoring contexts; Support Workspace retains its existing
limitations. Transition Editor names the graph tool. Simulator denotes the future
distinct simulation experience; existing physics controls remain accurately named
rigid-body playback. No Simulator navigation or premature coaster manager is added.
Public window titles now include Editor. Internal types and CLI names remain intact.
The label/help audit found no literal “Track Editor” name requiring compatibility
renames. Historical reports retain their original milestone evidence.

`authoring/coaster_properties.rs` provides **Edit → Coaster Properties**, usable
from either workspace, docking/focusing the existing Track pane. Its Coaster tab
also opens the panel. While viewing properties, the section list yields its space;
the Section context restores that list. Apply/Cancel are at the panel's top.
Narrow panes use a context selector instead of a tall stack of wrapped tabs.
Coaster/project name and description share existing single-coaster metadata;
author is an optional disclosure. Family browsing filters numbered styles without
implicitly assigning the first style. Selecting previews the proposed metadata;
Apply commits name/description/style together as one transaction. Cancel restores
only those drafts, preserving pending geometry and construction.

An **Unassigned** option clears style assignment. Planned, In Development and
Available statuses remain catalog-driven. Assignment records design intent and
never substitutes construction or models automatically. Descriptions, thumbnail
and large-preview slots, content availability, configured-train assessments and
catalog compatibility declarations are shown separately. All current catalog
styles are Planned and have no production previews. Missing/corrupt images show
unavailability, with no generated artwork. Detailed mechanics stay in Train
Workspace. Its compatibility display reports unknown when style is unassigned.

Construction settings and raw Library diagnostics remain optional, explicitly
development disclosures. Their existing visual-style application is disabled
until conflicting style/construction drafts are resolved. The normal properties
assignment never modifies sections, graph channels, initial conditions,
construction, heartline or trains. Existing Apply All & Save, history, close and
Welcome guards include style drafts and validate the entire candidate atomically.

At the time of this workflow pass the JSON schema remained **5** with the same
required `style` object. The subsequent [NURBS transition pass](transition-nurbs-regions.md)
introduced schema **6**, with explicit migrations from schemas 1–5. Exactly
`{"family_id":"","style_id":""}` represents deliberate nonassignment; partial
pairs and unknown IDs remain errors. Old assigned projects and schema 1–5
migration paths remain intact. Older application builds that require an assigned
catalog style will reject a newly unassigned project; assign a catalog style for
those older builds. No unknown data is discarded and no catalog sentinel is added.

`authoring/previews.rs` caches up to eight image jobs/textures, reads and decodes
PNG/JPEG off the interactive thread, limits input to 16 MiB, dimensions to 4096
per side and decoder allocation to 64 MiB, and reports failures. Manifest/catalog
paths use existing canonical containment checks. The app directly reuses the
MIT/Apache-2.0 `image` 0.25 PNG/JPEG dependency already used by qx-rendering; no new
package/version or network-backed image loader is introduced. Preview caches are
presentation state, outside project serialization and undo history.
Properties redraws validate just metadata/style fields; full document validation
runs on transaction acceptance. Header Apply/Cancel executes after text inputs
consume the native input batch.

Visual refinements retain Overpass Mono, Lucide/Carbon, original artwork and icon,
existing docking geometry, preferences and numeric editing. Near-black surfaces,
bright text, stronger hover/focus/selection borders, and distinct cyan, violet,
amber, green and coral functional colors replace the muted presentation. Labels,
units and selected states remain present alongside color. Raw developer notices
are removed from the main Welcome heading. These are reviewable implementations
of the approved colorful direction, not a finalized branding system. Exact palette,
spacing, control grouping, section-list disclosure and Welcome composition await
owner visual review. No new development-only screen is introduced.
The offscreen renderer now converts logical egui points and physical target pixels
at UI zoom boundaries, fixing cropped high-scale captures without changing native
camera controls, authored geometry or numerical algorithms.

The single-coaster model is unchanged. Recommendations for later independent
coasters in a shared environment are in [the separate roadmap note](multiple-coaster-roadmap.md).

## Verification

Native Windows/MSVC results, 2026-10-29:

| Command / check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | **243 passed**, zero failed/ignored |
| `cargo build --workspace --release --locked` | Passed |
| `./tools/package-windows.ps1 -OutputDirectory 'target/distribution/QUANTUM Dynamics-editor-workflow-verified-20261029'` | Passed; public Welcome/no document, direct edit/file routes, version, catalog, samples, original icon |
| Packaged `editor-workflow-scenario target/editor-workflow-final/workflow` and `python tools/verify-editor-workflow.py target/editor-workflow-final/workflow` | Passed; eight actual captures through 200% scaling, schema 5 round trips, unchanged geometry/construction/trains |
| Packaged `authoring-scenario target/editor-workflow-final/authoring` and `python tools/verify-m6-exports.py target/editor-workflow-final/authoring` | Passed; eight captures and independent numerical checks |
| Packaged `track-scenario target/editor-workflow-final/track` and `python tools/verify-m7-exports.py target/editor-workflow-final/track` | Passed; 14 fixtures, 40 actual captures, live edits, mesh/export/cache evidence |
| Packaged `train-asset-scenario target/editor-workflow-final/authoring/project.qx.json target/editor-workflow-final/train-assets` and `python tools/verify-m81-exports.py target/editor-workflow-final/train-assets` | Passed; original GLBs, transforms, pivots, history and file workflows |
| Packaged `train-playback-scenario target/editor-workflow-final/playback` and `python tools/verify-m82-exports.py target/editor-workflow-final/playback` | Passed; conventional, below-track and rotating-seat arrangements |
| Packaged `physics-diagnostic` | Passed; existing 240 Hz diagnostic, speed 2.899053227 m/s matching analytic result, max joint residual 1.178e-5 m |
| `git diff --check` | Passed |

The packaged scenarios were launched from outside the checkout with absolute
input/output paths. Artifacts live under `target/editor-workflow-final`. Full
workspace results are in `target/editor-workflow-tests-final.log`; package results
are in `target/editor-workflow-package-verified.log`. Individual scenario logs use
`target/editor-workflow-final-*.log`. Generated artifacts remain Git-ignored.
The portable folder contains both executable names, adjacent assets,
`startup-check.json`, `library-check.json`, `icon-check.json` and distribution
metadata. Icon resource 101 contains all seven approved source frames byte for
byte, and Windows decoded the checked sizes successfully.

Headless egui tests click actual Welcome/menu/property controls and exercise
Apply/Cancel, history, save/reopen, unassigned style, invalid candidates,
pending-geometry preservation and draft leave guards. Core tests cover partial/
unknown style rejection without history damage and unchanged generated geometry.
Preview tests decode a real synthetic PNG and reject malformed, over-dimensioned
and oversized inputs. Test images are numerical fixtures, not added catalog art.
Offscreen images were visually inspected at normal, small-window and 200% sizes;
native desktop interaction was **not manually verified**.

Representative captures: [Welcome](../target/editor-workflow-final/workflow/welcome.png),
[name-only creation](../target/editor-workflow-final/workflow/new-project.png),
[Coaster Properties](../target/editor-workflow-final/workflow/properties-assigned-1440.png),
[small window](../target/editor-workflow-final/workflow/properties-assigned-960.png),
[200% UI scale](../target/editor-workflow-final/workflow/properties-scale-2.png).

Verification found and fixed toolbar/context wrapping regressions and the
offscreen logical/physical pixel mismatch. No tests or acceptance thresholds were
weakened. An initial packaging build hit Windows' running-executable file lock;
the final build/package and scenario executions were separated and passed.
There is no measured FPS claim. At high scaling/small sizes, properties require
scrolling; native dialog behavior, continuous interaction, visual ergonomics and
final palette/layout acceptance still require the owner checklist below.

## Owner-executable manual checklist — not manually verified

1. Double-click the packaged `QUANTUM Dynamics.exe`. Expect Welcome, no open
   document, and the original icon in Explorer/title bar/taskbar.
2. New Project: enter a name, Cancel; repeat and Create. Expect Editor and no
   family/style prerequisite. Edit section length and a graph value while unassigned.
3. Edit → Coaster Properties: edit name/description; browse Hybrid and Monorail
   styles, inspect statuses and absent previews. Apply; undo/redo. Check geometry
   is unchanged by style assignment. Cancel a second set of properties edits.
4. Save As, close, reopen: verify name, description, style, geometry and train
   settings. Repeat with Unassigned. Open existing `.qx.json` projects.
5. Stage properties, graph and construction drafts. Save: Keep editing must retain
   them; Apply All & Save must write all once. Test invalid text/geometry, canceled
   Save As, failed destinations and Return to Welcome. No draft may vanish silently.
6. Switch Track/Train with Ctrl+1/2, float/dock Track, reopen Coaster Properties from
   Edit. Inspect compatibility, detailed train mechanics and GLB loading.
7. Resize and check 100%, 150%, 200% UI scaling, tab/keyboard focus, hover, selection,
   disabled states, scrolling, long descriptions and numeric text entry. Final
   visual usability and native dialog behavior need owner review.
8. Run `quantum-x.exe --help`, `edit`, `edit <project.qx.json>`, and the scenario
   verifiers. Simulator and multiple-coaster authoring remain future workflows.

All changes are local on the existing `codex/public-identity-library` branch,
preserving the substantial pre-existing working tree. No commit, push, merge or
PR is authorized by this request.
Remote state was fetched successfully with Git's Windows certificate backend
(`git -c http.sslBackend=schannel fetch origin`); repository settings were unchanged.
HEAD remains `8691591`. Nothing was committed, pushed, merged or published.
