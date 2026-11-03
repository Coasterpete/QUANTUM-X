//! Tests use isolated egui contexts/offscreen GPU targets, never desktop input.
use super::{
    graph_canvas, graph_curve,
    graph_view::View,
    plot::Plot,
    session::{Session, scene},
};
use qx_geometry::{
    authoring::{ChannelKey, Section, SectionKind},
    profile::{Continuity, NurbsProfile},
    transition::{Channel, Transition, Unit},
};
use qx_rendering::{DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput};

fn transition() -> Transition {
    let mut values = vec![0.; 11];
    values[5] = 1.;
    Transition::from_profile(
        1,
        1.,
        NurbsProfile {
            degree: 3,
            values,
            weights: vec![1.; 11],
            knots: vec![
                0., 0., 0., 0., 0.5, 0.5001, 0.5002, 0.5003, 0.5004, 0.5005, 0.5006, 1., 1., 1., 1.,
            ],
            incoming: Continuity::Free,
            interior: Continuity::C2,
        },
    )
}
fn project() -> qx_document::Project {
    let mut p = qx_document::sample::foundation_project();
    p.metadata.name = "M1.1 narrow C2 graph · synthetic test fixture".into();
    let mut section = Section::straight(1, "Narrow C2 graph fixture", 1.);
    section.kind = SectionKind::RiderLocal {
        pitch_rate: Channel {
            unit: Unit::RadiansPerMetre,
            transitions: vec![transition()],
        },
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 1., 0.),
    };
    p.sections = vec![section];
    p
}
fn session() -> Session {
    let mut s = Session::new(project(), None, qx_style::Catalog::built_in().unwrap()).unwrap();
    s.select_channel(ChannelKey::Pitch);
    s
}
fn mapping(view: View) -> Plot {
    Plot::new(
        egui::Rect::from_min_size(egui::pos2(200., 80.), egui::vec2(900., 400.)),
        [view.x.min, view.x.max],
        [view.y.min, view.y.max],
    )
    .unwrap()
}

#[test]
fn graph_precision_fit_includes_peak_and_keeps_constants_and_empty_policy() {
    let c = Channel {
        unit: Unit::RadiansPerMetre,
        transitions: vec![transition()],
    };
    let before = c.clone();
    let v = View::fit(&c).unwrap();
    let peak = c.transitions[0].evaluate(0.5003).unwrap().value;
    assert!(v.y.max > peak && v.y.min < 0.);
    assert!((v.y.max - peak * 1.15).abs() < 1e-6);
    assert_eq!(c, before);
    for unit in [Unit::G, Unit::RadiansPerMetre, Unit::RadiansPerSecond] {
        let zero = Channel::constant(unit, 1., 0.);
        let margin = if unit == Unit::RadiansPerMetre {
            0.05
        } else {
            1.
        };
        assert_eq!(
            View::fit(&zero).unwrap().bounds(),
            [0., 1., -margin, margin]
        );
    }
    assert!(
        View::fit(&Channel {
            unit: Unit::G,
            transitions: vec![]
        })
        .is_none()
    );
}

#[test]
fn graph_precision_draft_and_dashed_committed_both_paint_the_narrow_peak() {
    let c = Channel {
        unit: Unit::RadiansPerMetre,
        transitions: vec![transition()],
    };
    let view = View::fit(&c).unwrap();
    let mapping = mapping(view);
    let peak_y = mapping.map(0.5003, 2. / 3.).y;
    for appearance in [
        graph_curve::Appearance::Draft {
            color: super::theme::VIOLET,
            active: true,
        },
        graph_curve::Appearance::Committed,
    ] {
        let ctx = egui::Context::default();
        let mut budget = 16384;
        let mut output = ctx.run_pass(egui::RawInput::default(), |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            assert!(
                graph_curve::paint(
                    &painter,
                    &mapping,
                    &c.transitions[0],
                    0.,
                    appearance,
                    &mut budget
                )
                .is_none()
            );
        });
        assert!(
            output
                .shapes
                .iter()
                .any(|s| s.shape.visual_bounding_rect().top() < peak_y + 6.)
        );
        assert!(budget < 16384 && budget > 14000);
        output.textures_delta.clear();
    }
}

#[test]
fn graph_precision_view_navigation_preserves_drafts_documents_history_and_evaluation() {
    let mut s = session();
    let document = s.editor.project().clone();
    let draft = s.draft.clone();
    let evaluated: Vec<_> = [0., 0.5001, 0.50025, 0.5003, 0.5005, 1.]
        .map(|x| transition().evaluate(x).unwrap())
        .into();
    graph_canvas::fit(&mut s);
    let fitted = s.graph_view.view;
    for factor in [0.001, 0.1, 10., 1000.] {
        let v = fitted
            .zoom((0.5003, 0.3), factor, 1.)
            .pan((0.00001, 0.001), 1.);
        let m = mapping(v);
        assert!(
            !graph_curve::sample(&transition(), 0., &m, 2048)
                .unwrap()
                .limited
        );
        s.graph_view.set(v);
    }
    graph_canvas::fit(&mut s);
    assert_eq!(s.graph_view.view, fitted);
    assert_eq!(s.editor.project(), &document);
    assert_eq!(s.draft, draft);
    assert!(!s.editor.can_undo());
    for (i, x) in [0., 0.5001, 0.50025, 0.5003, 0.5005, 1.]
        .into_iter()
        .enumerate()
    {
        assert_eq!(evaluated[i], transition().evaluate(x).unwrap());
    }
    // Existing explicit apply/undo/redo behavior and serialization keep the same data.
    let channel = s.draft.channel_mut(ChannelKey::Pitch).unwrap();
    channel.transitions[0] = Transition::nurbs(1, 1., 0., 0.);
    assert!(s.apply_draft());
    s.undo(false);
    assert_eq!(s.editor.project(), &document);
    s.undo(true);
    let json = serde_json::to_string(s.editor.project()).unwrap();
    let reopened: qx_document::Project = serde_json::from_str(&json).unwrap();
    assert_eq!(&reopened, s.editor.project());
    assert!(
        View::fit(
            reopened.sections[0]
                .clone()
                .channel_mut(ChannelKey::Pitch)
                .unwrap()
        )
        .is_some()
    );
}

#[test]
fn graph_precision_exhausted_budget_is_visible_and_explained() {
    let ctx = egui::Context::default();
    let m = mapping(View::exact([0., 1., -0.1, 0.8], 1.).unwrap());
    let mut budget = 0;
    let mut output = ctx.run_pass(egui::RawInput::default(), |ctx| {
        let message = graph_curve::paint(
            &ctx.layer_painter(egui::LayerId::background()),
            &m,
            &transition(),
            0.,
            graph_curve::Appearance::Draft {
                color: super::theme::VIOLET,
                active: true,
            },
            &mut budget,
        )
        .unwrap();
        assert!(message.contains("outlined ranges"));
    });
    assert!(
        output
            .shapes
            .iter()
            .any(|s| s.shape.visual_bounding_rect().height() > 250.)
    );
    output.textures_delta.clear();
}

/// Development-only graph comparison surface. The old case reconstructs the M1.0
/// 129-point painter on the identical rectangle/view; it is labeled as such.
struct Comparison {
    old: bool,
    zoom: bool,
}
impl EditorSurface for Comparison {
    fn ui(
        &mut self,
        ctx: &egui::Context,
        _: &InspectionScene,
        _: &mut DisplayOptions,
    ) -> SurfaceOutput {
        super::theme::install(ctx);
        ctx.root_ui(|root| {
            egui::CentralPanel::default().show(root, |ui| {
                ui.heading(if self.old {
                    "M1.0 uniform sampling · reconstructed"
                } else {
                    "M1.1 knot-span bounds and adaptive sampling"
                });
                ui.label(
                "Development-only evidence · synthetic C2 profile · no native desktop interaction",
            );
                ui.label("Peak evaluator value: 0.666666666667 at x = 0.5003");
                let (_, outer) = ui.allocate_space(egui::vec2(
                    ui.available_width(),
                    ui.available_height() - 25.,
                ));
                let rect = graph_canvas::plot_rect(outer);
                let x = if self.zoom { [0.5, 0.5006] } else { [0., 1.] };
                let m = Plot::new(rect, x, [-0.1, 0.8]).unwrap();
                m.paint_grid(ui, outer, "m", "rad/m");
                let painter = ui.painter().with_clip_rect(rect);
                if self.old {
                    let points: Vec<_> = (0..=128)
                        .map(|i| {
                            let x = x[0] + (x[1] - x[0]) * i as f64 / 128.;
                            m.map(x, transition().evaluate(x).unwrap().value)
                        })
                        .collect();
                    painter.add(egui::Shape::line(
                        points.clone(),
                        egui::Stroke::new(4.5, egui::Color32::from_gray(190)),
                    ));
                    painter.add(egui::Shape::line(
                        points,
                        egui::Stroke::new(2.5, super::theme::VIOLET),
                    ));
                } else {
                    assert!(
                        graph_curve::paint(
                            &painter,
                            &m,
                            &transition(),
                            0.,
                            graph_curve::Appearance::Draft {
                                color: super::theme::VIOLET,
                                active: true
                            },
                            &mut 16384
                        )
                        .is_none()
                    );
                }
            });
        });
        SurfaceOutput {
            viewport: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(2., 2.)),
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
    fn open_path(&mut self, _: &std::path::Path) {}
    fn request_close(&mut self) -> bool {
        true
    }
}

#[test]
#[ignore = "opt-in GPU evidence export: cargo test -p quantum-x graph_precision_offscreen_evidence -- --ignored --nocapture"]
fn graph_precision_offscreen_evidence() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m1.1-evidence/captures");
    std::fs::create_dir_all(&root).unwrap();
    let p = project();
    let mut s = session();
    let scene = scene(&p, &s.catalog, s.options).unwrap();
    qx_document::persistence::save(&root.join("fixture.qx.json"), &p, &s.catalog).unwrap();
    let mut adapters = vec![];
    for (name, old, zoom) in [
        ("before-identical-view", true, false),
        ("after-identical-view", false, false),
        ("before-zoom", true, true),
        ("after-zoom", false, true),
    ] {
        adapters.push(
            qx_rendering::render_offscreen(
                &scene,
                &mut Comparison { old, zoom },
                &root.join(format!("{name}.png")),
                1200,
                650,
            )
            .unwrap(),
        );
    }
    graph_canvas::fit(&mut s);
    adapters.push(
        qx_rendering::render_offscreen(&scene, &mut s, &root.join("editor-fit.png"), 1600, 1000)
            .unwrap(),
    );
    s.graph_view
        .set(View::exact([0.5, 0.5006, -0.1, 0.8], 1.).unwrap());
    adapters.push(
        qx_rendering::render_offscreen(&scene, &mut s, &root.join("editor-zoom.png"), 1600, 1000)
            .unwrap(),
    );
    let t = transition();
    let plot = qx_geometry::profile_plot::sample(&t, [0., 1.], 0.001, 2048).unwrap();
    let max = plot
        .segments
        .iter()
        .flat_map(|s| [s.from[1], s.to[1]])
        .fold(0_f64, f64::max);
    let report = serde_json::json!({"fixture":"synthetic C2 cubic, not an owner-authored coaster",
        "old_129_point_max":(0..=128).map(|i|t.evaluate(i as f64/128.).unwrap().value).fold(0_f64,f64::max),
        "old_513_point_max":(0..=512).map(|i|t.evaluate(i as f64/512.).unwrap().value).fold(0_f64,f64::max),
        "evaluator_peak":t.evaluate(0.5003).unwrap().value,"new_display_max":max,
        "new_display_segments":plot.segments.len(),"limited":plot.limited,
        "conservative_bounds":qx_geometry::profile_plot::bounds(&t).unwrap(),
        "fit_view":View::fit(p.sections[0].clone().channel_mut(ChannelKey::Pitch).unwrap()).unwrap().bounds(),
        "comparison_view":[0.,1.,-0.1,0.8],"comparison_size":[1200,650],
        "before_images":"explicit reconstruction of old grid/painter; identical data, size and view",
        "native_gui_manually_tested":false,"adapters":adapters});
    std::fs::write(
        root.join("numerical-evidence.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
