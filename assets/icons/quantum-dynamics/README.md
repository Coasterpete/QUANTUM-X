# Source asset provenance

The ICO, Blender scene and master PNG in this directory are the owner's original
QUANTUM Dynamics artwork, supplied and approved for the application. The render
script operates on that scene. Builds embed `QUANTUM_Dynamics.ico` unchanged and
never run Blender or regenerate artwork.

The `.blend` and master PNG are retained as editable source assets. Blender
recovery copies (`.blend1`) and the `renders/` working output directory stay local
and are excluded from Git. Upstream Lucide/Carbon research checkouts in sibling
folders also stay local; the application uses the curated SVGs and license
notices in `assets/editor-icons/`.

See [Windows icon integration](../../../docs/windows-icon.md) for resource
verification and the original icon's SHA-256.
