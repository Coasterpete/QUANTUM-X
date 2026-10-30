use super::{
    session::{Session, blank_project},
    train_assets,
};
use qx_document::{
    command::Command,
    library::load_train_definition,
    persistence,
    train::{DevelopmentArrangement, Train},
};
use qx_style::{
    Catalog, DevelopmentStatus,
    library::{ContentManifest, Library, LibraryKind, PreviewImages, VisualAsset},
};

fn catalog_frame(
    s: &mut Session,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    train: bool,
) -> Vec<(String, egui::Rect)> {
    fn labels(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => out.push((t.galley.text().into(), t.visual_bounding_rect())),
            egui::Shape::Vec(v) => {
                for s in v {
                    labels(s, out);
                }
            }
            _ => {}
        }
    }
    let mut output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200., 1600.),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            ctx.root_ui(|root| {
                egui::CentralPanel::default().show(root, |ui| {
                    if train {
                        super::library_ui::trains(s, ui);
                    } else {
                        super::library_ui::tracks(s, ui);
                    }
                });
            });
        },
    );
    let mut out = vec![];
    output.textures_delta.clear();
    for shape in output.shapes {
        labels(&shape.shape, &mut out);
    }
    out
}
fn click_catalog(s: &mut Session, ctx: &egui::Context, text: &str, train: bool, last: bool) {
    let items = catalog_frame(s, ctx, vec![], train);
    let matching: Vec<_> = items.iter().filter(|(s, _)| s == text).collect();
    let point = if last {
        matching.last()
    } else {
        matching.first()
    }
    .unwrap_or_else(|| panic!("missing {text}: {items:?}"))
    .1
    .center();
    for pressed in [true, false] {
        catalog_frame(
            s,
            ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
            train,
        );
    }
}
#[test]
fn existing_panes_expose_public_families_and_planned_entries_cannot_apply() {
    let mut s = Session::new(blank_project(), None, Catalog::built_in().unwrap()).unwrap();
    let original = s.editor.project().clone();
    let ctx = egui::Context::default();
    ctx.global_style_mut(|style| style.animation_time = 0.);
    catalog_frame(&mut s, &ctx, vec![], false);
    click_catalog(&mut s, &ctx, "Hybrid", false, false);
    let labels = catalog_frame(&mut s, &ctx, vec![], false);
    assert!(labels.iter().any(|(s, _)| s == "Hybrid-200 · Planned"));
    click_catalog(&mut s, &ctx, "Apply visual style", false, true);
    assert_eq!(s.editor.project(), &original);
    let ctx = egui::Context::default();
    ctx.global_style_mut(|style| style.animation_time = 0.);
    click_catalog(
        &mut s,
        &ctx,
        "Train Library · development inspection",
        true,
        false,
    );
    let labels = catalog_frame(&mut s, &ctx, vec![], true);
    for name in [
        "4D",
        "Free Spin",
        "Flying",
        "Inverted",
        "Motorcycle Suspended",
    ] {
        assert!(labels.iter().any(|(s, _)| s == name), "{name}");
    }
    assert!(
        !labels
            .iter()
            .any(|(s, _)| ["Conventional", "BelowTrack", "RotatingSeats"].contains(&s.as_str()))
    );
    assert_eq!(s.editor.project(), &original);
}

#[test]
fn catalog_definition_flows_through_existing_glb_packaging_and_rigid_body_solver() {
    let root = std::env::temp_dir().join(format!(
        "qx-library-integrated-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut catalog = Catalog::built_in().unwrap();
    let style = catalog
        .train_styles
        .iter_mut()
        .find(|s| s.id == "hybrid-100")
        .unwrap();
    style.status = DevelopmentStatus::Available;
    let directory = root.join(&style.directory);
    std::fs::create_dir_all(&directory).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/development/train/development-blender-body.glb");
    std::fs::copy(source, directory.join("body.glb")).unwrap();
    let mut train = Train::development_consist(
        "library-test".into(),
        2,
        DevelopmentArrangement::Conventional,
    )
    .unwrap();
    train.components[0].asset = Some(train_assets::identity("body.glb".into()));
    std::fs::write(
        directory.join("mechanics.json"),
        serde_json::to_string(&train).unwrap(),
    )
    .unwrap();
    let manifest = ContentManifest {
        version: 1,
        kind: LibraryKind::Train,
        style_id: style.id.clone(),
        visuals: vec![VisualAsset {
            id: "body".into(),
            path: "body.glb".into(),
        }],
        previews: PreviewImages {
            thumbnail: Some("thumbnail.png".into()),
            large: Some("preview.png".into()),
            references: vec![],
        },
        construction: None,
        mechanical_definition: Some("mechanics.json".into()),
    };
    std::fs::write(
        directory.join("style.json"),
        serde_json::to_string(&manifest).unwrap(),
    )
    .unwrap();
    for name in ["thumbnail.png", "preview.png"] {
        std::fs::write(directory.join(name), b"path resolution only").unwrap();
    }
    let library = Library::from_catalog(catalog.clone(), &root).unwrap();
    let mut resolved = load_train_definition(&library, "hybrid-100").unwrap();
    let mut project = blank_project();
    project.sections[0] =
        qx_geometry::authoring::Section::straight(1, "Synthetic straight test", 100.);
    let mut s = Session::new(project, None, catalog.clone()).unwrap();
    for component in &mut resolved.definition.components {
        if let Some(asset) = &mut component.asset {
            asset.path = s
                .assets
                .import_now(&resolved.visual_sources[&asset.path], &s.path)
                .unwrap();
        }
    }
    s.execute(vec![Command::SetTrains(vec![resolved.definition])]);
    let project_path = root.join("project.qx.json");
    s.save_to(&project_path);
    assert!(s.save_failure.is_none(), "{}", s.status);
    let reopened = persistence::load(&project_path, &catalog).unwrap();
    assert_eq!(&reopened, s.editor.project());
    let asset = reopened.trains[0].components[0].asset.as_ref().unwrap();
    assert!(root.join(&asset.path).is_file());
    assert_eq!(
        reopened.trains[0].compatibility.train_style_id.as_deref(),
        Some("hybrid-100")
    );
    let track = reopened.generate(&catalog, Default::default()).unwrap();
    let mut world = qx_physics::TrackSimulation::new(
        reopened.trains[0].clone(),
        std::sync::Arc::new(track),
        20.,
        4.,
    )
    .unwrap();
    for _ in 0..8 {
        world.step().unwrap();
    }
    assert_eq!(world.ticks(), 8);
    std::fs::remove_dir_all(root).unwrap();
}
