# M1 track PBR material library

M1.1 adds the normal Editor material workflow described in
[Live track materials](m11-live-track-materials.md). The development-selector
instructions below document the original M1 pass; use Edit → Track Appearance /
Materials for ordinary authoring now.

M1 extends the existing `wgpu` mesh and GLB renderers. It does not introduce a
second renderer, integrate the owner's Box-Spined models, or change centerline
construction, NURBS authoring, physics, document commands, or startup behavior.

## Supplied maps

`assets/materials/library.json` indexes the owner's existing PNGs. No source map
is generated, recolored, overwritten, repacked, or converted on disk.

| Stable material ID | Maps | Explicit limitation |
| --- | --- | --- |
| `track-painted-metal` | normal, metallic, smoothness, AO | No base color supplied; explicit white linear base factor |
| `track-bolt-metal` | original diffuse, normal, metallic, smoothness, AO | Normal convention provisionally OpenGL |
| `track-pine` | neutral base color, normal, smoothness, AO | Original `pinewood_roughness.png` is smoothness despite its filename; never used on train fallbacks |
| `track-grout` | normal, metallic, smoothness, AO | No base color supplied; explicit white linear base factor |

Height and edge paths are retained as metadata. They are **not rendered** as
displacement, parallax, or edge wear in M1. All supplied maps are 8-bit PNGs:
2048×2048 except pine at 2240×1390. The owner selected provisional OpenGL (+Y)
because the original normal convention is unknown. `normal_convention: "direct_x"`
flips Y in the shader if later needed; it never edits the image.

## Import and use

1. Keep original maps under `assets/materials/` in any subdirectory. PNG and JPEG
   are supported, up to 4096 pixels per axis and 64 MiB per encoded file. M1
   explicitly rejects higher bit depths rather than silently reducing precision.
2. Add a definition to `assets/materials/library.json`. IDs are stable lowercase
   letters/digits/hyphens; paths are relative to the material directory. Absolute,
   parent-traversing, URI, and escaping symlink paths are rejected. Manifest size
   is bounded at 1 MiB, 256 definitions, and referenced decoded maps at 512 MiB.
3. Reopen the project to reload external maps/manifest. Existing generation
   workers load only assigned definitions; unassigned geometry jobs do not read
   or decode texture files. Sessions read the small manifest for the development selector. There is no live file watcher in M1.
4. In the Editor, enable **View → Development tools**, open the existing Coaster
   construction controls, then **Component materials**. Choose a library material
   for each slot. UV scale/offset are precise numeric values. Add slots as needed;
   each sweep/repeater's development material selector chooses a slot.
5. Use the existing **Apply construction / Cancel construction** transaction.
   Undo/redo, drafts, save, and reopen preserve library IDs and UV transforms.
   Scalars below a bound library material are its explicit missing-file fallback,
   not a tint multiplied into the owner's maps.

Example definition (use actual supplied files, not these illustrative paths):

```json
{
  "id": "my-track-metal",
  "name": "My track metal",
  "base_color": [1, 1, 1, 1],
  "metallic": 1,
  "roughness": 1,
  "normal_convention": "open_gl",
  "normal_scale": 1,
  "occlusion_strength": 1,
  "maps": {
    "base_color": "my-metal/albedo.png",
    "normal": "my-metal/normal.png",
    "metallic": { "path": "my-metal/metallic.png", "channel": "r" },
    "smoothness": { "path": "my-metal/smoothness.png", "channel": "r" },
    "ambient_occlusion": { "path": "my-metal/ao.png", "channel": "r" }
  }
}
```

Use `maps.roughness` **or** `maps.smoothness`, never both; validation rejects an
ambiguous definition. Metadata determines interpretation, never filename suffixes.
Per the owner's clarification, **all four supplied surface maps are smoothness**,
including `tracks/lumber/pine/pinewood_roughness.png`. That original name and its
bytes are preserved, with the path explicitly registered under `maps.smoothness`.

Smoothness converts to `roughness = 1 - smoothness_sample` exactly once in
`pbr.wgsl`. The loader only sets the inversion flag; decoded samples, shared
textures, GPU uploads and mip levels remain uninverted. The scalar roughness
factor is applied **after** conversion:
`roughness = roughness_factor * (1 - smoothness_sample)` in linear space. All
supplied definitions use factor 1. Future genuine roughness maps belong under
`maps.roughness` and use `roughness_factor * roughness_sample` without inversion;
standard glTF metallic/roughness maps retain those genuine roughness semantics.
The existing GGX stability floor clamps final shading roughness to `[0.045, 1]`;
it does not modify source samples or change the conversion. Scalar
channels can be `r`, `g`, `b`, or `a`, so packed ORM data can be reused without
rewriting a source image. Optional `emissive` map and linear RGB `emissive` factor
are supported; the default emission is zero. Height/edge are metadata only.

Base color and emission use `Rgba8UnormSrgb`; normal, metallic, roughness,
smoothness and AO use `Rgba8Unorm`. Base factors are linear. White preserves
the map's colors; no automatic tint, exposure, or normalization is applied.
GPU-generated mip levels filter colors in linear light and data in linear space.
Mip zero is the original decoded image. Normal vectors are normalized during
shading. Linear/trilinear filtering and 4× anisotropic filtering reduce aliasing;
explicit nearest GLB samplers retain nearest filtering without generated mips.

## Reuse, persistence and imported hardware

`qx-style::materials` owns renderer-independent metadata and validation. The
existing construction material has an optional additive `library` binding:

```json
"library": {
  "id": "track-bolt-metal",
  "uv_scale": [1, 1],
  "uv_offset": [0, 0]
}
```

Style construction definitions provide slot defaults. Every sweep and repeater
already references a slot by stable ID, providing component overrides. Rails,
spines, cross-ties, bracing, and bolts can use separate slots or share them.
No family-name checks or Box-Spined-specific assignments exist. The same
definition works with generic dual rails, single rails, future Box-Spined
construction, and imported primitives. Procedural UVs remain the existing
perimeter/metres-along-track convention; hardware and GLBs retain their own UVs.

`qx-rendering::material_library::assign_imported` applies a loaded binding to a
chosen GLB primitive without changing vertices, node transforms or other
primitives. This is a tested adapter for future track hardware integration; M1
does not install or automatically assign the owner's six Box-Spined GLBs.

The bounded GLB loader now accepts normal, standard packed metallic/roughness
(B/G), AO (R), and emissive maps on TEXCOORD_0. Authored tangents are supported;
otherwise a UV-gradient cotangent frame handles procedural surfaces, seams and
mirrored UVs. Degenerate UVs retain the geometric normal. Additional UV sets,
extensions, animation/skinning, and alpha blending remain explicitly unsupported.
See the [glTF 2.0 material specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#materials).

Old projects deserialize unchanged, and unbound materials serialize identically.
No schema migration is required for this additive optional field. Older binaries
reject the new binding field rather than silently losing it. External maps are
library resources, not embedded in `.qx.json`; moving a project between installs
requires the same material IDs and maps. Save As does not duplicate the shared
material library. The Windows portable packager now copies `assets/materials`
beside its existing assets and executables.

Missing/corrupt referenced maps invalidate the complete slot and emit a specific
diagnostic while retaining its authored scalar fallback. Omitted optional maps
use neutral sampling defaults. An intentionally absent base-color map uses the
definition's explicit constant and reports it. Texture uploads are shared by
resource identity and color space; no image decode occurs in GPU upload/draw.

Temporary train fallback geometry is a neutral-gray unit box, metallic 0,
roughness 0.9, with no maps. Track assignments never affect train fallbacks or
existing intentionally imported train assets.

## Verification and remaining limits

Generate actual GPU PNGs and machine-readable readback evidence without desktop
input:

```powershell
cargo run -p quantum-x -- material-scenario target/m1-material-evidence-new
```

The command refuses an existing output directory. It captures each supplied
material on existing generic construction, a separate preexisting GLB diagnostic,
and a close-up unit cube. A separate gray box remains untextured in every
overview. These are labeled diagnostic fixtures, not production coaster assets.
It exercises save/reopen and construction undo/redo, then compares actual GPU
pixels for sRGB color/emission, linear roughness/AO, packed metallic channels,
smoothness inversion, normal-Y convention, map effects and UV scale. Interpretation
probes also load a generated, misleadingly named `source_roughness.png` through
the real manifest/decoder path as both smoothness and genuine roughness, comparing
GPU results to linear scalar references with a non-unit factor. Black/white
smoothness and minified smoothness probes guard endpoints and mip filtering.
`evidence.json` records each supplied material's map kind, original path, channel,
linear color space and factor. Generated probe files stay in the output directory;
no owner assets are generated or altered.

The shading is a shared GGX/Smith/Schlick metallic-roughness model with one
directional light and a simple hemispheric ambient approximation. It has no HDR
environment lighting, calibrated exposure, shadow mapping, displacement, or
specular antialiasing. It is not a Blender lighting match or final material-
quality certification. Source detail and unusually strong normal maps remain
visible; M1 does not artistically correct them.

Native interaction is **not manually verified**. Owner checklist:

- Run `cargo run -p quantum-x -- edit examples/foundation.qx.json` (or another
  existing project), enable Development tools, and assign different rail/spine/
  hardware slots using the controls above.
- Confirm the draft preview updates, Cancel restores it, Apply/Undo/Redo work,
  then save/reopen and verify the same assignments and numeric UV values.
- Switch generic construction/style only deliberately; confirm authored track
  sections and train mechanics remain unchanged. Check viewport resize/display
  modes and that train fallbacks remain plain gray.
- Temporarily move a map using normal file tools, reopen, inspect the diagnostic
  and fallback, then restore the original file and reopen.

UX status: the owner requested reusable materials, component/style overrides,
neutral train placeholders and a diagnostic preview. New selectors and numeric
controls are explicitly development-only additions to existing construction
transactions. A final material-browser/import workflow, previews, visual layout,
and identity remain human design decisions.

## Prior M1 verification (before the smoothness clarification)

The following evidence predates the correction to pine's map interpretation.
New smoothness-correction evidence is recorded separately below.

- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: 271 passed, 0 failed. The log is
  `target/m1-workspace-tests.log`; physics, NURBS/document compatibility and
  editor regression suites are included.
- `tools/package-windows.ps1 -OutputDirectory
  C:/Dev8/QUANTUM-X/target/distribution/m1-track-pbr-complete`: native Release
  build, executable/icon resources, both executable names, catalog, Welcome,
  and direct project startup checks passed. An initially stale Release cache
  required rebuilding the affected workspace packages; final verification used
  the rebuilt artifacts.
- The packaged executable's `material-scenario` produced
  `target/m1-complete-material-evidence/`: eight overview/close-up PNGs plus
  thirteen GPU comparison pairs and `evidence.json`. All comparisons passed;
  equivalent results differed by at most one 8-bit channel value. The backend
  was Vulkan on an NVIDIA GeForce RTX 4070. DX12 was not separately tested.
- The packaged executable also rendered a saved material-bound project in the
  actual Editor to `target/m1-packaged-editor.png`. GPU screenshots were visually
  inspected, including source-map comparison and the separate neutral-gray
  train fallback. Preview GLB swatches reuse only the preexisting unit-box
  primitive, with the train hierarchy excluded.
- All 25 supplied texture files match pre-change SHA-256 hashes; their packaged
  copies match too. No source map or owner Box-Spined GLB was modified.
- `git diff --check`: passed. Work remains local and uncommitted on
  `codex/m1-track-pbr-materials`; no push, PR, merge or publication was performed.

The assignments and previews implement the requested material foundation.
Selectors remain development-only; final material-library UX requires human
design. Native mouse/keyboard acceptance and performance benchmarking were not
performed. No claim of 60 FPS, final lighting fidelity or completed Box-Spined
construction is made.

## Smoothness clarification verification (2026-10-30)

The focused correction changes pine's map registration from `maps.roughness` to
`maps.smoothness`. The other three definitions already registered smoothness.
The existing single shader conversion and linear GPU formats are retained;
there is no renderer redesign, geometry change, or train material assignment.

- `cargo fmt --all -- --check`, `cargo check --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`:
  passed.
- `cargo test -p qx-rendering --test material_library --test assets`: 11 passed.
  `cargo test --workspace`: 272 passed, 0 failed, including shared-source map
  semantics, metadata round trips, ambiguous-map rejection, genuine glTF
  roughness, and the neutral untextured train fallback. Logs are
  `target/m1-smoothness-{check,clippy,tests}.log`.
- `cargo run -p quantum-x -- material-scenario
  target/m1-smoothness-correction-evidence`: passed on native Windows with the
  NVIDIA GeForce RTX 4070 Vulkan backend. Eight supplied-material captures and
  18 GPU comparison pairs were produced. All new smoothness/roughness checks
  matched their scalar references exactly (maximum channel difference 0);
  all equivalent pairs differed by at most 1. Save/reopen and undo/redo passed
  for all four definitions. See the output directory's `evidence.json` and
  `target/m1-smoothness-render.log`.
- Visually inspected the corrected pine overview and bolt close-up. Pine's
  separate train placeholder remains plain gray; material swatches remain
  explicitly diagnostic cubes, not track hardware or train models.
- SHA-256 comparison confirms all 25 supplied material PNGs and all 10 existing
  Blender/GLB assets are byte-for-byte unchanged (94 asset files checked in
  total). Evidence: `target/m1-smoothness-asset-hashes.json`.

No UI or workflow changes were introduced by this correction. Existing
development-only selectors remain provisional; final material-library UX still
requires human design. Native mouse/keyboard interaction, DX12, Release
repackaging, and performance were not tested again for this focused change.
Work remains local and uncommitted on `codex/m1-track-pbr-materials`, preserving
the preexisting working tree. Nothing was pushed, published, or submitted as a PR.
