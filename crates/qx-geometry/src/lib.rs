//! Double-precision graph authoring and reference-speed track generation.
//! This crate contains no vehicle dynamics, graphics, or editor widgets.
pub mod arc_length;
pub mod authoring;
pub mod construction;
pub mod curve;
pub mod curve_edit;
pub mod heartline;
pub mod math;
pub mod solver;
mod spline_track;
pub mod transition;

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeometryError(pub String);
impl fmt::Display for GeometryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for GeometryError {}
pub type Result<T> = std::result::Result<T, GeometryError>;
/// Input-boundary tolerance stays below the shortest supported transition,
/// even at maximum channel extent. It avoids roundoff-sized integration spans.
pub(crate) fn same_input(a: f64, b: f64) -> bool {
    approximately_equal(a, b, 8.0 * f64::EPSILON, 8.0 * f64::EPSILON)
}
#[derive(Default)]
pub(crate) struct InputSum {
    value: f64,
    correction: f64,
}
impl InputSum {
    pub fn add(&mut self, value: f64) -> f64 {
        let adjusted = value - self.correction;
        let next = self.value + adjusted;
        self.correction = (next - self.value) - adjusted;
        self.value = next;
        next
    }
}
pub(crate) fn ensure(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(GeometryError(message.into()))
    }
}
pub fn approximately_equal(a: f64, b: f64, absolute: f64, relative: f64) -> bool {
    a.is_finite()
        && b.is_finite()
        && absolute.is_finite()
        && relative.is_finite()
        && absolute >= 0.0
        && relative >= 0.0
        && (a - b).abs() <= absolute + relative * a.abs().max(b.abs())
}
