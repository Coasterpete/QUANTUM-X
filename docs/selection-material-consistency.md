# Selection without material recoloring

Owner-requested focused correction, 2026-10-31. This extends the existing local,
uncommitted appearance work. It does not redesign the Editor or alter authored
geometry, source textures, library definitions, PBR values or project schema.

## Behavior and integration

Rendered and Rendered + Centerline modes use a thin gold exterior silhouette with
a dark rim. Selection is enabled by default. The existing highlight boolean and
View checkbox are retained; its label is now **Selected section outline**.
Wireframe mode highlights selected edges. Technical mode retains its selected
centerline and boundary markers. Picking, Fit Selection and section navigation
continue to use the existing shared selection.

`mesh.wgsl` no longer mixes gold into PBR base color. There is no
appearance-panel flag in rendering or special treatment when changing panels.
Metallic, roughness/smoothness, maps, normals and reflected environment use the
same existing shader inputs in every panel and workspace.

`selection.rs` / `selection.wgsl` render a separate R8 visibility mask using the
same geometry, placements, station range, component visibility and camera as the
surface draw. The mask depth-tests against the finished scene, without writing
depth. A second pass dilates that visible mask by at most two physical pixels,
clipped to the viewport. Its shader discards every pixel containing opaque scene
geometry, including unselected track and imported GLB assets. Only background
pixels can receive the outline. Thus a selected surface retains exactly the same
GPU pixel values as one rendered with highlighting disabled.

Native and offscreen rendering use the same mask/composite implementation.
Resize recreates its mask and scene-depth binding. No mask pass runs when
highlighting is disabled, nothing is selected, or the mode is technical/wireframe.
Technical lines and the Editor UI draw after the outline, preserving axis and
inspection-marker visibility as well as the underlying material.
The mask is derived GPU state, never saved or entered into undo history.
Appearance preview, Apply/Cancel, undo/redo and save/reopen retain their original
document/command paths.

## Verification

The opt-in diagnostic uses the actual Editor surface with in-memory egui input
and offscreen GPU renders. It never opens or controls a desktop window:

```powershell
cargo run --release -p quantum-x --locked -- selection-scenario target/selection-evidence
cargo run --release -p quantum-x --locked -- material-editor-scenario target/material-editor-evidence
python tools/verify-material-editor.py target/material-editor-evidence
```

Each output directory must be new. `selection-scenario` checks identical viewport,
camera target/distance and GPU pixels for selected glossy red track in Appearance,
Coaster Properties, Section, Reference and Project. It also compares highlighting
on/off inside both Track and Train workspaces, plus glossy red/green/magenta/cyan,
matte red, polished red, satin blue and a textured red finish. Every changed pixel
must have been background in the baseline; shaded pixels must remain identical,
and the selection indicator must produce visible pixels.

GLB checks use actual imported meshes with glossy, polished and textured finishes
overlapping selected track. Their surface pixels must remain identical and
correctly occlude the selection mask. **Existing limitation:** the production
geometry generator explicitly rejects GLB track-construction components
(`HardwareShape::Asset`). These checks validate the working GLB material and
occlusion renderer; they do not claim a complete GLB track-construction workflow.
No unrelated importer or authoring workflow is introduced by this correction.

The material-editor diagnostic exercises live preview, Apply/Cancel, undo/redo,
save/reopen, per-component colors and unchanged track/reference/train data.
Unit coverage also switches all property tabs and both workspaces with the
highlight preference enabled and disabled, asserting selection, material data
and undo history are unaffected.

### Captured evidence (Windows, NVIDIA GeForce RTX 4070, Vulkan)

The final [selection evidence](../target/selection-fix-evidence/evidence.json)
contains **28 actual Editor PNGs / 16 passing GPU pixel comparisons**. The
[Appearance capture](../target/selection-fix-evidence/gloss-red.png) and
[Section capture](../target/selection-fix-evidence/gloss-red-section.png) have
identical viewport pixels, including the selected outline. Identity/style,
Reference and Project comparisons also change zero viewport pixels.

For red track, enabling selection adds 2,352 background outline pixels while
preserving all 40,239 non-background pixels in that viewport. The imported GLB
occlusion fixture preserves all 69,490 non-background pixels while adding 872
outline pixels. Train-workspace selection adds 3,145 background pixels without
altering its 58,107 existing non-background pixels. These counts include existing
technical markers; no tolerance, excluded edge band or recolored surface pixels
are allowed by the comparison.

Captures for glossy red, matte red, polished red, vivid cyan and imported textured
GLB occlusion were opened for visual inspection. These are offscreen test fixtures,
not native desktop screenshots. DX12/other GPUs and sustained FPS were not tested.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --locked` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 286 tests, zero failures/ignored; [final test log](../target/selection-fix-tests-final.log) |
| `cargo build --workspace --release --locked` | Passed; [release log](../target/selection-fix-release.log) |
| `target/release/quantum-x.exe selection-scenario target/selection-fix-evidence` | Passed; [GPU log](../target/selection-fix-evidence.log) |
| `target/release/quantum-x.exe material-editor-scenario target/selection-fix-materials-final` | Passed; twelve captures and [workflow evidence](../target/selection-fix-materials-final/evidence.json) |
| `python tools/verify-material-editor.py target/selection-fix-materials-final` | Passed; [verifier log](../target/selection-fix-material-verifier.log) |

## Limits and owner-executable check

The outline uses physical pixels, with no new appearance preferences. It marks
visible silhouettes; fully occluded or hidden components have no outline. It
does not draw an internal section-boundary stripe across the material. Existing
centerline/boundary overlays remain separately configurable. Additional mask and
fullscreen passes have not been benchmarked for sustained frame rate.

Native desktop interaction is **not manually verified**. Launch the current build:

```powershell
& './target/release/quantum-x.exe' edit
```

Select a section, choose glossy red and leave the camera fixed. Switch Track
Appearance → Identity & style → Section → Reference → Project; confirm the finish
and outline stay consistent. Repeat with matte/metallic finishes and vivid colors.
Toggle Selected section outline, select another section, and hide/show components.
Apply, undo, redo, cancel a second preview, save and reopen. Resize/minimize/restore
and check the silhouette after changing Windows DPI. No desktop control was used
to perform those manual checks.

## UX and Git status

The owner-approved change is material-preserving selection feedback. No new
shipping developer UI or unrelated workflow was added. All diagnostic track/GLB
fixtures are development evidence, not an authored production coaster.
Existing uncommitted work is preserved. No commit, push, PR, merge or publication
is authorized or performed for this correction.
