//! Read-only external geometry. Never becomes Regions, document state or physics.
mod alignment;
mod csv;
mod support_xml;
pub use alignment::{AlignmentReport, ConnectorResidual, alignment};
pub use csv::{CsvFormat, ImportedTrack, NATIVE_HEADER, NL2_HEADER, parse_csv, write_native_csv};
pub use support_xml::{
    Beam, Footer, ImportedSupports, Prefab, RailConnector, SupportNode, parse_support_xml,
};

use crate::{GeometryError, Result, construction::Bounds, math::Vec3};
use std::{io::Read, path::Path};

pub const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_SAMPLES: usize = 100_000;
pub const MAX_SUPPORT_NODES: usize = 20_000;
pub const MAX_BEAMS: usize = 10_000;

/// The extra byte detects oversize inputs without trusting file metadata.
pub fn read_text(path: &Path) -> Result<String> {
    let file =
        std::fs::File::open(path).map_err(|e| GeometryError(format!("{}: {e}", path.display())))?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| GeometryError(format!("{}: {e}", path.display())))?;
    crate::ensure(
        bytes.len() <= MAX_INPUT_BYTES,
        "diagnostic input exceeds 32 MiB",
    )?;
    String::from_utf8(bytes).map_err(|e| GeometryError(format!("input must be UTF-8: {e}")))
}

fn number(text: &str) -> Result<f64> {
    let value: f64 = text
        .trim()
        .parse()
        .map_err(|_| GeometryError(format!("invalid number: {text:.80}")))?;
    crate::ensure(
        value.is_finite() && value.abs() <= 1e8,
        "number is nonfinite or exceeds diagnostic limits",
    )?;
    Ok(value)
}
fn position(v: Vec3) -> Result<Vec3> {
    crate::ensure(
        v.is_finite() && v.length() <= 1e8,
        "position exceeds finite world limits",
    )?;
    Ok(v)
}
fn bounds(points: impl IntoIterator<Item = Vec3>) -> Option<Bounds> {
    let mut points = points.into_iter();
    let mut b = Bounds::point(points.next()?);
    for p in points {
        b.include(p);
    }
    Some(b)
}
