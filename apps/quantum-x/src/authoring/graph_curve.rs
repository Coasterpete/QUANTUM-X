//! Shared draft/committed painting. Dashing is applied after mathematical sampling.
use super::plot::Plot;
use egui::{Color32, Pos2, Stroke};
use qx_geometry::{profile_plot, transition::Transition};

#[derive(Clone, Copy)]
pub(super) enum Appearance {
    Draft { color: Color32, active: bool },
    Committed,
}

pub(super) fn sample(
    transition: &Transition,
    offset: f64,
    mapping: &Plot,
    budget: usize,
) -> qx_geometry::Result<profile_plot::Plot> {
    let range = [mapping.x.min, mapping.x.max].map(|x| (x - offset).clamp(0., transition.length));
    // Half a logical pixel vertically. Knot spans remain mandatory even when
    // their horizontal width is smaller than a pixel in the current view.
    profile_plot::sample(
        transition,
        range,
        mapping.y.span() * 0.5 / mapping.rect.height() as f64,
        budget.clamp(1, 2048),
    )
}

pub(super) fn paint(
    painter: &egui::Painter,
    mapping: &Plot,
    transition: &Transition,
    offset: f64,
    appearance: Appearance,
    budget: &mut usize,
) -> Option<String> {
    let plot = match sample(transition, offset, mapping, *budget) {
        Ok(plot) => plot,
        Err(e) => return Some(format!("Graph cannot be sampled: {e}")),
    };
    *budget = budget.saturating_sub(plot.segments.len());
    let stroke = match appearance {
        Appearance::Draft { color, active } => Stroke::new(
            if active { 2.5 } else { 1.5 },
            if active {
                color
            } else {
                color.gamma_multiply(0.65)
            },
        ),
        Appearance::Committed => Stroke::new(1., Color32::from_gray(155)),
    };
    let flush = |points: &mut Vec<Pos2>| {
        if points.len() < 2 {
            return;
        }
        match appearance {
            Appearance::Committed => {
                painter.extend(egui::Shape::dashed_line(points, stroke, 5., 5.))
            }
            Appearance::Draft { .. } => {
                painter.add(egui::Shape::line(
                    points.clone(),
                    Stroke::new(stroke.width + 2., Color32::from_gray(190)),
                ));
                painter.add(egui::Shape::line(std::mem::take(points), stroke));
            }
        }
        points.clear();
    };
    let mut points = Vec::new();
    for segment in plot.segments {
        let a = mapping.map(offset + segment.from[0], segment.from[1]);
        let b = mapping.map(offset + segment.to[0], segment.to[1]);
        if segment.resolved {
            if points.is_empty() {
                points.push(a);
            }
            points.push(b);
        } else {
            flush(&mut points);
            // An unresolved curve is an enclosure, not a fabricated straight line.
            // Keep even subpixel intervals visible; the status line explains it.
            let mut rect = egui::Rect::from_two_pos(
                mapping.map(offset + segment.from[0], segment.bounds[0]),
                mapping.map(offset + segment.to[0], segment.bounds[1]),
            );
            if rect.width() < 1. {
                rect = rect.expand2(egui::vec2(0.5, 0.));
            }
            painter.rect_stroke(rect, 0., stroke, egui::StrokeKind::Inside);
        }
    }
    flush(&mut points);
    plot.limited.then(|| "Graph detail limit: outlined ranges enclose unresolved curve values; zoom in for detail.".into())
}
