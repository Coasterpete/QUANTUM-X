# M7 — Integrated procedural track geometry and rendering engine

## Executive summary and recovered work

M7 adds an integrated, renderer-independent construction engine downstream of the
existing mathematical track solver. Both generic dual-rail and single-rail
assemblies follow the existing 90 m M6 straight/force-vector-hill fixture. The
same paths are exercised with strong banking, vertical inversions, and actual
egui graph gestures. No production coaster layout was reauthored.

The resumed pass recovered the construction data model, CPU sweeps, GPU mesh and
instance rendering, editor integration, schema-4 migration, tests, and 22
initial offscreen captures. It extended verification to both construction
strategies through banking and inversions, added captured graph-edit workflows,
and completed this report. The short straight captures are supplementary profile
inspection; they are not the milestone's end-to-end evidence.

This is a usable engine baseline with provisional configurations. It is not a
manufacturer asset catalog, a complete PBR renderer, or train/contact physics.
Native desktop interaction remains an owner acceptance check.

## Git and preservation

Work is local on `codex/m7-procedural-track-engine`, based on `c63ed97` (the PR #4
M4–M6 merge). Before implementation, `origin/main` was fetched with Windows
certificate validation and local `main` was fast-forwarded. No merge commit was
created. The resume preserved that branch and all accumulated changes.

No commit, push, PR, force-push, reset, or release was performed. The owner's
untracked `assets/icons/` Carbon/Lucide source trees remain untouched. No external
packages were added; the dependency changes are links between existing workspace
crates. The reference checkouts and generated evidence are ignored under `target/`.

## Architecture and module responsibilities

| Area | Implementation |
| --- | --- |
| `crates/qx-style/src/construction.rs` | Version-1 validated construction definitions, strategies, profiles, component roles, materials, repeaters and future asset references |
| `crates/qx-geometry/src/construction.rs` | f64 sweeps, concave caps, normals, UVs, bounds, topology checks, allocation budgets, shared hardware templates and placements |
| `crates/qx-document/src/{lib,command,persistence}.rs` | Schema 4, construction persistence, transactional `SetConstruction`, explicit migration from schemas 1–3 |
| `crates/rendering/src/{mesh_gpu.rs,mesh.wgsl}` | Indexed triangle and edge pipelines, reusable GPU buffers, material lighting, hardware instances and station-based section highlighting |
| `crates/rendering/src/{scene,workspace,gpu,offscreen}.rs` | Mesh-aware fitting, display controls, cached uploads, existing surface lifecycle, actual offscreen rendering |
| `apps/quantum-x/src/authoring/{session,construction_ui,panels,layout_ui,commands}.rs` | Background generation/cache, construction drafts, existing Coaster/View contexts, previews and diagnostics |
| `apps/quantum-x/src/authoring/{m7_scenario,m7_live}.rs` | Reproducible curved/inverted fixtures and headless pointer-event workflow evidence |
| Geometry/document/application tests and `tools/verify-m7-exports.py` | Numerical, transactional, input-event, topology, export and GPU-image verification |

The dependency direction is `qx-style` data → `qx-geometry` construction →
`qx-rendering` GPU adaptation. Geometry and document types contain no widgets or
GPU resources. The source `GeneratedTrack` is authoritative and unchanged by
mesh generation. No Bézier conversion or replacement solver was introduced.

`TrackMesh` carries component IDs, material IDs, f64 vertices/normals/UVs,
indices, bounds, and placements. Continuous components use world coordinates;
repeated hardware uses one local template plus transforms. The renderer does not
infer construction from display names, family names, or manufacturer keywords.

## Historical reference findings

The implementation was independently written in Rust. Reference source was read
for architectural context; no C++/C# implementation was transplanted.

- [QUANTUM TrackStyle.hpp](https://github.com/Coasterpete/QUANTUM/blob/9a61ee2b204a049ac8ffaa42c36994bdd07df656/core/include/quantum/coaster/TrackStyle.hpp)
  separates presets, offsets, spine parameters, materials, hardware instances and
  renderer-independent mesh output. Its inspected strategy enum contains only
  `DualRailTubular`; M7 therefore uses a broader component/profile model rather
  than inheriting that restriction.
- [QUANTUM TrackStyle.cpp](https://github.com/Coasterpete/QUANTUM/blob/9a61ee2b204a049ac8ffaa42c36994bdd07df656/core/src/TrackStyle.cpp)
  validates configuration, sweeps profiles and treats hardware presentation
  separately. Region-specific overrides are a useful future extension; M7 does
  not claim to implement those overrides.
- [QuantumCoasterWorksLib TrackFrame](https://github.com/Coasterpete/QuantumCoasterWorksLib/blob/a4ad4663d3b7ab8cf20748c9442fd19bb9d166ec/Quantum.Track/TrackFrame.cs)
  makes global station and the coaster-facing frame an explicit contract.
  [Transform3d](https://github.com/Coasterpete/QuantumCoasterWorksLib/blob/a4ad4663d3b7ab8cf20748c9442fd19bb9d166ec/Quantum.Math/Transform3d.cs)
  distinguishes point translation from direction rotation. M7 retains QUANTUM-X's
  existing X=T, Y=N, Z=L convention and quaternion math.

## Profiles, continuous construction and precision

Profiles support circles (3–128 sides), rectangular boxes and simple closed
polygonal loops (3–128 vertices), including concave polygons. Profile coordinates
are `(N,L)` in metres, with positive CCW area and no repeated closing vertex.
Validation rejects nonfinite values, zero/tiny edges, collinear vertices,
self-intersections and incorrect winding. Polygon caps use ear clipping.

Each sweep transforms an offset profile with the supplied rider-local frame.
Roll is applied only through that frame, plus any explicitly configured profile
rotation. World up is not substituted for N. Side vertices preserve hard polygon
edges; circular profiles use smooth radial normals corrected against the swept
longitudinal direction. Caps have independent end normals. U measures profile
perimeter distance and V measures longitudinal station in metres.

There is one longitudinal stream per component across the entire ordered track,
including shared section samples. Internal section boundaries do not add caps
or duplicate rings. Ends can be capped or left open. Coincident start/end
positions alone do not declare a closed ride circuit; closed-circuit seam
welding is not implemented.

Construction validates triangle indices, nondegenerate faces, normal length and
winding. It rejects frame jumps above 0.75 rad between samples and local sweep
foldbacks instead of repairing authored geometry. Default resource limits are
2 million vertices, 12 million indices, 100,000 placements and 384 MiB of CPU
vertex/index/placement payload. Allocations and integer arithmetic are checked.

All construction calculations remain f64. GPU conversion subtracts a common
scene origin before converting world positions to f32; hardware translations use
the same origin. Tests exercise coordinates around ±500 million metres within
the existing solver's supported position norm. This prevents the loss caused by
converting absolute world coordinates directly to floats, but does not promise
arbitrary precision across a scene spanning enormous distances.

For rider-authored projects, the existing constrained inverse supplies physical
track geometry. Physical arc station drives hardware spacing and UVs; a separate
source station maps selection back to the authored reference. Duplicate one-sided
reference derivative records are coalesced only in the derived mesh input after
checking their positions. The inverse solver's existing compatibility limits
remain in force.

## Dual rail, single rail and future styles

`Strategy` distinguishes DualRail, SingleRail, Hybrid, Wooden and Custom.
Validation requires exactly two primary running components for DualRail and
exactly one for SingleRail. Other strategies can compose multiple supported
members; named production configurations and specialized construction techniques
are future work.

The dual validation definition contains two independent circular rails, a box
spine and repeated cross-ties, with separate material IDs. The provisional ties
intersect both the rails and spine; components remain separate meshes rather
than a boolean-welded manufactured assembly.

The single-rail validation definition contains one concave, flanged beam profile
and its own bracket dimensions, offset and spacing. It is a separate construction
definition with one running component, not a hidden second rail. The shared sweep
and instance code interprets its data without a special renderer.

All dimensions are original provisional validation values. `Hybrid-100`,
`Monorail-100` and `Monorail-200` retain their identities and Concept status in
`assets/catalog/styles.json`. No manufacturer-specific model was introduced.
The two monorail concepts can receive independent profiles, offsets, materials
and repeaters later. Optional typed geometry in a catalog style can populate a
project's construction transactionally; selecting a concept without geometry
leaves the separate validation construction unchanged and says so in the UI.

## Repeating components and future assets

A repeater defines spacing, start station, end margin, local translation,
X/Y/Z rotation, alignment mode, material and visibility. Stations are calculated
from arc distance (`start + k * spacing`), never sample indices or per-section
counters. End margins control endpoint inclusion, and a section join cannot
produce duplicate placements.

The existing Hermite position query and shortest-path quaternion slerp provide
interpolated poses. TrackFrame alignment follows N/L through inversions;
WorldAxes is an explicit alternative. Each hardware component shares its local
box mesh across all GPU instances. These are construction components, not lift,
launch, brake or block-operation devices.

`HardwareShape::Asset` reserves an ID, relative path, version and scale contract.
It validates traversal/identifier/transform inputs and returns an explicit
unresolved-asset error. GLB/glTF loading is **not implemented**. A future importer
can supply a local mesh to the existing component/instance renderer. Units are
metres; local X/Y/Z mean T/N/L, with rotation then translation composed into the
track frame. Material references and asset replacement remain data concerns.

Profiles, placements and reference stations are useful future attachment data.
Render triangles are not validated wheel-contact surfaces or train constraints.
No train dynamics, collision model or ride-safety claim is included.

## Renderer, displays and viewport integration

The existing wgpu backend now has indexed triangle draws with back-face culling,
depth testing/writes, vertex normals and component materials. Base color,
metallic and roughness feed an initial directional/ambient/specular model.
This is not a full energy-conserving PBR/IBL system; textures, shadows and
advanced environments remain future work.

The four modes are Rendered Track, Rendered + Centerline, Technical Lines and
Wireframe / Mesh Inspection. Wireframe uses indexed edges and does not require
an optional GPU polygon-mode feature. Independent controls cover running
components, structural members, repeated hardware, centerline, frames, section
boundaries and selected-section highlighting. Existing diagnostics remain.

Meshes draw after the restrained ground grid. Diagnostic lines draw as explicit
reference overlays in solid modes so they do not vanish behind the track.
Fit All/Selection include visible component bounds and authored station ranges.
Orbit/pan/zoom and world camera state persist across edits. Picking retains the
existing projected-centerline method; arbitrary triangle ray picking is not
implemented.

Vertex/index/edge/instance buffers grow when necessary and are reused otherwise.
An unchanged shared mesh and origin skip uploads. Camera and selection uniforms
update separately. The offscreen path verifies that a repeated identical upload
is skipped. Existing resize, zero-sized window and surface recreation handling
remain; device loss still reports an actionable failure requiring restart.

## Live authoring, caching and persistence

Construction settings live in Track Workspace → Coaster. Display modes are in
View → Track display and the existing View panel; mesh statistics are in
Diagnostics. Controls edit validatable drafts with explicit Apply/Cancel and
ordinary document history. Shape, visibility, offsets, spacing and materials
persist in project schema 4. Schemas 1–3 migrate explicitly; older authored
sections, stable IDs, heartline data and numerical results are preserved.

The existing coalescing worker generates math and meshes for changed candidates.
The newest revision alone publishes. An unchanged candidate or accepted identical
preview does not regenerate. Unchanged initial/section/options inputs reuse the
mathematical result; metadata/concept selection changes also reuse the mesh.
Material or construction changes invalidate construction while preserving source
geometry. Repaints and display visibility switches do not run the CPU generator.

Invalid drafts/generation preserve the prior visible scene and report that it is
not the current invalid candidate. Apply retains the existing transaction rules;
graph gestures over prior drafts stay staged, while clean gestures commit once.
Mesh failure cannot write or reshape the mathematical document. A document whose
initial mesh cannot be built can still be read/generated by document/CLI services,
but opening it directly into the mesh editor can fail with the construction error;
a separate startup technical-only recovery view remains future work.

## Automated demonstrations and screenshots

Reproduce from the repository root:

```powershell
cargo build --workspace --release
.\target\release\quantum-x.exe track-scenario target/m7-review/final
python tools/verify-m7-exports.py target/m7-review/final
```

All images are actual wgpu/egui offscreen renders at 1440×960 and 960×640.
They are not desktop screenshots, native mouse tests, or synthetic renderings.
The evidence directory also contains indexed OBJ meshes, saved projects and
`evidence.json` with geometry, placement, upload and timing data.

| Capture prefix | Evidence |
| --- | --- |
| `a-centerline` | Original M6 hill mathematical path |
| `b-dual-rails`, `c-dual-spine`, `d-dual-hardware` | Same complete hill with rails, then spine, then ties |
| `e-single-beam` | Genuine single-beam assembly on the same hill |
| `k-banked-dual`, `k-banked-single` | Strong-bank hill variants with frame overlays |
| `f-inverted-loop`, `f-inverted-single` | Both strategies through a vertical inversion |
| `g-edited-force-roll` | Explicit changed force/roll fixture and resulting solid geometry |
| `h-technical`, `h-wireframe` | Technical and topology modes |
| `live-dual-*`, `live-single-*` | Before, graph-produced draft, and accepted/reopened mesh |
| `i-dual-detail`, `j-single-detail` | Supplementary short straight profile inspection |

The live workflow sends real egui primary press/move/held-idle/release events
through the workbench. It checks changed mesh publication **while the pointer is
held**, verifies idle revisions stop changing, releases into a preceding name
draft, then captures that exact staged preview. The `draft` screenshot is taken
after pointer release and before Apply; it does not pretend a fresh offscreen
context retains native capture. Apply reuses the published revision. Undo/Redo,
Cancel, disk save and reopen compare actual meshes/documents. Both strategies and
both capture sizes execute this workflow. Separate input tests cover Normal,
Lateral and Roll channel gestures.

## Verification results

Final Release validation on Windows completed successfully:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --workspace` | 152 passed, 0 failed |
| `cargo build --workspace --release` | Passed |
| `python tools/verify-m7-exports.py target/m7-review/final` | 14 fixtures, 40 captures; geometry, placements, viewport changes, live history and upload/cache evidence passed |
| M6 authoring scenario and `verify-m6-exports.py` | Passed; 901 samples, 90 m, peak elevation 10.232834342674371 m, normal-load residual 1.5897799327396767e-5 g |
| `verify-authoring-exports.py` | Passed; analytic total-length error 1.2278178473934531e-11 m |
| `verify-heartline-exports.py` | Passed; radius error 8.107292615022743e-12 m, length error 1.1119993814645568e-10 m |
| `git diff --check` | Passed (line-ending normalization warnings only) |

Offscreen captures used NVIDIA GeForce RTX 4070, Vulkan, driver 617.42.
The numerical authoring and heartline verifiers rechecked existing exported
fixtures; the M6 and M7 scenarios were regenerated for this validation.

Single-run Release CPU wall-clock measurements from `evidence.json`:

| Path / strategy | Samples | Vertices | Expanded triangles | Mesh generation | CPU payload |
| --- | ---: | ---: | ---: | ---: | ---: |
| 90 m dual | 901 | 50,536 | 51,344 | 2.0705 ms | 4,653,752 bytes |
| 90 m single | 901 | 14,456 | 15,012 | 0.5555 ms | 1,333,168 bytes |
| 1,000 m dual | 10,001 | 560,136 | 570,040 | 23.0923 ms | 51,591,528 bytes |
| 1,000 m single | 10,001 | 160,056 | 166,684 | 6.4688 ms | 14,764,800 bytes |

The 90 m cases use the original M6 hill. The 1,000 m cases are development
performance fixtures. These measurements exclude solver time and are not
statistical benchmarks or frame-rate measurements. Live edit tests verify
publication and cache behavior, without claiming an end-to-end latency bound.

Generated evidence is local and ignored by Git:

- [Dual hill](../target/m7-review/final/d-dual-hardware-1440.png)
- [Single hill](../target/m7-review/final/e-single-beam-1440.png)
- [Banked dual](../target/m7-review/final/k-banked-dual-1440.png)
- [Banked single](../target/m7-review/final/k-banked-single-1440.png)
- [Inverted dual](../target/m7-review/final/f-inverted-loop-1440.png)
- [Inverted single](../target/m7-review/final/f-inverted-single-1440.png)
- [Graph-produced draft](../target/m7-review/final/live-dual-draft-1440.png)
- [Machine-readable evidence](../target/m7-review/final/evidence.json)

Native desktop input, DPI, window lifecycle and owner UX acceptance remain
unverified. Automated egui event injection and offscreen rendering are reported
separately from those checks.

## Limitations and next engineering work

- Single simple profile loops and solid capped members are implemented. Hollow
  sections, multiple loops, compound cap topology and boolean joints are not.
- Construction rebuilds the full track after a shape edit. There are no section
  dirty ranges, adaptive mesh LOD or deferred edge-buffer generation yet. CPU
  budgets can reject very large dense tracks; they do not silently decimate them.
- The source sampling controls mesh fidelity. There is no independent adaptive
  sweep tessellator or global self-intersection/collision certification.
- GPU upload timing is CPU-side packing/allocation/queue submission time, not GPU
  execution time. No 60 FPS, frame-time, native responsiveness or memory-peak
  claim is made. Reported CPU payload excludes vector spare capacity, source
  tracks, caches, history, temporary GPU packing and driver allocations.
- Generic materials/geometry are provisional. Production styles, train assets,
  wooden assemblies, region overrides and asset importing are unfinished.
- Windows Vulkan offscreen validation is available. Native DX12, device-loss
  recovery, monitor/DPI interaction and extended native editing need owner tests.
- Exact construction-panel presentation and final visual identity remain subject
  to owner review. The requested existing workbench, graph workflow, typography
  and icon system are retained. No separate temporary authoring application was
  added. Development-only content consists of labeled generic configurations,
  fixtures and CLI evidence tools.

The next engineering milestone should pair an owner-supplied, dimensioned track
style with a tested GLB component importer and section-aware construction cache.
That will exercise the same contracts with real assets while keeping dedicated
contact geometry and train dynamics separate.

## Owner-executable Windows acceptance checklist

Native desktop interaction was **not manually verified**. Start with:

```powershell
.\target\release\quantum-x.exe edit target/m7-review/final/d-dual-hardware.qx.json
```

1. Inspect the entire hill in Rendered + Centerline. Orbit below the spine and
   inspect ties through the curved section; compare Technical and Wireframe.
2. In Coaster, choose the generic single-rail configuration. Preview, Apply,
   Undo and Redo. The centerline must not change with construction strategy.
3. Drag a Normal, Lateral and Roll endpoint. Hold still, move again and release.
   Solids should update during valid motion; idle frames must not regenerate.
4. Stage a numeric/name edit before dragging. Release must retain a draft; Apply
   accepts it once. Undo/Redo must restore the corresponding complete mesh.
5. Cancel a construction and a section draft. Try an invalid dimension and verify
   the explicit error and retained prior view, then correct it.
6. Save, close and reopen. Check construction parameters, materials, sections and
   source graph values. Confirm concept catalog identities remain Concept.
7. Open the two `k-banked-*` and two `f-inverted-*` projects. Inspect running
   members and hardware from both sides with frame overlays on/off.
8. Hide each component role, centerline, frames and boundaries independently;
   Fit All/Selection should frame relevant geometry. Inspect the station cursor.
9. Position the camera, edit a transition, and confirm navigation is preserved.
10. Switch Track/Train workspaces and return. Resize, minimize/restore and test
    100%, 125%, 150% and 200% scaling. Existing graph gestures/docking should remain
    usable. Train/support modeling capabilities are unchanged.
