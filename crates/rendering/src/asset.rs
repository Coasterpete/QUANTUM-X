//! Bounded, self-contained glTF 2.0 GLB adapter. No file or network URI is opened.
//! Model coordinates remain right handed, Y up, in metres; author corrections
//! belong to component instances, never to mechanical dimensions or inertia.
use glam::{DMat3, DMat4, DVec3};
use std::{io::Read, path::Path, sync::Arc, time::Instant};

pub const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_VERTICES: usize = 1_000_000;
const MAX_INDICES: usize = 3_000_000;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// glTF tangent and handedness; zero means derive a frame from UV gradients.
    pub tangent: [f32; 4],
}
#[derive(Debug)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub wrap_s: gltf::texture::WrappingMode,
    pub wrap_t: gltf::texture::WrappingMode,
    pub nearest: bool,
}
#[derive(Clone, Debug)]
pub struct Material {
    pub color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub alpha_cutoff: Option<f32>,
    pub double_sided: bool,
    pub texture: Option<Arc<Texture>>,
    pub normal: Option<Arc<Texture>>,
    pub metallic_map: Option<crate::material_library::ScalarTexture>,
    pub roughness_map: Option<crate::material_library::ScalarTexture>,
    pub occlusion: Option<crate::material_library::ScalarTexture>,
    pub emissive_map: Option<Arc<Texture>>,
    pub emissive: [f32; 3],
    pub normal_scale: f32,
    pub normal_y: f32,
    pub occlusion_strength: f32,
    pub uv_transform: [f32; 4],
}
impl Material {
    pub fn textures(&self) -> [Option<&Arc<Texture>>; 6] {
        [
            self.texture.as_ref(),
            self.normal.as_ref(),
            self.metallic_map.as_ref().map(|m| &m.texture),
            self.roughness_map.as_ref().map(|m| &m.texture),
            self.occlusion.as_ref().map(|m| &m.texture),
            self.emissive_map.as_ref(),
        ]
    }
}
impl Default for Material {
    fn default() -> Self {
        Self {
            color: [1.; 4],
            metallic: 0.,
            roughness: 1.,
            alpha_cutoff: None,
            double_sided: false,
            texture: None,
            normal: None,
            metallic_map: None,
            roughness_map: None,
            occlusion: None,
            emissive_map: None,
            emissive: [0.; 3],
            normal_scale: 1.,
            normal_y: 1.,
            occlusion_strength: 1.,
            uv_transform: [1., 1., 0., 0.],
        }
    }
}
#[derive(Debug)]
pub struct Primitive {
    pub vertices: Vec<Vertex>,
    pub has_uv: bool,
    pub indices: Vec<u32>,
    pub material: Material,
    pub bounds: [[f64; 3]; 2],
}
#[derive(Clone, Debug)]
pub struct Node {
    pub primitive: usize,
    pub transform: [[f64; 4]; 4],
}
#[derive(Debug)]
pub struct Model {
    pub primitives: Vec<Primitive>,
    pub nodes: Vec<Node>,
    pub import_ms: f64,
    pub warnings: Vec<String>,
}
/// Shared CPU geometry plus one rigid component transform. Equality intentionally
/// compares resource identity, so UI change detection never scans vertex arrays.
#[derive(Clone, Debug)]
pub struct AssetInstance {
    pub model: Arc<Model>,
    pub component_id: String,
    pub transform: [[f64; 4]; 4],
}
impl PartialEq for AssetInstance {
    fn eq(&self, b: &Self) -> bool {
        Arc::ptr_eq(&self.model, &b.model)
            && self.component_id == b.component_id
            && self.transform == b.transform
    }
}
pub fn read_glb(path: &Path) -> Result<Vec<u8>, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_FILE_BYTES as u64 {
        return Err("GLB exceeds the 64 MiB import limit; simplify/export selected objects".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err("GLB grew past 64 MiB during import".into());
    }
    Ok(bytes)
}
fn check(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn matrix_valid(m: DMat4) -> bool {
    let d = m.determinant();
    let linear = DMat3::from_mat4(m);
    m.is_finite()
        && d.is_finite()
        && d.abs() > 1e-15
        && linear.to_cols_array().iter().all(|v| v.abs() <= 1e6)
        && linear
            .inverse()
            .to_cols_array()
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1e6)
        && m.x_axis.w == 0.
        && m.y_axis.w == 0.
        && m.z_axis.w == 0.
        && m.w_axis.w == 1.
}
pub fn import_glb(bytes: &[u8]) -> Result<Model, String> {
    let start = Instant::now();
    check(
        bytes.len() <= MAX_FILE_BYTES && bytes.len() >= 20,
        "GLB size must be 20 bytes..64 MiB",
    )?;
    check(
        &bytes[..4] == b"glTF",
        "Expected a binary glTF 2.0 .glb file",
    )?;
    check(
        u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize <= 4 * 1024 * 1024,
        "GLB JSON chunk exceeds 4 MiB; reduce object/material count",
    )?;
    check(
        u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize == bytes.len(),
        "GLB declared length differs from file length",
    )?;
    let glb = gltf::Gltf::from_slice(bytes).map_err(|e| format!("Invalid GLB: {e}"))?;
    let blob = glb.blob.as_deref().ok_or("GLB has no embedded BIN data")?;
    check(
        glb.extensions_used().next().is_none() && glb.extensions_required().next().is_none(),
        "glTF extensions are unsupported; export uncompressed core glTF 2.0 with basic materials",
    )?;
    check(
        glb.animations().next().is_none() && glb.skins().next().is_none(),
        "Animation/skinning is unsupported; export a static rigid model",
    )?;
    check(
        glb.nodes().len() <= 4096
            && glb.meshes().len() <= 1024
            && glb.materials().len() <= 512
            && glb.textures().len() <= 128,
        "GLB object count exceeds import limits",
    )?;
    check(
        glb.buffers().len() == 1,
        "Only one embedded GLB buffer is supported",
    )?;
    for b in glb.buffers() {
        check(
            matches!(b.source(), gltf::buffer::Source::Bin) && b.length() <= blob.len(),
            "Buffer must be embedded and fit the BIN chunk",
        )?;
    }
    for view in glb.views() {
        check(
            view.offset()
                .checked_add(view.length())
                .is_some_and(|n| n <= view.buffer().length()),
            "Buffer view extends beyond embedded data",
        )?;
    }
    // Validate exact byte ranges before the library's convenience readers. Sparse
    // accessors are rejected explicitly rather than risking unchecked iteration.
    for a in glb.accessors() {
        check(
            a.sparse().is_none(),
            "Sparse accessors are unsupported; export dense mesh attributes",
        )?;
        let view = a.view().ok_or("Accessor has no buffer view")?;
        let stride = view.stride().unwrap_or(a.size());
        check(
            stride >= a.size()
                && a.count() <= MAX_INDICES
                && a.offset().is_multiple_of(a.data_type().size())
                && view
                    .offset()
                    .checked_add(a.offset())
                    .is_some_and(|offset| offset.is_multiple_of(a.data_type().size()))
                && stride.is_multiple_of(a.data_type().size()),
            "Invalid accessor stride or excessive element count",
        )?;
        let end = a
            .count()
            .saturating_sub(1)
            .checked_mul(stride)
            .and_then(|n| n.checked_add(a.offset()))
            .and_then(|n| n.checked_add(if a.count() == 0 { 0 } else { a.size() }));
        check(
            end.is_some_and(|n| n <= view.length()),
            "Accessor extends beyond its buffer view",
        )?;
    }
    let mut decoded_bytes = 0usize;
    let mut textures = Vec::new();
    for t in glb.textures() {
        let gltf::image::Source::View { view, mime_type } = t.source().source() else {
            return Err(
                "External image URIs are unsupported; embed PNG/JPEG textures in the GLB".into(),
            );
        };
        let format = match mime_type {
            "image/png" => image::ImageFormat::Png,
            "image/jpeg" => image::ImageFormat::Jpeg,
            _ => return Err("Only embedded PNG/JPEG textures are supported".into()),
        };
        let data = &blob[view.offset()..view.offset() + view.length()];
        let mut reader = image::ImageReader::with_format(std::io::Cursor::new(data), format);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let image = reader
            .decode()
            .map_err(|e| format!("Texture decode failed: {e}"))?;
        check(
            image.color().bits_per_pixel() / image.color().channel_count() as u16 <= 8,
            "Only 8-bit PNG/JPEG GLB maps are supported; source was preserved",
        )?;
        let image = image.into_rgba8();
        decoded_bytes += image.as_raw().len();
        check(
            decoded_bytes <= 128 * 1024 * 1024,
            "Decoded textures exceed 128 MiB; reduce texture sizes",
        )?;
        let sampler = t.sampler();
        textures.push(Arc::new(Texture {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
            wrap_s: sampler.wrap_s(),
            wrap_t: sampler.wrap_t(),
            nearest: sampler.mag_filter() == Some(gltf::texture::MagFilter::Nearest),
        }));
    }
    let mut primitives = vec![];
    let mut mesh_primitives = vec![];
    let (mut vertex_count, mut index_count) = (0, 0);
    for mesh in glb.meshes() {
        let mut ids = vec![];
        for p in mesh.primitives() {
            check(
                primitives.len() < 4096 && p.mode() == gltf::mesh::Mode::Triangles,
                "Only triangle mesh primitives are supported (limit 4096)",
            )?;
            check(
                p.morph_targets().next().is_none(),
                "Morph targets are unsupported; export an applied static mesh",
            )?;
            check(
                p.attributes().all(|(semantic, _)| {
                    matches!(
                        semantic,
                        gltf::Semantic::Positions
                            | gltf::Semantic::Normals
                            | gltf::Semantic::Tangents
                            | gltf::Semantic::TexCoords(0)
                    )
                }),
                "Only POSITION, NORMAL, TEXCOORD_0 and unused TANGENT attributes are supported; remove vertex colors/skinning/extra UV sets",
            )?;
            let reader = p.reader(|_| Some(blob));
            let position = p
                .get(&gltf::Semantic::Positions)
                .ok_or("Mesh primitive has no POSITION data")?;
            check(
                position.data_type() == gltf::accessor::DataType::F32
                    && position.dimensions() == gltf::accessor::Dimensions::Vec3
                    && !position.normalized()
                    && vertex_count + position.count() <= MAX_VERTICES,
                "POSITION must be float VEC3, at most one million total vertices",
            )?;
            if let Some(n) = p.get(&gltf::Semantic::Normals) {
                check(
                    n.data_type() == gltf::accessor::DataType::F32
                        && !n.normalized()
                        && n.dimensions() == gltf::accessor::Dimensions::Vec3
                        && n.count() == position.count(),
                    "NORMAL must be float VEC3 with matching vertex count",
                )?;
            }
            if let Some(uv) = p.get(&gltf::Semantic::TexCoords(0)) {
                check(
                    uv.dimensions() == gltf::accessor::Dimensions::Vec2
                        && (uv.normalized() == (uv.data_type() != gltf::accessor::DataType::F32))
                        && uv.count() == position.count()
                        && matches!(
                            uv.data_type(),
                            gltf::accessor::DataType::F32
                                | gltf::accessor::DataType::U8
                                | gltf::accessor::DataType::U16
                        ),
                    "TEXCOORD_0 must be float or unsigned VEC2 with matching vertex count",
                )?;
            }
            if let Some(t) = p.get(&gltf::Semantic::Tangents) {
                check(
                    t.data_type() == gltf::accessor::DataType::F32
                        && !t.normalized()
                        && t.dimensions() == gltf::accessor::Dimensions::Vec4
                        && t.count() == position.count(),
                    "TANGENT must be float VEC4 with matching vertex count",
                )?;
            }
            if let Some(i) = p.indices() {
                check(
                    i.dimensions() == gltf::accessor::Dimensions::Scalar
                        && !i.normalized()
                        && matches!(
                            i.data_type(),
                            gltf::accessor::DataType::U8
                                | gltf::accessor::DataType::U16
                                | gltf::accessor::DataType::U32
                        )
                        && index_count + i.count() <= MAX_INDICES,
                    "Indices must be unsigned SCALAR, at most three million total",
                )?;
            }
            let positions: Vec<_> = reader
                .read_positions()
                .ok_or("Mesh primitive has no POSITION data")?
                .collect();
            vertex_count += positions.len();
            check(
                !positions.is_empty() && vertex_count <= MAX_VERTICES,
                "Mesh is empty or exceeds one million vertices",
            )?;
            let indices: Vec<u32> = reader
                .read_indices()
                .map(|i| i.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());
            index_count += indices.len();
            check(
                !indices.is_empty()
                    && indices.len().is_multiple_of(3)
                    && index_count <= MAX_INDICES
                    && indices.iter().all(|i| (*i as usize) < positions.len()),
                "Invalid triangle indices or mesh exceeds three million indices",
            )?;
            let mut normals: Vec<_> = reader
                .read_normals()
                .map(Iterator::collect)
                .unwrap_or_default();
            if normals.is_empty() {
                normals.resize(positions.len(), [0.; 3]);
                for t in indices.as_chunks::<3>().0 {
                    let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
                    let n = (glam::Vec3::from(positions[b]) - glam::Vec3::from(positions[a]))
                        .cross(glam::Vec3::from(positions[c]) - glam::Vec3::from(positions[a]));
                    for i in [a, b, c] {
                        normals[i] = (glam::Vec3::from(normals[i]) + n).to_array();
                    }
                }
            }
            let uv: Vec<_> = reader
                .read_tex_coords(0)
                .map(|v| v.into_f32().collect())
                .unwrap_or_else(|| vec![[0.; 2]; positions.len()]);
            check(
                normals.len() == positions.len() && uv.len() == positions.len(),
                "Mesh attribute counts do not match POSITION",
            )?;
            let material = p.material();
            check(
                material.alpha_mode() != gltf::material::AlphaMode::Blend,
                "Alpha BLEND is unsupported; use OPAQUE or MASK",
            )?;
            let pbr = material.pbr_metallic_roughness();
            let normal_tex = material.normal_texture();
            let ao_tex = material.occlusion_texture();
            let emissive_tex = material.emissive_texture();
            let mr_tex = pbr.metallic_roughness_texture();
            let mut map_uvs = vec![];
            if let Some(t) = &normal_tex {
                map_uvs.push(t.tex_coord());
            }
            if let Some(t) = &ao_tex {
                map_uvs.push(t.tex_coord());
            }
            if let Some(t) = &emissive_tex {
                map_uvs.push(t.tex_coord());
            }
            if let Some(t) = &mr_tex {
                map_uvs.push(t.tex_coord());
            }
            check(
                map_uvs
                    .iter()
                    .all(|uv| *uv == 0 && p.get(&gltf::Semantic::TexCoords(0)).is_some()),
                "PBR maps require TEXCOORD_0; additional UV sets are not supported",
            )?;
            check(
                normal_tex
                    .as_ref()
                    .is_none_or(|t| t.scale().is_finite() && (0. ..=4.).contains(&t.scale()))
                    && ao_tex.as_ref().is_none_or(|t| {
                        t.strength().is_finite() && (0. ..=1.).contains(&t.strength())
                    })
                    && material
                        .emissive_factor()
                        .iter()
                        .all(|v| v.is_finite() && (0. ..=1.).contains(v)),
                "Invalid normal, occlusion or emissive factors",
            )?;
            let scalar = |texture: &gltf::texture::Info<'_>, channel| {
                crate::material_library::ScalarTexture {
                    texture: textures[texture.texture().index()].clone(),
                    channel,
                    invert: false,
                }
            };
            let tex = pbr.base_color_texture();
            check(
                tex.as_ref().is_none_or(|t| {
                    t.tex_coord() == 0 && p.get(&gltf::Semantic::TexCoords(0)).is_some()
                }),
                "Base-color texture requires TEXCOORD_0",
            )?;
            let color = pbr.base_color_factor();
            check(
                color
                    .iter()
                    .all(|v| v.is_finite() && (0. ..=1.).contains(v))
                    && pbr.metallic_factor().is_finite()
                    && pbr.roughness_factor().is_finite(),
                "Nonfinite or invalid material factors",
            )?;
            check(
                (0. ..=1.).contains(&pbr.metallic_factor())
                    && (0. ..=1.).contains(&pbr.roughness_factor())
                    && material
                        .alpha_cutoff()
                        .is_none_or(|v| v.is_finite() && (0. ..=1.).contains(&v)),
                "Material metallic, roughness and cutoff factors must be in [0,1]",
            )?;
            let tangents: Vec<_> = reader
                .read_tangents()
                .map(Iterator::collect)
                .unwrap_or_else(|| vec![[0.; 4]; positions.len()]);
            check(tangents.len() == positions.len(), "Tangent count mismatch")?;
            let mut vertices = Vec::with_capacity(positions.len());
            for (((position, normal), uv), tangent) in
                positions.into_iter().zip(normals).zip(uv).zip(tangents)
            {
                check(
                    tangent.iter().all(|v| v.is_finite())
                        && (tangent == [0.; 4]
                            || ((tangent[3].abs() - 1.).abs() < 1e-5
                                && glam::Vec3::from_slice(&tangent).length_squared() > 1e-12)),
                    "Invalid tangent or handedness",
                )?;
                check(
                    position
                        .iter()
                        .chain(&normal)
                        .chain(&uv)
                        .all(|x| x.is_finite()),
                    "Mesh contains nonfinite vertex data",
                )?;
                let normal = glam::Vec3::from(normal)
                    .try_normalize()
                    .ok_or("Zero/degenerate mesh normal; recalculate normals in Blender")?
                    .to_array();
                if p.get(&gltf::Semantic::Tangents).is_some() {
                    let t = glam::Vec3::from_slice(&tangent);
                    check(
                        (t.length_squared() - 1.).abs() <= 1e-3
                            && t.dot(glam::Vec3::from(normal)).abs() <= 1e-3
                            && (tangent[3].abs() - 1.).abs() <= 1e-5,
                        "Tangent must be normalized, perpendicular to normal, with handedness +1 or -1",
                    )?;
                }
                vertices.push(Vertex {
                    position,
                    normal,
                    uv,
                    tangent,
                });
            }
            ids.push(primitives.len());
            let bounds = vertex_bounds(&vertices);
            primitives.push(Primitive {
                vertices,
                has_uv: p.get(&gltf::Semantic::TexCoords(0)).is_some(),
                indices,
                bounds,
                material: Material {
                    color,
                    metallic: pbr.metallic_factor(),
                    roughness: pbr.roughness_factor(),
                    double_sided: material.double_sided(),
                    alpha_cutoff: (material.alpha_mode() == gltf::material::AlphaMode::Mask)
                        .then(|| material.alpha_cutoff().unwrap_or(0.5)),
                    texture: tex.map(|t| textures[t.texture().index()].clone()),
                    normal: normal_tex
                        .as_ref()
                        .map(|t| textures[t.texture().index()].clone()),
                    normal_scale: normal_tex.as_ref().map_or(1., |t| t.scale()),
                    metallic_map: mr_tex.as_ref().map(|t| scalar(t, 2)),
                    roughness_map: mr_tex.as_ref().map(|t| scalar(t, 1)),
                    occlusion: ao_tex
                        .as_ref()
                        .map(|t| crate::material_library::ScalarTexture {
                            texture: textures[t.texture().index()].clone(),
                            channel: 0,
                            invert: false,
                        }),
                    occlusion_strength: ao_tex.as_ref().map_or(1., |t| t.strength()),
                    emissive_map: emissive_tex.map(|t| textures[t.texture().index()].clone()),
                    emissive: material.emissive_factor(),
                    ..Default::default()
                },
            });
        }
        mesh_primitives.push(ids);
    }
    // Validate the complete graph, including unused nodes, before traversing it.
    let nodes: Vec<_> = glb.nodes().collect();
    let mut parents = vec![0u32; nodes.len()];
    for node in &nodes {
        let m = DMat4::from_cols_array_2d(&node.transform().matrix().map(|c| c.map(f64::from)));
        check(
            matrix_valid(m),
            "Node has a nonfinite, singular or non-affine transform",
        )?;
        for c in node.children() {
            parents[c.index()] += 1;
            check(parents[c.index()] <= 1, "Node has multiple parents")?;
        }
    }
    fn visit(
        node: gltf::Node<'_>,
        parent: DMat4,
        stack: &mut Vec<usize>,
        visited: &mut [bool],
        meshes: &[Vec<usize>],
        output: &mut Vec<Node>,
    ) -> Result<(), String> {
        check(
            stack.len() < 128 && !stack.contains(&node.index()),
            "Cyclic node hierarchy or depth exceeds 128",
        )?;
        stack.push(node.index());
        visited[node.index()] = true;
        let m = parent
            * DMat4::from_cols_array_2d(&node.transform().matrix().map(|c| c.map(f64::from)));
        check(
            matrix_valid(m),
            "Composed node transform is nonfinite or singular",
        )?;
        if let Some(mesh) = node.mesh() {
            for &primitive in &meshes[mesh.index()] {
                check(output.len() < 16384, "Too many mesh node placements")?;
                output.push(Node {
                    primitive,
                    transform: m.to_cols_array_2d(),
                });
            }
        }
        for child in node.children() {
            visit(child, m, stack, visited, meshes, output)?;
        }
        stack.pop();
        Ok(())
    }
    let mut visited = vec![false; nodes.len()];
    let mut discarded = vec![];
    for node in nodes.iter().filter(|n| parents[n.index()] == 0) {
        visit(
            node.clone(),
            DMat4::IDENTITY,
            &mut vec![],
            &mut visited,
            &mesh_primitives,
            &mut discarded,
        )?;
    }
    check(visited.iter().all(|b| *b), "Cyclic node hierarchy")?;
    let scene = glb
        .default_scene()
        .or_else(|| glb.scenes().next())
        .ok_or("GLB has no scene")?;
    let mut output = vec![];
    for node in scene.nodes() {
        check(parents[node.index()] == 0, "Scene root also has a parent")?;
        visit(
            node,
            DMat4::IDENTITY,
            &mut vec![],
            &mut visited,
            &mesh_primitives,
            &mut output,
        )?;
    }
    check(
        !output.is_empty(),
        "Selected GLB scene has no triangle meshes",
    )?;
    for node in &output {
        let m = DMat4::from_cols_array_2d(&node.transform);
        let [a, b] = primitives[node.primitive].bounds;
        for x in [a[0], b[0]] {
            for y in [a[1], b[1]] {
                for z in [a[2], b[2]] {
                    let p = m.transform_point3(DVec3::new(x, y, z));
                    check(
                        p.is_finite() && p.abs().max_element() <= 1e9,
                        "Transformed GLB bounds exceed one billion metres; correct export units/transforms",
                    )?;
                }
            }
        }
    }
    let warnings = if glb.cameras().next().is_some() {
        vec!["GLB cameras are ignored; the workspace camera is used".into()]
    } else {
        vec![]
    };
    Ok(Model {
        primitives,
        nodes: output,
        import_ms: start.elapsed().as_secs_f64() * 1000.,
        warnings,
    })
}
impl AssetInstance {
    pub fn bounds(&self) -> (qx_geometry::math::Vec3, qx_geometry::math::Vec3) {
        let mut min = DVec3::splat(f64::INFINITY);
        let mut max = -min;
        for n in &self.model.nodes {
            let m = DMat4::from_cols_array_2d(&self.transform)
                * DMat4::from_cols_array_2d(&n.transform);
            let [a, b] = self.model.primitives[n.primitive].bounds;
            for x in [a[0], b[0]] {
                for y in [a[1], b[1]] {
                    for z in [a[2], b[2]] {
                        let p = m.transform_point3(DVec3::new(x, y, z));
                        min = min.min(p);
                        max = max.max(p);
                    }
                }
            }
        }
        (
            qx_geometry::math::Vec3::new(min.x, min.y, min.z),
            qx_geometry::math::Vec3::new(max.x, max.y, max.z),
        )
    }
}
fn vertex_bounds(vertices: &[Vertex]) -> [[f64; 3]; 2] {
    let mut min = DVec3::splat(f64::INFINITY);
    let mut max = -min;
    for v in vertices {
        let p = glam::Vec3::from(v.position).as_dvec3();
        min = min.min(p);
        max = max.max(p);
    }
    [min.to_array(), max.to_array()]
}
impl Model {
    pub fn cpu_bytes(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        self.nodes.len() * std::mem::size_of::<Node>()
            + self
                .primitives
                .iter()
                .map(|p| {
                    p.vertices.len() * std::mem::size_of::<Vertex>()
                        + p.indices.len() * 4
                        + p.material
                            .textures()
                            .into_iter()
                            .flatten()
                            .filter(|t| seen.insert(Arc::as_ptr(t)))
                            .map(|t| t.rgba.len())
                            .sum::<usize>()
                })
                .sum::<usize>()
    }
    /// Original, deliberately plain unit-box fallback; dimensions come from
    /// component metadata, never from inferred physical mass properties.
    pub fn development_box() -> Self {
        let mut vertices = vec![];
        let mut indices = vec![];
        for (normal, u, v) in [
            (glam::Vec3::X, glam::Vec3::Y, glam::Vec3::Z),
            (-glam::Vec3::X, glam::Vec3::Z, glam::Vec3::Y),
            (glam::Vec3::Y, glam::Vec3::Z, glam::Vec3::X),
            (-glam::Vec3::Y, glam::Vec3::X, glam::Vec3::Z),
            (glam::Vec3::Z, glam::Vec3::X, glam::Vec3::Y),
            (-glam::Vec3::Z, glam::Vec3::Y, glam::Vec3::X),
        ] {
            let base = vertices.len() as u32;
            for (x, y) in [(-1., -1.), (1., -1.), (1., 1.), (-1., 1.)] {
                vertices.push(Vertex {
                    position: ((normal + u * x + v * y) * 0.5).to_array(),
                    normal: normal.to_array(),
                    uv: [(x + 1.) * 0.5, (1. - y) * 0.5],
                    tangent: [0.; 4],
                });
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self {
            primitives: vec![Primitive {
                vertices,
                has_uv: true,
                indices,
                bounds: [[-0.5; 3], [0.5; 3]],
                material: Material {
                    color: [0.35, 0.35, 0.35, 1.],
                    metallic: 0.,
                    roughness: 0.9,
                    alpha_cutoff: None,
                    double_sided: false,
                    texture: None,
                    ..Default::default()
                },
            }],
            nodes: vec![Node {
                primitive: 0,
                transform: DMat4::IDENTITY.to_cols_array_2d(),
            }],
            import_ms: 0.,
            warnings: vec![],
        }
    }
}
