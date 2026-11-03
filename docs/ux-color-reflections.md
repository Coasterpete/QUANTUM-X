# Exact colors, component appearance and procedural reflections

Owner-approved implementation request: 2026-10-31. This pass extends the existing
uncommitted M1/M1.1 material implementation. It preserves the product identity,
startup flow, document/physics systems, original icon and owner assets.

## User workflow and UX status

Welcome retains the centered two-line wordmark, Library / Editor / Info / News /
Setup, New / Open, and recents on black. New asks only for the project name.
Family, numbered style and description remain in Editor → Coaster Properties.
Track stays left, 3D central, Support right and Transition Editor below. Train
and docking retain their existing workspaces and behavior.

Open **Edit → Track Appearance / Materials**, or **Coaster Properties → Track
appearance**. Each actual construction appearance shows its consumers, a swatch,
editable `#RRGGBB`, and its selected finish. Click the swatch for egui's native
interactive color picker. **Color and finish options → RGB / HSV and presets**
provides exact byte RGB, hue in degrees, saturation/brightness percentages,
unrestricted color presets and up to twelve saved colors for this session.
The nine interface colors do not restrict coaster colors. Track shading is
opaque; no nonfunctional alpha control is exposed.

Click the current finish to open the scrollable material gallery. It shows
registered names, actual base-map thumbnails (when supplied), solid-color samples
otherwise, and metal/paint classifications. Selection is explicit; the existing
viewport is the live finish preview. Glossy painted steel, polished steel, satin
steel and matte paint are added as map-free library entries. Original registered
materials, IDs, external paths, conventions and source pixels remain intact.

Appearance changes preview as drafts. Apply appearance creates the existing
construction transaction; Cancel restores the applied construction. Undo, redo,
Apply All & Save, leave guards and atomic persistence remain in use. Other
pending section/identity/train drafts survive an appearance transaction.

Color and finish options also offers Restore color and Remove assignment.
Restore removes a library tint, or returns a default finish to its last applied
color. Removing a library assignment retains the tint as the fallback color.
Replacing a library material preserves tint and UV placement. Selection uses a
thin exterior silhouette in all rendered panels and workspaces; it never tints
the shaded material. The View pane's Selected section outline preference retains
the existing highlight toggle. Selection, picking and Fit Selection remain available.
See [the selection correction](selection-material-consistency.md). For patterned
materials, color multiplies the original pattern; it cannot turn a dark texture
into a lighter flat paint. Choose the solid painted-steel finish for plain paint.

Shared rails initially share an appearance. **Individual component overrides →
Separate appearance** isolates an existing component; Share appearance explicitly
joins another group. No absent hardware, supports, catwalks or train component
IDs are invented. Family/style IDs, construction strategies and original Blender
GLBs are preserved. No major Box-Spined geometry work is included.

This is the owner's approved workflow implemented with existing egui controls.
No new shipping developer-only UI was introduced. The straight track, material
spheres and neutral box train in verification are explicitly development fixtures.
Future support construction, finished train assets/component IDs, detailed family
hardware and weather/sky presentation still require human design and development.

## Before / after

| Before this pass | Result |
| --- | --- |
| Approximate brightened interface hues | Exact owner tokens, with neutral contrast aids for dark saturated text |
| Color edits available mainly on scalar fallbacks | Direct swatch/hex controls tint library materials while preserving their finish |
| Mostly long material dropdowns | Named gallery with thumbnails/samples, selection feedback and live previews |
| Multiple technical disclosures per component | Compact color/finish rows; maps, UVs and scalar controls under optional options |
| Raw train IDs and constraint residuals in routine inspection | Readable component labels; residuals and IDs retained in Development tools |
| Constant hemispheric ambient/specular approximation | Direction-dependent GGX-filtered sky reflections with a BRDF lookup |

Precision curves, NURBS regions, continuity/splitting, graph gestures, force-vector
controls, exact values and numerical diagnostics remain available. Normal errors,
draft status and save failures remain visible. No diagnostic capability was deleted.

## Palette verification

`authoring/theme.rs` centralizes all nine exact sRGB byte values:

| Token | Hex |
| --- | --- |
| LIME | #9AFF00 |
| BLUE | #0000FF |
| VIOLET | #5C00FF |
| GREEN | #1BFF00 |
| ORANGE | #FF4400 |
| ELECTRIC_PURPLE | #7906FF |
| DEEP_PURPLE | #9304B3 |
| CYAN | #0DDFDF |
| PURPLE | #6D05CE |

Reference red and yaw yellow remain separate semantic tokens. Body text remains
neutral. Black/dark-gray backgrounds are preserved. The 2026-11-02 owner correction
removes light rectangular backings from ordinary labels: dark saturated small
text uses the existing neutral foreground, while icons, rules and graphs retain
the exact accent tokens. Large blue branding retains its neutral outline, and
graphs retain their neutral under-stroke.
Keyboard focus/hover/selection borders and text labels remain distinguishable.
The token test checks every exact value; the UI PNG verifier checks actual
Welcome pixels at the existing size/scale matrix.

## Document and rendering integration

`qx-style::materials::Binding` gains optional linear-RGB `tint`, defaulting to
white and omitted when white. Schema remains 6 / construction version 1. Existing
bindings and schema 1–5 migrations continue to load. The reader rejects invalid,
nonfinite or out-of-range tints. Older binaries that deny unknown binding fields
will reject tinted documents explicitly; they cannot silently erase tint.

The picker/hex controls convert sRGB bytes to linear factors once at the UI
boundary. The library adapter multiplies only the definition's linear base-color
factor by the tint. Metallic, roughness, map channels, normal convention/scale,
AO, emission, alpha and UV transforms retain their original semantics. The
original source images are neither recolored nor repacked. Procedural and GLB
pipelines share `material_gpu.rs` / `pbr.wgsl`. Train fallback materials remain
equal-channel gray, metallic 0, roughness 0.9, with no texture maps.

Smoothness remains **roughness_factor × (1 − sample)** exactly once in the shader.
Genuine roughness remains **roughness_factor × sample**. Filenames never determine
semantics. The existing 0.045 numerical shading floor remains.

`rendering/environment.rs` generates an original analytic sky/ground radiance
field and seven GGX-prefiltered cubemap levels (64² down to 1²), plus a 32²
correlated-Smith split-sum BRDF table. Deterministic Hammersley sampling uses 128
samples. Roughness selects the reflection mip; polished surfaces retain the
sky horizon, rough surfaces blur it. Existing directional sunlight supplies the
direct GGX highlight. The broadest convolution approximates diffuse illumination.
This is a small distant-light foundation, not a weather/atmosphere system.

The formulation follows the documented split-sum approach described by
[Filament's IBL discussion](https://google.github.io/filament/main/filament.html).
Linear factors and sRGB color textures follow the
[glTF material specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html).
No reference implementation code or third-party sky images were copied.

Linear shaded radiance receives a local Reinhard shoulder before the sRGB target
conversion, preventing simple hard clipping of highlights. UI colors are drawn
afterward and bypass this mapping. Reflections contain **only the procedural
sky/ground environment**, not track geometry, scenery, trains or nearby objects.
No reflection probe, screen-space reflection, ray tracing, shadows, visible sun,
clouds or full atmospheric model is claimed. The simple sky has a deliberate
horizon discontinuity; it is not a calibrated outdoor-light measurement.

## Performance and verification method

CPU environment data is generated once during renderer initialization and cached
for the process. Each material pipeline uploads 270,320 bytes of environment/LUT
data once and retains it across appearance edits. No per-frame convolution or
texture decoding runs on the drawing thread. Centerline/reference caches are
reused when their inputs match. The existing worker still rebuilds construction
meshes and can decode assigned maps again after an appearance edit; large
textured projects may therefore preview more slowly while dragging the picker.
Jobs coalesce, and unchanged frames reuse resources. This pass does not claim a
material-only uniform-update optimization. Gallery images decode on workers and
downsample only oversized derived previews to at most 512²; small
images retain their original size. Original files are unchanged.

GPU probes render through the actual procedural and imported pipelines on the
available NVIDIA RTX 4070 Vulkan adapter. They compare color-space handling,
roughness/smoothness (including non-unit factors and misleading filenames), normal
conventions, AO, metallic channels, mip filtering, UVs, emission, finish response
and material-resource replacement on an unchanged mesh. A numerical environment
test checks finite values and roughness-dependent softening of the horizon.

No sustained native-frame-rate benchmark or 60 FPS claim is made. The supplied
screenshots are genuine offscreen wgpu/egui application captures, not composited
mockups or desktop screenshots. Automated in-memory events exercise the widgets;
they do not control the owner's active desktop. Native desktop interaction is
**not manually verified**. Rendered PNGs were visually examined separately.

## Reproduce

Run from `C:\Dev8\QUANTUM-X` with new output directories:

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
& target/release/quantum-x.exe material-editor-scenario target/color-editor-new
python tools/verify-material-editor.py target/color-editor-new
& target/release/quantum-x.exe material-scenario target/color-gpu-new
& target/release/quantum-x.exe examples target/color-numerical-new
python tools/verify-authoring-exports.py target/color-numerical-new
& target/release/quantum-x.exe transition-scenario target/color-transitions-new
python tools/verify-transition-reference.py target/color-transitions-new
```

The [completion report](ux-color-reflections-report.md) records final check results, screenshots,
package location and remaining owner-executable checks.

Git remains local on `codex/m1-track-pbr-materials`. Existing uncommitted work is
preserved. No commit, push, PR, merge or publication is authorized in this pass.
