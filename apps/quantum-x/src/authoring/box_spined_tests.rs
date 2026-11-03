use super::{coaster_properties, material_editor_scenario::wait, session::Session};
use qx_style::{Catalog, StyleSelection, construction::Profile, materials::Binding};

fn selection() -> StyleSelection {
    StyleSelection {
        family_id: "box-spined".into(),
        style_id: "box-spined-100".into(),
    }
}
#[test]
fn original_source_style_preserves_authoring_appearances_history_and_persistence() {
    let catalog = Catalog::built_in().unwrap();
    let mut p = qx_document::sample::mixed_authoring_project();
    // A real authored scalar NURBS region, alongside the positional NURBS
    // section already present in this mixed-method fixture.
    p.sections[1].roll_rate.transitions[0] =
        p.sections[1].roll_rate.transitions[0].to_nurbs().unwrap();
    p.construction
        .override_component_material("rail-left")
        .unwrap();
    for (i, m) in p.construction.materials.iter_mut().enumerate() {
        let mut binding = Binding::new("gloss-painted-steel");
        binding.tint = [0.1 + 0.1 * i as f32, 0.3, 0.4];
        binding.uv_scale = [2., 3.];
        m.library = Some(binding);
        m.base_color = [0.1, 0.2, 0.3];
        m.roughness = 0.18;
    }
    let mut s = Session::new(p.clone(), None, catalog.clone()).unwrap();
    s.regenerate();
    wait(&mut s).unwrap();
    coaster_properties::select_style(&mut s, selection()).unwrap();
    wait(&mut s).unwrap();
    for id in ["rail-left", "rail-right", "spine"] {
        let a = p
            .construction
            .materials
            .iter()
            .find(|m| Some(m.id.as_str()) == p.construction.material_slot(id))
            .unwrap();
        let b = s
            .construction_draft
            .materials
            .iter()
            .find(|m| Some(m.id.as_str()) == s.construction_draft.material_slot(id))
            .unwrap();
        assert_eq!(a.library, b.library);
        assert_eq!(a.base_color, b.base_color);
        assert_eq!(a.roughness, b.roughness);
    }
    assert_eq!(s.editor.project(), &p);
    coaster_properties::cancel(&mut s);
    wait(&mut s).unwrap();
    assert_eq!(s.construction_draft, p.construction);
    coaster_properties::select_style(&mut s, selection()).unwrap();
    wait(&mut s).unwrap();
    assert!(coaster_properties::apply(&mut s));
    wait(&mut s).unwrap();
    let applied = s.editor.project().clone();
    assert_eq!(applied.sections, p.sections);
    assert_eq!(applied.heartline, p.heartline);
    assert_eq!(applied.trains, p.trains);
    assert_eq!(applied.initial, p.initial);
    assert_eq!(applied.construction.repeated.len(), 1);
    assert_eq!(applied.construction.repeated[0].spacing_m, 1.5);
    let json = qx_document::persistence::to_json(&applied, &catalog).unwrap();
    assert_eq!(
        qx_document::persistence::from_json(&json, &catalog).unwrap(),
        applied
    );
    s.undo(false);
    wait(&mut s).unwrap();
    assert_eq!(s.editor.project(), &p);
    s.undo(true);
    wait(&mut s).unwrap();
    assert_eq!(s.editor.project(), &applied);
}
#[test]
fn missing_source_cannot_be_applied_and_cancel_restores_document() {
    let p = super::session::blank_project();
    let mut s = Session::new(p.clone(), None, Catalog::built_in().unwrap()).unwrap();
    s.regenerate();
    wait(&mut s).unwrap();
    coaster_properties::select_style(&mut s, selection()).unwrap();
    wait(&mut s).unwrap();
    if let Profile::AssetSection { path, .. } = &mut s.construction_draft.continuous[2].profile {
        *path = "tracks/box-spined/100/missing-source.glb".into();
    } else {
        panic!("original source profile missing");
    }
    s.refresh_view();
    assert!(wait(&mut s).is_err());
    assert!(!coaster_properties::apply(&mut s));
    assert_eq!(s.editor.project(), &p);
    coaster_properties::cancel(&mut s);
    wait(&mut s).unwrap();
    assert_eq!(s.construction_draft, p.construction);
}

#[test]
fn assembly_dimensions_are_configurable_persisted_and_independent_of_authored_track() {
    let catalog = Catalog::built_in().unwrap();
    let mut s = Session::new(super::session::blank_project(), None, catalog.clone()).unwrap();
    s.regenerate();
    wait(&mut s).unwrap();
    coaster_properties::select_style(&mut s, selection()).unwrap();
    wait(&mut s).unwrap();
    assert!(coaster_properties::apply(&mut s));
    let before = wait(&mut s).unwrap();
    let original = s.editor.project().clone();
    let mut construction = original.construction.clone();
    for (rail, lateral) in construction.continuous[..2].iter_mut().zip([-0.7, 0.7]) {
        rail.offset_m[2] = lateral;
        if let Profile::Circle { radius_m, .. } = &mut rail.profile {
            *radius_m = 0.08;
        }
    }
    construction.repeated[0].spacing_m = 0.9;
    construction.repeated[0].start_m = 0.45;
    construction.repeated[0].end_margin_m = 0.45;
    s.construction_draft = construction.clone();
    s.refresh_view();
    let after = wait(&mut s).unwrap();
    assert_eq!(after.track, before.track);
    assert_eq!(after.reference, before.reference);
    assert_eq!(s.editor.project(), &original);
    assert_ne!(after.mesh, before.mesh);
    assert!(
        after.mesh.as_ref().unwrap().stats.instances
            > before.mesh.as_ref().unwrap().stats.instances
    );
    assert_eq!(
        after.mesh.as_ref().unwrap().components[3].mesh,
        before.mesh.as_ref().unwrap().components[3].mesh,
        "changing pitch never stretches hardware"
    );
    assert!(super::material_ui::apply(&mut s));
    let reopened = qx_document::persistence::from_json(
        &qx_document::persistence::to_json(s.editor.project(), &catalog).unwrap(),
        &catalog,
    )
    .unwrap();
    assert_eq!(reopened.construction, construction);
    assert_eq!(reopened.sections, original.sections);
    assert_eq!(reopened.trains, original.trains);
    assert_eq!(reopened.heartline, original.heartline);
    assert_eq!(
        reopened.construction.materials,
        original.construction.materials
    );
    let mesh = after.mesh.unwrap();
    s.reload_material_assets();
    let reloaded = wait(&mut s).unwrap();
    assert!(
        !std::sync::Arc::ptr_eq(&mesh, reloaded.mesh.as_ref().unwrap()),
        "external construction is rebuilt on explicit library reload"
    );
    assert_eq!(
        mesh,
        reloaded.mesh.unwrap(),
        "reimport preserves exact geometry and appearances"
    );
}
