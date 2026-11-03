//! GPU readback regression probes, using the same upload and draw paths as the editor.
use crate::{
    DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput,
    asset::{AssetInstance, Material, Model, Texture},
    material_library::ScalarTexture,
};
use qx_geometry::math::Vec3;
use std::{path::Path, sync::Arc};

struct Probe {
    model: Arc<Model>,
}
impl EditorSurface for Probe {
    fn ui(
        &mut self,
        ctx: &egui::Context,
        _: &InspectionScene,
        display: &mut DisplayOptions,
    ) -> SurfaceOutput {
        *display = DisplayOptions {
            asset_instances: vec![AssetInstance {
                model: self.model.clone(),
                component_id: "GPU diagnostic cube".into(),
                transform: glam::DMat4::IDENTITY.to_cols_array_2d(),
            }],
            train_bounds: Some((Vec3::new(-0.7, -0.7, -0.7), Vec3::new(0.7, 0.7, 0.7))),
            grid: false,
            centerline: false,
            boundaries: false,
            highlights: false,
            inspection_marker: false,
            ..Default::default()
        };
        let mut viewport = egui::Rect::NOTHING;
        ctx.root_ui(|root| {
            viewport = root.available_rect_before_wrap();
        });
        SurfaceOutput {
            viewport,
            view_command: None,
            close: false,
            window_command: None,
            profile: 0,
            reset_navigation: false,
        }
    }
    fn poll_scene(&mut self) -> Option<InspectionScene> {
        None
    }
    fn open_path(&mut self, _: &Path) {}
    fn request_close(&mut self) -> bool {
        true
    }
}
fn texture(rgba: &[u8]) -> Arc<Texture> {
    Arc::new(Texture {
        width: (rgba.len() / 4) as u32,
        height: 1,
        rgba: rgba.to_vec(),
        wrap_s: gltf::texture::WrappingMode::Repeat,
        wrap_t: gltf::texture::WrappingMode::Repeat,
        nearest: true,
    })
}
fn scalar(value: u8, invert: bool) -> Option<ScalarTexture> {
    Some(ScalarTexture {
        texture: texture(&[value, value, value, 255]),
        channel: 0,
        invert,
    })
}
// Exercise manifest -> decoder -> GPU, including a misleading source filename.
// These tiny generated fixtures live only in the diagnostic output directory.
fn imported_scalar_probes(root: &Path) -> Result<[Material; 2], String> {
    use qx_style::{construction, materials::Binding};
    let root = root.join("interpretation-fixture");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let path = root.join("source_roughness.png");
    image::save_buffer(&path, &[0, 64, 192, 255], 1, 1, image::ColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    let original = std::fs::read(&path).map_err(|e| e.to_string())?;
    let kinds = ["smoothness", "roughness"];
    let definitions = kinds.map(|kind| {
        serde_json::json!({
            "id":kind, "name":kind, "base_color":[0.45,0.45,0.45,1],
            "metallic":0, "roughness":0.8,
            "maps":{(kind):{"path":"source_roughness.png","channel":"g"}}
        })
    });
    std::fs::write(
        root.join("library.json"),
        serde_json::to_vec(&serde_json::json!({"version":1,"materials":definitions}))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let slots = kinds.map(|kind| construction::Material {
        id: kind.into(),
        library: Some(Binding::new(kind)),
        base_color: [0.45; 3],
        metallic: 0.,
        roughness: 0.8,
    });
    let mut loaded = crate::material_library::load_bindings(&root, &slots);
    let mut take = |id| {
        loaded
            .slots
            .remove(id)
            .ok_or_else(|| format!("GPU fixture {id}: {:?}", loaded.warnings))
    };
    let result = [take("smoothness")?, take("roughness")?];
    if std::fs::read(&path).map_err(|e| e.to_string())? != original {
        return Err("Scalar interpretation modified the source file".into());
    }
    Ok(result)
}
pub fn verify_material_rendering(
    scene: &InspectionScene,
    root: &Path,
) -> Result<serde_json::Value, String> {
    verify_mesh_material_reload(scene)?;
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let mut scene = scene.clone();
    scene.mesh = None;
    let capture = |name: &str, material: Material| -> Result<Vec<u8>, String> {
        let mut model = Model::development_box();
        model.primitives[0].material = material;
        let mut probe = Probe {
            model: Arc::new(model),
        };
        let path = root.join(format!("{name}.png"));
        crate::render_offscreen(&scene, &mut probe, &path, 640, 480)?;
        Ok(image::open(&path)
            .map_err(|e| e.to_string())?
            .into_rgba8()
            .into_raw())
    };
    let base = Material {
        color: [0.45, 0.45, 0.45, 1.],
        roughness: 0.6,
        ..Default::default()
    };
    let mut records = vec![];
    let mut compare = |name: &str,
                       a: Material,
                       b: Material,
                       equivalent: bool|
     -> Result<(), String> {
        let a = capture(&format!("{name}-a"), a)?;
        let b = capture(&format!("{name}-b"), b)?;
        let maximum = a
            .iter()
            .zip(&b)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        let changed = a
            .iter()
            .zip(&b)
            .filter(|(a, b)| a.abs_diff(**b) > 2)
            .count();
        if (equivalent && maximum > 2) || (!equivalent && changed < 100) {
            return Err(format!(
                "GPU {name} comparison failed: max {maximum}, changed {changed}"
            ));
        }
        records.push(serde_json::json!({"probe":name,"equivalent":equivalent,"max_channel_difference":maximum,"changed_channels_over_2":changed}));
        Ok(())
    };
    let linear_gray = ((128. / 255. + 0.055_f32) / 1.055).powf(2.4);
    let mut filtered = texture(&[0, 0, 0, 255, 255, 255, 255, 255]);
    Arc::get_mut(&mut filtered).unwrap().nearest = false;
    compare(
        "srgb-mip-linear-light",
        Material {
            color: [1.; 4],
            texture: Some(filtered.clone()),
            uv_transform: [4096., 4096., 0., 0.],
            ..base.clone()
        },
        Material {
            color: [0.5, 0.5, 0.5, 1.],
            ..base.clone()
        },
        true,
    )?;
    compare(
        "data-mip-linear",
        Material {
            roughness: 1.,
            roughness_map: Some(ScalarTexture {
                texture: filtered.clone(),
                channel: 0,
                invert: false,
            }),
            uv_transform: [4096., 4096., 0., 0.],
            ..base.clone()
        },
        Material {
            roughness: 128. / 255.,
            ..base.clone()
        },
        true,
    )?;
    compare(
        "smoothness-mip-linear",
        Material {
            roughness: 1.,
            roughness_map: Some(ScalarTexture {
                texture: filtered,
                channel: 0,
                invert: true,
            }),
            uv_transform: [4096., 4096., 0., 0.],
            ..base.clone()
        },
        Material {
            roughness: 1. - 128. / 255.,
            ..base.clone()
        },
        true,
    )?;
    compare(
        "srgb-base",
        Material {
            color: [1.; 4],
            texture: Some(texture(&[128, 128, 128, 255])),
            ..base.clone()
        },
        Material {
            color: [linear_gray, linear_gray, linear_gray, 1.],
            ..base.clone()
        },
        true,
    )?;
    compare(
        "linear-roughness",
        Material {
            roughness: 1.,
            roughness_map: scalar(128, false),
            ..base.clone()
        },
        Material {
            roughness: 128. / 255.,
            ..base.clone()
        },
        true,
    )?;
    compare(
        "smoothness-inversion",
        Material {
            roughness: 1.,
            roughness_map: scalar(64, true),
            ..base.clone()
        },
        Material {
            roughness: 1.,
            roughness_map: scalar(191, false),
            ..base.clone()
        },
        true,
    )?;
    let [smoothness, roughness] = imported_scalar_probes(root)?;
    compare(
        "loaded-smoothness-linear-once",
        smoothness,
        Material {
            roughness: 0.8 * (1. - 64. / 255.),
            ..base.clone()
        },
        true,
    )?;
    compare(
        "loaded-roughness-linear-uninverted",
        roughness,
        Material {
            roughness: 0.8 * 64. / 255.,
            ..base.clone()
        },
        true,
    )?;
    for (name, value) in [("smoothness-black", 0), ("smoothness-white", 255)] {
        compare(
            name,
            Material {
                roughness: 1.,
                roughness_map: scalar(value, true),
                ..base.clone()
            },
            Material {
                roughness: 1. - value as f32 / 255.,
                ..base.clone()
            },
            true,
        )?;
    }
    compare(
        "metallic-blue-channel",
        Material {
            metallic: 1.,
            metallic_map: Some(ScalarTexture {
                texture: texture(&[0, 64, 192, 255]),
                channel: 2,
                invert: false,
            }),
            ..base.clone()
        },
        Material {
            metallic: 192. / 255.,
            ..base.clone()
        },
        true,
    )?;
    compare(
        "ao-linear",
        Material {
            occlusion: scalar(128, false),
            ..base.clone()
        },
        Material {
            occlusion: scalar(0, false),
            occlusion_strength: 1. - 128. / 255.,
            ..base.clone()
        },
        true,
    )?;
    compare(
        "normal-convention",
        Material {
            normal: Some(texture(&[128, 192, 240, 255])),
            normal_y: -1.,
            ..base.clone()
        },
        Material {
            normal: Some(texture(&[128, 63, 240, 255])),
            ..base.clone()
        },
        true,
    )?;
    compare(
        "normal-effect",
        Material {
            normal: Some(texture(&[200, 192, 240, 255])),
            ..base.clone()
        },
        base.clone(),
        false,
    )?;
    compare(
        "roughness-effect",
        Material {
            metallic: 1.,
            roughness: 0.1,
            ..base.clone()
        },
        Material {
            metallic: 1.,
            roughness: 0.95,
            ..base.clone()
        },
        false,
    )?;
    compare(
        "uv-scale",
        Material {
            texture: Some(texture(&[255, 40, 40, 255, 40, 40, 255, 255])),
            ..base.clone()
        },
        Material {
            texture: Some(texture(&[255, 40, 40, 255, 40, 40, 255, 255])),
            uv_transform: [3., 1., 0., 0.],
            ..base.clone()
        },
        false,
    )?;
    compare(
        "emissive-srgb",
        Material {
            emissive: [1.; 3],
            emissive_map: Some(texture(&[128, 128, 128, 255])),
            ..base.clone()
        },
        Material {
            emissive: [linear_gray; 3],
            ..base.clone()
        },
        true,
    )?;
    compare(
        "metallic-effect",
        Material {
            metallic: 0.,
            ..base.clone()
        },
        Material {
            metallic: 1.,
            ..base
        },
        false,
    )?;
    Ok(serde_json::json!(records))
}

/// Exercise the live cache with one unchanged procedural mesh and replacement
/// appearance resources, as happens after missing-map recovery in the Editor.
fn verify_mesh_material_reload(scene: &InspectionScene) -> Result<(), String> {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|e| e.to_string())?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| e.to_string())?;
        let mut gpu = crate::mesh_gpu::MeshGpu::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
        gpu.upload(&device, &queue, scene, Vec3::ZERO)?;
        let mut reloaded = scene.clone();
        let mesh = scene
            .mesh
            .as_ref()
            .ok_or("Reload probe requires procedural geometry")?;
        let mut materials = crate::material_library::LoadedMaterials::default();
        materials.slots.insert(
            mesh.materials[0].id.clone(),
            Material {
                color: [0.3, 0.4, 0.5, 1.],
                metallic: 0.5,
                roughness: 0.2,
                ..Default::default()
            },
        );
        reloaded.materials = Arc::new(materials);
        gpu.upload(&device, &queue, &reloaded, Vec3::ZERO)?;
        gpu.upload(&device, &queue, &reloaded, Vec3::ZERO)?;
        if gpu.uploads != 2 {
            return Err("Unchanged mesh failed to refresh changed material resources, or reuploaded unchanged resources".into());
        }
        Ok(())
    })
}
