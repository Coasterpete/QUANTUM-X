# QUANTUM Dynamics — appearance and reflection completion report

2026-10-31 · Windows / native MSVC build · local development distribution.

The approved exact-palette, component recoloring, material-browser and focused
interface cleanup are implemented on top of the existing uncommitted M1/M1.1
work. The shared PBR renderer now reflects a filtered procedural sky, with
distinct polished and rough appearances. The original engineering systems and
assets remain in place. See [implementation and compatibility details](ux-color-reflections.md).

## Executable

```text
C:\Dev8\QUANTUM-X\target\distribution\QUANTUM Dynamics-appearance-20261031\QUANTUM Dynamics.exe
```

Keep the adjacent `assets` directory. The same folder's `quantum-x.exe` supplies
the preserved development CLI. This is a portable unsigned Alpha development
distribution, not an installer or a finished train-physics release.

Executable SHA-256 (both packaged names):
`D7E01E2523A83750B3E4161A83D16B4BFFC45ECDDE9FFDBE9E890B5999DF456D`.

Packaging verifies the original ICO payload in the source and both packaged
executables. Source icon SHA-256 remains
`C1B8F3571FCCA5A95A9D63ED20EB247159B91554395E345C6BE74862C5830A22`.
See [distribution metadata](../target/distribution/QUANTUM%20Dynamics-appearance-20261031/distribution.json).

## Completed behavior

- All nine exact UI colors are centralized and tested. Welcome retains the
  centered lime/blue wordmark, black background and five colorful destinations.
  Dark saturated text gets neutral backing or outlines without changing its hue.
  Menu, workspace, command and graph identities follow the owner references.
- Welcome → name-only New Project → Editor → Coaster Properties is preserved.
  Setup remains application preferences. Docking, Train Workspace, native dialog
  plumbing, section authoring, graph editing and project lifecycle remain.
- Coaster Properties → Track appearance provides component swatches, egui's
  interactive picker, editable hex, exact RGB and HSV controls. Presets can be
  reused across components and twelve saved colors are available per session.
  Color and material are separate. Restore, assignment removal, shared/private
  component appearances, live drafts, Apply/Cancel, history and save/reopen work.
- Material selection uses names, source-map thumbnails or solid-color samples,
  explicit selection and live viewport previews. Four simple finish choices join
  the original library: glossy painted steel, polished steel, satin steel and
  matte paint. No original texture or GLB is changed.
- Updated by the [selection correction](selection-material-consistency.md):
  gold surface tint is removed in every panel. A thin exterior silhouette marks
  selected track without touching shaded pixels; the user's highlight toggle,
  selected section and Fit Selection remain available.
- The existing GGX path gains direction-dependent sky reflections, roughness
  filtering, a correlated-Smith BRDF lookup and a highlight shoulder. Procedural
  and imported meshes use the same lighting/material shader. Metallic and
  nonmetallic finishes remain distinct. Smoothness inversion still occurs once.
- Routine Train inspection uses readable component labels. Raw IDs, constraint
  residuals and experimental mechanics stay in Development tools. Detailed curve
  settings, map inspection, UV placement and fallback material parameters remain
  in optional controls. Error and draft/save feedback stay visible.

Primary changes are in `authoring/theme.rs`, `color_ui.rs`, `material_ui.rs`,
`panels.rs`, `layout_ui.rs`, `graph.rs`, `graph_canvas.rs`, `startup_ui.rs`,
`train_asset_ui.rs`, `qx-style/materials.rs`, and rendering's `environment.rs`,
`material_gpu.rs`, `material_library.rs`, `pbr.wgsl`, `mesh_gpu.rs` and
`offscreen.rs`. Existing M1/M1.1 modules remain the foundation.

## Before / after UX

| Before | After |
| --- | --- |
| Approximate shades | Exact #9AFF00, #0000FF, #5C00FF, #1BFF00, #FF4400, #7906FF, #9304B3, #0DDFDF and #6D05CE |
| Editing fallback color did not tint a library finish | A component's color tints that finish while retaining maps, metallic/roughness and UV placement |
| Mostly technical material dropdowns | Named gallery with image/color samples and live draft preview |
| Several technical disclosures cluttered each component | Compact color/finish rows and one options disclosure |
| Raw train identifiers and numerical residuals in normal inspection | Readable labels and optional Development tools |
| Ambient specular approximation lacked directional reflections | Polished sky horizon/highlights and roughness-filtered reflections |

## Verification

The final check results and logs are listed below. Screenshots were also opened
and visually examined; the image checks are not being treated as visual review.

| Check | Result / evidence |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked` | Passed; [log](../target/ux-pbr-reference/check-completion.log) |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed; [log](../target/ux-pbr-reference/clippy-completion.log) |
| `cargo test --workspace --locked` | Passed: **285 tests**, zero failures/ignored; [log](../target/ux-pbr-reference/tests-completion.log) |
| `cargo build --workspace --release --locked` | Passed; [log](../target/ux-pbr-reference/release-final-preview.log) |
| `material-scenario target/ux-pbr-final-gpu` | Passed: eight materials, eighteen actual GPU comparisons; [evidence](../target/ux-pbr-final-gpu/evidence.json) |
| `material-editor-scenario target/appearance-editor` | Passed: twelve real Editor captures, actual widget events, Apply/Cancel, undo/redo, save/reopen, independent components and unchanged geometry/reference/train data |
| `python tools/verify-material-editor.py target/appearance-editor` | Passed; [log](../target/ux-pbr-reference/appearance-editor-verifier.log) |
| `examples target/ux-pbr-numerical` and `verify-authoring-exports.py` | Passed; independent analytic geometry/frame checks |
| `transition-scenario target/ux-pbr-transitions` and `verify-transition-reference.py` | Passed; 10,001 independently evaluated split samples, C2 edits and independent integration |
| `tools/package-windows.ps1` | Passed; original icon and packaged identity verified |
| `tools/verify-owner-ui.ps1` against the final package | Passed after isolated authoring retry; all remaining checks passed serially. [Results](../target/appearance-verification/checks.json), [log](../target/ux-pbr-reference/package-checks-resumed.log) |
| `ui-reference-scenario … target/appearance-ui` and `verify-ui-reference.py` | Passed: 26 captures, 1440/960 widths, 100/150/200% UI scaling, bounded viewports, actual Welcome palette pixels and unchanged project |
| Original asset preservation | 16,751 existing files hashed; only `assets/materials/library.json` changed, none missing; [evidence](../target/ux-pbr-reference/asset-preservation.json) |
| Native desktop interaction | **Not manually verified.** No desktop control was used. |

New coverage includes all nine exact tokens, sRGB round trips and invalid hex,
legacy white-tint defaults, invalid tints, rendered-library color multiplication,
preserved source pixels/finish/UVs, independent component colors, unchanged track
samples and meshes, draft isolation, history/persistence and appearance-preview
highlight behavior. The environment test verifies finite results and a softened
horizon for rough surfaces. Existing geometry, physics and document tests remain.

One packaged headless authoring process exited partway without a Rust diagnostic
while other GPU checks were active. The identical isolated retry exited 0 and
completed its captures and authoring checks. Its cause is unresolved; reliable
parallel GPU-capture stress is not claimed. The remaining package checks ran
serially. See [retry log](../target/ux-pbr-reference/authoring-retry.log).

GPU evidence uses the available NVIDIA GeForce RTX 4070 / Vulkan path. Windows
DX12 and other GPUs were not separately benchmarked. The GLB/physics PNG verifiers
now accept sky-lit cyan/gold shifts in the existing colored test assets; all
transform, file, simulation and mesh-visibility assertions remain. Neutral train
validation independently asserts equal base channels, no maps, metallic 0 and
roughness 0.9, and measures visible pixels inside the projected train-body bounds.

## Requested screenshots

These are genuine offscreen renders of the application or its explicit material
diagnostic. No screenshot is a painted replacement or composited UI. Test tracks
and material spheres are development fixtures, not owner-authored coaster designs.

1. **Welcome — exact branding/navigation palette.**

   ![Welcome](../target/appearance-ui/welcome-1440-scale-1.png)

2. **Editor — workspace colors and working transition graph.**

   ![Editor](../target/appearance-ui/editor-1440-scale-1.png)

3. **Actual egui component color picker.**

   ![Component color picker](../target/appearance-editor/10-native-egui-color-picker.png)

4. **Independent running rail, spine and cross-tie colors.**

   ![Component colors](../target/appearance-editor/09-component-colors.png)

5. **The same glossy painted-steel finish, recolored blue and red.**

   ![Blue finish](../target/appearance-editor/07-gloss-blue.png)

   ![Red finish](../target/appearance-editor/08-gloss-red.png)

6. **Polished versus rough steel under the same procedural sky/sunlight.**

   ![Polished and rough material swatches](../target/ux-pbr-final-gpu/polished-versus-rough.png)

   [Polished track rails](../target/appearance-editor/11-polished-track.png) and
   [satin rails](../target/appearance-editor/12-satin-track.png) also use the real
   procedural construction path.

7. **Unchanged neutral, matte, untextured placeholder train.**

   ![Neutral placeholder train](../target/appearance-editor/06-neutral-train-placeholders.png)

The blue/red comparison changes 29,472 viewport pixels, excluding UI/status text.
The polished/satin comparison changes 28,036 viewport pixels. Those measurements
establish visible response; the opened renders establish the observed gloss and
roughness distinction. Normal painted finishes retain a white dielectric highlight
while metal uses colored specular response.

## Limits and performance

Reflections contain the procedural sky/ground field only. There are no local
track/scenery reflections, screen-space reflection, ray tracing, shadows, visible
sun/cloud system or weather simulation. The sky is deliberately simple, with a
sharp horizon. Broad diffuse lighting is an approximation. No calibrated outdoor
lighting or ride-safety claim is made.

The environment/LUT is generated once at initialization and retained; each
material pipeline uploads 270,320 bytes once. The existing generation worker
coalesces edits and keeps decoding off the drawing thread. It can still rebuild
construction meshes and reload assigned textures after appearance changes, so
large textured projects can preview more slowly during dragging. No sustained
frame-rate benchmark or 60 FPS result is claimed.

Color multiplies existing texture patterns. It does not replace them with flat
paint or make dark texels brighter. Color presets are session-local, and track
alpha controls are absent because the track shading path is opaque. Library maps
remain external references. Older readers that reject unknown binding fields will
explicitly reject new tinted documents rather than silently dropping their tint.

At 960×640 with 200% UI zoom, a very short graph dock asks the owner to resize or
float it; this is an existing responsive limit. Native Windows dialogs, monitor
DPI changes, dock dragging and pointer feel still require owner desktop checks.

## UX status and owner check

The approved composition, palette, startup flow, precise recoloring and finish
separation are implemented. No new shipping developer-only interface was added.
The CLI captures/spheres/straight track and box train remain explicit development
fixtures. Support authoring, finished train components, full family hardware and
weather presentation remain future human-directed work. No major Box-Spined
geometry integration or final train component-ID scheme was invented.

Owner-executable manual checks:

1. Launch the executable above; check Welcome, keyboard focus, New/Open and
   recents. Create a name-only project and edit family/style inside the Editor.
2. Open Track appearance. Enter exact hex/RGB/HSV colors; drag the picker. Select
   a finish, recolor it, restore color, remove its assignment and separate/share
   an actual component. Confirm feedback, camera commands and color preview.
3. Exercise Apply/Cancel, undo/redo, a second pending section draft, Save and
   reopen. Inspect polished/satin finishes while orbiting the viewport.
4. Open Train Workspace and confirm the unassigned placeholder is matte gray.
   Toggle Development tools to inspect existing mechanics/diagnostics.
5. Resize, float/redock panes and test actual Windows DPI/native dialogs.

## Git

Branch remains `codex/m1-track-pbr-materials`. Existing uncommitted M1/M1.1 and
owner asset work was preserved. All changes remain local and uncommitted, as
explicitly requested. No push, commit, merge, PR or publication was performed.
