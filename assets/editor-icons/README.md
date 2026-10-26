# Curated editor command icons

Runtime assets are embedded from this directory by `authoring/icons.rs`. The
user-provided `assets/icons/lucide` and `assets/icons/carbon` source trees remain
untouched and are not required at runtime or embedded wholesale.

- Lucide: common file, history, insertion, ordering, Apply/Cancel and preferences
  commands. Preserve **LICENSE-LUCIDE**, including its ISC and inherited notices.
- IBM Carbon: graph fitting, camera fitting and perspective commands. Preserve
  **LICENSE-CARBON** (Apache-2.0). Source artwork belongs to IBM and contributors.

The sole artwork modification is replacing Lucide's `currentColor` stroke with
white, or adding a white root fill to Carbon, to create tintable masks. Geometry
is unchanged. `manifest.json` records original paths and SHA-256 hashes of both
original source bytes and prepared assets. These are owner-provided checkouts;
no upstream revision is asserted.

Semantic command IDs live in the Rust registry. Images use stable `bytes://qx/`
URIs and egui_extras' SVG/texture caches, sized in logical UI points and rasterized
at device/UI DPI. Button image tint follows the text color for hover, selection
and disabled states. Icon-only compact camera buttons retain tooltips and
accessibility labels. No file or HTTP image loaders are enabled.

Only registered assets and these attribution files need to accompany source
distribution. Include license notices alongside any executable distribution.
