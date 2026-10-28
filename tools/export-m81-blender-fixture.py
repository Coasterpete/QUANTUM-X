"""Run with Blender --background --factory-startup --python this-file -- output.glb.
Round-trip our original development shell through Blender's real glTF exporter.
No native window or desktop interaction. Blender is not a build dependency.
"""
import sys
from pathlib import Path
import bpy

root = Path(__file__).resolve().parents[1]
output = Path(sys.argv[sys.argv.index("--")+1]).resolve()
bpy.ops.object.select_all(action="SELECT")
bpy.ops.object.delete(use_global=False)
bpy.ops.import_scene.gltf(filepath=str(root / "assets/development/train/development-body.glb"))
bpy.ops.export_scene.gltf(filepath=str(output), export_format="GLB", export_yup=True,
                          export_animations=False, export_cameras=False, export_lights=False)
print(f"Original development shell exported by Blender {bpy.app.version_string}: {output}")
