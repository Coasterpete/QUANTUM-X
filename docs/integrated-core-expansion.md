# Integrated core expansion — 2026-11-01

Implemented locally in the existing Rust 2024 / wgpu / egui Editor. Existing
uncommitted work, owner colors, layouts, train mechanics, cameras, PBR materials,
Blender/GLB assets and authoring representations are retained. No dependencies,
commits, pushes, PRs, merges or publication were introduced.

**Implemented and integrated**

- Track Workspace → **Operations** selects Open, Shuttle or Circuit intent and
  reports endpoint position, tangent and complete-frame residuals. Circuit
  guidance requires explicit Circuit intent and verified geometric closure:
  ≤0.001 m position and ≤0.001 rad tangent/frame, including internal pose joins.
  A visually close open track never implicitly wraps.
- **Connect endpoints → Preview connecting section** runs a bounded worker
  search (41 tangent-handle candidates) with preferred length, allowed length
  deviation and sampled minimum radius. A successful candidate previews an
  additional local cubic section and explicit roll correction. Apply commits it
  atomically; Cancel removes it. Failure and stale results retain authored data.
  Existing sections are never fitted or moved.
- **Engineering plots** in Operations (also in graph-view/tools menus) switches
  the existing Transition pane to one selectable diagnostic plot. Elevation,
  curvature, finite radius, bank, pitch, heading, authored speed, three specific
  G components, kinematic acceleration and train-mass-scaled forces share the
  viewport's generated measurement rows. Whole-track/selected-section scope,
  hover inspection and pinned stations link to the 3D marker. Undefined radius
  and bank values are gaps. Wrapped angles are explicitly labeled. Speed is
  constant within each design section; speed jumps are not physical acceleration
  measurements. Experimental playback telemetry is separately labeled.
- Launches and brakes are saved authored regions with stable IDs, enabled state,
  half-open station intervals, direction, acceleration magnitude and a total
  device force limit. Optional editable linear profiles use local metres and
  require explicit profile fitting after interval resizing. Seam-crossing regions
  require valid circuits. Devices are independent of section boundaries; new
  devices initially use the selected section's range.
- Experimental train playback applies dimensioned `m*a` longitudinal forces to
  independent component mass centers. Force limits are shared across occupied
  masses; brakes oppose actual travel, including rollback. Passive braking and
  resistance are limited against numerical reversal. Per-device command,
  occupancy, requested/applied force and force-limit telemetry accompany measured
  train velocity, traveled distance, elapsed time, acceleration, mass-averaged
  specific G and existing constraint reactions. Settings persist initial velocity,
  gravity, rolling coefficient, air density and total train CdA. The model uses
  `Crr*m*g + 0.5*rho*CdA*v²`, with zero resistance by default. No velocities are
  prescribed by devices. Outer integration remains exactly 1/240 s, with existing
  internal substeps. Failed steps retain bodies, hints, time and telemetry.
- Support Workspace creates/deletes structures and editable world nodes,
  section-relative track mounts and foundations. Circular/rectangular members
  connect named nodes with editable dimensions and axial roll. Existing PBR
  rendering draws capped solids with independent support visibility and
  selectable silhouettes. Viewport clicks select actual member triangles;
  Fit Selection frames their bounds. Node inspection shows a 3D marker.
- Traditional timber bents and hybrid timber lattice generate ordinary editable
  nodes/members. Range, maximum spacing, width, foundation elevation and timber
  dimensions are configurable. Mounts follow the regenerated section's physical
  track; following foundations retain their world elevation. Rider-authored
  projects use the existing derived physical-track inverse for support attachment.
  Explicit recipe replacement stays a draft until Apply; ordinary regeneration
  preserves individual member edits.
- Schema **7** adds operations/settings and supports without rewriting existing
  representations. Schemas 1–6 migrate explicitly; existing authored geometry,
  transitions, trains and construction remain intact. Unsupported fields/variants,
  invalid references, dimensions, profiles and resource counts are rejected.
  Apply All & Save, unsaved-work guards, atomic file replacement and document
  undo/redo include the new edits. Runtime poses and telemetry remain transient.

Core modules: `qx-geometry::{topology,engineering,supports}`, document
`operations`, physics `devices`/`playback`/`solver`, and Editor
`expansion`, `operations_ui`, `engineering_ui`, `support_ui`. Rendering uses the
existing mesh/material pipeline and camera contracts. The C++ topology,
completion, devices, supports and timber systems, plus C# EngineeringSnapshot,
were inspected as conceptual references; this pass independently implements the
algorithms in the Rust architecture.

**Running and evidence**

Portable Windows package:
`target/distribution/QUANTUM Dynamics-integrated-core-20261101/QUANTUM Dynamics.exe`.
Keep its adjacent assets directory. Open a project, author sections, inspect plots,
classify/complete the layout, author devices, apply physical settings, associate
a train, initialize Experimental physics playback and choose Rider POV or Chase.
Build supports in the existing Support pane, Apply, save and reopen.

Actual offscreen Editor captures, the reopenable numerical fixture, 1,440-step
physics CSV, GPU metadata and independent evidence verification are under
`target/integrated-core-evidence-20261101-verified/`:

1. `01-engineering-plots.png` — shared design-speed plot and actual track.
2. `02-closure-diagnostics.png` / `03-completed-circuit.png` — before/after closure.
3. `04-authored-support.png` — editable circular/rectangular portal.
4. `05-procedural-timber.png` — editable hybrid lattice (159 members).
5. `06-launch-chase.png`, `07-launch-rider.png`, `08-brake-chase.png` — frozen
   genuine experimental-physics states, measured telemetry and attached cameras.

These are real Session/egui/wgpu draws, without desktop control, retouching or
synthetic interface images. The layout and gray train are explicitly numerical
development fixtures, not an owner-designed ride or finished artwork.

Reproduce noninteractively with a fresh output directory:

```powershell
cargo run --release -p quantum-x -- integrated-core-scenario target/new-core-evidence
python tools/verify-integrated-core.py target/new-core-evidence
& tools/package-windows.ps1 -OutputDirectory 'target/distribution/new-core-package'
```

**Limits and UX status**

Completion solves G1 position/tangent/frame closure within the stated numerical
tolerances. It does not guarantee G2 curvature continuity, obstacle clearance,
ride quality or a feasible answer for every endpoint arrangement. The radius
bound is checked on generated samples. Impossible bounded cases fail explicitly.
Shuttle classification does not add automatic endpoint reversal or a station
controller. Initial train assembly still needs a station at which the existing
bounded assembly solver can place the whole consist.

Devices are ideal distributed longitudinal actuators sampled at 240 Hz, not
electromagnetic/traction/contact models. Resistance has no static-friction or
weather model. Existing experimental bilateral guides, joint stabilization and
underconstrained rotations can produce behavior unsuitable for a production
vehicle; component/constraint measurements expose this. A complete wheel/rail
solver, lifts, stations, blocks, interlocks and cables are not implemented here.
Timber recipes are simple bents/connected lattice, not the older project's full
structural catalog. No support load analysis, terrain solver or safety validation
is claimed. Foundations are authored nodes; detailed footing hardware is future work.

The requested workflows and existing colorful workspace arrangement are followed.
Exact new control placement/grouping is provisional and reviewable, not final
owner-approved visual design. Experimental simulation controls and gray assets
remain labeled development content. No separate frontend was added.

Visible desktop interaction, subjective ride comfort, native FPS and broad
hardware performance were **not manually verified**. Owner acceptance checklist:
open the evidence project; hover/click plots; preview/cancel/apply completion on
an open copy; edit and undo a device/profile; initialize playback with nonzero
initial speed; play/pause/reset in POV and Chase; create/select/edit/delete support
members; generate both timber families; save/reopen and undo/redo. Test normal
window resizing and preferred DPI/layout on the owner's desktop.

**Verified**

- `cargo fmt --all -- --check` — passed.
- `cargo check --workspace` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `cargo test --workspace` — **326 passed, 0 failed, 0 ignored**, including
  doctests. Full output: `target/integrated-core-tests.log`. Seventeen new tests
  cover topology/frame mismatches, bounded/failed closure, shared plot references,
  device forces/limits/profiles/partial occupancy/reversal/seams/rollback,
  support transforms/regeneration and rider-reference attachments, migration,
  history/save/reopen, actual Editor controls, synchronized graph markers,
  powered/unpowered cameras, and solid-support picking/framing. Existing PBR,
  GLB, original construction, train assembly and authoring regressions pass.
- `cargo build --release --workspace` — passed on native Windows.
- `integrated-core-scenario` and `python tools/verify-integrated-core.py` —
  passed: eight authentic 2000×1400 Editor captures, distinct attached POV/Chase
  views of identical body poses, 1,440 fixed physics steps, save/reopen and
  completion undo/redo. Geometric closure residual: 3.56e-15 m position and
  3.47e-18 rad tangent/frame. Launch snapshot: 4.920 m/s at 1 s with 4,800 N;
  braking snapshot: 8.039 m/s at 5 s with 7,200 N. These are experimental model
  measurements; body rotation and constraint behavior have not been concealed.
- Portable packaging, icon resources, product identity, startup, five legacy
  samples and explicit opening of the schema-7 integrated project — passed.
  All **77** copied runtime asset files, including **9 GLBs**, match their source
  hashes. Package verification JSON files remain beside the executable.
- `git diff --check` — passed. Working branch remains
  `codex/m1-track-pbr-materials`; all existing and new changes remain local,
  unstaged and uncommitted. Nothing was pushed or published.

