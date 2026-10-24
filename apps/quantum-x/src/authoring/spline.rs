use qx_geometry::{
    authoring::{Section, SectionKind},
    curve::{Curve, Parameterization},
    curve_edit::{BezierContinuity, CurveEdit},
};
pub(super) fn controls(section: &mut Section, ui: &mut egui::Ui, status: &mut String) {
    let SectionKind::Spline { curve } = &mut section.kind else {
        return;
    };
    ui.label("Section-local metres. Start must remain zero with tangent +X. Apply publishes one undo step.");
    egui::Grid::new("spline points")
        .striped(true)
        .show(ui, |ui| {
            ui.label("ID");
            ui.label("X m");
            ui.label("Y m");
            ui.label("Z m");
            ui.end_row();
            for point in curve.points_mut() {
                ui.label(point.id.to_string());
                for value in [
                    &mut point.position_m.x,
                    &mut point.position_m.y,
                    &mut point.position_m.z,
                ] {
                    ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(12));
                }
                ui.end_row();
            }
        });
    match curve {
        Curve::Nurbs {
            weights,
            degree,
            knots,
            points,
        } => {
            ui.label("Positive rational weights");
            ui.horizontal_wrapped(|ui| {
                for w in weights {
                    ui.add(egui::DragValue::new(w).speed(0.01).max_decimals(12));
                }
            });
            knots_controls(ui, degree, knots, points.len(), status);
        }
        Curve::BSpline {
            degree,
            knots,
            points,
        } => knots_controls(ui, degree, knots, points.len(), status),
        Curve::CatmullRom {
            parameterization, ..
        } => {
            egui::ComboBox::from_id_salt("catmull parameterization")
                .selected_text(format!("{parameterization:?}"))
                .show_ui(ui, |ui| {
                    for p in [
                        Parameterization::Uniform,
                        Parameterization::Chordal,
                        Parameterization::Centripetal,
                    ] {
                        ui.selectable_value(parameterization, p, format!("{p:?}"));
                    }
                });
        }
        _ => {}
    }
    ui.collapsing("Explicit curve operations (staged; Apply / Cancel)", |ui| {
        let mut operation = None;
        if ui.button("Align start to local zero / +X").clicked() {
            operation = Some(CurveEdit::AlignStart);
        }
        if matches!(curve, Curve::Bezier { .. } | Curve::BSpline { .. })
            && ui.button("Exact supported conversion to NURBS").clicked()
        {
            operation = Some(CurveEdit::ConvertToNurbs);
        }
        if let Curve::Bezier { points } = curve {
            for point in points
                .iter()
                .skip(3)
                .step_by(3)
                .take(points.len().saturating_sub(4) / 3)
            {
                ui.horizontal(|ui| {
                    ui.label(format!("Join {}", point.id));
                    for (label, continuity) in [
                        ("Fit G1", BezierContinuity::G1),
                        ("Fit C1", BezierContinuity::C1),
                        ("Fit C2", BezierContinuity::C2),
                    ] {
                        if ui.button(label).clicked() {
                            operation = Some(CurveEdit::FitBezierJoin {
                                endpoint_id: point.id,
                                continuity,
                            });
                        }
                    }
                });
            }
        }
        if let Some(operation) = operation {
            let mut candidate = curve.clone();
            match operation.apply(&mut candidate) {
                Ok(()) => *curve = candidate,
                Err(e) => *status = format!("Curve edit rejected: {e}"),
            }
        }
    });
}
fn knots_controls(
    ui: &mut egui::Ui,
    degree: &mut usize,
    knots: &mut Vec<f64>,
    count: usize,
    status: &mut String,
) {
    ui.horizontal(|ui| {
        ui.label("Degree (explicit reinterpretation)");
        ui.add(egui::DragValue::new(degree).range(1..=8));
    });
    if ui
        .button("Rebuild clamped knots for current degree (reshape)")
        .clicked()
    {
        match qx_geometry::curve::clamped_knots(count, *degree) {
            Ok(new) => *knots = new,
            Err(error) => *status = error.to_string(),
        }
    }
    ui.label("Knots (count must match controls + degree + 1)");
    ui.horizontal_wrapped(|ui| {
        for knot in knots {
            ui.add(egui::DragValue::new(knot).speed(0.01).max_decimals(12));
        }
    });
}
