use qx_rendering::asset::{AssetInstance, import_glb};
use serde_json::{Value, json};
use std::sync::Arc;
const BODY: &[u8] = include_bytes!("../../../assets/development/train/development-body.glb");
fn parts() -> (Value, Vec<u8>) {
    let n = u32::from_le_bytes(BODY[12..16].try_into().unwrap()) as usize;
    (
        serde_json::from_slice(&BODY[20..20 + n]).unwrap(),
        BODY[28 + n..].to_vec(),
    )
}
fn pack(v: &Value, mut bin: Vec<u8>) -> Vec<u8> {
    let mut json = serde_json::to_vec(v).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let mut out = b"glTF".to_vec();
    out.extend(2u32.to_le_bytes());
    out.extend(((28 + json.len() + bin.len()) as u32).to_le_bytes());
    out.extend((json.len() as u32).to_le_bytes());
    out.extend(b"JSON");
    out.extend(json);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(b"BIN\0");
    out.extend(bin);
    out
}
#[test]
fn imports_actual_glb_hierarchy_uv_texture_indices_and_mirrored_nodes() {
    let model = Arc::new(import_glb(BODY).unwrap());
    assert_eq!(model.primitives.len(), 1);
    assert_eq!(model.nodes.len(), 5);
    let p = &model.primitives[0];
    assert_eq!(p.vertices.len(), 24);
    assert_eq!(p.indices.len(), 36);
    let texture = p.material.texture.as_ref().unwrap();
    assert_eq!(
        (texture.width, texture.height, texture.rgba.len()),
        (2, 2, 16)
    );
    let instance = AssetInstance {
        model,
        component_id: "car".into(),
        transform: [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ],
    };
    let (min, max) = instance.bounds();
    assert!((min.x + 1.5).abs() < 1e-6);
    assert!((max.y - 1.1).abs() < 1e-6);
    assert!(instance.model.nodes.iter().any(|n| n.transform[0][0] < 0.));
    for t in p_indices(&instance).as_chunks::<3>().0 {
        assert!(t.iter().all(|i| *i < 24));
    }
}
fn p_indices(i: &AssetInstance) -> &[u32] {
    &i.model.primitives[0].indices
}
#[test]
fn accepts_nonindexed_triangles_and_generates_normals() {
    let (mut v, b) = parts();
    let p = &mut v["meshes"][0]["primitives"][0];
    p.as_object_mut().unwrap().remove("indices");
    p["attributes"].as_object_mut().unwrap().remove("NORMAL");
    v["accessors"][0]["count"] = json!(3);
    v["accessors"][2]["count"] = json!(3);
    let model = import_glb(&pack(&v, b)).unwrap();
    assert_eq!(model.primitives[0].indices.len(), 3);
}
#[test]
fn real_blender_export_and_hierarchy_rotation_are_supported() {
    let bytes = include_bytes!("../../../assets/development/train/development-blender-body.glb");
    let model = import_glb(bytes).unwrap();
    assert_eq!(model.primitives.len(), 1);
    assert_eq!(model.nodes.len(), 5);
    assert!(model.primitives[0].material.texture.is_some());
    let (mut v, b) = parts();
    v["nodes"][0]["rotation"] = json!([
        0.,
        0.,
        std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2
    ]);
    let rotated = import_glb(&pack(&v, b)).unwrap();
    let n = &rotated.nodes[0].transform;
    assert!(n[0][0].abs() < 1e-6);
    assert!((n[0][1] - 3.).abs() < 1e-6);
    assert!((n[3][0] + 0.1).abs() < 1e-6);
    assert!((n[3][1] - 0.2).abs() < 1e-6);
}
#[test]
fn rejects_malformed_ranges_indices_nonfinite_data_cycles_and_features_without_panics() {
    for mutate in 0..20 {
        let (mut v, mut b) = parts();
        match mutate {
            0 => v["bufferViews"][0]["byteLength"] = json!(999999999),
            1 => v["accessors"][0]["count"] = json!(9999999),
            2 => v["nodes"][1]["children"] = json!([0]),
            3 => v["nodes"][1]["scale"] = json!([0, 1, 1]),
            4 => v["buffers"][0]["uri"] = json!("../../secret.bin"),
            5 => v["images"][0] = json!({"uri":"https://example.invalid/image.png"}),
            6 => v["meshes"][0]["primitives"][0]["mode"] = json!(1),
            7 => v["extensionsRequired"] = json!(["KHR_draco_mesh_compression"]),
            8 => v["materials"][0]["alphaMode"] = json!("BLEND"),
            9 => b[..4].copy_from_slice(&f32::NAN.to_le_bytes()),
            10 => b[768..770].copy_from_slice(&999u16.to_le_bytes()),
            11 => v["accessors"][0]["type"] = json!("SCALAR"),
            12 => v["nodes"][1]["children"] = json!([2]),
            13 => v["bufferViews"][0]["byteStride"] = json!(4),
            14 => v["accessors"][2]["componentType"] = json!(5123),
            15 => v["accessors"][2]["normalized"] = json!(true),
            16 => v["accessors"][3]["normalized"] = json!(true),
            17 => v["accessors"][1]["normalized"] = json!(true),
            18 => v["bufferViews"][0]["byteOffset"] = json!(2),
            _ => {
                let offset = v["bufferViews"][4]["byteOffset"].as_u64().unwrap() as usize;
                b[offset..offset + 8].fill(0);
            }
        }
        let bytes = pack(&v, b);
        let result = std::panic::catch_unwind(|| import_glb(&bytes));
        assert!(result.is_ok(), "panic case {mutate}");
        assert!(result.unwrap().is_err(), "accepted malformed case {mutate}");
    }
    for len in 0..BODY.len() {
        if len % 31 == 0 {
            assert!(
                std::panic::catch_unwind(|| import_glb(&BODY[..len]))
                    .unwrap()
                    .is_err()
            );
        }
    }
}
