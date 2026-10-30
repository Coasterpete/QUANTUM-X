//! Shared engineering plot presentation. Coordinates stay f64 until painting;
//! the display lattice never participates in authored values or snapping.
use egui::{Color32, Pos2, Rect, Stroke, Ui};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Axis {
    pub min: f64,
    pub max: f64,
}
impl Axis {
    pub fn new(min: f64, max: f64) -> Option<Self> {
        (min.is_finite() && max.is_finite() && (max - min).is_finite() && max > min)
            .then_some(Self { min, max })
    }
    pub fn fraction(self, value: f64) -> f64 {
        (value - self.min) / (self.max - self.min)
    }
    pub fn span(self) -> f64 {
        self.max - self.min
    }
    pub fn zoom(self, anchor: f64, factor: f64) -> Option<Self> {
        if !anchor.is_finite() || !factor.is_finite() || factor <= 0. {
            return None;
        }
        Self::new(
            anchor + (self.min - anchor) * factor,
            anchor + (self.max - anchor) * factor,
        )
    }
    pub fn shifted(self, delta: f64) -> Option<Self> {
        Self::new(self.min + delta, self.max + delta)
    }
    pub fn within(self, min: f64, max: f64) -> Option<Self> {
        let bounds = Self::new(min, max)?;
        let span = (self.max - self.min).min(bounds.max - bounds.min);
        let left = self.min.clamp(min, max - span);
        Self::new(left, left + span)
    }
    pub fn ticks(self, pixels: f32) -> Vec<f64> {
        if !pixels.is_finite() || pixels <= 0. {
            return vec![];
        }
        let intervals = (pixels as f64 / 24.).floor().clamp(1., 128.);
        let raw = (self.max - self.min) / intervals;
        let decade = 10_f64.powf(raw.log10().floor());
        if !decade.is_finite() || decade <= 0. {
            return vec![];
        }
        let fraction = raw / decade;
        let step = (if fraction <= 1. {
            1.
        } else if fraction <= 2. {
            2.
        } else if fraction <= 5. {
            5.
        } else {
            10.
        }) * decade;
        if !step.is_finite() || step <= 0. {
            return vec![];
        }
        let first = (self.min / step).ceil() * step;
        if !first.is_finite() {
            return vec![];
        }
        (0..=128)
            .map(|i| first + i as f64 * step)
            .filter(|v| v.is_finite() && *v >= self.min && *v <= self.max)
            .fold(Vec::new(), |mut out, v| {
                if out.last() != Some(&v) {
                    out.push(v);
                }
                out
            })
    }
}
pub(super) struct Plot {
    pub rect: Rect,
    pub x: Axis,
    pub y: Axis,
}
impl Plot {
    pub fn new(rect: Rect, x: [f64; 2], y: [f64; 2]) -> Option<Self> {
        if !rect.is_finite() || rect.width() < 2. || rect.height() < 2. {
            return None;
        }
        Some(Self {
            rect,
            x: Axis::new(x[0], x[1])?,
            y: Axis::new(y[0], y[1])?,
        })
    }
    pub fn map(&self, x: f64, y: f64) -> Pos2 {
        // Bound extreme offscreen values before the GPU f32 conversion. Curves are clipped.
        egui::pos2(
            self.rect.left() + self.x.fraction(x).clamp(-1e4, 1e4) as f32 * self.rect.width(),
            self.rect.bottom() - self.y.fraction(y).clamp(-1e4, 1e4) as f32 * self.rect.height(),
        )
    }
    pub fn unmap(&self, point: Pos2) -> (f64, f64) {
        (
            self.x.min
                + (point.x - self.rect.left()) as f64 / self.rect.width() as f64 * self.x.span(),
            self.y.max
                - (point.y - self.rect.top()) as f64 / self.rect.height() as f64 * self.y.span(),
        )
    }
    pub fn paint_grid(&self, ui: &Ui, outer: Rect, x_unit: &str, y_unit: &str) {
        let p = ui
            .painter()
            .with_clip_rect(self.rect.intersect(ui.clip_rect()));
        p.rect_filled(self.rect, 0., Color32::from_gray(13));
        let xs = self.x.ticks(self.rect.width());
        let ys = self.y.ticks(self.rect.height());
        for &x in &xs {
            for &y in &ys {
                p.circle_filled(self.map(x, y), 0.85, Color32::from_gray(66));
            }
        }
        if self.y.min <= 0. && self.y.max >= 0. {
            p.line_segment(
                [self.map(self.x.min, 0.), self.map(self.x.max, 0.)],
                Stroke::new(1., Color32::from_gray(94)),
            );
        }
        if self.x.min <= 0. && self.x.max >= 0. {
            p.line_segment(
                [self.map(0., self.y.min), self.map(0., self.y.max)],
                Stroke::new(1., Color32::from_gray(94)),
            );
        }
        let labels = ui.painter().with_clip_rect(outer.intersect(ui.clip_rect()));
        let font = egui::FontId::monospace(13.);
        // Label density is independent of the quiet dot lattice density.
        let label_width = xs
            .iter()
            .map(|v| number(*v, self.x).len())
            .max()
            .unwrap_or(1) as f32
            * 8.
            + 18.;
        let x_stride = (xs.len() as f32 * label_width / self.rect.width())
            .ceil()
            .max(1.) as usize;
        let y_stride = (ys.len() as f32 * 18. / self.rect.height()).ceil().max(1.) as usize;
        for &x in xs.iter().step_by(x_stride) {
            let point = self.map(x, self.y.min);
            labels.text(
                point + egui::vec2(0., 3.),
                egui::Align2::CENTER_TOP,
                number(x, self.x),
                font.clone(),
                Color32::from_gray(200),
            );
        }
        // Keep exact view bounds legible even in a short pane whose lattice has
        // only a zero tick. Interior labels may yield to these two references.
        let mut y_labels = vec![self.y.min, self.y.max];
        for &y in ys.iter().step_by(y_stride) {
            let point = self.map(self.x.min, y);
            if y_labels
                .iter()
                .all(|v| (self.map(self.x.min, *v).y - point.y).abs() >= 17.)
            {
                y_labels.push(y);
            }
        }
        for y in y_labels {
            let point = self.map(self.x.min, y);
            let text = number(y, self.y);
            let available = (self.rect.left() - outer.left() - 10.).max(1.);
            let measured = ui
                .painter()
                .layout_no_wrap(text.clone(), font.clone(), Color32::WHITE);
            let label_font =
                egui::FontId::monospace(font.size * (available / measured.size().x).min(1.));
            labels.text(
                point - egui::vec2(5., 0.),
                egui::Align2::RIGHT_CENTER,
                text,
                label_font,
                Color32::from_gray(200),
            );
        }
        labels.text(
            outer.left_top(),
            egui::Align2::LEFT_TOP,
            y_unit,
            font.clone(),
            Color32::WHITE,
        );
        labels.text(
            outer.right_bottom(),
            egui::Align2::RIGHT_BOTTOM,
            x_unit,
            font,
            Color32::WHITE,
        );
    }
}
pub(super) fn number(value: f64, axis: Axis) -> String {
    // Precision follows visible differences, including a small span at a large offset.
    let decimals = (-(axis.span() / 20.).log10().floor()).clamp(0., 14.) as usize;
    if value != 0. && (value.abs() < 0.001 || value.abs() >= 1e5) {
        let significant = (value.abs().log10().floor() - (axis.span() / 20.).log10().floor())
            .clamp(2., 14.) as usize;
        format!("{value:.significant$e}")
    } else {
        let text = format!("{value:.decimals$}");
        if decimals == 0 {
            text
        } else {
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lattice_adapts_to_range_and_pixels_without_nonfinite_or_unbounded_work() {
        for (min, max) in [
            (-1., 3.),
            (0., 1e-10),
            (-1e12, 1e12),
            (1e10, 1e10 + 0.1),
            (-1e300, 1e300),
        ] {
            let axis = Axis::new(min, max).unwrap();
            for width in [1., 100., 500., 10000.] {
                let ticks = axis.ticks(width);
                assert!(ticks.len() <= 129);
                assert!(
                    ticks
                        .iter()
                        .all(|v| v.is_finite() && *v >= min && *v <= max)
                );
                assert!(ticks.windows(2).all(|v| v[0] < v[1]));
            }
        }
        assert!(Axis::new(f64::NAN, 1.).is_none());
        assert!(Axis::new(1., 1.).is_none());
        assert!(Axis::new(-f64::MAX, f64::MAX).is_none());
        assert!(Axis::new(0., 1.).unwrap().ticks(f32::NAN).is_empty());
        assert!(
            Axis::new(0., 100.).unwrap().ticks(1000.).len()
                > Axis::new(0., 100.).unwrap().ticks(100.).len()
        );
    }
    #[test]
    fn plot_maps_origin_endpoints_and_rejects_degenerate_rectangles() {
        let rect = Rect::from_min_max(egui::pos2(50., 10.), egui::pos2(450., 210.));
        let p = Plot::new(rect, [10., 30.], [-2., 2.]).unwrap();
        assert_eq!(p.map(10., -2.), rect.left_bottom());
        assert_eq!(p.map(30., 2.), rect.right_top());
        assert_eq!(p.map(20., 0.), rect.center());
        assert!(Plot::new(Rect::NOTHING, [0., 1.], [0., 1.]).is_none());
    }
    #[test]
    fn zoom_keeps_anchor_fraction_and_pan_bounds_preserve_span() {
        let axis = Axis::new(10., 30.).unwrap();
        let zoom = axis.zoom(15., 0.25).unwrap();
        assert!((zoom.fraction(15.) - axis.fraction(15.)).abs() < 1e-14);
        let bounded = zoom.shifted(-100.).unwrap().within(0., 40.).unwrap();
        assert_eq!([bounded.min, bounded.max], [0., 5.]);
        let bounded = zoom.shifted(100.).unwrap().within(0., 40.).unwrap();
        assert_eq!([bounded.min, bounded.max], [35., 40.]);
        let wide = axis.zoom(15., 100.).unwrap().within(0., 40.).unwrap();
        assert_eq!([wide.min, wide.max], [0., 40.]);
        assert!(axis.zoom(15., 0.).is_none());
        assert!(axis.zoom(f64::NAN, 1.).is_none());
        assert!(axis.shifted(f64::INFINITY).is_none());
        assert!(axis.within(1., 1.).is_none());
    }
    #[test]
    fn labels_keep_integer_zeros_and_resolve_small_differences_at_large_offsets() {
        let axis = Axis::new(0., 60.).unwrap();
        for value in [0., 10., 20., 40., 60.] {
            assert_eq!(number(value, axis), format!("{value:.0}"));
        }
        for axis in [
            Axis::new(0., 1e-10).unwrap(),
            Axis::new(1e10, 1e10 + 0.1).unwrap(),
        ] {
            let labels: Vec<_> = axis
                .ticks(600.)
                .into_iter()
                .map(|v| number(v, axis))
                .collect();
            assert!(labels.windows(2).all(|v| v[0] != v[1]));
        }
    }
}
