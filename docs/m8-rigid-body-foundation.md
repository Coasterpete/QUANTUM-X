# M8 investigation and experimental rigid-body foundation

**M8.2 supersedes historical playback limitations below:** see
[universal vehicle definitions and playback](m82-universal-vehicle-playback.md)
for the compatible extensions, integration, numerical boundaries and acceptance commands.

The current asset workflow is described in **M8.1: GLB assets and focused train
inspection (2026-10-28)** at the end of this document. Earlier sections are
historical implementation reports; their statements that GLB import, rendering
and train-focused framing are absent have been superseded. Physics playback
remains outside M8.1.

This is a partial M8 implementation, not a completed train workspace or a
validated coaster dynamics model. Run the noninteractive synthetic fixture with
`cargo run -p quantum-x -- physics-diagnostic`. It constructs two separate cars
and two separate bogies, joins them with ball joints, and guides each bogie on
an ideal straight inclined line. It reports speed against the analytical
frictionless slope result and maximum anchor and guide residuals. No project,
track geometry, asset or user-authored coaster is modified.

## Reference investigation

Inspected Coasterpete/QuantumCoasterWorksLib at commit
`a4ad4663d3b7ab8cf20748c9442fd19bb9d166ec`, including the requested M165 and
transform review documents, definitions, provider, internal samplers and
solvers, transform types, physics follower, editor session, preview and tests.
The review is historical: current `TrainPoseResult` and wheel aggregate return
copies/read-only collections, and the provider has been split into internal
collaborators. Its remaining assumptions must be assessed against current code.

* The consist definition uses one repeated geometry, spacing and bogie layout;
  it validates positive finite values and limits bogie spacing to car length.
  M165 commits a candidate only after validation, draws a 2D schematic and
  preserves separate workspace layouts. It has no train document, GLB import,
  3D train-on-track editor or physics controls.
* The car sampler evaluates center frames at fixed station offsets. The bogie
  solver samples two frames at center distance plus/minus half-spacing. The
  articulation solver places the car at their midpoint and builds a basis
  using the chord and averaged normals, with fallback axes. It is geometric
  placement, not a mechanical articulation joint. On a curve the sampled
  arc spacing differs from the rigid chord length.
* Wheel offsets are generated in pairs from parity; wheel frames/matrices
  inherit the bogie frame, and local offsets are separate. These are pose
  references, not simulated wheel contact or rotation. Frames use tangent,
  normal and binormal, with +X forward, +Y up and +Z lateral in the unrolled
  example; matrix conversion passes through float precision.
* The follower integrates scalar track distance and speed with gravity projected
  onto a sampled tangent, resistance and diagnostics. It does not establish
  independent rigid chassis, physical couplers, joints or wheel/rail contact.
  A debug coupler link is a visual marker, not evidence of a constrained joint.
  Existing tests cover finite inputs, track poses, geometry and deterministic
  snapshots; they do not establish multi-body dynamics.

Reused concepts: renderer-independent definitions, separate body and running
gear, local component references, explicit frame conventions and deterministic
validation. Improved here: arbitrary individually identified car/bogie
components, shared component references, f64 state, independent snapshots, and
real impulse-based ball joints. Replaced the fixed station-offset pose model in
the dynamic fixture with force-integrated world-space bodies. Deferred: the
reference's kinematic train placement API and schematic editor, until they can
be integrated with QUANTUM-X's own documented workspace and project model.
No C# code was copied.

## Numerical model and boundary

`qx-physics` uses SI units, right-handed +X forward/+Y up/+Z lateral, world
linear/angular velocity and body-local principal inertia. Mass properties must
be positive and satisfy the principal-moment triangle inequalities. The box
helper assumes a uniform cuboid centered at the center of mass. World-space
torque includes the gyroscopic term; force integration is semi-implicit Euler,
and orientation is advanced by a normalized exponential rotation. Applied
forces persist until the caller changes them. Outer steps are fixed at 1/240 s;
the caller chooses 1–32 equal substeps and 1–128 sequential impulse iterations.
The `Simulation` API supplies pause, single-step and reset without coupling
simulation time to a rendering clock. The solver uses a 0.2 Baumgarte bias per substep, which can add or remove
energy. Impulse-derived forces are outer-step averages, not continuous contact
measurements. Residuals are reported rather than asserted to be converged;
invalid configurations and nonfinite/singular numerical failures roll back the
entire step. Determinism means repeatability on the same build and platform,
not cross-platform bit-identical floating point.

Ball joints enforce anchor coincidence while allowing all three relative
rotations; arbitrary car/bogie and shared-bogie arrangements are representable.
Line guides are bilateral, infinite, fixed in world space and frictionless:
they constrain two point coordinates and allow sliding along the line. They
are a diagnostic approximation with no wheel, rail geometry, collision,
separation, rolling friction, flange or guide-wheel mechanics. Explicit
component sizes are metadata and do not participate in collision detection.
The current component IDs are in-memory references, not a project-file schema.

Not implemented: angular limits or hinge axes, track-frame/curved contact
geometry, wheel/guide contact solving, real track attachment, physical
resistance, pause/play/reset UI, GLB importing or rendering, document
persistence/undo, kinematic inspection, and integration into either workspace.
No hills/valleys or curved-track behavior is claimed. The synthetic line case
does not prove a complete connected coaster train. These are required before
M8 can be called complete. The existing Train Workspace correctly continues to
say train geometry and dynamics are unavailable.

## Curved-track continuation (partial, 2026-10-26)

`World::step_on_track` now optionally constrains independent rigid-body points
against the double-precision `GeneratedTrack` centerline. It uses the existing
ball-joint impulse solve, generated normal/lateral frames, and a locally projected
station hint for each guide. The station is derived from the body position, not a
prescribed motion variable. Reactions are average impulses divided by the fixed
outer step, appended after any straight-guide reactions. Track samples are not
modified. An attempted open-end escape, failed projection, or invalid input
rolls back both bodies and station hints. The local projection deliberately
rejects corrections over 2 m and may fail on highly curved, widely separated,
ambiguous or fast-moving configurations; it is not a general closest-point
solver. This point constraint is frictionless and bilateral, does not constrain
bogie orientation or model rail profiles, wheels, contact, detachment, braking or
collision. Discontinuities, excessive speed and mechanism overconstraint require
further study. The fixed-step bias remains energy-changing.

`inspection::inspect` evaluates explicit component reference offsets on the
same track frames, including bank and inversion. This is geometric inspection;
it does not solve rigid car/bogie attachment or imply that referenced bodies
are mechanically connected. Neither API is wired into the application workspace,
playback controller, renderer or project document.

The `track_guidance` tests cover a gravity-driven curved two-body ball-joint
fixture with separate guide points and deterministic replay, banked reference
inspection, endpoint rollback, and an inverted-loop point guide. In one debug
run the 100 kg loop fixture at 8 m/s and 10 m radius reported 639.320 N mean
reaction (ideal 640 N), 0.00006944 m maximum guide error, and 0.00000660
relative speed drift over one second. These are fixture observations, not
universal convergence, energy bounds, performance, or safety claims.

M8 remains incomplete: train consist/document model and migration, full
articulation configurations, track-relative acceleration, GLB import/rendering,
workspace integration and controls, history and save/reopen, complete train
inspection, independent end-to-end verifier, release benchmarks and owner
manual review have not been delivered. The workspace's unavailable label remains
accurate. Do not present this continuation as a completed M8 deliverable.

The historical C++ `CarPose.cpp` and `TrainPhysics.hpp` were also reviewed:
they include rigid-bogie station refinement, named front/rear bogie roles and
a generalized train dynamics/force-recovery path. Their geometric placement and
scalar generalized state must not be mistaken for this crate's independently
integrated rigid bodies. No reference implementation was copied; neither those
fixed role assumptions nor their contact-feasibility analysis is implemented
by the new idealized guide.

Verification for this continuation on Windows: `cargo fmt --all -- --check`,
`cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, and `cargo build --workspace --release` passed.
`cargo test -p qx-physics --release --test track_guidance -- --nocapture` also
passed and produced the loop observation above. The preserved Release-mode
`physics-diagnostic` reported 2.899053227 m/s against the same analytic value,
1.178e-5 m maximum joint error, and 5.551e-16 m maximum guide error in its
one-second inclined-line fixture. M6/M7 verifier scripts passed against existing
`target/m7-save-correction` exports, not newly generated M8 captures. No M8
end-to-end captures, independent M8 verifier, representative physics/pose/asset
benchmarks, or native desktop interaction were performed.

## Persisted-consist continuation (partial, 2026-10-27)

Schema 5 adds renderer-independent `trains` definitions in `qx-document`, with
stable train/component/joint IDs, ordered car body references and center offsets,
shared bogie references, dimensions, mass and principal moments, joint anchors,
wheel and guide reference points, and component-local asset path and pivot data.
Paths must be project-relative with forward slashes and cannot contain parent
traversal. This is a stored reference only: files are not yet opened, imported,
or resolved against a project directory. Versions 1–4 migrate to an empty train
list. Their existing nominal heartline ID is preserved even if unresolved; the
application reports that condition rather than fabricating a physical train.
Schema 5 train lists are required, unknown fields are rejected, and validated
`SetTrains` transactions participate in existing undo/redo and save/reopen.

The provisional Train Configuration controls create an explicitly labeled
two-car development fixture, edit body dimensions, car offsets and joint anchors,
and stage an association with the existing reference-settings Apply control.
Summary reports joint residuals at a selectable track station. These controls are
not an approved final train-editor UX. They do not provide car-count editing,
asset importing, train rendering, or simulation playback; the Preview still shows
only track/reference geometry. Body dimensions do not recalculate independently
specified mass or inertia. The older unrestricted nominal-ID editor remains
available, so dangling IDs are reported, not rejected.

`qx_physics::consist::inspect` evaluates a persisted definition on generated
track frames, positions bogies from a first attached car joint, aligns a car to
the chord between its first two bogie anchors, and fits translation to all its
bogie anchors. It reports world-space ball-joint anchors and residuals. This is
an approximate geometric inspection, not a general mechanism solver: a single
bogie leaves body orientation at its track frame, shared bogies are placed from
first use, and extra anchors, changing curvature, arbitrary off-axis pivots and
couplers can produce nonzero residuals. Orphan/unplaceable components and
out-of-track stations return errors. Wheel references are stored but not yet
returned as poses, and car-to-car articulation is not yet reported.

`consist::initialize` creates independent bodies, spherical joints and bilateral
point guides from the same definition and inspection poses. It rejects initial
joint errors above 0.05 m. During simulation, `World::step_on_track` integrates
forces and solves the existing constraints; it does not prescribe per-car
stations. Guides are idealized frictionless captive points, not wheel/rail
contact. The connected test uses two cars, four bogies, five joints and four
guides on a banked curved track, checks nonzero constraint reactions, error
bounds and deterministic replay. It does not validate all train mechanisms or
establish general dynamic stability. No simulation controls are wired into the
workspace.

M8 remains incomplete. No GLB parser, GPU asset rendering, component visibility,
complete configuration editing, visual train preview, independently evaluated
wheel positions, complete consist mechanism fitting, physics playback, M8
independent export verifier or representative M8 benchmarks are implemented.
No M8 end-to-end acceptance scenario or native desktop manual verification has
been completed. The independent M6/M7 export verifiers accept schema 4 and 5 and
check that their schema-5 fixtures contain no trains. See the current turn's
verification report for executed commands and results.

### Owner-executable check for the provisional controls

Run `cargo run -p quantum-x --release -- edit` on Windows. Switch to Train
Workspace with Ctrl+2. In Configuration, add the two-car development fixture,
expand it, edit body dimensions, car offset and joint anchors, and associate it
with reference settings. Apply the reference settings. Summary should show the
train and joint residuals while its inspection station is changed. Save and
reopen, then check that edits remain; use Undo/Redo on committed transactions.
This checklist has not been manually performed. Preview will still show track
geometry only, and controls are not a demonstration of GLB import or simulation.

Verification of this continuation on Windows: `cargo fmt --all -- --check`,
`cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, `cargo build --workspace --release`, and
`cargo test -p qx-physics --release --test consist` passed. Fresh Release M6/M7
scenario exports were generated under `target/m8-final/`; their independent
M6/M7 verifiers passed. These captures concern existing track workflows and are
not M8 train screenshots. The provisional inspector rejects rider-authored
geometry until it is connected to the derived physical track.

## Asset-independent preview continuation (partial, 2026-10-27)

The Train Workspace now draws transient wgpu line overlays for the associated
train using `consist::inspect` poses and the saved component dimensions. Car
and bogie boxes are development references, not assets. Each component has a
local coordinate triad; wheel-reference crosses use the persisted positions
(the schema does not specify a wheel radius), while separate joint-anchor
crosses and residual segments expose every connection, including couplers.
Guide markers and segments compare each persisted guide point with its
inspection station's track sample; they are geometric diagnostics, not wheel
contact or the dynamic solver's closest-point residual. The development fixture
now supplies two synthetic wheel-reference points per bogie.

The train station is transient state independent of track hover. Summary reports
joint residuals, total relative orientation between adjacent car bodies, and
guide-to-station distances. The representation does not modify the train, track,
physics, or Schema 5 persistence. Out-of-range or unsupported placement still
reports an inspection error; rider-reference-authored tracks remain unsupported
by this preview. The camera still frames the full track, so users may need to
zoom in to inspect the train. The preview is a wireframe, not finished train art.

Reproduce a numerical curved, inverted, two-car inspection and independent
constraint simulation with `cargo test -p qx-physics --test consist -- --nocapture`.
For this fixture the inspection reports 2.864789 degrees relative orientation,
a world-up dot product of -0.593816, and a maximum joint residual of
0.000520818 m. These numbers describe this synthetic fixture only. The test
also checks deterministic force-integrated stepping, but does not demonstrate
an interactive simulation or certify dynamic realism.

This remains an incomplete M8: no GLB parser/import, asset assignment or
rendering, shaded procedural wheels/couplers, visible simulation playback,
complete configuration editor, or M8 end-to-end GPU verification is provided.
Persisted asset references are not loaded, and the development overlay does not
indicate that an assigned asset has been imported. There is no proof of Blender
GLB compatibility. Do not present the preview as completing the requested
asset-independent train integration. Existing mechanical assumptions and the
independent force-integrated solver are unchanged.

Owner-executable visual check (not performed by the agent): run
`cargo run -p quantum-x --release -- edit`, switch to Train Workspace, create
and associate the development train, apply Reference settings, and change the
Summary inspection station. Zoom to the selected area and check the body and
bogie boxes, axes, wheel and guide crosses, and connection anchors. Save, reopen,
and check Undo/Redo for the train-definition edits. The Train Preview is a
provisional developer interface, not an approved final train-editor design.

A noninteractive Release `render-editor` capture of the two-car fixture on the
M6 hill was generated at `target/m8-preview-m6/train-preview.png` on the local
Vulkan adapter. The screenshot shows the diagnostic train overlay and measured
joint/relative-orientation readouts, though full-track framing makes the train
small. This is an offscreen smoke check, not interactive desktop verification or
an automated image-based M8 acceptance test. The temporary test project is at
`target/m8-preview-m6/train-preview.qx.json`.

## M8.1: GLB assets and focused train inspection (2026-10-28)

### Implemented vertical slice

`qx-rendering::asset` loads actual binary glTF 2.0 using `gltf` 1.4.1
(MIT OR Apache-2.0), with bounded PNG/JPEG decoding through the existing `image`
dependency. No GLB URI is opened or fetched. The supported static subset includes
indexed or unindexed triangle primitives; float positions/normals; UV0; mesh
reuse; default-scene node hierarchies; translation, quaternion rotation and scale
or affine matrix transforms; mirrored node transforms; base-color factors and
embedded textures; metallic/roughness factors; opaque and masked materials; and
double-sided surfaces. Missing normals are generated from triangle areas.

The loader checks container/buffer/accessor ranges, alignment and strides,
attribute counts/types, indices, finite data, nonsingular transforms, hierarchy
cycles/multiple parents, decoded images and explicit resource budgets. Limits:
64 MiB file, 4 MiB JSON chunk, one million vertices, three million indices,
4096 primitives/nodes, 128 hierarchy levels, 16384 mesh-node placements, textures
up to 4096 by 4096 and 128 MiB decoded pixels. Local mesh bounds after node
transforms must remain within ±1e9 metres; node linear transforms and inverses
are bounded to 1e6. The transient editor library retains at most 64 assets and
256 MiB of source/decoded geometry/texture data. One worker processes imports;
its bounded result channel prevents an unattended queue of decoded assets.

Unsupported content produces an actionable error: external resources, required
or optional extensions, compression, sparse accessors, non-triangle primitives,
skins/animations/morph targets, vertex colors/extra UV sets, alpha blending,
normal/occlusion/emissive/metallic-roughness maps and emissive materials. Tangents
are unused. Cameras are ignored with a visible diagnostic. This is a deliberately
limited basic material renderer: directional/ambient lighting, no texture mipmap
chain, environment lighting or full glTF PBR. Export static Blender meshes with
embedded resources, basic Principled material factors/base color, and no optional
extensions. Unsupported materials should be simplified explicitly in the source.

`asset_gpu` keeps static vertex/index/texture resources per shared imported model.
Cars and model nodes supply instance transforms, with view-relative translation
and inverse-transpose normals. Mirrored face orientation is handled explicitly.
Moving the inspection station updates instance data and overlays, reusing mesh
and texture resources. Geometry is never deformed or fitted to the track.
Unassigned or failed components retain plainly labeled development boxes;
load/correction errors remain visible in Summary. No alternate imported model
is substituted after failure. GPU resource counts and upload timings are included
in the offscreen reports.

### Document integration and controls

The existing Schema 5 asset path, translation, quaternion and scale remain
authoritative. Optional `pivot_m` defaults to zero when reading earlier Schema 5
assets. Source positions are transformed in double precision as:

`world = component_pose * T(translation_m) * R(rotation) * S(scale) * T(-pivot_m) * model_node * vertex`

Visual corrections accept scale 1e-6 through 1e6 and translation/pivot within
±1e9 metres. These are renderer limits, not mechanical parameters. An invalid
correction fails its transaction or displays an explicit diagnostic on reopen.
Mass, inertia, body dimensions, car offsets, joint anchors and guide constraints
are never inferred from a GLB or changed by assignment. No physics solver source
was changed by M8.1.

Configuration has provisional Import GLB, train/component and imported-library
selectors, Assign / replace, Remove assignment, and explicit Apply visual
correction controls. Rotation uses a numeric quaternion normalized on Apply.
Assignments and corrections use the existing `SetTrains` document transaction
and undo/redo. Unapplied corrections participate in the existing Save/Undo draft
review, Apply All and Cancel All. Component/assignment changes wait for explicit
Apply or Cancel, and replacement reseeds correction controls from the new asset.
Selection includes individual car bodies and bogies; the same
import can be assigned to several components. The existing train-definition
and reference-settings controls remain available below these controls.

Preview provides Fit Train and Focus Car. Configuration exposes the same camera
commands and independent engineering wireframe/axes/guides, bogie, wheel-reference,
joint and coupler visibility controls. Couplers are identified by their connected
body kinds rather than their IDs. `F` in Train Preview fits the train. Fit uses
assembled component/model bounds; Focus includes the selected car body and its
associated bogies. Summary provides both the existing station slider and exact
numeric station entry. Stations outside the existing consist inspector's usable
track domain report the placement error instead of clamping each component.

The owner-approved Configuration / Preview / Summary organization is preserved.
The listed actions follow the explicit M8.1 request; their detailed presentation,
numeric quaternion editor and development boxes are replaceable developer UI.
Final train-editor presentation, picking/selection design and asset-browser UX
still require human design. Track geometry and authored layouts are unchanged.

### Storage and recovery

Imports stage validated bytes and models in memory, so unsaved projects can
assign assets. Save packages source GLBs under `assets/train/` beside the project;
references use forward-slash project-relative names containing a content hash.
Byte comparisons prevent name collisions from overwriting another asset.
Save As copies retained assets, including sources needed by undo history, before
the existing atomic JSON save. Failed copy/write leaves the document identity
and project bytes intact; a failed new-file write removes its partial copy.
Different existing destination bytes are never silently overwritten. Directory
ancestors are checked to prevent symlinks redirecting packaging outside the
chosen project directory. Unreferenced retained files are not automatically
deleted, and unassigned library selection is transient.

Opening a project resolves referenced GLBs relative to that project's directory
on the import worker. Missing/corrupt files produce visible errors while retaining
the saved assignment. Retry missing assets reloads failures; importing a new or
relocated source and Assign / replace repairs a reference. Saving missing assets
preserves their paths and explicitly reports missing-reference warnings. Schema
1–4 migration, old Schema 5 files, history and project compatibility remain covered
by the existing and M8.1 regression tests.

### Original assets and reproducible acceptance

`assets/development/train/` contains tiny original development shell and bogie
GLBs with a checker texture, hierarchical boxes and a mirrored side. They are
verification data under the repository licenses, not release rolling stock or
manufacturer replicas. `tools/make-m81-fixtures.py` regenerates the source assets.
`development-blender-body.glb` was actually imported/exported with Blender 4.5.4
LTS in background mode (Khronos glTF Blender I/O v4.5.49), using
`tools/export-m81-blender-fixture.py`. Blender is not required to build/run the app.

From the workspace root, the automated acceptance sequence is:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
.\target\release\quantum-x.exe authoring-scenario target/m81-verification/m6
.\target\release\quantum-x.exe track-scenario target/m81-verification/m7
python tools/verify-m6-exports.py target/m81-verification/m6
python tools/verify-m7-exports.py target/m81-verification/m7
.\target\release\quantum-x.exe train-asset-scenario target/m81-verification/m6/project.qx.json target/m81-verification/train
python tools/verify-m81-exports.py target/m81-verification/train
```

The train scenario loads the saved authored M6 project, creates the existing
two-car development consist, imports the actual Blender-exported shell and the
bogie GLB through the application asset service, assigns both bodies and four
bogies through editor commands, and checks remove/undo/redo, save/reopen and
Save As relocation. It inspects stations at 40% and 60% of track length. The
resulting project is `target/m81-verification/train/train-inspection.qx.json`,
with another self-contained copy in `save-as/`.

Actual Train Workspace captures are `station-a-1440x960.png`,
`station-a-960x640.png`, `station-b-1440x960.png`, `station-b-960x640.png` and
`assets-without-overlays.png`. Each has a `.capture.json` camera/resource report.
The independent Python verifier decodes the saved GLB/accessor data, checks the
hierarchy, component quaternion/pivot transforms, car chord alignment, bogie
track references, world bounds and joint residuals. It checks both capture sizes,
that component bounds are visible and occupy a useful portion of Preview,
shared GPU resources and actual body/bogie pixels even without train overlays.
Headless egui input tests additionally click the real assignment, removal,
camera and visibility controls, type an exact station, and inject successful and
failed native-dialog results into the import worker. These do not operate the
Windows desktop or prove native file-picker behavior.

### Verification boundary and owner manual acceptance

On Windows on 2026-10-28, formatting, workspace check, strict all-target Clippy,
all 198 workspace tests, and the workspace Release build passed. The 198 include
103 application tests (eight for M8.1 editor/services) and four GLB-specific
rendering tests; malformed import coverage exercises 20 variants plus truncated
containers. All 15 `qx-physics` tests also passed in Release mode. Fresh Release
M6/M7 scenarios and their independent verifiers passed: eight M6 PNGs and 14 M7
fixtures with 40 PNGs, including the live-edit workflows. The M8.1 independent
verifier passed against the five current focused train captures and packaged
Blender-exported GLB project. With train overlays disabled it identified 51113
body-colored pixels and 6427 bogie-colored pixels in the actual rendered viewport.

In the final local run, reading/importing both tiny fixture files took 0.511 ms.
The four main captures reported 0.063–0.068 ms of CPU-side GPU asset resource
preparation/enqueueing, two unique model resources, two textures and 22 primitive
instances; a second unchanged upload reused static resources. These are single
local measurements on an NVIDIA RTX 4070 / Vulkan, not GPU execution latency,
general asset benchmarks or frame-rate results. The complete numeric evidence is
in `target/m81-verification/train/evidence.json` and the `.capture.json` files.

No FPS benchmark, arbitrary Blender-file compatibility,
native desktop acceptance, physical wheel contact, complete articulation solver,
physics playback or full rigid-body coaster realism is claimed. The existing
kinematic inspection assumptions and independently force-integrated dynamics
remain distinct. M8.2 playback is intentionally not implemented.

Owner-executable manual check (not performed by the agent):

1. Run `.\target\release\quantum-x.exe edit target/m81-verification/train/train-inspection.qx.json`.
2. Switch to Train Workspace (Ctrl+2). Confirm both imported shells and all bogies;
   use Fit Train, select the second body or its bogie, and Focus Selected Car.
3. Change exact station from 36 to 54 m. Observe every body, bogie and overlay
   move/rotate together, then Fit Train again.
4. Import a compatible custom Blender GLB using the native dialog. Assign it to
   a body and a bogie independently; adjust local translation/quaternion/scale/
   pivot, Apply, and check undo/redo. Toggle each engineering reference group.
5. Remove/replace an assignment. Save, Save As to another existing directory,
   reopen, and verify the assignments and packaged files. Temporarily relocate a
   copied GLB, reopen to see its error, then repair with Import and Assign / replace.

All M8.1 changes remain local on `codex/m8-integrated-train-system`. The explicit
request prohibits commits, pushes, PRs, merges and distribution-name changes;
none are part of this pass. Existing uncommitted M8 work is preserved.
