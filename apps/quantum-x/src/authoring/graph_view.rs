//! Session-only graph navigation. No view operation can write a Section or command.
use super::{plot::Axis, session::Session};
use qx_geometry::{
    authoring::Domain,
    transition::{Channel, Unit},
};
use std::collections::BTreeMap;

const MAX_Y: f64 = 1e24;
const MIN_SPAN: f64 = 1e-10;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct View {
    pub x: Axis,
    pub y: Axis,
}
impl Default for View {
    fn default() -> Self {
        Self {
            x: Axis { min: 0., max: 1. },
            y: Axis { min: -1., max: 3. },
        }
    }
}
impl View {
    pub fn bounds(self) -> [f64; 4] {
        [self.x.min, self.x.max, self.y.min, self.y.max]
    }
    pub fn exact(bounds: [f64; 4], extent: f64) -> Result<Self, &'static str> {
        Axis::new(0., extent).ok_or("Section domain must have a finite, positive extent.")?;
        let x = Axis::new(bounds[0], bounds[1]).ok_or("X bounds must be finite and From < To.")?;
        let y = Axis::new(bounds[2], bounds[3]).ok_or("Y bounds must be finite and From < To.")?;
        if x.min < 0. || x.max > extent || x.span() < min_x_span(extent) {
            return Err("X range must stay inside the section domain and have a resolvable span.");
        }
        if y.min < -MAX_Y || y.max > MAX_Y || y.span() < min_y_span(y) {
            return Err(
                "Y range must stay inside ±1e24 and have a resolvable span (at least 1e-10).",
            );
        }
        Ok(Self { x, y })
    }
    pub fn reset(channel: &Channel) -> Self {
        let y = match channel.unit {
            Unit::G => Axis { min: -1., max: 3. },
            Unit::RadiansPerMetre => Axis {
                min: -0.05,
                max: 0.05,
            },
            _ => Axis { min: -1., max: 1. },
        };
        Self {
            x: Axis {
                min: 0.,
                max: channel.length(),
            },
            y,
        }
    }
    pub fn fit(channel: &Channel) -> Option<Self> {
        channel.validate().ok()?;
        let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
        for t in &channel.transitions {
            for i in 0..=512 {
                let value = t.evaluate(t.length * (i as f64 / 512.)).ok()?.value;
                min = min.min(value);
                max = max.max(value);
            }
        }
        let constant_margin: f64 = match channel.unit {
            Unit::RadiansPerMetre => 0.05,
            _ => 1.,
        };
        let margin = if max == min {
            constant_margin.max(min.abs() * 0.05)
        } else {
            ((max - min) * 0.15).max(MIN_SPAN)
        };
        let view = Self {
            x: Axis::new(0., channel.length())?,
            y: Axis::new(min - margin, max + margin)?,
        };
        Self::exact(view.bounds(), channel.length()).ok()
    }
    pub fn zoom(self, anchor: (f64, f64), factor: f64, extent: f64) -> Self {
        if !factor.is_finite() || factor <= 0. || Self::exact(self.bounds(), extent).is_err() {
            return self;
        }
        let fx = factor.clamp(min_x_span(extent) / self.x.span(), extent / self.x.span());
        let fy = factor.clamp(
            min_y_span(self.y) / self.y.span(),
            2. * MAX_Y / self.y.span(),
        );
        let x = self.x.zoom(anchor.0, fx).and_then(|v| v.within(0., extent));
        let y = self
            .y
            .zoom(anchor.1, fy)
            .and_then(|v| v.within(-MAX_Y, MAX_Y));
        match (x, y) {
            (Some(x), Some(y)) => Self::exact(Self { x, y }.bounds(), extent).unwrap_or(self),
            _ => self,
        }
    }
    pub fn pan(self, delta: (f64, f64), extent: f64) -> Self {
        if Self::exact(self.bounds(), extent).is_err() {
            return self;
        }
        let x = self.x.shifted(delta.0).and_then(|v| v.within(0., extent));
        let y = self
            .y
            .shifted(delta.1)
            .and_then(|v| v.within(-MAX_Y, MAX_Y));
        match (x, y) {
            (Some(x), Some(y)) => Self::exact(Self { x, y }.bounds(), extent).unwrap_or(self),
            _ => self,
        }
    }
}
fn min_x_span(extent: f64) -> f64 {
    (extent * 1e-9).max(MIN_SPAN).min(extent)
}
fn min_y_span(axis: Axis) -> f64 {
    MIN_SPAN.max(axis.min.abs().max(axis.max.abs()) * f64::EPSILON * 32.)
}

#[derive(Default)]
pub(super) struct State {
    pub view: View,
    active: Option<(u64, u8)>,
    domain: Option<(Domain, Unit)>,
    saved: BTreeMap<(u64, u8), ((Domain, Unit), View)>,
    pub range_edit: [f64; 4],
    pub error: Option<&'static str>,
}
impl State {
    pub fn set(&mut self, view: View) {
        self.view = view;
        self.range_edit = view.bounds();
        self.error = None;
    }
    pub fn activate(&mut self, id: u64, key: u8, domain: Domain, channel: &Channel) {
        let context = (id, key);
        let units = (domain, channel.unit);
        if self.active != Some(context) || self.domain != Some(units) {
            if let (Some(old), Some(units)) = (self.active, self.domain) {
                self.saved.insert(old, (units, self.view));
            }
            let restored = self
                .saved
                .get(&context)
                .filter(|(u, _)| *u == units)
                .map(|(_, v)| *v);
            self.set(
                restored
                    .unwrap_or_else(|| View::fit(channel).unwrap_or_else(|| View::reset(channel))),
            );
            self.active = Some(context);
            self.domain = Some(units);
        }
        // Extent edits/undo constrain X, without refitting Y or altering drafts.
        if let Some(x) = self.view.x.within(0., channel.length())
            && x != self.view.x
        {
            self.set(View { x, ..self.view });
        }
    }
    pub fn apply_range(&mut self, extent: f64) {
        match View::exact(self.range_edit, extent) {
            Ok(view) => self.set(view),
            Err(error) => self.error = Some(error),
        }
    }
}
pub(super) fn sync(session: &mut Session) {
    let domain = session.draft.domain;
    let channel = session.draft.channel_mut(session.channel).unwrap();
    session
        .graph_view
        .activate(session.selected.0, session.channel as u8, domain, channel);
}
