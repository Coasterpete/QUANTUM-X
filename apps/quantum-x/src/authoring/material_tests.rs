use super::{
    coaster_properties,
    material_editor_scenario::{click, frame, wait},
    material_ui,
    session::{Session, blank_project},
};
use qx_document::{command::Command, persistence};
use qx_style::{Catalog, construction::TrackConstruction, materials::Binding};
use std::sync::Arc;

fn session() -> Session {
    let mut s = Session::new(blank_project(), None, Catalog::built_in().unwrap()).unwrap();
    coaster_properties::open(&mut s);
    s.material_ui.appearance_tab = true;
    s.regenerate();
    s
}

#[test]
fn selection_highlight_preference_and_materials_are_independent_of_panels_and_workspaces() {
    use super::workbench::Profile;
    use qx_rendering::{DisplayOptions, EditorSurface};
    let mut s = session();
    let scene = wait(&mut s).unwrap();
    let original = s.editor.project().clone();
    let ctx = egui::Context::default();
    for highlights in [true, false] {
        let mut display = DisplayOptions {
            highlights,
            ..Default::default()
        };
        for profile in [Profile::Track, Profile::Train, Profile::Track] {
            s.workbench.active = profile;
            for (tab, appearance) in [
                (1, true),
                (1, false),
                (0, false),
                (2, false),
                (3, false),
                (1, true),
            ] {
                s.track_tab = tab;
                s.material_ui.appearance_tab = appearance;
                let mut output = ctx.run_pass(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(2000., 1600.),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        s.ui(ctx, &scene, &mut display);
                    },
                );
                output.textures_delta.clear();
                assert_eq!(display.highlights, highlights);
                assert_eq!(display.selected, Some(0));
                assert_eq!(s.editor.project(), &original);
                assert_eq!(s.construction_draft, original.construction);
            }
        }
    }
    assert!(!s.editor.can_undo());
    assert!(!s.has_drafts());
}

#[test]
fn normal_editor_named_selection_live_preview_cancel_apply_history_and_save() {
    let mut s = session();
    let initial = wait(&mut s).unwrap();
    let project = s.editor.project().clone();
    let ctx = egui::Context::default();
    for _ in 0..4 {
        frame(&mut s, &ctx, &initial, vec![]);
    }
    assert!(!s.development);
    let mut display = qx_rendering::DisplayOptions::default();
    let mut output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(2000., 1600.),
            )),
            ..Default::default()
        },
        |ctx| {
            use qx_rendering::EditorSurface;
            s.ui(ctx, &initial, &mut display);
        },
    );
    output.textures_delta.clear();
    assert!(display.highlights);
    assert!(display.selected.is_some());
    let appearance_display = display.clone();
    s.material_ui.appearance_tab = false;
    let mut output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(2000., 1600.),
            )),
            ..Default::default()
        },
        |ctx| {
            use qx_rendering::EditorSurface;
            s.ui(ctx, &initial, &mut display);
        },
    );
    output.textures_delta.clear();
    assert_eq!(display, appearance_display);
    s.material_ui.appearance_tab = true;
    click(&mut s, &ctx, &initial, "Default finish", 0).unwrap();
    click(&mut s, &ctx, &initial, "Track bolt metal", 0).unwrap();
    let preview = wait(&mut s).unwrap();
    assert!(s.visible_preview);
    assert!(preview.materials.slots.contains_key("running"));
    assert_eq!(preview.track.samples, initial.track.samples);
    assert_eq!(preview.reference, initial.reference);
    assert_eq!(
        preview.mesh.as_ref().unwrap().components,
        initial.mesh.as_ref().unwrap().components
    );
    assert_eq!(s.editor.project(), &project);
    assert!(s.has_drafts());
    click(&mut s, &ctx, &preview, "Cancel appearance", 0).unwrap();
    let canceled = wait(&mut s).unwrap();
    assert_eq!(canceled.mesh, initial.mesh);
    assert!(!s.has_drafts());
    click(&mut s, &ctx, &canceled, "Default finish", 0).unwrap();
    click(&mut s, &ctx, &canceled, "Track pine", 0).unwrap();
    let pine = wait(&mut s).unwrap();
    assert!(
        pine.materials.slots["running"]
            .roughness_map
            .as_ref()
            .unwrap()
            .invert
    );
    click(&mut s, &ctx, &pine, "Apply appearance", 0).unwrap();
    let applied = s.editor.project().clone();
    assert!(!s.has_drafts());
    let json = persistence::to_json(&applied, &s.catalog).unwrap();
    assert_eq!(persistence::from_json(&json, &s.catalog).unwrap(), applied);
    s.undo(false);
    wait(&mut s).unwrap();
    assert_eq!(s.editor.project(), &project);
    s.undo(true);
    wait(&mut s).unwrap();
    assert_eq!(s.editor.project(), &applied);
    click(&mut s, &ctx, &pine, "Color and finish options", 0).unwrap();
    click(&mut s, &ctx, &pine, "Remove assignment", 0).unwrap();
    let removed = wait(&mut s).unwrap();
    assert!(!removed.materials.slots.contains_key("running"));
    assert!(material_ui::apply(&mut s));
    assert!(
        s.editor.project().construction.materials[0]
            .library
            .is_none()
    );
}

#[test]
fn overrides_work_for_shared_rails_bracing_and_hardware_without_reauthoring() {
    let mut c = TrackConstruction::generic_dual();
    let mut brace = c.continuous[2].clone();
    brace.id = "diagonal-bracing".into();
    c.continuous.push(brace);
    let mut bolt = c.repeated[0].clone();
    bolt.id = "connection-hardware".into();
    c.repeated.push(bolt);
    let original = c.clone();
    for (component, material) in [
        ("rail-left", "track-bolt-metal"),
        ("rail-right", "track-painted-metal"),
        ("spine", "track-pine"),
        ("diagonal-bracing", "track-grout"),
        ("cross-tie", "track-pine"),
        ("connection-hardware", "track-bolt-metal"),
    ] {
        c.override_component_material(component).unwrap();
        let slot = c.material_slot(component).unwrap().to_owned();
        c.assign_material(&slot, Some(Binding::new(material)))
            .unwrap();
    }
    c.validate().unwrap();
    assert_eq!(c.continuous.len(), original.continuous.len());
    for (a, b) in c.continuous.iter().zip(&original.continuous) {
        assert_eq!(a.profile, b.profile);
        assert_eq!(a.offset_m, b.offset_m);
        assert_eq!(a.rotation_rad, b.rotation_rad);
    }
    assert_ne!(c.material_slot("rail-left"), c.material_slot("rail-right"));
    assert_ne!(
        c.material_slot("spine"),
        c.material_slot("diagonal-bracing")
    );
    assert_ne!(
        c.material_slot("cross-tie"),
        c.material_slot("connection-hardware")
    );
    let slot_count = c.materials.len();
    c.override_component_material("rail-left").unwrap();
    assert_eq!(c.materials.len(), slot_count);
    let right = c.material_slot("rail-right").unwrap().to_owned();
    c.share_component_material("rail-left", &right).unwrap();
    assert_eq!(c.material_slot("rail-left"), c.material_slot("rail-right"));
    let before = c.clone();
    assert!(c.override_component_material("missing-component").is_err());
    assert!(
        c.share_component_material("rail-left", "unknown-slot")
            .is_err()
    );
    assert_eq!(c, before);
    let mut p = blank_project();
    p.construction = c;
    let catalog = Catalog::built_in().unwrap();
    let json = persistence::to_json(&p, &catalog).unwrap();
    assert_eq!(persistence::from_json(&json, &catalog).unwrap(), p);
}

#[test]
fn reload_preserves_mesh_and_drafts_but_invalidates_material_resources() {
    let mut s = session();
    let initial = wait(&mut s).unwrap();
    s.construction_draft.materials[0].library = Some(Binding::new("track-bolt-metal"));
    s.preview();
    let preview = wait(&mut s).unwrap();
    let draft = s.construction_draft.clone();
    s.reload_material_assets();
    let reload = wait(&mut s).unwrap();
    assert!(Arc::ptr_eq(
        preview.mesh.as_ref().unwrap(),
        reload.mesh.as_ref().unwrap()
    ));
    assert!(!Arc::ptr_eq(&preview.materials, &reload.materials));
    assert_eq!(s.construction_draft, draft);
    assert!(reload.materials.slots.contains_key("running"));
    assert_eq!(reload.track.samples, initial.track.samples);
    assert!(s.visible_preview);
}

#[test]
fn compatible_slots_reuse_bindings_and_uvs_without_replacing_destination_geometry() {
    let mut dual = TrackConstruction::generic_dual();
    let mut binding = Binding::new("track-bolt-metal");
    binding.uv_scale = [0.25, 2.];
    binding.uv_offset = [0.5, -1.];
    dual.assign_material("running", Some(binding.clone()))
        .unwrap();
    let mut single = TrackConstruction::generic_single();
    let components = single.continuous.clone();
    single.reuse_material_bindings(&dual);
    assert_eq!(single.continuous, components);
    assert_eq!(single.materials[0].library, Some(binding));
    assert!(single.materials[1].library.is_none());
    single.validate().unwrap();
}

#[test]
fn missing_ids_remain_saveable_and_appearance_apply_preserves_other_pending_edits() {
    let mut s = session();
    s.construction_draft.materials[0].library =
        Some(Binding::new("temporarily-unavailable-material"));
    s.draft.extent += 1.;
    let section_draft = s.draft.clone();
    s.metadata_draft.description = "Pending properties".into();
    assert!(material_ui::apply(&mut s));
    assert_eq!(s.draft, section_draft);
    assert_eq!(s.metadata_draft.description, "Pending properties");
    let json = persistence::to_json(s.editor.project(), &s.catalog).unwrap();
    assert_eq!(
        persistence::from_json(&json, &s.catalog).unwrap(),
        *s.editor.project()
    );
    s.cancel_all_drafts();
    let original = s.editor.project().clone();
    s.construction_draft.materials[0].library = None;
    assert!(s.apply_all_drafts());
    s.undo(false);
    assert_eq!(s.editor.project(), &original);
    let style = qx_style::StyleSelection {
        family_id: "monorail".into(),
        style_id: "monorail-200".into(),
    };
    assert!(s.execute(vec![Command::SetStyle(style)]));
    assert_eq!(s.editor.project().construction, original.construction);
}

#[test]
fn invalid_uv_and_slot_exhaustion_leave_document_and_history_intact() {
    let mut s = session();
    let original = s.editor.project().clone();
    let mut binding = Binding::new("track-bolt-metal");
    binding.uv_scale[0] = 0.;
    s.construction_draft.materials[0].library = Some(binding);
    assert!(!material_ui::apply(&mut s));
    assert_eq!(s.editor.project(), &original);
    assert!(!s.editor.can_undo());
    let mut c = original.construction;
    while c.materials.len() < 64 {
        let mut slot = c.materials[0].clone();
        slot.id = format!("unused-{}", c.materials.len());
        c.materials.push(slot);
    }
    let before = c.clone();
    assert!(c.override_component_material("rail-left").is_err());
    assert_eq!(c, before);
}

#[test]
fn exact_component_recolor_preserves_finish_geometry_drafts_and_history() {
    let mut s = session();
    s.construction_draft
        .assign_material("running", Some(Binding::new("gloss-painted-steel")))
        .unwrap();
    s.preview();
    let before = wait(&mut s).unwrap();
    assert!(material_ui::apply(&mut s));
    let original = s.editor.project().clone();
    let finish = before.materials.slots["running"].clone();
    let ctx = egui::Context::default();
    for _ in 0..4 {
        frame(&mut s, &ctx, &before, vec![]);
    }
    click(&mut s, &ctx, &before, "#FFFFFF", 0).unwrap();
    frame(
        &mut s,
        &ctx,
        &before,
        vec![
            egui::Event::ModifiersChanged(egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            }),
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                },
            },
            egui::Event::Text("#2468FF".into()),
        ],
    );
    let blue = wait(&mut s).unwrap();
    let color = super::color_ui::linear([0x24, 0x68, 0xFF]);
    assert_eq!(
        s.construction_draft.materials[0]
            .library
            .as_ref()
            .unwrap()
            .tint,
        color
    );
    let m = &blue.materials.slots["running"];
    assert_eq!(&m.color[..3], &color);
    assert_eq!(m.roughness, finish.roughness);
    assert_eq!(m.metallic, finish.metallic);
    assert_eq!(m.uv_transform, finish.uv_transform);
    assert_eq!(m.normal_scale, finish.normal_scale);
    assert_eq!(blue.track.samples, before.track.samples);
    assert_eq!(
        blue.mesh.as_ref().unwrap().components,
        before.mesh.as_ref().unwrap().components
    );
    assert_eq!(s.editor.project(), &original);
    s.draft.extent += 1.;
    let pending = s.draft.clone();
    assert!(material_ui::apply(&mut s));
    assert_eq!(s.draft, pending);
    let applied = s.editor.project().clone();
    assert_eq!(applied.sections, original.sections);
    assert_eq!(
        applied.construction.materials[1..],
        original.construction.materials[1..]
    );
    let json = persistence::to_json(&applied, &s.catalog).unwrap();
    assert_eq!(persistence::from_json(&json, &s.catalog).unwrap(), applied);
    s.cancel_section();
    s.undo(false);
    assert_eq!(s.editor.project(), &original);
    s.undo(true);
    assert_eq!(s.editor.project(), &applied);
    s.construction_draft
        .override_component_material("rail-left")
        .unwrap();
    let left = s
        .construction_draft
        .material_slot("rail-left")
        .unwrap()
        .to_owned();
    let right = s
        .construction_draft
        .material_slot("rail-right")
        .unwrap()
        .to_owned();
    s.construction_draft
        .materials
        .iter_mut()
        .find(|m| m.id == left)
        .unwrap()
        .library
        .as_mut()
        .unwrap()
        .tint = [1., 0., 0.];
    assert_eq!(
        s.construction_draft
            .materials
            .iter()
            .find(|m| m.id == right)
            .unwrap()
            .library
            .as_ref()
            .unwrap()
            .tint,
        color
    );
    material_ui::cancel(&mut s);
    assert_eq!(s.construction_draft, applied.construction);
}

#[test]
fn legacy_binding_defaults_to_white_and_rejects_invalid_tint() {
    let binding: Binding =
        serde_json::from_str(r#"{"id":"track-pine","uv_scale":[1,1],"uv_offset":[0,0]}"#).unwrap();
    assert_eq!(binding.tint, [1.; 3]);
    assert!(!serde_json::to_string(&binding).unwrap().contains("tint"));
    for invalid in [[-0.1, 1., 1.], [1.1, 0., 0.], [f32::NAN, 0., 0.]] {
        let mut b = binding.clone();
        b.tint = invalid;
        assert!(b.validate().is_err());
    }
}
