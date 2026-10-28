# M8.1 original development assets

These tiny box assemblies are original QUANTUM-X verification fixtures, licensed
MIT OR Apache-2.0 under the repository licenses. They are neither production
rolling stock nor manufacturer replicas. Bogie blocks are visual references,
not wheel/contact geometry. Body and bogie use +X forward, +Y up, +Z lateral,
metres, embedded checker textures and node hierarchies. The body's mirrored
side also exercises negative node scale and face orientation.

Regenerate deterministically with `python tools/make-m81-fixtures.py`.

`development-blender-body.glb` is the same original shell imported/exported in
background mode with Blender 4.5.4 LTS (Khronos glTF Blender I/O v4.5.49).
Regenerate with:
`blender --background --factory-startup --python tools/export-m81-blender-fixture.py -- assets/development/train/development-blender-body.glb`.
Blender is optional for regeneration and is not a QUANTUM-X build dependency.
