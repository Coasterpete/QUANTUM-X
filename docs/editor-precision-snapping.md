# Integrated Region authoring and precision snapping

Owner-directed improvement within the existing Track / viewport / Support /
Transition Editor arrangement. Colors, Overpass Mono, icons, Train Workspace,
domain math and project serialization remain unchanged. No developer fixture or
new provisional screen was introduced. Final native visual acceptance remains
with the owner; no desktop automation or graphical Editor launch was performed.

## Authoring

- The Region List offers immediate **Before / After** insertion using the chosen
  type. **Insert Region…** offers Straight, Force-Based and Geometric; coordinate
  frames, circular arcs and the existing spline representations are secondary.
  Every type supports insertion on either side without converting existing data.
- Selected Region essentials precede starting-pose controls. Length, name,
  design-speed override, Apply and Cancel remain part of the Region draft.
  Navigator lengths explicitly say committed; draft notices and the focused
  resolution dialog show exact preview and committed extents, including units.
- Apply & Continue, Discard Draft & Continue and Keep Editing retain the existing
  transactional behavior, validation failures and separate insertion undo step.
- The graph identifies its Region, active curve, units and Draft / Committed
  state. Returning to a Region restores its last active curve and transition.
  Advanced NURBS shape/continuity controls are collapsed; direct graph handles,
  interpolation/easing choices and custom acceleration editing remain available.
- **Transitions → Entrance → plateau → exit** explicitly reshapes the selected
  transition into three connected C2 profiles. Drag the plateau bar vertically
  to set its exact constant level; the adjacent ramps retain their timing and
  outer boundary jets. Boundary diamonds transfer extent between neighbors.
  Transition length is also available numerically, including the final transition.
- Square points edit values and internal stations together. Circular shape
  handles edit scalar ordinates and easing timing: a monotone warp redistributes
  existing scalar NURBS knots, retaining multiplicities and both physical endpoint
  jets. Polynomial presets convert exactly only when a handle is edited. Viewing
  handles does not change data. Analytic cosine retains its representation and
  requires an explicit reshape before using these shape handles.
- Select a graph handle, then **Edit values** for its station/value coordinates.
  The ordinary Start/End fields retain their individual endpoint semantics;
  the selected plateau's value edits the entire hold. Solid colored drafts and
  dashed neutral committed curves remain distinguishable without relying on hue.
- Fit Region restores the full horizontal domain while retaining Y. Fit Curve
  fits the sampled curve; Reset View restores the unit-specific range. A full
  domain view follows length increases and decreases; deliberate zoom/pan is
  preserved and bounded after length changes. Views remain per Region/curve.

## Snap controls

**Snap: On / Off** is beside graph tools and Support controls. The adjacent
existing preferences icon and **Preferences → Snap Settings…** open the same
global settings window. Graph and spatial snapping have independent enable
switches; graph X/Y and point/handle/boundary categories are independent.
Snapping starts disabled for existing preference files.

- Graph X uses configurable metres (including a fine increment), seconds or
  normalized units. Y has independent increments for g, rad/m, rad/s, rad/u and
  m/s². Points, shape handles and boundaries use their respective existing snap
  categories on each supported axis. Outer endpoint stations stay fixed;
  transition boundaries retain horizontal-only editing and fixed total extent.
- Optional nearby-key attraction uses a configurable logical-pixel radius,
  converted through the mapping captured at press. Values use keys in the same
  curve; boundaries can align with other curves in the Region. The nearest
  eligible key wins when closer than the increment lattice. Context insertion
  also respects horizontal snapping.
- Existing numeric placement drags support world XYZ / foundation Y grid,
  Region-local spline coordinates, track-local offsets, station attachments,
  arc radius/plane and support member roll. Angle settings are in degrees and
  converted to authored radians. Nearby point/node alignment operates on the
  edited axis in the same frame; attachments retain stable Region fractions.
  No viewport gizmo or new modeling workflow is added.
- **Alt** temporarily bypasses snapping, including without another pointer move.
  **Shift** retains fine dragging and selects the configured fine distance step.
  Exact typed numeric input stays exact. Snapping acts on authored `f64` values,
  using raw gesture accumulators rather than repeated rounded deltas. It never
  rewrites unrelated axes, Regions or idle committed data. Invalid candidates
  retain the last valid draft and existing error recovery.
- Ctrl+wheel zooms both axes; Ctrl+Shift+wheel zooms X only and Ctrl+Alt+wheel Y
  only. Middle-button pan remains view-only. Major/minor grid lines and the
  crosshair report physical coordinates. All graph gestures remain staged on
  release, including gestures begun from a clean document. Only explicit Apply
  commits; Esc during capture restores the preceding draft.

Preferences use the existing atomic user store:
`%LOCALAPPDATA%\QUANTUM-X\workbench-v1.json` (or `QX_WORKBENCH_PREFS`). Older
files default the new fields; invalid increments cannot replace a valid file.
Window/layout reset retains snap preferences. Project schema and compatibility
are unaffected.

## Verification and owner check

Headless tests cover real egui point/handle/boundary drags, X/Y independence,
stationary bypass and enable changes, precision, graph view navigation, custom
acceleration units, support world/local/station/foundation controls, draft
resolution, insertion, history, preferences on disk and project round trips.
The 2026-11-03 refinement studied `C:/Dev8/openFVeiD` read-only, specifically
`core/subfunction.cpp`, `core/function.cpp`, `core/secgeometric.cpp`,
`ui/graphview.cpp` and `ui/transitionview.cpp`. Useful techniques were channel
semantics, normalized easing, explicit extents and authoring-triggered geometry
updates. No GPL source was transplanted; QUANTUM uses its own existing scalar
profiles, exact subdivision and constrained editing. The referenced screenshot
was not attached in this chat; native visual acceptance remains with the owner.

Verification passed: `cargo fmt --all -- --check`,
`cargo check --workspace --locked`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked` (368 tests, zero failures), and the single final
`cargo build --release -p quantum-x --locked`. The existing CLI integration tests
also passed explicitly with `cargo test -p quantum-x --test headless --locked --
--test-threads=1`; the final unsupported-axis snapping guard passed with
`cargo test -p quantum-x precision_ --locked`. Workspace/Release logs are in
`target/transition-refinement-tests.log` and `target/transition-refinement-release.log`.
The executable is `target/release/quantum-x.exe`. Changes remain local and
uncommitted on `codex/m1-track-pbr-materials`; nothing was pushed or published.

For owner verification, run `& .\target\release\quantum-x.exe edit` from the repository.
Insert each common type on either side, edit length and graph shapes, configure
increments, try Alt/Shift and zoom/pan, Apply/Cancel, undo/redo and save/reopen.
Check support coordinates and custom acceleration profiles, then restart to
confirm preferences. Native pointer comfort, final layout and visual appearance
have **not** been manually verified in this pass.
