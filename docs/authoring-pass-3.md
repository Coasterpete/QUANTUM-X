# Interactive authoring and reference-point mathematics

Pass 3 builds on the completed Pass 2 representations and shared generator.
`qx-physics` is unchanged; its future 1/240 s rigid-body timestep remains separate
from spatial geometry sampling and design-speed calculations.

[Integrated Workbench M4](workbench-m4.md) supersedes this pass's provisional
panel/window presentation and ordinary spline insertion menu. Its workspace
profiles retain the editing, document and generation behavior described below.

## Run

```powershell
cargo run -p quantum-x -- edit
cargo run -p quantum-x -- edit examples/mixed-authoring.qx.json
cargo run -p quantum-x -- edit examples/heartline-reference.qx.json
cargo run -p quantum-x -- heartline-demo target/pass3-heartline
python tools/verify-heartline-exports.py target/pass3-heartline
cargo run -p quantum-x -- render-editor examples/heartline-reference.qx.json target/editor.png
```

`edit` without a file creates an untitled document with one editable 10 m straight,
not a generated coaster layout. M5 adds native Open/Save As dialogs; Save prompts
for an untitled project. See [M5](editor-m5.md) for current commands. `inspect` retains the
Pass 2 development inspector. `render-editor` renders the actual workspace and
shared line shader to a PNG using an offscreen GPU target; it creates no desktop
window and never injects native input. It requires a compatible GPU adapter.

## Human editing and transactions

The explicitly provisional egui workspace uses the brief's top command area, left
Track Workspace, center 3D viewport, right Support Workspace and bottom Transition
Editor. Only `docs/ux/coaster-properties.md` was present in the checkout; the
referenced editor/welcome sketches were absent. Panel sizing, graph handles,
dialogs, controls, and styling are replaceable development UI, not approved final
interaction design. Support construction is not implemented; its development
region contains actual diagnostics and viewport controls, with no pretend tools.

Select sections from the ordered list or bottom timeline. Insert force-vector,
world-geometric, rider-local, straight, arc, Bezier, B-spline, NURBS or Catmull-Rom
sections after the selection. Reorder or remove them through stable-ID commands.
At least one section is retained. No automatic smoothing or layout repair occurs.

Choose Roll, Normal/Lateral or Pitch/Yaw as applicable. The graph evaluates the
actual Rust transition functions. Click a transition region to select it. Drag its
square endpoint handles to edit values; release commits one logical undo step.
Other transitions' endpoints are not silently changed to hide discontinuities.
Numeric controls edit length, values, amplitude, function and Hermite derivatives.
These edits remain drafts until Apply. Draft geometry and offset changes generate
an explicitly labeled 3D preview without entering document history. Cancel restores
document values. Invalid drafts give errors and keep the previous valid view.

Section extent changes explicitly scale all channel durations and inverse-scale
Hermite slopes. Force values and angular rate values are retained, so changing
extent can change shape and total roll. Time/distance conversion instead scales
input duration and angular values/slopes using the stated constant design speed,
preserving the section's mathematical shape. Normalized spline domains remain
normalized physical arc distance, not time or native parameter.

Historical behavior below is superseded by [shape-preserving NURBS regions](transition-nurbs-regions.md).
Transition insertion was labeled **Split into two Hermite regions**. It matched
the source's endpoint and split values/slopes; higher-order, amplitude and cosine
sources can change between these constraints. It is an explicit reversible reshape,
not an exact general function subdivision. Removal explicitly extends a neighbor
over the vacated duration. Reordering retains transition IDs and values and may
create reported discontinuities. Changing a single transition length must still
leave all channels covering the section extent; users can coordinate numeric
draft changes, or change the overall section extent.

Complementary spline controls edit original f64 positions, NURBS weights, knots,
degree and Catmull-Rom parameterization. Degree edits are explicit reinterpretation;
an optional clamped-knot rebuild is labeled as a reshape. Alignment, supported exact
NURBS conversion and Bezier G1/C1/C2 fitting are explicit staged operations. No
operation fits automatically on an ordinary point edit. Point-count changes and
general conversion remain available through the existing Rust editing API;
interactive control-point insertion/removal and 3D handle picking are not added.

Ctrl+Enter applies the section; Esc cancels its draft. Ctrl+Z undoes; Ctrl+Y or
Ctrl+Shift+Z redoes. Ctrl+S saves the entered path; Ctrl+O opens it; Ctrl+N creates
a new project. Apply or cancel all drafts before save/undo. Opening/new/closing
protect unsaved edits with a recoverable prompt. Metadata, initial pose/design
speed and nominal reference configuration have their own Apply/Cancel controls.
Keyboard Tab navigation and native numeric text entry remain available.

Coaster Properties keeps family/numbered-style selection separate. Existing concept
entries show descriptions, compatibility and missing-preview status before Apply;
this pass adds no asset, thumbnail or manufacturer-accuracy claim.

## Distinct geometric references

The document's `authored_reference` specifies the meaning of the solver's source
path. In the default `TrackCenterline` mode, the generated source is the mathematical
track centerline and `reference_geometry` returns the rider path. In
`RiderReference` mode, the source is the authored rider curve and the calculated
path is a constrained physical track reference. `ReferenceTrack::role` distinguishes
these outputs; `Project::generate` always preserves the original authored source.
Changing offsets never edits the sections, control points or solver equations.
Changing authoring intent is an explicit reinterpretation, not a fitted layout.

Neither path is a physical rail surface. Spine, left/right rail and wheel-contact
geometry require separate track-family cross sections and are not inferred from
the rider path. A nominal train configuration ID associates seating assumptions
without coupling geometry to visual style or assuming universal seat geometry.
The generic point-kinematics API accepts independent reference offsets for future
seats/vehicle points; no car body, seat asset or contact mechanics is invented.

In the transported, explicitly banked right-handed frame, T is forward, N is rider
normal and L is lateral, with T cross N = L. Offsets `(a,h,l)` in metres mean signed
longitudinal, normal and lateral distances at the **same source station**. A
longitudinal offset is not a query at `P(s+a)` along a curved track.

With frame matrix R, local offset r and spatial frame rotation vector Omega:

```text
H = P + R r
H_s  = P_s + Omega cross (R r) + R r_s
H_ss = P_ss + Omega_s cross (R r)
       + Omega cross (Omega cross (R r))
       + 2 Omega cross (R r_s) + R r_ss
H tangent = H_s / |H_s|
H curvature = |H_s cross H_ss| / |H_s|^3
H arc length = integral |H_s| ds
```

Constant and C2 quintic profiles support all three offset components. The latter
uses `u=s/total_authored_length`, `r=r0+(r1-r0)*(10u^3-15u^4+6u^5)` with analytic
first/second derivatives. Offsets must be finite with norm <=100 m. Offset path
tangent norm <=1e-9 is rejected as singular with corrective context. Near-singular
regular paths can still be ill-conditioned; there is no global regularity proof.

`track_from_rider` subtracts an offset jet using an explicitly supplied orientation
jet. It validates resulting track tangent and curvature against that field (1e-6
direction and 1e-5 spatial-derivative compatibility tolerances). For example, a
radius-20 m rider circle with +2 m inward offset recovers radius-22 m track. Many
rolled/variable-offset rider curves are incompatible with their supplied frames:
generation returns a diagnostic rather than inventing bank or solving an
underdetermined inverse. General inverse orientation/endpoint solving is absent.

## Design-speed point kinematics

Each source section retains its existing constant reference speed v. For an
offset point with local relative time derivatives, the independent public
`point_motion` API evaluates:

```text
v_point = v_reference + omega cross r_world + R v_relative
a_point = a_reference + alpha cross r_world
          + omega cross (omega cross r_world)
          + 2 omega cross (R v_relative) + R a_relative
```

The generated design calculation uses omega=v*Omega, alpha=v^2*Omega_s,
v_relative=v*r_s, a_relative=v^2*r_ss and a_reference=v^2*P_ss. Equivalently,
v_point=v*H_s and a_point=v^2*H_ss. This includes variable-offset relative motion,
not just constant rigid attachments. No unmodeled longitudinal source acceleration
is assumed. Reference-speed jumps have undefined impulsive acceleration and are
reported, not assigned a finite load.

Specific acceleration is `a_point - gravity`, with gravity `(0,-g,0)`. Normal,
lateral and longitudinal g estimates project it on N, L and T of the attached
source orientation, not a redefined Frenet normal. Curvature and reference speed
belong to the calculated path. In rider-authoring mode calculated-path loads are
track-reference estimates; the source graph remains the rider-source diagnostic.
These are design-time kinematic estimates, never actual vehicle telemetry or a
replacement for independent rigid-body dynamics.

## Numerical method and diagnostics

The source generator now records smooth sample ranges ending at every section,
channel transition and native spline span. The heartline adapter fits degree-four
local Lagrange interpolants to T/N/L using five samples within one range. It scales
local coordinates, uses stack arrays and never differentiates large world positions.
At endpoints it uses one-sided stencils. It derives
`Omega=0.5*sum(axis cross axis_s)` and
`Omega_s=0.5*sum(axis cross axis_ss)`. Source P_s uses the transported tangent;
P_ss comes from its differentiated interpolant. Offset derivatives are analytic.
Arc length uses Simpson integration of the resulting speed factor per source
interval, including a midpoint derivative evaluation.

The point equations are algebraic; sampled frame derivatives and length are
numerical approximations. On smooth resolved frames, first derivatives have local
fourth-order interpolation accuracy and second derivatives generally third-order
one-sided accuracy. Underlying spline frame transport remains second-order and can
dominate. No global error bound or adaptive heartline refinement is claimed.
Refine spatial sampling and compare when using highly curved/rapidly rolled paths.
The step rotation and sample-budget guards remain in force. Very tiny smooth
ranges and ill-conditioned stencils return actionable errors.

Duplicate source stations at knots retain both derivative/load limits. Compatible
offsets and frames give the same boundary position; curvature/twist jumps can make
an offset path's tangent discontinuous. Both section and internal discontinuity
warnings explain that finite impulsive loads are unavailable. No stencil crosses
the join to disguise it. Frames remain quaternion transported through vertical
tangents and inversions; gravity-referenced bank remains undefined near vertical.

Exports add `reference-points.csv` (separate source/reference arc coordinates,
position, tangent, curvature, design speed, acceleration and three loads) and
`reference-info.txt` (role and warnings). The analytic v3 fixture has track radius
20 m and rider radius 18 m, with source speed 20 m/s and rider speed 18 m/s.
`tools/verify-heartline-exports.py` independently checks its analytic geometry/loads.

## Application integration and limits

Domain crates remain free of egui/wgpu. `EditorSurface` composes application widgets
over the existing renderer. One generation worker uses a bounded queue and a latest
pending snapshot; older revision results are ignored. Full authored regeneration
is still the correctness baseline. Generation runs off the UI thread; JSON
validation and explicit file Open/Save remain synchronous bounded development
commands and may pause on slow filesystems. History remains bounded to 64 authored
snapshots in the UI. Staged drafts are separate and not silently saved.

3D paths, offsets, reference points, frames, curvature and calculated-path load
vectors are individually toggleable. Load/curvature vector lengths are diagnostic
scales. Selection highlights the section by stable ID. Camera buttons provide
perspective/top/side/front, orthographic toggle and all/selected fitting; viewport
left drag orbits, middle/right drag pans and wheel zooms. Foreground egui popups
block viewport input. Camera changes update uniforms; vertex buffers are reused
when large enough, and pipelines/devices are retained through edits. There is no
frame-rate guarantee or production rail/material rendering claim.

Schema 3 persists all previous authoring data plus reference intent, constant or
variable offsets and nominal train ID. Loading v1/v2 explicitly supplies zero
track-centered offsets, preserving their geometry. Old versions cannot smuggle
v3 settings; v3 requires its settings, and unknown fields/versions still fail.
Existing recoverable sibling-file save behavior is retained. No downgrade exists.

Dependencies added are egui/egui-winit/egui-wgpu 0.37.0 (MIT/Apache-2.0), bundled
egui fonts, and png 0.18.1 for offscreen verification. Default fonts must be enabled
explicitly in egui 0.37. Native clipboard integration supports ordinary text entry;
no browser-link integration, networking service or analytics is added. Font assets
carry OFL-1.1 and Ubuntu-font-1.0 terms in addition to the package's permissive
code terms. Original notices are retained in `docs/licenses/egui-default-fonts/`
for distribution with the bundled fonts. See Cargo.lock and package license files.
Primary API reference: [egui 0.37](https://docs.rs/egui/0.37.0/egui/).

## Owner-executable native graphical checklist

No active desktop interaction was performed during this pass. Run:

```powershell
cargo run -p quantum-x --release -- edit examples/mixed-authoring.qx.json
```

1. Select a force section and Normal/Lateral/Roll channels. Drag endpoints; confirm
   graph and 3D preview change, release, then Ctrl+Z/Y restores/reapplies the change.
2. Edit function, amplitude and Hermite slopes numerically; Apply and Cancel. Split,
   remove and reorder transition regions, inspect discontinuity diagnostics.
3. Insert and reorder force/world/rider-local sections. Resize and convert time/
   distance domains. Confirm selected-section highlight and explicit rate warnings.
4. Change spline controls and rational weights. Apply/cancel; verify a broken local
   join gives a specific error instead of an automatic repair.
5. Set normal/lateral/longitudinal offsets and variable endpoints. Apply; toggle each
   overlay independently. Open the analytic heartline fixture and inspect the two
   different-radius paths and different lengths/speeds.
6. Save to a new path, reopen, and confirm parameters/geometry. Verify invalid paths,
   invalid JSON, unsaved-new/open/close prompts, and draft protection. Test undo/redo
   after heartline changes and section movement.
7. Orbit/pan/zoom inside the viewport; interact with popups over it. Fit all/selected,
   change projections, resize/minimize/restore and vary Windows DPI. Confirm text,
   focus traversal, numeric entry and graph handles remain usable.
8. Induce a resolvable generation error using tight radius/large step, then correct
   it or undo. Verify revision feedback and retained previous valid geometry.

Native input, resize/DPI, device/surface loss and sustained FPS remain unverified
until these runtime checks are performed. Offscreen images do not establish them.
