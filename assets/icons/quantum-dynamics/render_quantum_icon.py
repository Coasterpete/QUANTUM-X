
import bpy
from pathlib import Path

# QUANTUM Dynamics — Render existing Blender icon scene.
# Preserves the scene's original materials, lighting,
# camera, world shaders, compositor, and color management.

scene = bpy.context.scene

if not bpy.data.filepath:
    raise RuntimeError("Save the Blender project before rendering.")

blend_dir = Path(bpy.data.filepath).resolve().parent

output_dir = blend_dir / "renders"
output_dir.mkdir(parents=True, exist_ok=True)

output_file = output_dir / "quantum_icon_current_scene.png"

# Set only the output destination and image format.
scene.render.image_settings.file_format = "PNG"
scene.render.use_file_extension = True
scene.render.filepath = output_file.as_posix()

print(f"Rendering existing scene: {scene.name}")
print(f"Camera: {scene.camera.name if scene.camera else 'None'}")
print(f"Output: {output_file}")

# Render the scene AS IT IS.
bpy.ops.render.render(write_still=True)

if not output_file.is_file():
    raise RuntimeError("Render completed, but PNG was not created.")

print("QUANTUM Dynamics icon render complete.")
