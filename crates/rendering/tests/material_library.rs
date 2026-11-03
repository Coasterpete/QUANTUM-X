use qx_rendering::{
    asset::Model,
    material_library::{assign_imported, load_bindings, read_manifest},
};
use qx_style::{
    construction::TrackConstruction,
    materials::{Binding, Library},
};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn recolor_multiplies_linear_base_and_keeps_original_maps_and_finish() {
    let fixture = Fixture::new();
    let original = std::fs::read(fixture.0.join("color.png")).unwrap();
    let mut c = TrackConstruction::generic_dual();
    let mut binding = Binding::new("shared-metal");
    binding.tint = [0.1, 0.2, 0.9];
    binding.uv_scale = [0.5, 2.];
    c.materials[0].library = Some(binding.clone());
    let loaded = load_bindings(&fixture.0, &c.materials);
    let material = &loaded.slots["running"];
    assert_eq!(material.color, [0.1, 0.2, 0.9, 1.]);
    assert_eq!(material.metallic, 1.);
    assert_eq!(material.roughness, 1.);
    assert_eq!(material.texture.as_ref().unwrap().rgba, [128, 64, 192, 255]);
    assert!(material.roughness_map.as_ref().unwrap().invert);
    assert_eq!(material.roughness_map.as_ref().unwrap().channel, 1);
    assert_eq!(material.uv_transform, [0.5, 2., 0., 0.]);
    assert_eq!(
        std::fs::read(fixture.0.join("color.png")).unwrap(),
        original
    );
    assert!(c.materials[1].library.is_none());
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "qx-m1-material-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        image::save_buffer(
            path.join("color.png"),
            &[128, 64, 192, 255],
            1,
            1,
            image::ColorType::Rgba8,
        )
        .unwrap();
        let f = Self(path);
        f.write(json!({"version":1,"materials":[{"id":"shared-metal","name":"Diagnostic shared material","base_color":[1,1,1,1],"metallic":1,"roughness":1,
            "maps":{"base_color":"color.png","normal":"color.png","metallic":{"path":"color.png","channel":"b"},"smoothness":{"path":"color.png","channel":"g"},"ambient_occlusion":{"path":"color.png"}}}]}));
        f
    }
    fn write(&self, value: serde_json::Value) {
        std::fs::write(
            self.0.join("library.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let parent = self.0.parent().unwrap().canonicalize().unwrap();
        assert_eq!(parent, std::env::temp_dir().canonicalize().unwrap());
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("qx-m1-material-test-")
        );
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn shared_assignments_preserve_pixels_and_smoothness_and_adapt_imported_meshes() {
    let f = Fixture::new();
    let original = std::fs::read(f.0.join("color.png")).unwrap();
    let mut dual = TrackConstruction::generic_dual();
    let mut single = TrackConstruction::generic_single();
    for c in [&mut dual, &mut single] {
        for m in &mut c.materials {
            m.library = Some(Binding::new("shared-metal"));
        }
        c.validate().unwrap();
        let loaded = load_bindings(&f.0, &c.materials);
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        assert_eq!(loaded.slots.len(), c.materials.len());
        let a = loaded.slots.values().next().unwrap();
        assert_eq!(a.texture.as_ref().unwrap().rgba, [128, 64, 192, 255]);
        let roughness = a.roughness_map.as_ref().unwrap();
        assert!(roughness.invert);
        assert_eq!(roughness.channel, 1);
        assert_eq!(roughness.texture.rgba, [128, 64, 192, 255]);
        for b in loaded.slots.values() {
            assert!(Arc::ptr_eq(
                a.texture.as_ref().unwrap(),
                b.normal.as_ref().unwrap()
            ));
        }
        let mut model = Model::development_box();
        let vertices = bytemuck::cast_slice::<_, u8>(&model.primitives[0].vertices).to_vec();
        assign_imported(&mut model, 0, a).unwrap();
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&model.primitives[0].vertices),
            vertices
        );
        assert!(assign_imported(&mut model, 100, a).is_err());
        model.primitives[0].has_uv = false;
        assert!(
            assign_imported(&mut model, 0, a)
                .unwrap_err()
                .contains("UV unwrap")
        );
        assert_eq!(a.color, [1.; 4]);
    }
    assert_eq!(std::fs::read(f.0.join("color.png")).unwrap(), original);
}
#[test]
fn roughness_and_smoothness_share_original_pixels_and_follow_metadata_not_filenames() {
    let f = Fixture::new();
    let original = std::fs::read(f.0.join("color.png")).unwrap();
    std::fs::write(f.0.join("source_roughness.png"), &original).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(f.0.join("library.json")).unwrap()).unwrap();
    value["materials"][0]["maps"]["smoothness"]["path"] = json!("source_roughness.png");
    let mut roughness = value["materials"][0].clone();
    roughness["id"] = json!("genuine-roughness");
    let map = roughness["maps"]
        .as_object_mut()
        .unwrap()
        .remove("smoothness")
        .unwrap();
    roughness["maps"]["roughness"] = map;
    value["materials"].as_array_mut().unwrap().push(roughness);
    f.write(value);

    let library = read_manifest(&f.0).unwrap();
    assert_eq!(
        serde_json::from_slice::<Library>(&serde_json::to_vec(&library).unwrap()).unwrap(),
        library
    );
    let mut slots = TrackConstruction::generic_dual().materials;
    slots.truncate(1);
    slots[0].library = Some(Binding::new("shared-metal"));
    let mut other = slots[0].clone();
    other.id = "genuine-roughness-slot".into();
    other.library = Some(Binding::new("genuine-roughness"));
    slots.push(other);
    let loaded = load_bindings(&f.0, &slots);
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let smoothness = loaded.slots[&slots[0].id].roughness_map.as_ref().unwrap();
    let roughness = loaded.slots[&slots[1].id].roughness_map.as_ref().unwrap();
    assert!(smoothness.invert);
    assert!(!roughness.invert);
    assert_eq!(smoothness.channel, 1);
    assert_eq!(roughness.channel, 1);
    assert!(Arc::ptr_eq(&smoothness.texture, &roughness.texture));
    assert_eq!(smoothness.texture.rgba, [128, 64, 192, 255]);
    assert_eq!(
        std::fs::read(f.0.join("source_roughness.png")).unwrap(),
        original
    );
}
#[test]
fn missing_corrupt_unknown_and_absent_libraries_are_recoverable_and_unassigned_is_lazy() {
    let f = Fixture::new();
    let mut c = TrackConstruction::generic_dual();
    assert!(
        load_bindings(&f.0.join("absent"), &c.materials)
            .warnings
            .is_empty()
    );
    c.materials[0].library = Some(Binding::new("shared-metal"));
    std::fs::remove_file(f.0.join("color.png")).unwrap();
    let loaded = load_bindings(&f.0, &c.materials);
    assert!(loaded.slots.is_empty());
    assert!(loaded.warnings[0].contains("scalar fallback retained"));
    std::fs::write(f.0.join("color.png"), b"invalid PNG").unwrap();
    assert!(load_bindings(&f.0, &c.materials).slots.is_empty());
    c.materials[0].library = Some(Binding::new("unknown-material"));
    assert!(load_bindings(&f.0, &c.materials).warnings[0].contains("Unknown material ID"));
    assert!(
        !load_bindings(&f.0.join("absent"), &c.materials)
            .warnings
            .is_empty()
    );
}

#[test]
fn browser_file_inspection_and_restored_maps_recover_without_changing_bindings() {
    let f = Fixture::new();
    let library = read_manifest(&f.0).unwrap();
    assert!(qx_rendering::material_library::inspect_maps(&f.0, &library)["color.png"].is_ok());
    let mut c = TrackConstruction::generic_dual();
    c.materials[0].library = Some(Binding::new("shared-metal"));
    let slots = c.materials.clone();
    let bytes = std::fs::read(f.0.join("color.png")).unwrap();
    std::fs::remove_file(f.0.join("color.png")).unwrap();
    assert!(qx_rendering::material_library::inspect_maps(&f.0, &library)["color.png"].is_err());
    assert!(load_bindings(&f.0, &slots).slots.is_empty());
    std::fs::write(f.0.join("color.png"), bytes).unwrap();
    assert!(qx_rendering::material_library::inspect_maps(&f.0, &library)["color.png"].is_ok());
    assert!(load_bindings(&f.0, &slots).slots.contains_key("running"));
    assert_eq!(c.materials, slots);
}
#[test]
fn rejects_ambiguous_maps_invalid_paths_duplicate_ids_versions_and_unknown_fields() {
    let f = Fixture::new();
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(f.0.join("library.json")).unwrap()).unwrap();
    for case in 0..8 {
        let mut v = original.clone();
        match case {
            0 => v["version"] = json!(2),
            1 => v["materials"][0]["maps"]["base_color"] = json!("../color.png"),
            2 => v["materials"][0]["maps"]["base_color"] = json!("C:/color.png"),
            3 => v["materials"][0]["maps"]["roughness"] = json!({"path":"color.png"}),
            4 => v["materials"][0]["metallic"] = json!(2),
            5 => {
                let m = v["materials"][0].clone();
                v["materials"].as_array_mut().unwrap().push(m);
            }
            6 => v["materials"][0]["maps"]["invented"] = json!("color.png"),
            _ => v["materials"][0]["maps"]["base_color"] = json!("https://invalid/color.png"),
        }
        f.write(v);
        assert!(read_manifest(&f.0).is_err(), "case {case}");
    }
    let old = TrackConstruction::generic_dual();
    let json = serde_json::to_string(&old).unwrap();
    assert!(!json.contains("library"));
    assert_eq!(
        serde_json::from_str::<TrackConstruction>(&json).unwrap(),
        old
    );
    let mut binding = Binding::new("shared-metal");
    binding.uv_scale[0] = 0.;
    assert!(binding.validate().is_err());
    binding.uv_scale[0] = f32::NAN;
    assert!(binding.validate().is_err());
}
#[test]
fn owner_manifest_is_valid_and_temporary_train_is_neutral_untextured_matte() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/materials");
    let library: Library = read_manifest(&root).unwrap();
    assert_eq!(library.materials.len(), 8);
    let supplied: Vec<_> = library
        .materials
        .iter()
        .filter(|d| d.id.starts_with("track-"))
        .collect();
    assert_eq!(supplied.len(), 4);
    for (id, metal, rough) in [
        ("gloss-painted-steel", 0., 0.16),
        ("polished-steel", 1., 0.07),
        ("satin-steel", 1., 0.42),
        ("matte-paint", 0., 0.85),
    ] {
        let preset = library.materials.iter().find(|d| d.id == id).unwrap();
        assert!(preset.maps.entries().is_empty());
        assert_eq!(preset.metallic, metal);
        assert_eq!(preset.roughness, rough);
    }
    for d in supplied {
        assert_eq!(d.base_color, [1.; 4]);
        assert_eq!(d.roughness, 1.);
        assert!(
            d.maps.roughness.is_none(),
            "{} supplied map is smoothness",
            d.id
        );
        let smoothness = d.maps.smoothness.as_ref().unwrap();
        assert_eq!(smoothness.channel, qx_style::materials::Channel::R);
        if d.id == "track-pine" {
            assert_eq!(smoothness.path, "tracks/lumber/pine/pinewood_roughness.png");
        }
    }
    let model = Model::development_box();
    let m = &model.primitives[0].material;
    assert_eq!(m.color[0], m.color[1]);
    assert_eq!(m.color[1], m.color[2]);
    assert_eq!(m.metallic, 0.);
    assert!(m.roughness >= 0.9);
    assert!(m.textures().into_iter().all(|t| t.is_none()));
}
