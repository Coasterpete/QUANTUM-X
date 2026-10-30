use super::{
    scenario,
    session::{Session, blank_project, scene},
};
use qx_style::construction::TrackConstruction;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
fn wait(s: &mut Session) -> qx_rendering::InspectionScene {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(scene) = s.poll() {
            return scene;
        }
        assert!(Instant::now() < end, "{}", s.status);
        assert!(!s.status.starts_with("Generation failed"), "{}", s.status);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn construction_draft_cancel_apply_undo_reopen_and_mesh_failure_are_recoverable() {
    let mut s = scenario::build().unwrap();
    s.regenerate();
    let original = wait(&mut s);
    let mathematical = original.track.clone();
    s.construction_draft = TrackConstruction::generic_single();
    s.preview();
    let single = wait(&mut s);
    assert_eq!(single.track, mathematical);
    assert_eq!(single.mesh.as_ref().unwrap().components.len(), 2);
    assert!(s.has_drafts());
    s.construction_draft = s.editor.project().construction.clone();
    s.refresh_view();
    let canceled = wait(&mut s);
    assert_eq!(original.mesh, canceled.mesh);
    s.construction_draft = TrackConstruction::generic_single();
    s.preview();
    let preview = wait(&mut s);
    let revision = s.revision;
    assert!(
        s.execute(vec![qx_document::command::Command::SetConstruction(
            s.construction_draft.clone()
        )])
    );
    assert_eq!(s.revision, revision);
    assert!(!s.has_drafts());
    s.undo(false);
    let undone = wait(&mut s);
    assert_eq!(undone.mesh, original.mesh);
    s.undo(true);
    let redone = wait(&mut s);
    assert_eq!(redone.mesh, preview.mesh);
    let json = qx_document::persistence::to_json(s.editor.project(), &s.catalog).unwrap();
    let reopened = qx_document::persistence::from_json(&json, &s.catalog).unwrap();
    assert_eq!(&reopened, s.editor.project());
    assert_eq!(
        scene(&reopened, &s.catalog, s.options).unwrap().mesh,
        redone.mesh
    );
    let accepted = s.editor.project().clone();
    s.construction_draft.continuous[0].profile = qx_style::construction::Profile::Box {
        height_m: -1.,
        width_m: 1.,
    };
    s.preview();
    let end = Instant::now() + Duration::from_secs(10);
    while s.draft_error.is_none() {
        assert!(s.poll().is_none());
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(s.editor.project(), &accepted);
    assert!(s.status.contains("Previous view retained"));
    assert!(
        !s.execute(vec![qx_document::command::Command::SetConstruction(
            s.construction_draft.clone()
        )])
    );
    assert_eq!(s.editor.project(), &accepted);
    s.construction_draft = s.editor.project().construction.clone();
    s.refresh_view();
    assert_eq!(wait(&mut s).mesh, redone.mesh);
}
#[test]
fn metadata_and_catalog_changes_reuse_mesh_arc_but_construction_changes_invalidate_it() {
    let mut s = Session::new(
        blank_project(),
        None,
        qx_style::Catalog::built_in().unwrap(),
    )
    .unwrap();
    s.regenerate();
    let first = wait(&mut s);
    let mut metadata = s.editor.project().metadata.clone();
    metadata.name = "Mesh cache regression".into();
    s.execute(vec![qx_document::command::Command::SetMetadata(metadata)]);
    let named = wait(&mut s);
    assert!(Arc::ptr_eq(
        first.mesh.as_ref().unwrap(),
        named.mesh.as_ref().unwrap()
    ));
    let selection = qx_style::StyleSelection {
        family_id: "monorail".into(),
        style_id: "monorail-200".into(),
    };
    s.execute(vec![qx_document::command::Command::SetStyle(selection)]);
    let concept = wait(&mut s);
    assert!(Arc::ptr_eq(
        named.mesh.as_ref().unwrap(),
        concept.mesh.as_ref().unwrap()
    ));
    s.construction_draft.materials[0].base_color = [0.2, 0.4, 0.8];
    s.preview();
    let color = wait(&mut s);
    assert_eq!(color.track, first.track);
    assert!(!Arc::ptr_eq(
        concept.mesh.as_ref().unwrap(),
        color.mesh.as_ref().unwrap()
    ));
    let revision = s.revision;
    s.preview();
    assert_eq!(s.revision, revision);
}
#[test]
fn rider_authored_constrained_inverse_drives_the_physical_mesh() {
    let mut s = Session::new(
        blank_project(),
        None,
        qx_style::Catalog::built_in().unwrap(),
    )
    .unwrap();
    s.heartline_draft.authored_reference =
        qx_geometry::heartline::AuthoredReference::RiderReference;
    s.heartline_draft.rider_offset = qx_geometry::heartline::OffsetProfile::Constant {
        offset_m: qx_geometry::math::Vec3::Y * 2.,
    };
    s.preview();
    let view = wait(&mut s);
    assert_eq!(view.track.samples[0].position_m.y, 0.);
    assert!(
        (view.mesh.as_ref().unwrap().components[0].mesh.vertices[0]
            .position_m
            .y
            + 2.
            - 0.065)
            .abs()
            < 1e-12
    );
}
#[test]
fn both_strategies_follow_hill_banking_and_inversion_at_every_source_ring() {
    use qx_geometry::{
        authoring::{ChannelKey, Section, SectionKind},
        math::Vec3,
    };
    use qx_style::construction::Role;
    let session = scenario::build().unwrap();
    for fixture in 0..3 {
        let mut project = session.editor.project().clone();
        if fixture == 1 {
            project.sections[1] = qx_document::region_edit::endpoint(
                &project.sections[1],
                ChannelKey::Roll,
                0,
                true,
                0.04,
            )
            .unwrap();
        } else if fixture == 2 {
            let mut section =
                Section::straight(1, "inversion fixture", std::f64::consts::TAU * 10.);
            section.kind = SectionKind::Arc {
                radius_m: 10.,
                plane_angle_rad: 0.,
            };
            project.sections = vec![section];
        }
        let track = project.generate(&session.catalog, session.options).unwrap();
        let original = track.clone();
        if fixture == 1 {
            assert!(
                track
                    .samples
                    .iter()
                    .filter_map(|s| s.frame.bank_rad())
                    .any(|b| b.abs() > 0.8)
            );
        }
        if fixture == 2 {
            assert!(track.samples.iter().any(|s| s.frame.normal().y < -0.99));
        }
        for configuration in [
            TrackConstruction::generic_dual(),
            TrackConstruction::generic_single(),
        ] {
            let mesh = qx_geometry::construction::build(&track, &configuration, Default::default())
                .unwrap();
            for (component, definition) in mesh
                .components
                .iter()
                .filter(|c| c.role != Role::Hardware)
                .zip(&configuration.continuous)
            {
                let profile = definition.profile.points().unwrap();
                let stride = profile.len() * 2;
                let local = Vec3::new(
                    definition.offset_m[0],
                    definition.offset_m[1] + profile[0][0],
                    definition.offset_m[2] + profile[0][1],
                );
                for (i, sample) in track.samples.iter().enumerate() {
                    let expected = sample.position_m + sample.frame.local_to_world(local);
                    assert!(
                        (component.mesh.vertices[i * stride].position_m - expected).length()
                            < 1e-12
                    );
                }
                component.mesh.validate().unwrap();
            }
            for (component, definition) in mesh
                .components
                .iter()
                .filter(|c| c.role == Role::Hardware)
                .zip(&configuration.repeated)
            {
                let offset = Vec3::new(
                    definition.offset_m[0],
                    definition.offset_m[1],
                    definition.offset_m[2],
                );
                for placement in &component.placements {
                    let sample = track.sample_at_distance(placement.station_m).unwrap();
                    assert!(
                        (placement.position_m
                            - sample.position_m
                            - sample.frame.local_to_world(offset))
                        .length()
                            < 1e-12
                    );
                    assert!((placement.frame.normal() - sample.frame.normal()).length() < 1e-12);
                }
            }
        }
        assert_eq!(track, original);
    }
}
