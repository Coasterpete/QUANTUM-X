//! Bounded, read-only material import, outside the interactive GPU upload path.
use crate::asset::{Material, Texture};
use qx_style::{
    construction,
    materials::{Binding, Library, NormalConvention, ScalarMap},
};
use std::{collections::BTreeMap, io::Read, path::Path, sync::Arc};

#[derive(Clone, Debug)]
pub struct ScalarTexture {
    pub texture: Arc<Texture>,
    pub channel: usize,
    /// Shader-only `1 - sample`; decoded pixels and GPU uploads stay original.
    pub invert: bool,
}
#[derive(Default, Debug)]
pub struct LoadedMaterials {
    /// Keyed by construction material slot, not track family or component role.
    pub slots: BTreeMap<String, Material>,
    pub warnings: Vec<String>,
}
pub fn read_manifest(root: &Path) -> Result<Library, String> {
    let bytes = bounded_read(&root.join("library.json"), 1024 * 1024)?;
    let library: Library =
        serde_json::from_slice(&bytes).map_err(|e| format!("Material library: {e}"))?;
    library.validate().map_err(|e| e.to_string())?;
    Ok(library)
}
/// Small file-availability inspection for a browser worker; decoded validity is
/// established by load_bindings when assigned. Metadata-only maps are inspected
/// too, but their absence never invalidates an otherwise renderable material.
pub fn inspect_maps(root: &Path, library: &Library) -> BTreeMap<String, Result<(), String>> {
    let canonical = root.canonicalize().map_err(|e| e.to_string());
    library
        .materials
        .iter()
        .flat_map(|d| d.maps.entries())
        .map(|(_, path, _, _)| {
            let result = (|| {
                let root = canonical.as_ref().map_err(Clone::clone)?;
                let resolved = root.join(path).canonicalize().map_err(|e| e.to_string())?;
                if !resolved.starts_with(root) {
                    return Err("Map escapes material library".into());
                }
                let metadata = resolved.metadata().map_err(|e| e.to_string())?;
                if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
                    return Err("Map must be a file of at most 64 MiB".into());
                }
                Ok(())
            })();
            (path.to_owned(), result)
        })
        .collect()
}
fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err(format!(
            "{} exceeds {} MiB",
            path.display(),
            limit / 1024 / 1024
        ));
    }
    Ok(bytes)
}
struct Loader<'a> {
    root: &'a Path,
    textures: BTreeMap<String, Arc<Texture>>,
    decoded: usize,
}
impl Loader<'_> {
    fn texture(&mut self, path: &str) -> Result<Arc<Texture>, String> {
        if let Some(t) = self.textures.get(path) {
            return Ok(t.clone());
        }
        let root = self.root.canonicalize().map_err(|e| e.to_string())?;
        let resolved = self
            .root
            .join(path)
            .canonicalize()
            .map_err(|e| format!("{path}: {e}"))?;
        if !resolved.starts_with(&root) {
            return Err(format!("Map escapes material library: {path}"));
        }
        let bytes = bounded_read(&resolved, 64 * 1024 * 1024)?;
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(128 * 1024 * 1024);
        reader.limits(limits);
        let decoded = reader.decode().map_err(|e| format!("{path}: {e}"))?;
        // Reject high-bit-depth sources explicitly instead of silently quantizing.
        if decoded.color().bits_per_pixel() / decoded.color().channel_count() as u16 > 8 {
            return Err(format!(
                "{path}: M1 supports 8-bit PNG/JPEG maps; source was preserved"
            ));
        }
        let image = decoded.into_rgba8();
        self.decoded += image.as_raw().len();
        if self.decoded > 512 * 1024 * 1024 {
            return Err("Referenced material maps exceed the 512 MiB decoded budget".into());
        }
        let texture = Arc::new(Texture {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
            wrap_s: gltf::texture::WrappingMode::Repeat,
            wrap_t: gltf::texture::WrappingMode::Repeat,
            nearest: false,
        });
        self.textures.insert(path.into(), texture.clone());
        Ok(texture)
    }
    fn scalar(
        &mut self,
        map: &Option<ScalarMap>,
        invert: bool,
    ) -> Result<Option<ScalarTexture>, String> {
        map.as_ref()
            .map(|m| {
                Ok(ScalarTexture {
                    texture: self.texture(&m.path)?,
                    channel: m.channel.index(),
                    invert,
                })
            })
            .transpose()
    }
    fn material(&mut self, library: &Library, binding: &Binding) -> Result<Material, String> {
        binding.validate().map_err(|e| e.to_string())?;
        let d = library
            .materials
            .iter()
            .find(|m| m.id == binding.id)
            .ok_or_else(|| format!("Unknown material ID: {}", binding.id))?;
        Ok(Material {
            color: [
                d.base_color[0] * binding.tint[0],
                d.base_color[1] * binding.tint[1],
                d.base_color[2] * binding.tint[2],
                d.base_color[3],
            ],
            metallic: d.metallic,
            roughness: d.roughness,
            texture: d
                .maps
                .base_color
                .as_ref()
                .map(|p| self.texture(p))
                .transpose()?,
            normal: d
                .maps
                .normal
                .as_ref()
                .map(|p| self.texture(p))
                .transpose()?,
            metallic_map: self.scalar(&d.maps.metallic, false)?,
            // Metadata is authoritative even for mislabelled filenames. Keep
            // source samples intact; only the PBR shader inverts smoothness.
            roughness_map: self.scalar(
                d.maps
                    .roughness
                    .as_ref()
                    .map_or(&d.maps.smoothness, |_| &d.maps.roughness),
                d.maps.smoothness.is_some(),
            )?,
            occlusion: self.scalar(&d.maps.ambient_occlusion, false)?,
            emissive_map: d
                .maps
                .emissive
                .as_ref()
                .map(|p| self.texture(p))
                .transpose()?,
            emissive: d.emissive,
            normal_scale: d.normal_scale,
            normal_y: if d.normal_convention == NormalConvention::DirectX {
                -1.
            } else {
                1.
            },
            occlusion_strength: d.occlusion_strength,
            uv_transform: [
                binding.uv_scale[0],
                binding.uv_scale[1],
                binding.uv_offset[0],
                binding.uv_offset[1],
            ],
            ..Default::default()
        })
    }
}
/// Missing requested maps invalidate that slot as a unit. The caller retains
/// its scalar fallback and reports the error; no partial texture substitution.
pub fn load_bindings(root: &Path, slots: &[construction::Material]) -> LoadedMaterials {
    let mut result = LoadedMaterials::default();
    if !slots.iter().any(|m| m.library.is_some()) {
        return result;
    }
    let library = match read_manifest(root) {
        Ok(l) => l,
        Err(e) => {
            result.warnings.push(e);
            return result;
        }
    };
    let mut loader = Loader {
        root,
        textures: BTreeMap::new(),
        decoded: 0,
    };
    for slot in slots {
        if let Some(binding) = &slot.library {
            match loader.material(&library, binding) {
                Ok(material) => {
                    if material.texture.is_none()
                        && !library
                            .materials
                            .iter()
                            .any(|d| d.id == binding.id && d.maps.entries().is_empty())
                    {
                        let warning = format!(
                            "{}: no base-color map supplied; using the library's explicit constant color",
                            binding.id
                        );
                        if !result.warnings.contains(&warning) {
                            result.warnings.push(warning);
                        }
                    }
                    result.slots.insert(slot.id.clone(), material);
                }
                Err(e) => result
                    .warnings
                    .push(format!("{}: {e}; scalar fallback retained", slot.id)),
            }
        }
    }
    result
}
/// Apply a reusable track binding to an imported primitive without changing its
/// geometry, embedded source maps or other primitives. Train placeholders never
/// call this adapter. Load work should run on an asset/generation worker.
pub fn assign_imported(
    model: &mut crate::asset::Model,
    primitive: usize,
    material: &Material,
) -> Result<(), String> {
    let p = model
        .primitives
        .get_mut(primitive)
        .ok_or("Unknown imported primitive")?;
    if !p.has_uv && material.textures().into_iter().any(|t| t.is_some()) {
        return Err("Track material maps require TEXCOORD_0 on the imported primitive; UV unwrap it in Blender".into());
    }
    p.material = material.clone();
    Ok(())
}
