use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StationWindow {
    /// Explicit unwrapped physical stations. Each wheel gets its own window.
    pub min_m: f64,
    pub max_m: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchOptions {
    pub max_span_m: f64,
    /// Maximum chord-deviation allowance; NOT a mechanical acceptance limit.
    pub approximation_m: f64,
    /// Branches this close in distance, including numerical uncertainty, are ambiguous.
    pub ambiguity_m: f64,
    pub max_segments: usize,
}
impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_span_m: 16.,
            approximation_m: 1e-6,
            ambiguity_m: 1e-5,
            max_segments: 4096,
        }
    }
}
impl SearchOptions {
    pub fn validate(self) -> Result<()> {
        ensure(
            self.max_span_m.is_finite()
                && (1e-6..=1000.).contains(&self.max_span_m)
                && self.approximation_m.is_finite()
                && (1e-9..=1e-3).contains(&self.approximation_m)
                && self.ambiguity_m.is_finite()
                && (0. ..=0.1).contains(&self.ambiguity_m)
                && (1..=16384).contains(&self.max_segments),
            "invalid bounded rail search options",
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchStatus {
    Resolved,
    /// Multiple near-minimum neighborhoods, or a flat/uncertain neighborhood
    /// reaching both endpoints. No unique local station is asserted.
    Ambiguous,
    /// Minimum neighborhood touches a search/track endpoint; extend or revise
    /// the explicit window. A zero perpendicular error here is not alignment.
    BoundaryMinimum,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PathCandidate {
    pub sample: PathSample,
    /// Linearized segment containing the candidate, not a certified root bracket.
    pub station_interval_m: [f64; 2],
    pub separation_m: f64,
    /// Tangent-normal distance on a smooth locus; full separation at a corner
    /// where there is no unique transverse plane.
    pub perpendicular_error_m: f64,
    pub longitudinal_error_m: f64,
    /// Rail-relative transverse basis, not gravity vertical (valid in inversions).
    pub lateral_error_m: Option<f64>,
    pub vertical_error_m: Option<f64>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct NearestPath {
    pub status: SearchStatus,
    /// One representative per disconnected near-minimum neighborhood. Ambiguous
    /// results deliberately do not designate any representative as authoritative.
    pub candidates: Vec<PathCandidate>,
    pub segments_tested: usize,
    /// Distance at the chosen station is within this of the window's true
    /// minimum under the documented smooth-piece/roundoff assumptions. This is
    /// a distance bound, NOT a longitudinal station-accuracy guarantee.
    pub distance_uncertainty_m: f64,
}
struct SegmentCandidate {
    station: f64,
    interval: [f64; 2],
    distance: f64,
}
impl PhysicalRailGeometry<'_> {
    /// Deterministic bounded point-to-curve measurement via adaptively bounded
    /// chord linearization. Never projects onto a remote station outside window.
    pub fn nearest(
        &self,
        path: &WheelCenterPath,
        point_m: Vec3,
        window: StationWindow,
        options: SearchOptions,
    ) -> Result<NearestPath> {
        options.validate()?;
        let offset = path.offset()?;
        ensure(
            point_m.is_finite()
                && window.min_m.is_finite()
                && window.max_m.is_finite()
                && window.min_m.abs() <= 1e7
                && window.max_m.abs() <= 1e7
                && window.max_m > window.min_m
                && window.max_m - window.min_m <= options.max_span_m,
            "invalid or excessive wheel station window",
        )?;
        let mut seam_uncertainty = 0.;
        if self.circuit {
            ensure(
                window.max_m - window.min_m < self.length_m(),
                "wheel window must span less than one circuit",
            )?;
            // Existing topology accepts millimetre seams. Do not turn a tolerated
            // discontinuity into a fictitious smooth wheel path across the seam.
            let first = &self.track.samples[0];
            let last = self.track.samples.last().unwrap();
            let gap = (first.position_m + first.frame.local_to_world(offset)
                - last.position_m
                - last.frame.local_to_world(offset))
            .length();
            let crosses = (window.min_m / self.length_m()).floor()
                != (window.max_m / self.length_m()).floor();
            ensure(
                !crosses || gap <= options.approximation_m,
                "wheel path has a discontinuous circuit seam",
            )?;
            if crosses {
                seam_uncertainty = gap;
            }
        } else {
            ensure(
                window.min_m >= 0. && window.max_m <= self.length_m(),
                "wheel window outside open/shuttle track",
            )?;
        }
        let mut segments = Vec::new();
        let mut start = window.min_m;
        let mut roundoff: f64 = 0.;
        while start < window.max_m {
            let canonical = if self.circuit {
                start.rem_euclid(self.length_m())
            } else {
                start
            };
            let shift = start - canonical;
            let index = self
                .track
                .samples
                .partition_point(|s| s.distance_m <= canonical)
                .saturating_sub(1)
                .min(self.track.samples.len() - 2);
            let piece = self.piece(index)?;
            let end = (self.track.samples[index + 1].distance_m + shift).min(window.max_m);
            ensure(end > start, "wheel station precision exhausted")?;
            let scale = piece
                .origin
                .length()
                .max(piece.t0.length() + piece.c2.length() + piece.c3.length() + offset.length())
                .max(point_m.length())
                .max(window.max_m.abs())
                .max(window.min_m.abs())
                .max(1.);
            let margin = 256. * f64::EPSILON * scale;
            roundoff = roundoff.max(margin);
            ensure(
                margin < options.approximation_m * 0.25,
                "insufficient coordinate/station precision for wheel search",
            )?;
            let second = piece.second_derivative_bound(offset);
            let count = ((second * (end - start).powi(2)
                / (8. * (options.approximation_m - margin)))
                .sqrt()
                .ceil() as usize)
                .max(1);
            ensure(
                second.is_finite() && count <= options.max_segments.saturating_sub(segments.len()),
                "wheel search segment budget exceeded",
            )?;
            let at = |s: f64| -> Result<Vec3> {
                let (p, _, f) = piece.evaluate(s - shift, offset)?;
                let p = p + f.local_to_world(offset);
                ensure(p.is_finite(), "wheel path overflow")?;
                Ok(p)
            };
            let mut a = at(start)?;
            for j in 0..count {
                let lo = start + (end - start) * (j as f64 / count as f64);
                let hi = if j + 1 == count {
                    end
                } else {
                    start + (end - start) * ((j + 1) as f64 / count as f64)
                };
                let b = at(hi)?;
                let d = b - a;
                let d2 = d.dot(d);
                ensure(d2.is_finite(), "wheel-center segment overflow")?;
                // A window can clip only a few station ulps from a sample.
                // Measure that roundoff-sized tail as a point, within the
                // existing margin; do not reject an otherwise straight locus.
                ensure(
                    d2 > 1e-24 || hi - lo <= margin,
                    "degenerate wheel-center path segment",
                )?;
                let u = if d2 > 1e-24 {
                    ((point_m - a).dot(d) / d2).clamp(0., 1.)
                } else {
                    0.
                };
                let distance = (point_m - (a + d * u)).length();
                ensure(distance.is_finite(), "wheel distance overflow")?;
                segments.push(SegmentCandidate {
                    station: lo + (hi - lo) * u,
                    interval: [lo, hi],
                    distance,
                });
                a = b;
            }
            start = end;
        }
        let best = segments
            .iter()
            .map(|s| s.distance)
            .min_by(f64::total_cmp)
            .unwrap();
        let distance_uncertainty_m = 2. * options.approximation_m + roundoff + seam_uncertainty;
        let threshold = best + distance_uncertainty_m + options.ambiguity_m;
        let mut groups: Vec<usize> = Vec::new();
        let mut previous = None;
        for (i, s) in segments.iter().enumerate() {
            if s.distance <= threshold {
                if previous != i.checked_sub(1) || groups.is_empty() {
                    groups.push(i);
                } else if s.distance < segments[*groups.last().unwrap()].distance {
                    *groups.last_mut().unwrap() = i;
                }
                previous = Some(i);
            }
        }
        let near_start =
            (point_m - self.sample(path, window.min_m)?.wheel_center_m).length() <= threshold;
        let near_end =
            (point_m - self.sample(path, window.max_m)?.wheel_center_m).length() <= threshold;
        let status = if groups.len() > 1 || (near_start && near_end) {
            SearchStatus::Ambiguous
        } else if near_start || near_end {
            SearchStatus::BoundaryMinimum
        } else {
            SearchStatus::Resolved
        };
        let candidates = groups
            .into_iter()
            .map(|i| {
                let s = &segments[i];
                let sample = self.sample(path, s.station)?;
                let delta = point_m - sample.wheel_center_m;
                let longitudinal_error_m = delta.dot(sample.tangent);
                let perpendicular = delta - sample.tangent * longitudinal_error_m;
                let lateral = sample.track_frame.lateral();
                let axes = (lateral - sample.tangent * lateral.dot(sample.tangent))
                    .normalized()
                    .ok()
                    .filter(|_| sample.incoming_tangent.is_none())
                    .map(|l| (l, l.cross(sample.tangent)));
                let error = if sample.incoming_tangent.is_some() {
                    delta.length()
                } else {
                    perpendicular.length()
                };
                Ok(PathCandidate {
                    sample,
                    station_interval_m: s.interval,
                    separation_m: delta.length(),
                    perpendicular_error_m: error,
                    longitudinal_error_m,
                    lateral_error_m: axes.map(|(l, _)| delta.dot(l)),
                    vertical_error_m: axes.map(|(_, n)| delta.dot(n)),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(NearestPath {
            status,
            candidates,
            segments_tested: segments.len(),
            distance_uncertainty_m,
        })
    }
}
