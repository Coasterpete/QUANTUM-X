# M2 — original Box-Spined construction

Box Spined-100 is selectable in the actual Editor through Coaster Properties.
It installs continuous parametric rails, the exact extracted cross section of
the owner's spine, and rigid instances of the owner's crosstie. The other 16
track styles remain Planned. Available means this visual construction and its
dependencies have been exercised; it does not mean finalized production
dimensions, structural certification, or validated wheel/rail contact.

## Owner direction and provisional dimensions

The owner explicitly authorized these development defaults during M2:

| Quantity | Initial value | Stored setting |
| --- | --- | --- |
| Running rail center spacing | 1.10 m | Rail lateral offsets −0.55 / +0.55 m |
| Running rail diameter | 0.13 m | Circle profile radius 0.065 m |
| Crosstie repeat spacing | 1.50 m | Repeater spacing |

These are configurable values in `assets/tracks/box-spined/100/style.json`, not
Box-Spined family rules. The optional **Advanced construction** section exposes
rail center spacing, diameter, and cross-tie spacing with precise numeric entry.
It edits the document's construction draft, previews the result, and uses the
existing Apply/Cancel and history transactions. Changing tie pitch retains its
relative initial phase and end margin. Expert source transforms and raw material
slots remain development-only.

## Source inventory and unresolved mappings

All six original GLBs were inspected before implementation. Their original
SHA-256 hashes are recorded by `tools/audit-box-spined.py`. No original GLB,
Blender file, or material map was rewritten. No track `.blend` source is present
under `assets/tracks`; the repository's Blender icon source is unrelated.

| Existing source | Established role | Integration status |
| --- | --- | --- |
| `100/Box-Spine-100.glb` | Box spine; material named `box spine` | Exact cross-section sweep |
| `100/Box-Spine-100-crosstie.glb` | Crosstie; material named `crossties` | Original mesh, rigid repeated placement |
| `200/Box-Spine-200.glb` | Box spine for numbered file 200 | Preserved; Planned |
| `200/Box-Spine-200-crosstie.glb` | Crosstie for numbered file 200 | Preserved; Planned |
| `300/Box-Spine-300.glb` | Box spine for numbered file 300 | Preserved; Planned |
| `300/Box-Spine-300-crosstie.glb` | Crosstie for numbered file 300 | Preserved; Planned |

Paths above are relative to `assets/tracks/box-spined`. The numbering and material
names establish file identity and role. They do not establish manufacturer,
train compatibility, structural properties, or a finalized assembly definition.
Style 200's spine is on the opposite side of its source Y origin from style 100,
and its tie lacks the 180-degree node rotation present in 100/300. Style 300 is
wider. Their intended assembly conventions and dimensions remain unresolved;
no inversion meaning, train arrangement, or additional hardware is invented.
No rail meshes, bolts, welds, support mounts, or connector assets were found in
these six files; none were invented.

## Coordinates, placement and measured discrepancies

GLB source coordinates are interpreted as metres, Y up, Z longitudinal.
After each original scene-node transform, `(Z, Y, −X)` maps to the existing
right-handed construction frame `(T forward, N rider up, L lateral)`. Positive
source scales apply before this basis conversion. The asset importer retains
node hierarchy transforms, normal inverse-transpose transforms, and UVs. A
negative node determinant reverses triangle winding. No part is recentered,
mirrored, stretched, or silently fitted to a different rail gauge.

Style 100's source spine spans approximately T=0..1.5 m, N=−0.975..−0.236 m,
and L=−0.260604..+0.260604 m. Its eight-edge section retains all four 10 mm
corner chamfers. Its 56 vertices / 44 triangles include extra triangulation
points along the longitudinal faces; those do not constitute changing sections.

The original crosstie contains 184 vertices / 180 triangles and one UV set.
Its node's 180-degree Z rotation is essential: after conversion, its central
structure extends downward toward the spine, rather than upward into the rider.
Its source longitudinal center is effectively zero (sub-micrometre export
roundoff is retained). It is approximately 1.20 m wide and 40.0005 mm thick.

The repeater uses station 0.75 m, then 1.50 m increments, and a 0.75 m end margin.
The half-pitch phase centers each tie within the source's 1.5 m module length;
this is an explicit development placement convention, not assembly metadata
embedded in the GLB. Stations are global physical arc distances, so phase does
not restart at authored section boundaries. Shorter tracks can contain no tie
when the configured endpoint margins leave no valid station.

Measured discrepancies are preserved:

- The crosstie's two outer pad heights differ by about **20.5346 mm**. The GLB
  contains this asymmetry; it is not corrected or flattened.
- The provisional rail outer span is **1.23 m**, versus the crosstie's **1.20 m**
  total width: each rail extends approximately 15 mm past the tie's lateral
  extent. This is a dimension comparison, not a conclusion about attachment
  validity. Rail/pad intersection and welded attachment mechanics are unverified.
- One isolated tie cannot establish a longitudinal repeat pitch. The 1.50 m
  pitch comes from the owner's explicit direction, not an inferred measurement.

The owner should review those mounting relationships before finalizing the
construction dimensions. No source edit or alternative part was substituted.

## Architecture and continuous geometry

`qx-style::construction` retains a compositional definition: continuous sweeps,
repeated components, independent profiles, and independent material slots.
The new `AssetSection` profile is a versioned reference to a GLB extrusion.
Existing circle, box, polygon, generic dual-rail and single-beam constructions
continue to work. Family names never dispatch mesh generation.

`qx-rendering::construction_assets::Sources` adapts bounded, validated GLB data
to renderer-independent f64 meshes. Its worker-owned cache retains at most 64
source meshes and is explicitly cleared on library reload. Each component
source is limited to 16,384 vertices / 65,536 indices and one material primitive;
multiple materials must be represented as explicit components instead of being
collapsed. The existing GLB loader additionally limits file size, decoded
textures, node hierarchy and input data. Missing/invalid sources return errors;
no substitute box is generated.

`qx-geometry::source_profile` slices the source halfway along T, welds slice
intersections at a 2 µm tolerance, reconstructs a single closed loop, removes
only collinear triangulation points, and validates winding and simplicity.
It checks that all non-cap source faces lie on the extracted extrusion sides
and that both caps and every side cover the expected area. Nonprismatic
details, holes, open/branched loops and excess complexity fail explicitly. This
is deliberately conservative, rather than a generic mesh-to-sweep approximation.

`build_with_assets` and `build_reference_with_assets` extend the existing sweep
and placement engine through a small resolver interface. Geometry/document
crates import no GPU, widget, or GLB types. Physical track frames, including the
heartline inverse when appropriate, drive all visual construction. Sections and
NURBS transition functions remain authoritative and untouched.

Rails use smooth circular normals and 48 circumferential facets by default.
Longitudinal sampling remains controlled by the existing numerical solver
(normally at most 0.1 m). Sweep rings cross shared section boundaries once;
only the complete path endpoints are capped. Sweep U is cross-section perimeter
in metres; V is physical track arc station in metres. Hardware preserves its
original UVs and normals and shares one local mesh with rigid frame placements.
Very tight curvature, folds or abrupt frame changes produce diagnostics rather
than quiet geometric corrections. The mathematical path stays independent.

Closed-loop topology/welded endpoints are not added by M2. The analytic full
circle is an intentionally open-chain numerical fixture with coincident end
poses; its endpoint caps can coincide. It is not an authored production coaster.

## Materials, transactions and UX

All construction passes through the existing MeshGpu PBR pipeline, procedural
sky reflections, selection outline, texture handling and smoothness conversion.
No shader or lighting replacement is introduced. Rails, spine and cross-ties
have separate component identities and appearance slots. Matching components
carry their complete prior appearance across style assignment: private slots,
colors, finish bindings, tint and UV transforms. New components use the style's
registered defaults. This retains user colors rather than resetting finishes.
Embedded original GLB material maps remain intact in the source files; the
construction's visible finish uses the existing reusable material assignments.

Selection previews on the existing generation worker. Apply is gated on a valid
current preview and commits the style and construction together. Cancel restores
the pre-selection construction draft. Apply style and appearance also commits
the combined change as one transaction. Save-draft resolution, undo/redo and
reopen retain the persisted source references and all numerical settings.
Missing dependencies retain the prior rendered scene and report the source.

Owner-approved behavior implemented: family/style selection in Coaster
Properties; actual construction updates; independent appearance; precise
optional geometry controls; Apply/Cancel, history and persistence. No workspace,
theme, nine UI colors, typography or primary authoring workflow was redesigned.
No new developer-only production workflow is required. The evidence CLI and raw
construction inspector remain development-only. Final pad/rail mounting and
production dimensions still require human design. Trains retain the existing
neutral matte-gray box models and unchanged mechanics.

## Reproduction and manual checks

From the repository root:

```powershell
cargo run -p quantum-x -- edit
cargo run -p quantum-x -- box-spined-scenario target/m2-new-evidence
python tools/audit-box-spined.py target/m2-source-audit.json
```

In the Editor, use **Edit → Coaster Properties → Box Spined → Box Spined-100**,
then Apply after the valid preview. Choose rail/spine/cross-tie finishes and
colors in Track appearance. Advanced construction exposes the three dimensions.
The packaged public executable retains Welcome → New Project → Editor.

The scenario captures the actual egui Editor and wgpu mesh/material renderer
offscreen. It exercises the visible family/style selectors and Apply/Cancel,
then appearance changes, history and save/reopen. Numerical layouts cover a
straight, a smooth hill, horizontal and banked curves, an inverted analytic
circle, and multiple connected sections. It also renders the exact unmodified
source spine and a single original tie for comparison. Catalog preview PNGs
are rendered from registered construction, not generated illustrations.

Native desktop input, continuous interactive frame rate, resize/device loss,
DX12 and DPI behavior are **not manually verified**. Owner checklist:

1. Launch the packaged public executable, create a project, and choose style 100.
2. Inspect rail spacing, chamfers, tie orientation and source pad asymmetry while
   navigating curves, banks and inverted sections.
3. Change all three advanced numeric dimensions, cancel, apply, undo and redo.
4. Recolor rails/spine independently, switch tabs/workspaces, and save/reopen.
5. Inspect custom texture tiling and glossy sky reflections on the desired layout.

See `target/m2-box-spined-final/report.json` and the verification report below
for executed automated checks. No 60 FPS, structural or contact-physics claim
is made from these images or tests.

## Executed verification — 2026-10-31

| Check | Result / evidence |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --workspace` | 291 passed, zero failures; `target/m2-workspace-tests-final-v2.log` |
| `cargo test -p quantum-x box_spined_tests -- --nocapture` | All three passed after the final source-reload regression and explicit scalar-NURBS fixture updates; `target/m2-focused-tests-final.log` |
| `cargo build --release --workspace` | Native Windows Release passed |
| `box-spined-scenario target/m2-box-spined-final` | Six layouts, visible style selector actions, original-source comparison, history and persistence passed |
| `python tools/verify-box-spined.py target/m2-box-spined-final` | Passed; 111,444 changed 3D viewport pixels for independent spine recoloring |
| `python tools/audit-box-spined.py target/m2-box-spined-final/asset-audit.json` | All six originals unchanged |
| `track-scenario target/m2-track-regression-v2` and `python tools/verify-m7-exports.py target/m2-track-regression-v2` | 14 fixtures / 40 real captures, nondegenerate OBJ faces, inverted placement, live edits and cached uploads passed |
| `transition-scenario target/m2-transition-regression` and `python tools/verify-transition-reference.py target/m2-transition-regression` | 10,001 independently evaluated split samples, C2 shape editing and geometry integration passed |
| `material-scenario target/m2-pbr-regression` | 18 GPU readback probes passed, including color spaces, mipmaps, smoothness inversion, normals, UVs, roughness, metallic and AO |
| `material-editor-scenario target/m2-material-editor-regression` and `python tools/verify-material-editor.py target/m2-material-editor-regression` | 12 real Editor captures; named materials, picker/recolor, persistence and neutral gray train checks passed |
| `selection-scenario target/m2-selection-regression` | Surface colors unchanged across selection and property/workspace changes; all comparisons passed |
| `tools/package-windows.ps1 -OutputDirectory target/distribution/m2-box-spined-windows-final` | Both Windows executables, icon resources, startup, catalogs, included projects and imported source construction passed |
| Packaged public executable `render-editor` on the persisted M2 project | Passed; `target/m2-packaged-editor.png` and `.log` |
| Packaged source-asset hashes | All six match the original GLBs; packaged self-check reports four verified construction components |

GPU captures used the NVIDIA GeForce RTX 4070 / Vulkan backend reported by wgpu.
Upload measurements are initialization diagnostics, not interactive frame rates.
M7's input-only test context was corrected to explicitly release unused egui
texture deltas, allowing its full existing verifier to complete.

The portable build is local at
`target/distribution/m2-box-spined-windows-final/QUANTUM Dynamics.exe`.
No commit, push, merge, PR, release or upload was performed. Work remains on the
existing `codex/m1-track-pbr-materials` branch alongside preserved local changes.

## Genuine Editor evidence

These local captures are actual application renders, with no image synthesis or
retouching. They are generated artifacts under ignored `target/`.

| Evidence | Capture |
| --- | --- |
| Style selection and original construction | [Editor](../target/m2-box-spined-final/01-editor-style-selection.png) |
| Parametric rails, glossy spine and original cross-ties | [Close-up](../target/m2-box-spined-final/02-independent-gloss-and-polished-rails.png) |
| Spine recoloring with rail finish retained | [Recolor](../target/m2-box-spined-final/03-independent-spine-recolor-draft.png) |
| Straight | [Editor](../target/m2-box-spined-final/04-straight.png) |
| Smooth hill | [Editor](../target/m2-box-spined-final/05-smooth-hill.png) |
| Horizontal curve | [Editor](../target/m2-box-spined-final/06-horizontal-curve.png) |
| Banked curve | [Editor](../target/m2-box-spined-final/07-banked-curve.png) |
| Inverted analytic loop | [Editor](../target/m2-box-spined-final/08-inverted-loop.png) |
| Connected sections | [Editor](../target/m2-box-spined-final/09-connected-sections.png) |
| Exact original GLB surfaces for comparison | [Source geometry](../target/m2-box-spined-final/10-original-glb-surfaces-comparison.png) |
| Native packaged executable rendering the saved project | [Packaged Editor](../target/m2-packaged-editor.png) |
