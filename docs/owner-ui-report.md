# Owner-designed UI implementation and verification

Owner direction approved 2026-10-29; continued verification completed 2026-10-30.

The approved Welcome/Editor direction is implemented in the Rust application.
The primary references were the owner's original Inkscape drawings, not earlier
development screenshots. [The UX mapping](ux/owner-mockups.md) records the approved
behavior, conservative presentation choices and manual acceptance checklist.

## Deliverable

Public executable:

```text
C:\Dev8\QUANTUM-X\target\distribution\QUANTUM Dynamics-owner-ui-delivery-20261029\QUANTUM Dynamics.exe
```

Keep its adjacent `assets` directory. `quantum-x.exe` in the same folder retains
the development CLI. This is an unsigned portable Alpha development distribution,
not an installer or a claim of finished train/contact physics.

Executable SHA-256 (both packaged names):
`7BDDFA64F322DD7D7577BDAE40551D36E8D1BFE66DB2A2E8A49801274BD331DA`.

The original ICO's seven embedded images are byte-identical in both executables,
resource 101, including 16/24/32/48/64/128/256-pixel sizes. Source ICO SHA-256:
`C1B8F3571FCCA5A95A9D63ED20EB247159B91554395E345C6BE74862C5830A22`.
See [packaged icon evidence](../target/distribution/QUANTUM%20Dynamics-owner-ui-delivery-20261029/icon-check.json)
and [distribution metadata](../target/distribution/QUANTUM%20Dynamics-owner-ui-delivery-20261029/distribution.json).

## Functioning changes

- Centered two-line Welcome branding and five large, colorful destinations on
  black. Editor presents New/Open with no automatic document. New remains
  name-only; Coaster Properties remains inside Editor. Setup configures window/UI
  preferences. Info is factual, News explicitly unavailable, and Library retains
  existing installed content and empty states. Recents, cancellation and unsaved
  guards use the existing lifecycle services.
- Red Track/Support headings, violet Train headings, blue Transition identity,
  colored menus/commands and the reference graph colors. Overpass Mono, existing
  icons, actual meshes/GLBs, keyboard commands and camera navigation remain.
- Larger text/controls, aligned numeric fields, explicit focus/hover borders,
  selected-section reveal, compact command rows, and responsive graph tools.
  Saved docking layouts/preferences remain intact; changed defaults do not reset
  an owner's layout. Short docks preserve numerical tools in menus.
- Optional Development tools hold fixtures, experimental playback/mechanics and
  engineering markers. Constraint, import and solver details remain accessible.
  Normal errors, draft/apply status, save feedback and missing-asset messages stay
  visible. No diagnostics were deleted or fake generators/ride operations added.
- Presentation stays separate from documents, geometry and physics. No schema,
  section representation, force-channel meaning, fixed timestep, asset contents,
  project identity, undo boundary or one-coaster-per-project architecture changed.

Primary modules: `startup_ui`, new `startup_pages`, `startup`, `theme`, `commands`,
`layout_ui`, `panels`, `graph`, `graph_canvas`, `plot`, `timeline`, `train_asset_ui`
and `session`. Renderer changes keep offscreen defaults consistent with native
defaults and independently gate inspection markers. New `ui_reference_scenario`
and verification scripts provide reproducible GPU evidence. No dependency added.

## Before / after visual comparison

| Area | Before | Updated implementation |
| --- | --- | --- |
| Welcome | Small upper-left heading/action row and a largely empty page | Centered lime/blue wordmark, violet/blue/green/red destinations, orange Setup, accessible project actions and recents |
| Editor | Small workspace tabs, mostly gray commands, weak hierarchy | Large red workspace headings, vivid command and graph colors, stronger selected sections, clearer numeric/property controls |
| Train | Fixture creation, import details, experimental playback and residuals mixed into the primary view | Model assignment/transforms and station inspection first; experimental tools, markers and detailed diagnostics optional |
| Smaller windows | Dense labels and cramped controls | Wrapped destinations, shorter labels, truncated selectors with full dropdown choices, compact menus and a clear minimum graph-height state |

Before examples are the **older captures**, included only for comparison:
[Welcome](../target/editor-workflow-final/workflow/welcome.png),
[Editor](../target/editor-workflow-final/track/live-single-after-1440.png),
[Train](../target/editor-workflow-final/train-assets/station-a-1440x960.png).
Fixture geometry and framing vary between these examples; this comparison concerns
presentation, not a claim that any coaster layout was redesigned.

Fresh screenshots from the final packaged executable:

| Surface | 1440×960 / 100% | 960×640 / 100% |
| --- | --- | --- |
| Welcome | [Capture](../target/owner-ui-review-20261030/welcome-1440-scale-1.png) | [Capture](../target/owner-ui-review-20261030/welcome-960-scale-1.png) |
| Editor | [Capture](../target/owner-ui-review-20261030/editor-1440-scale-1.png) | [Capture](../target/owner-ui-review-20261030/editor-960-scale-1.png) |
| Train | [Capture](../target/owner-ui-review-20261030/train-1440-scale-1.png) | [Capture](../target/owner-ui-review-20261030/train-960-scale-1.png) |

The same three surfaces also have fresh captures at both dimensions and 150/200%
UI zoom. [Capture manifest](../target/owner-ui-review-20261030/evidence.json).
The 2026-10-30 capture refresh used the same byte-identical verified executable.
Formatting, workspace check, strict Clippy and Release freshness were rechecked;
the completed full test run remains 246/246 passing.

Additional captures cover Library, Editor choice, Info, News, Setup, New Project,
development Train and Diagnostics. They use real wgpu/egui UI and project data;
the track/train content is an existing numerical development fixture, not a
production coaster or production vehicle model.

## Verification

| Exact check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | 246 passed; zero failed/ignored; [full log](../target/owner-ui-tests-delivery.log) |
| `cargo build --release --workspace --locked` | Passed, native Windows |
| `tools/package-windows.ps1 -OutputDirectory 'target/distribution/QUANTUM Dynamics-owner-ui-delivery-20261029'` | Passed: public Welcome/no document, direct edit/file routes, installed catalog/samples, original icon |
| `tools/verify-owner-ui.ps1 -Executable '<distribution>/quantum-x.exe' -OutputDirectory 'target/owner-ui-delivery'` | Passed: all 13 packaged scenario/verifier steps; [checks.json](../target/owner-ui-delivery/checks.json) |

The packaged verification runs outside the checkout. It invokes the existing
M6 authoring/independent numerical verifier, M7 geometry/mesh verifier, M8.1
GLB/transform/save verifier, M8.2 physical-playback verifier, editor workflow
round-trip verifier and the 240 Hz physics diagnostic. It also renders and checks
26 fresh UI images, palette pixels, dimensions, scaling and viewport containment.
Logs live beside `checks.json`. Source/destination equality checks retain project
schemas, authored geometry, construction, trains and input file bytes.

New automated UI coverage checks actual destination actions and Setup isolation,
the absence of experimental controls/markers by default, toggling them without
document/history changes, and visibility of all five Welcome labels at small
window sizes/large scaling. Existing exact numerical assertions remain; UI tests
locate numeric fields by their visible row instead of obsolete fixed spacing.

## Limits and UX status

Native desktop interaction is **not manually verified**. GPU captures are real
offscreen application renders, not physical-window screenshots. Windows monitor
DPI changes, Explorer double-click interaction, native dialogs and manual docking
still need the owner-executable checklist in the UX mapping. UI zoom was tested
through 200%; no measured native FPS or rendering-performance claim is made.

At 960×640 with 150–200% UI zoom there is insufficient space for all full-size panes
and a useful graph simultaneously. The short graph pane clearly offers its tools
and asks to resize/float it; sidebar content scrolls. Welcome keeps all five
destinations visible and scrolls secondary project actions/recents when needed.

The owner-approved branding, palette direction, workspace placement and workflow
are implemented. No new developer-only screen is presented as final product UX;
existing experimental controls are explicitly optional. Exact colors/proportions
remain reviewable refinements. News content, final catalog previews, support
authoring and finished simulation interactions remain future owner-directed work.

## Git

Local changes only on `codex/public-identity-library`, preserving the substantial
incoming uncommitted work. No commit, push, PR, merge, branch deletion or destructive
cleanup was performed, as requested. A snapshot of the incoming UI and diff was
retained locally under `target/owner-ui-baseline` before editing.
