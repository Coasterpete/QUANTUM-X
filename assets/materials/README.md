# Track materials

`library.json` indexes the owner's supplied original maps. Source PNGs are read
only. The library is independent of coaster family/style; construction slots
select stable material IDs. It never textures temporary train placeholders.

All four supplied materials register their surface maps as **smoothness** in
`maps.smoothness`, including pine's original `pinewood_roughness.png`. That
filename is misleading; keep it and the source bytes unchanged. Metadata, not
filenames, determines interpretation. The shader converts linear samples once
using `roughness = 1 - smoothness`; import, GPU upload and mip generation never
invert pixels. Scalar maps use linear `Rgba8Unorm`, never sRGB.

Future genuine roughness maps use `maps.roughness` and pass through without
inversion. A definition cannot specify both map kinds. The scalar `roughness`
factor remains a multiplier after conversion (currently 1 for all supplied maps).

Import fields, color spaces, limits, missing-map behavior, development assignment
controls, GPU previews and known limitations are documented in
[`docs/m1-track-pbr-materials.md`](../../docs/m1-track-pbr-materials.md).

Track painted metal and grout currently have no supplied albedo. Their white
base factors are explicit constants, not replacement textures. Normal maps use
provisional OpenGL (+Y), configurable per material. Height and edge maps are
retained metadata and do not affect rendering in M1.

The presence of these user-provided files does not establish third-party
redistribution rights. M1 adds no new license or provenance claims for them.
