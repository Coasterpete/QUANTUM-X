use super::{MAX_INPUT_BYTES, MAX_SAMPLES, bounds, number, position};
use crate::{
    GeometryError, Result,
    construction::Bounds,
    ensure,
    math::{Frame, Quaternion, Vec3, rotation_between},
    solver::{CenterlineSample, GeneratedTrack},
};
use std::io::Write;

pub const NL2_HEADER: &str =
    "No.\tPosX\tPosY\tPosZ\tFrontX\tFrontY\tFrontZ\tLeftX\tLeftY\tLeftZ\tUpX\tUpY\tUpZ";
pub const NATIVE_HEADER: &str = "distance_m,design_time_s,x_m,y_m,z_m,tangent_x,tangent_y,tangent_z,rider_normal_x,rider_normal_y,rider_normal_z,lateral_x,lateral_y,lateral_z,quaternion_w,quaternion_x,quaternion_y,quaternion_z";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CsvFormat {
    Native,
    NoLimits2,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedTrack {
    pub format: CsvFormat,
    pub samples: Vec<CenterlineSample>,
    pub bounds: Bounds,
    pub polyline_length_m: f64,
    pub endpoint_gap_m: f64,
    /// Largest source axis change when rounded vectors become a rotation.
    pub max_frame_correction: f64,
}
impl ImportedTrack {
    /// An adapter for existing mesh construction only: no regions, loads or speed.
    pub fn construction_track(&self) -> GeneratedTrack {
        GeneratedTrack {
            samples: self.samples.clone(),
            graphs: vec![],
            sections: vec![],
            diagnostics: vec![],
            connections: vec![],
            numerical: vec![],
            smooth_ranges: vec![],
        }
    }
}
fn vector(v: &[f64], i: usize) -> Vec3 {
    Vec3::new(v[i], v[i + 1], v[i + 2])
}
fn csv_number(cell: &str) -> Result<f64> {
    let cell = cell.trim();
    let value = if let Some(inner) = cell.strip_prefix('"') {
        inner
            .strip_suffix('"')
            .ok_or_else(|| GeometryError("unterminated quoted numeric cell".into()))?
    } else {
        cell
    };
    number(value)
}
fn frame(front: Vec3, up: Vec3, right: Vec3) -> Result<(Frame, f64)> {
    const TOL: f64 = 5e-5; // NL2 decimal export has six fractional places.
    ensure(
        [front, up, right]
            .iter()
            .all(|v| (v.length() - 1.).abs() <= TOL)
            && front.dot(up).abs() <= TOL
            && front.dot(right).abs() <= TOL
            && up.dot(right).abs() <= TOL
            && (front.cross(up) - right).length() <= 3. * TOL,
        "CSV orientation must be an orthonormal right-handed frame (front/up/minus-left for NL2)",
    )?;
    let t = front.normalized()?;
    let n = (up - t * up.dot(t)).normalized()?;
    let q = rotation_between(Vec3::X, t)?;
    let base = Frame::new(q)?.normal();
    let roll = t.dot(base.cross(n)).atan2(base.dot(n));
    let f = Frame::new(Quaternion::axis_angle(t, roll)?.compose(q))?;
    Ok((
        f,
        (f.tangent() - front)
            .length()
            .max((f.normal() - up).length())
            .max((f.lateral() - right).length()),
    ))
}
pub fn parse_csv(text: &str) -> Result<ImportedTrack> {
    ensure(text.len() <= MAX_INPUT_BYTES, "CSV exceeds 32 MiB")?;
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    let header = lines
        .next()
        .ok_or_else(|| GeometryError("CSV has no header".into()))?;
    let clean = header
        .split('\t')
        .map(|v| v.trim().trim_matches('"'))
        .collect::<Vec<_>>()
        .join("\t");
    let (format, delimiter, columns) = if clean == NL2_HEADER {
        (CsvFormat::NoLimits2, '\t', 13)
    } else if header.trim() == NATIVE_HEADER {
        (CsvFormat::Native, ',', 18)
    } else {
        return Err(GeometryError(
            "unsupported CSV header; expected native centerline.csv or NL2 13-column tab export"
                .into(),
        ));
    };
    let mut samples: Vec<CenterlineSample> = Vec::new();
    let mut length = 0.;
    let mut correction: f64 = 0.;
    for (i, line) in lines.enumerate() {
        let mut parse = || -> Result<CenterlineSample> {
            ensure(samples.len() < MAX_SAMPLES, "CSV exceeds 100000 samples")?;
            ensure(line.len() <= 4096, "CSV row exceeds 4096 bytes")?;
            let v = line
                .split(delimiter)
                .map(csv_number)
                .collect::<Result<Vec<_>>>()?;
            ensure(v.len() == columns, "incorrect CSV column count")?;
            let (p, f, station, time) = match format {
                CsvFormat::NoLimits2 => {
                    ensure(
                        v[0] == (i + 1) as f64,
                        "NL2 ordinals must start at 1 and increase by one",
                    )?;
                    let (f, c) = frame(vector(&v, 4), vector(&v, 10), -vector(&v, 7))?;
                    correction = correction.max(c);
                    (vector(&v, 1), f, 0., 0.)
                }
                CsvFormat::Native => {
                    let (f, c) = frame(vector(&v, 5), vector(&v, 8), vector(&v, 11))?;
                    let q = Quaternion {
                        w: v[14],
                        x: v[15],
                        y: v[16],
                        z: v[17],
                    };
                    ensure(
                        (q.w.hypot(q.x).hypot(q.y).hypot(q.z) - 1.).abs() <= 1e-9,
                        "native quaternion must have unit norm",
                    )?;
                    let fq = Frame::new(q)?;
                    ensure(
                        (fq.tangent() - f.tangent())
                            .length()
                            .max((fq.normal() - f.normal()).length())
                            <= 1e-9,
                        "native quaternion disagrees with frame vectors",
                    )?;
                    correction = correction.max(c);
                    (vector(&v, 2), fq, v[0], v[1])
                }
            };
            position(p)?;
            ensure(time >= 0., "negative design time")?;
            if let Some(prev) = samples.last() {
                let chord = (p - prev.position_m).length();
                ensure(chord > 1e-8, "duplicate consecutive CSV position")?;
                ensure(
                    (p - prev.position_m).dot(prev.frame.tangent() + f.tangent()) > 0.,
                    "sample ordering opposes tangent",
                )?;
                let angle = prev
                    .frame
                    .orientation()
                    .conjugate()
                    .compose(f.orientation())
                    .rotation_vector()?
                    .length();
                ensure(
                    angle <= 0.75,
                    "frame discontinuity exceeds construction limit (0.75 rad)",
                )?;
                length += chord;
                if format == CsvFormat::Native {
                    ensure(
                        station > prev.distance_m && time >= prev.design_time_s,
                        "native distance/time ordering invalid",
                    )?;
                    ensure(
                        station - prev.distance_m >= chord * (1. - 1e-7),
                        "native arc interval is shorter than its chord",
                    )?;
                }
            } else {
                ensure(station == 0., "native station must start at zero")?;
            }
            Ok(CenterlineSample {
                position_m: p,
                frame: f,
                distance_m: if format == CsvFormat::NoLimits2 {
                    length
                } else {
                    station
                },
                design_time_s: time,
            })
        };
        samples.push(parse().map_err(|e| GeometryError(format!("CSV row {}: {e}", i + 2)))?);
    }
    ensure(samples.len() >= 2, "CSV requires at least two samples")?;
    Ok(ImportedTrack {
        format,
        bounds: bounds(samples.iter().map(|s| s.position_m)).unwrap(),
        polyline_length_m: length,
        endpoint_gap_m: (samples[0].position_m - samples.last().unwrap().position_m).length(),
        samples,
        max_frame_correction: correction,
    })
}
/// The same schema and decimal round-trip precision as the existing native exporter.
pub fn write_native_csv(
    samples: &[CenterlineSample],
    mut output: impl Write,
) -> std::io::Result<()> {
    writeln!(output, "{NATIVE_HEADER}")?;
    for s in samples {
        let p = s.position_m;
        let t = s.frame.tangent();
        let n = s.frame.normal();
        let l = s.frame.lateral();
        let q = s.frame.orientation();
        let row = [
            s.distance_m,
            s.design_time_s,
            p.x,
            p.y,
            p.z,
            t.x,
            t.y,
            t.z,
            n.x,
            n.y,
            n.z,
            l.x,
            l.y,
            l.z,
            q.w,
            q.x,
            q.y,
            q.z,
        ]
        .map(|v| v.to_string());
        writeln!(output, "{}", row.join(","))?;
    }
    Ok(())
}
