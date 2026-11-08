# M1.1 — Live track materials in the Editor

Authority: the owner's M1.1 request. This extends the existing colorful,
high-contrast Editor and M1 rendering path. No new renderer, track layout,
lighting system, train component design, or finished Box-Spined construction is
introduced.

## Investigation and implementation

M1 already resolved `construction.materials[*].library` into the existing mesh
renderer's PBR bind groups. Sweeps and repeated parts referenced stable material
slots, and procedural rails already had perimeter/metres-along-track UVs. The
missing connection was the normal authoring interface: named selectors and UV
controls required Development tools, while ordinary construction exposed mostly
colors and metallic/roughness scalars. Coaster Properties had no normal material
browser or map inspection/recovery action.

Use **Edit → Track Appearance / Materials**, or **Coaster Properties → Track
appearance**. No Development tools or track GLB is required.

- Each used appearance lists the actual components that consume it. Generic
  dual construction shows Running rails, Structural spine and Cross-ties.
  Other components appear by their construction IDs translated into readable
  names; absent parts are not fabricated. Running/Structure/Hardware remain
  rendering roles, rather than an exhaustive list of family-specific parts.
- Select a registered material by name. The existing coalescing worker loads
  its original maps and publishes a live draft preview. Authored sections,
  initial conditions, heartline/reference data and train settings do not change.
- **Inspect maps** opens the selected material's map inventory, source paths,
  color spaces, normal convention, availability and optional original-image
  previews. **Browse material library** inspects even unassigned materials.
  File presence is explicitly distinguished from successful decode on assignment.
  Height and edge maps are shown as metadata only.
- **Texture placement** edits precise UV scales/offsets. Replacing a material
  keeps those coordinates. **Remove assignment**, or Construction default,
  returns that appearance to its saved scalar fallback. Scalar controls live
  inside Fallback appearance, rather than forming the primary material workflow.
- **Individual component overrides → Separate appearance** gives a component
  an independent slot if it shares one. Select its material in the resulting
  named group. **Share appearance with…** explicitly joins another group again.
  Stable component/slot references persist; unused slots are retained so their
  authored data is not silently destroyed. The existing 64-slot bound applies.
- **Reload library** asynchronously refreshes manifest/file availability and
  invalidates material resources while keeping drafts, geometry and identifiers.
  Restore missing files under their original library paths, then reload. A missing
  manifest/map/ID or corrupt rendered map retains the explicit scalar fallback
  and reports an error. No missing binding is automatically erased. Optional
  omitted maps retain neutral sampling defaults. Missing metadata-only height/
  edge files do not invalidate shading.

Appearance uses the existing construction draft and its single **Apply appearance
/ Cancel appearance** transaction. Pending construction dimension edits, if any,
are part of that same draft. Other graph, identity/style and train drafts are
preserved. Existing Apply All & Save, leave guards, Undo/Redo and atomic file
writes handle it normally. Selecting a planned catalog style still only records
design intent. Explicit procedural/style replacement reuses registered bindings
and UV transforms on matching stable material slot IDs; unmatched destination
slots retain their own defaults. There are no family-name-based assignments.

The material refresh fix also keys `MeshGpu` uploads on material-resource identity:
an unchanged mesh must receive restored/replaced textures. Ordinary unchanged
frames still reuse GPU resources. Map checking and image decode run on workers;
there is no filesystem watcher or image decoding in the draw path.

## Interpretation, preservation and compatibility

No source texture or owner GLB is changed, generated, recolored, or repacked.
The M1 shader and map interpretation are retained: all four supplied surface maps
are registered as **smoothness**, including `pinewood_roughness.png`.
`roughness_factor * (1 - sample)` occurs exactly once in the shader, before its
existing numerical shading floor. Genuine roughness uses
`roughness_factor * sample`, without inversion. Base color/emission are sRGB;
normal/scalar textures and mip filtering use linear data. OpenGL (+Y) remains
the provisional default. The registered `normal_convention` can be configured
as `open_gl` or `direct_x`; inspection reports it, and Reload library applies a
manifest change. There is no automatic filename inference or material retuning.

The document remains schema **6**, construction version **1**, with the existing
optional M1 library binding. M1.1 introduces no persisted fields or schema change;
the explicit schema 1–5 migrations remain in force. Projects without bindings
retain valid defaults. Older binaries without M1 binding support reject the
unknown field instead of discarding it. Materials are shared external resources,
not embedded in a project; Save As does not copy the library. Moving projects
between installations requires matching registered IDs and map resources.

Temporary train geometry still uses the existing neutral-gray box material,
metallic 0, roughness 0.9, and no texture maps. Track assignments never reach that
fallback or any train data. No train component-ID work is included.

The existing directional light and hemispheric ambient approximation remain.
They cannot reproduce HDR environment reflections or establish calibrated gloss
fidelity. Strong source normal detail remains visible, including on a thin spine;
no artistic material changes compensate for lighting or scale. HDRI, dynamic sky,
atmosphere, volumetrics, displacement and production lighting remain outside M1.1.

## Reproduction and verification

Run from the repository root; scenario output directories must not already exist:

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
& target/release/quantum-x.exe material-editor-scenario target/m11-editor
python tools/verify-material-editor.py target/m11-editor
& target/release/quantum-x.exe material-scenario target/m11-pbr-gpu
& target/release/quantum-x.exe examples target/m11-numerical
python tools/verify-authoring-exports.py target/m11-numerical
& target/release/quantum-x.exe physics-diagnostic
```

`material-editor-scenario` drives the actual Session/Editor widgets with in-memory
egui events, exercises the live generation worker, and captures the complete
Editor through its existing wgpu offscreen renderer. It uses an opt-in six-metre
straight and one-car neutral-box fixture, never a completed track GLB or a final
coaster design. The six PNGs show defaults, registered procedural rails, a separate
spine material, a changed rail material, smoothness inspection and the ordinary
Train Workspace fallback. Sidecar capture metadata records actual viewport and
projected train bounds. These are real application renders, not desktop screen
grabs, illustrations, or composited replacement interfaces.

The independent Python verifier checks changed pixels inside the viewport,
visible neutral pixels within the projected train-body bounds, persistence and
map semantics. The preexisting M1 GPU probes establish numeric interpretation,
including non-unit roughness factors, single smoothness inversion, genuine
roughness, sRGB/linear data, metallic channels, normal-Y convention and mips.
They now also exercise material reload on an unchanged procedural GPU mesh.

Native Windows/MSVC verification, 2026-10-31:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked` | Passed; `target/m11-check-final.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed; `target/m11-clippy-final.log` |
| `cargo test --workspace --locked` | **279 passed**, zero failed/ignored; `target/m11-workspace-tests.log` |
| `cargo test -p qx-rendering --test material_library --test assets --locked` | **12 passed**; `target/m11-pbr-tests-final.log` |
| `cargo build --workspace --release --locked` | Passed; `target/m11-release-final.log`; no packaging/publication performed |
| Release `material-scenario target/m11-pbr-gpu` | Four supplied materials, eight overview/close-up PNGs, **18 GPU comparison pairs**, plus unchanged-mesh material reload check; all passed |
| Release `material-editor-scenario target/m11-editor-verified` and `python tools/verify-material-editor.py target/m11-editor-verified` | Six real Editor captures, named controls/live drafts, Apply/Cancel, history, save/reopen, unchanged geometry/UV/reference/train data and independent pixel checks |
| Release `examples target/m11-numerical` and `tools/verify-authoring-exports.py` | Passed; analytic length error `1.23e-11 m`; B-spline/NURBS/other authoring references included |
| Release `transition-scenario target/m11-transitions` and `tools/verify-transition-reference.py` | Passed; 10,001 split samples, shared C2 edits and independent geometry integration |
| Release `heartline-demo target/m11-heartline` and `tools/verify-heartline-exports.py` | Passed; radius error `8.11e-12 m`, length error `1.12e-10 m` |
| Release `physics-diagnostic` | Passed; 240 Hz, `2.899053227 m/s` matching analytic speed; maximum joint residual `1.178e-5 m` under its stated ideal-guide assumptions |
| SHA-256 asset comparison | All 16,695 baseline asset files unchanged, including all **25 supplied material PNGs** and **10 Blender/GLB files**; `target/m11-asset-verification.json` |
| `git diff --check` | Passed |

GPU evidence used **NVIDIA GeForce RTX 4070, Vulkan**. All equivalent PBR pairs
differed by at most one 8-bit channel value; smoothness inversion, non-unit-factor
loaded smoothness, genuine loaded roughness, smoothness endpoints and smoothness
mips matched their references exactly. The original Editor pixel verifier found
29,472 changed viewport pixels for rail assignment, 18,790 for the independent
spine assignment and 29,438 for changing the rail material, with 275,142 neutral
pixels in the projected train-body bounds. Final reproduction artifacts and its
verifier output are in `target/m11-editor-verified` and
`target/m11-editor-verified.log`; detailed PBR readbacks are in
`target/m11-pbr-gpu/evidence.json`. Artifacts remain Git-ignored.

Visually inspected real GPU captures:

- [Registered material on procedural rails](../target/m11-editor-verified/02-registered-procedural-rails-draft.png)
- [Independent rail and structural-spine materials](../target/m11-editor-verified/03-independent-spine-draft.png)
- [Live material change in the Editor](../target/m11-editor-verified/04-live-rail-material-change.png)
- [Smoothness semantics and original misleading filename](../target/m11-editor-verified/05-smoothness-inspection.png)
- [Untextured gray train fallback in the ordinary Train Workspace](../target/m11-editor-verified/06-neutral-train-placeholders.png)

These captures verify wiring and interpretation, not final gloss fidelity or
native interaction quality. The synthetic straight and deliberately visible
pine-on-spine comparison are test fixtures, not an authored production layout or
a proposed final track design.

## Remaining scope and UX status

The requested normal named assignment, component override, map inspection,
recovery and draft workflows are implemented using the current Editor palette,
menus, Coaster Properties and construction transactions. Raw construction-slot
controls remain development-only; shape controls retain their provisional
engine-validation labeling. No new developer-only screen
is used for material assignment. Exact grouping/spacing and final ergonomics
remain reviewable implementation choices, not a claim of final owner approval.

Future Box-Spined styles need an owner-approved construction definition with
separate stable material slots for rails, spine, cross-ties, bracing and connection
hardware where present. Procedural sweeps need valid profiles; imported hardware
needs supported geometry integration and suitable TEXCOORD_0/tangents, scale,
pivots and provenance. The current reserved repeated-asset shape still reports
an unresolved-asset error; M1's primitive material adapter does not complete that
import pipeline. The supplied Box-Spined GLBs are preserved and are not installed
as finished styles by this pass. Matching slot IDs allow intentional assignment
reuse; changed part topology needs an explicit mapping rather than a guess.

Native mouse/keyboard, dialogs, DX12, continuous interaction, display resizing
and performance are **not manually verified**. Owner checklist:

1. Run `cargo run -p quantum-x -- edit examples/foundation.qx.json`, then open
   Edit → Track Appearance / Materials with Development tools off.
2. Assign different named materials to rails, spine and cross-ties. Inspect maps,
   original previews and exact UV values; change/remove a binding. Check the
   current Rendered Track display mode if inspecting shading.
3. Separate a running rail's appearance, change it independently, then share it
   again. Confirm absent parts are not listed as available components.
4. Test Cancel, Apply, Ctrl+Z/Ctrl+Y and Apply All & Save, then close/reopen. Keep a
   separate graph draft to confirm appearance acceptance does not consume it.
5. Restore a temporarily missing map, Reload library and check recovery with the
   binding intact. Missing/corrupt maps should report fallback, not crash.
6. Inspect the configured train in Train Workspace. Fallbacks must remain plain
   matte gray boxes. Check narrow/high-DPI layouts and keyboard focus/navigation.

Work remains local on `codex/m1-track-pbr-materials`, preserving the preexisting
uncommitted M1 work. No commit, push, merge, PR, publication or active-desktop
control is authorized or performed.

## Subsequent owner-approved correction — 2026-10-31

The [color and reflection pass](ux-color-reflections.md) extends these bindings
with optional linear-RGB tint, direct component recoloring, a material gallery
and procedural GGX-filtered sky reflections. It supersedes the hemispheric
lighting limitation described for M1.1 above. Original maps and their smoothness
semantics remain unchanged. Refer to the new report for current verification.
