# M8.2 universal vehicle definitions and interactive rigid-body playback

M8.2 extends the existing M8/M8.1 foundation rather than replacing its Schema 5
document, GLB pipeline, consist inspector or solver. This is an experimental
multi-body implementation with tested idealized constraints, not validated
dynamics for every coaster family. All regression layouts and rolling-stock
assets described here are opt-in development fixtures.

## Definitions and compatibility

`qx-document::train` remains authoritative. Each independently identified
component has its own dimensions, mass, principal inertia about its mass center,
local mass-center offset, optional initial placement, reference points and visual
asset assignment. `car_body` is the retained Schema 5 chassis role, with no
above-track or seating assumption. `bogie` is retained; `running_gear`,
`passenger_assembly` and `auxiliary` extend the roles. Roles do not select a
different solver or enforce a rail count.

Optional `placement` is either a track-relative station offset plus local frame,
or a frame relative to another component. A frame includes translation and a unit
quaternion. Mounts may nest and need not follow component array order. Unknown
parents and cycles are rejected. The mount graph supplies **initial conditions**;
the independent joint graph supplies **mechanical relationships**. A mount alone
does not mechanically attach two bodies. All supported arrangements use the same
world-space integration and generated-track boundary.

The retained `cars` list supplies ordered inspector grouping and legacy placement,
with 1–256 cars and up to 1024 independently defined components. Cars may have
different masses, dimensions, assemblies and any number of running-gear references,
including zero or shared references. `bogie_ids` is the retained field name and
also accepts `running_gear`. Explicitly mounted passenger modules join their
parent car's focus bounds. No solver loop presumes two cars, two bogies or two rails.

Each connection retains its two local anchor points and adds an optional model:

| Model | Represented relationship | M8.2 dynamics |
| --- | --- | --- |
| Ball (default) | Coincident anchors, three free rotations | Impulse solved |
| Fixed | Coincident anchors and aligned local joint frames | Linear and angular impulses |
| Hinge / free | Coincident anchors, aligned hinge axes, one free rotation | Revolute impulses; unlimited rotation if limits are absent |
| Hinge / limits | Optional lower and upper principal-angle stops | Unilateral angular impulses |
| Hinge / torque motor | Target, stiffness, damping, maximum torque | Finite equal-and-opposite torque on physical bodies |
| Hinge / mechanical profile | Time- or track-distance-domain keys and transmission ID | Stored, validated, inspected; playback explicitly rejects the unimplemented transmission |

Joint-local Z is the hinge axis and X defines zero angle. Joint frames may orient
that axis in any component-local direction. Limits must increase strictly within
(-pi, pi); motor targets must lie in that interval and within any limits. Free
rotation is not restricted to one revolution, but the reported angle is principal,
not accumulated revolutions. A torque motor is not evidence of a mechanically
controlled 4D rack/linkage. A stored mechanical profile is distinct from free
spinning; it is never silently downgraded to a free hinge or visual animation.

Vehicle metadata has an optional train-style ID, declared track-style IDs and
required track-feature tags. Track catalog styles can declare `vehicle_features`.
The assessment is Unspecified, Declared or Incompatible, with reasons for missing
features or excluded IDs. Empty lists mean unspecified. Custom feature/identifier
tags are allowed and validated; neither a match nor the word Declared certifies
physical compatibility. Existing catalog train IDs and styles are preserved.

Schema version remains 5. Absent mass centers default to zero, placements to the
legacy inspector, joint models to Ball and compatibility to Unspecified. New
definitions round-trip through `SetTrains`, undo/redo and existing atomic saves.
Older executables that reject unknown fields cannot read the extensions; forward
compatibility with those executables is not claimed. Unknown data is still
rejected rather than discarded. No external dependencies or licenses were added.

## Representation versus tested dynamics

| Architecture | Representation available now | Verification boundary / future work |
| --- | --- | --- |
| Sit-down, floorless, stand-up, motorcycle | Independent chassis/passenger frames and visuals | Conventional synthetic incline tested; family-specific contact and rider mechanics absent |
| Inverted, flying | Below-track chassis and independently oriented passenger bodies | Below-track synthetic consist tested; flying posture mechanisms and operational locks require validation |
| Suspended / swinging | Offset mass centers, hinge axes and limits | Rotating-seat pendulum fixture tested; real suspended running gear is not validated |
| Wing / beside-track modules | Lateral mounting frames and arbitrary passenger-body connections | Representable; asymmetric full-vehicle dynamics and clearance not validated |
| True mechanically controlled 4D | Separate passenger body, hinge frames and distinct mechanical profile | Transmission, rack/linkage contacts and force propagation are future work; profile playback fails explicitly |
| Freely spinning | Unbounded free revolute degree of freedom with separately simulated mass/inertia | Free hinge regression crosses multiple revolutions; actual spinning-car contact/drag not validated |
| Single rail / monorail | Arbitrary running-gear bodies and reference-point counts | No two-rail assumption; real wheel/rail profile and contact models remain future work |
| Shared bogie / specialized articulation | Shared stable references and arbitrary connection graph | Existing shared-body impulse fixtures and schema tests; no claim of complete shared-bogie family validation |
| Custom vehicles | Same component, placement, mass and joint graph | Only implemented joint primitives are simulated; a complete custom mechanism editor is future UX work |

## Dynamics, placement and rendering

Body poses are at the center of mass. Authored anchors and guide points are
converted from component origins by subtracting the local mass-center offset.
Snapshot rendering reverses that conversion. GLB corrections and pivots are applied
after the simulated component pose, exactly as in M8.1. Replacing/scaling a visual
asset neither changes mass/inertia nor restarts the current simulation. Track,
mechanical-definition, association and geometry-draft changes invalidate playback
with a rebuild message. Save/reopen retains authoring data, not live poses or time.

`TrackSimulation` owns independent bodies and guide projection hints. Steps remain
1/240 s with four internal substeps and 32 sequential impulse iterations in the
playback adapter. Semi-implicit force integration and normalized orientation
integration remain in the existing foundation. Ball impulses enforce anchors;
fixed/revolute impulses enforce angular constraints, and the bounded motor applies
equal-and-opposite angular impulses once per substep. No body receives a prescribed
spline pose during stepping. Initial speed is an explicit initial condition only.

Captive guide references on **any** component constrain points to the generated
centerline, with station obtained by local projection of their physical positions.
They do not stand for wheel contact, rail count, or independently integrated scalar
followers. Nonzero local guide positions are projected separately at initialization.
Initial anchor/guide errors above 0.05 m or angular errors above 0.05 rad reject
playback rather than silently altering authored assemblies. Open-end escape and
projection failure roll back bodies, guide hints and tick count, then pause the
worker with a visible error. Reset restores original bodies and guide hints.

The existing 20% Baumgarte stabilization can inject/remove energy. Guides are
frictionless, bilateral and captive. There is no wheel separation, rolling/sliding
contact, collision, wheel spin, brake/lift/launch or operational block system. The
local projection's 2 m correction bound and initialization tolerances limit curved
and overconstrained arrangements. Explicit mounts do not perform automatic rigid
assembly fitting. The legacy inspector retains its historical chord-fit adapter;
new mounts avoid its two-anchor placement convention. Principal inertia axes must
be parallel to component axes; a general rotated inertia tensor is future work.
No general performance, ride-quality, realism or certification claim follows.

## Playback integration and UX status

The owner's Configuration / Preview / Summary organization is preserved.
Configuration can add variable-length conventional, below-track or rotating-seat
development consists; the historical two-car fixture remains for regression.
Existing association/reference Apply controls select the active train. Precision
development controls expose component dimensions, mass, inertia, mass center,
initial frames, guide references, connection frames, joint type, limits and motor
parameters. Accepted edits use the existing validated train transaction/history.
Invalid edits retain the previous document and report the validation error.

Summary provides initial speed, Initialize, Play/Pause, single-step, Reset and
Return to inspection. Ctrl+Space toggles playback; Ctrl+Shift+Space single-steps
when no text/number editor or modal owns input. The initial inspection station is
disabled during simulation. Time/ticks, body speeds, constraint residuals and
average reactions are separate from authoring graphs and design-speed loads.
Preview uses the same imported assets, engineering overlays and Fit/Focus commands
for both inspected and simulated poses. Paused snapshots remain visible.

The worker uses a wall clock only to schedule fixed steps. Catch-up is bounded to
eight steps per batch; excess elapsed wall time is discarded with a lag counter,
never converted to a larger physics step. Only the newest immutable render snapshot
is retained. Solver work runs outside the egui thread and outside the brief snapshot
mutex. Paused workers sleep until commanded; closing/replacing the session drops
their channel. There is no per-render-frame physics integration.

The requested playback behaviors and owner-directed pane organization are followed.
The detailed controls, mechanical numeric editors and fixture selectors are plainly
labeled **development controls**, not final product UX. Full custom topology editing,
profile editing, picking and final train-editor presentation still require human
design. Development boxes/GLBs remain development assets.

## Reproducible verification and manual checklist

Executed on Windows on 2026-10-28: formatting, workspace check, strict all-target
Clippy, all **213 workspace tests**, all **23 physics tests in Release**, and the
workspace Release build passed. The M8.2 scenario and independent verifier passed
for nine captures of 9/9/12 independent bodies in the three arrangements. The
largest measured anchor residual over the synthetic half-second runs was about
0.00000162 m. Conventional and below-track displacement was 1.244126 m from
2 m/s on the 0.2 rad incline; the independently rotating-seat consist moved
1.265086 m. These are fixture observations, not general solver error bounds.

Fresh M6, M7 and M8.1 scenarios and their independent verifiers also passed:
8 M6 captures, 14 M7 fixtures with 40 captures, and 5 M8.1 train-asset captures.
The offscreen straight/grid visibility assertion remains active for track
inspection; train-focused captures allow opaque bodies to occlude that reference
line. Static captures were inspected, without interacting with the desktop.

From the workspace root on Windows:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p qx-physics --release
cargo build --workspace --release
.\target\release\quantum-x.exe train-playback-scenario target/m82-verification
python tools/verify-m82-exports.py target/m82-verification
```

The scenario uses the actual playback worker, three separately configured
three-car consists, original Blender-exported GLBs and persisted Schema 5 projects.
It captures ticks 0 and 120 at 1440×960 and Reset at 960×640 for each arrangement.
Each case saves a self-contained `playback.qx.json` with packaged assets. Its
evidence retains poses, instance matrices, residuals, reactions and speeds. The
independent Python verifier recalculates anchor residuals and asset transforms,
checks packaged GLBs, projected bounds, rendered asset pixels, gravity-driven
incline speed for the conventional/below-track fixtures and rotating-seat motion.
Numerical tests separately cover deterministic replay, fixed/revolute constraints,
limits, motor torque bounds, momentum transfer, mass-center conversion, endpoint
rollback, schema compatibility and history. Headless egui tests click the real
playback/fixture buttons and verify displayed instances match worker poses.

Native desktop interaction and continuous visual smoothness are **not manually
verified**. No desktop control is used. Owner-executable acceptance:

1. Run `.\target\release\quantum-x.exe edit target/m82-verification/rotating-seats/playback.qx.json`.
2. Open Train Workspace (Ctrl+2), Fit Train, select each chassis/gear/seat module,
   and use Focus Selected Car. Confirm below-track placement and separate modules.
3. In Summary choose a valid middle station, Initialize, Play, Pause, single-step,
   and Reset. Verify the visible train and joint overlays follow physical snapshots.
4. Test Ctrl+Space and Ctrl+Shift+Space outside numeric editors. Resize and switch
   workspaces during playback; confirm the document and authored layout stay intact.
5. Pause, replace one GLB or change its correction, Apply, and confirm time/physical
   poses persist. Change a mass/mount/joint, then undo, and confirm playback requests
   reinitialization. Save/Save As/reopen and confirm definitions/assets are preserved.
6. Add a different car count/arrangement in Configuration, associate it and Apply
   the reference settings. Try an invalid station; confirm an actionable error and
   recovery through Initialize/Reset rather than silent clamping or reshaping.
