//! Opt-in geometric initialization research for ONE leaf, free limited hinge.
//! No automatic assembly/playback hooks, contact law, velocities or persistence.
mod mechanics;
use crate::{
    Result,
    assembly_diagnostics::{AssemblyDiagnostics, AssemblyResiduals},
    consist::{ConsistInspection, inspect_with_diagnostics},
    wheel_geometry::{self, TargetObservation, WheelAlignmentReport, WheelGeometry},
};
use mechanics::Coordinate;
use qx_document::train::Train;
use qx_geometry::{
    math::Vec3,
    rail_geometry::{PhysicalRailGeometry, SearchOptions, SearchStatus},
    solver::GeneratedTrack,
    topology::LayoutMode,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FitOptions {
    pub max_iterations: u32,
    /// Includes the baseline report and all attempted candidate reports.
    pub max_evaluations: u32,
    pub max_step_rad: f64,
    pub max_travel_rad: f64,
    /// Each wheel's movement from its ORIGINAL resolved station, metres.
    pub max_station_travel_m: f64,
}
impl Default for FitOptions {
    fn default() -> Self {
        Self {
            max_iterations: 24,
            max_evaluations: 32,
            max_step_rad: 0.05,
            max_travel_rad: 0.35,
            max_station_travel_m: 0.25,
        }
    }
}
impl FitOptions {
    fn validate(self) -> Result<()> {
        if !(1..=64).contains(&self.max_iterations)
            || !(1..=96).contains(&self.max_evaluations)
            || !self.max_step_rad.is_finite()
            || !(1e-6..=0.1).contains(&self.max_step_rad)
            || !self.max_travel_rad.is_finite()
            || !(self.max_step_rad..=0.5).contains(&self.max_travel_rad)
            || !self.max_station_travel_m.is_finite()
            || !(1e-6..=1.).contains(&self.max_station_travel_m)
        {
            return Err("invalid bounded bogie-fit options".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitTermination {
    Stationary,
    NoAdmissibleStep,
    AngularBound,
    RankDeficient,
    IterationLimit,
    EvaluationLimit,
    InvalidTarget,
    UnsupportedGraph,
    NumericalFailure,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FitEvaluation {
    pub angle_rad: f64,
    pub wheels: WheelAlignmentReport,
    pub constraints: AssemblyResiduals,
    /// A rejection is retained even when the least-squares objective decreased.
    pub rejection: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FitReport {
    /// False means `pose` is the original successful M2B assembly, exactly.
    pub accepted: bool,
    pub termination: FitTermination,
    pub detail: String,
    pub iterations: u32,
    /// Baseline plus candidate wheel reports, including unsupported graphs.
    pub evaluation_count: u32,
    /// Includes baseline when a supported angular coordinate can be assigned.
    pub evaluations: Vec<FitEvaluation>,
    pub baseline: WheelAlignmentReport,
    pub final_alignment: WheelAlignmentReport,
    pub final_constraints: AssemblyResiduals,
    pub pose: ConsistInspection,
}

/// Immutable M2B seed and validated PHYSICAL track, created outside fit timers.
/// Private fields prevent callers supplying fabricated/stale assembly evidence.
pub struct FitSeed<'a> {
    train: &'a Train,
    track: &'a GeneratedTrack,
    geometry: PhysicalRailGeometry<'a>,
    original: ConsistInspection,
    diagnostics: AssemblyDiagnostics,
}
impl<'a> FitSeed<'a> {
    /// Open-track assembly contract; no inference of Circuit intent or wrapping.
    pub fn assemble(
        train: &'a Train,
        physical_track: &'a GeneratedTrack,
        station_m: f64,
    ) -> Result<Self> {
        let geometry = PhysicalRailGeometry::new(physical_track, LayoutMode::Open)
            .map_err(|e| e.to_string())?;
        let mut diagnostics = AssemblyDiagnostics::default();
        let original =
            inspect_with_diagnostics(train, physical_track, station_m, &mut diagnostics)?;
        if diagnostics.measurement_error.is_some() || diagnostics.terminal_residuals.is_none() {
            return Err("bogie fitting needs successful measured M2B assembly".into());
        }
        Ok(Self {
            train,
            track: physical_track,
            geometry,
            original,
            diagnostics,
        })
    }
    pub fn original(&self) -> &ConsistInspection {
        &self.original
    }
    pub fn assembly(&self) -> &AssemblyDiagnostics {
        &self.diagnostics
    }

    /// Fit the B component of a named connection; parent and every other body
    /// stay fixed. Does not install the returned pose in a simulation world.
    pub fn fit(
        &self,
        connection_id: &str,
        wheels: &[WheelGeometry],
        options: FitOptions,
    ) -> Result<FitReport> {
        options.validate()?;
        // Bounding the whole snapshot as well as iteration count prevents a
        // diagnostic's quadratic attachment work becoming an optimizer hot path.
        if self.train.components.len() > 128
            || self.train.connections.len() > 256
            || wheels.len() > 256
            || self
                .train
                .components
                .iter()
                .any(|c| c.guide_references_m.len() + c.wheel_references_m.len() > 32)
        {
            return Err("bogie-fit graph/reference budget exceeded".into());
        }
        let baseline = wheel_geometry::diagnose(
            self.train,
            &self.original,
            &self.geometry,
            wheels,
            SearchOptions::default(),
        )?;
        let constraints = self
            .diagnostics
            .terminal_residuals
            .clone()
            .ok_or("missing assembly evidence")?;
        let mut report = FitReport {
            accepted: false,
            termination: FitTermination::UnsupportedGraph,
            detail: String::new(),
            iterations: 0,
            evaluation_count: 1,
            evaluations: vec![],
            baseline: baseline.clone(),
            final_alignment: baseline,
            final_constraints: constraints,
            pose: self.original.clone(),
        };
        let coordinate = match Coordinate::new(self.train, &self.original, connection_id) {
            Ok(c) => c,
            Err(e) => {
                report.detail = e;
                return Ok(report);
            }
        };
        report.evaluations.push(FitEvaluation {
            angle_rad: coordinate.initial_angle,
            wheels: report.baseline.clone(),
            constraints: report.final_constraints.clone(),
            rejection: None,
        });
        if !resolved(&report.baseline) {
            report.termination = FitTermination::InvalidTarget;
            report.detail = "every wheel needs one smooth, interior, unambiguous target".into();
            return Ok(report);
        }
        if !wheels
            .iter()
            .any(|w| w.component_id == coordinate.component_id)
        {
            report.detail = "selected carrier has no declared wheels".into();
            return Ok(report);
        }
        if let Some(reason) = mechanics::invariant_guides(self, &coordinate)? {
            report.detail = reason;
            return Ok(report);
        }
        let mut current = report.baseline.clone();
        let mut theta = coordinate.initial_angle;
        let mut damping = 1e-6; // dimensionless multiplier of scalar J^T J
        report.termination = FitTermination::IterationLimit;
        for iteration in 0..options.max_iterations {
            if report.evaluation_count >= options.max_evaluations {
                report.termination = FitTermination::EvaluationLimit;
                break;
            }
            report.iterations = iteration + 1;
            let mut gradient = 0.;
            let mut curvature = 0.;
            for row in &current.wheels {
                if row.component_id == coordinate.component_id {
                    let TargetObservation::Measured(n) = &row.target else {
                        unreachable!()
                    };
                    let target = &n.candidates[0];
                    let (r, j) = frozen_target_linearization(
                        row.center_world_m,
                        target.sample.wheel_center_m,
                        target.sample.tangent,
                        coordinate.pivot,
                        coordinate.axis,
                    );
                    gradient += j.dot(r);
                    curvature += j.dot(j);
                }
            }
            if !gradient.is_finite() || !curvature.is_finite() {
                report.termination = FitTermination::NumericalFailure;
                break;
            }
            // One scalar coordinate: zero column rank is detected BEFORE damping.
            // A regularization term must never disguise missing observability.
            if curvature <= 1e-16 {
                report.termination = FitTermination::RankDeficient;
                break;
            }
            let delta = (-gradient / (curvature * (1. + damping)))
                .clamp(-options.max_step_rad, options.max_step_rad);
            let lower = coordinate
                .min_angle
                .max(coordinate.initial_angle - options.max_travel_rad);
            let upper = coordinate
                .max_angle
                .min(coordinate.initial_angle + options.max_travel_rad);
            let next = (theta + delta).clamp(lower, upper);
            if (next - theta).abs() < 1e-10 {
                report.termination = if next == lower || next == upper {
                    FitTermination::AngularBound
                } else if (gradient / curvature).abs() >= 1e-10 {
                    FitTermination::NoAdmissibleStep
                } else {
                    FitTermination::Stationary
                };
                break;
            }
            let mut candidate = coordinate.pose(&self.original, next)?;
            let aligned = wheel_geometry::diagnose(
                self.train,
                &candidate,
                &self.geometry,
                wheels,
                SearchOptions::default(),
            )?;
            report.evaluation_count += 1;
            let mut measured = mechanics::measure(self, &candidate)?;
            mechanics::refresh(&mut candidate, &mut measured, &aligned, self.train)?;
            let mut rejection = mechanics::accept_constraints(&measured, &aligned);
            if rejection.is_none() {
                rejection =
                    wheel_acceptance(&report.baseline, &aligned, options.max_station_travel_m);
            }
            if rejection.is_none() && aligned.rms_error_m >= current.rms_error_m {
                rejection = Some("least-squares objective did not decrease".into());
            }
            report.evaluations.push(FitEvaluation {
                angle_rad: next,
                wheels: aligned.clone(),
                constraints: measured.clone(),
                rejection: rejection.clone(),
            });
            if rejection.is_some() {
                // Bounded LM retry. No rejected pose replaces the current pose.
                damping = (damping * 10.).max(1.);
                if damping > 1e12 {
                    report.termination = FitTermination::NoAdmissibleStep;
                    break;
                }
                continue;
            }
            theta = next;
            current = aligned;
            damping = (damping * 0.1).max(1e-9);
            // Improvement must exceed nearest-path distance uncertainty by a
            // useful margin, in BOTH RMS and maximum; every wheel was checked.
            if report.baseline.rms_error_m.unwrap() - current.rms_error_m.unwrap() > 1e-5
                && report.baseline.max_error_m.unwrap() - current.max_error_m.unwrap() > 1e-5
            {
                report.accepted = true;
                report.pose = candidate;
                report.final_alignment = current.clone();
                report.final_constraints = measured;
            }
        }
        report.detail = if report.accepted {
            "bounded improvement; residuals are geometric alignment, not contact".into()
        } else {
            "no candidate passed every improvement and mechanical gate; original pose retained"
                .into()
        };
        Ok(report)
    }
}

fn resolved(report: &WheelAlignmentReport) -> bool {
    report.rms_error_m.is_some()
        && report.wheels.iter().all(|w| {
            matches!(&w.target, TargetObservation::Measured(n) if n.status == SearchStatus::Resolved
            && n.candidates.len() == 1 && n.candidates[0].sample.incoming_tangent.is_none())
        })
}
fn wheel_acceptance(
    before: &WheelAlignmentReport,
    after: &WheelAlignmentReport,
    travel: f64,
) -> Option<String> {
    if !resolved(after) {
        return Some("unavailable, ambiguous, boundary or nonsmooth wheel target".into());
    }
    for (a, b) in before.wheels.iter().zip(&after.wheels) {
        let (TargetObservation::Measured(a), TargetObservation::Measured(b)) =
            (&a.target, &b.target)
        else {
            unreachable!()
        };
        if b.candidates[0].perpendicular_error_m > a.candidates[0].perpendicular_error_m + 1e-10 {
            return Some("an individual wheel worsened (roundoff allowance 1e-10 m)".into());
        }
        if (b.candidates[0].sample.station_m - a.candidates[0].sample.station_m).abs() > travel {
            return Some("wheel target exceeded its original station-travel bound".into());
        }
    }
    None
}

/// Analytic rigid-point Jacobian for a frozen target and tangent. This is the
/// scalar Gauss-Newton model, NOT the exact derivative of a curved nearest-path
/// projector away from zero residual. Targets are independently re-queried for
/// every candidate; nonlinear acceptance does not trust this linearization.
pub fn frozen_target_linearization(
    point: Vec3,
    target: Vec3,
    tangent: Vec3,
    pivot: Vec3,
    unit_axis: Vec3,
) -> (Vec3, Vec3) {
    let d = point - target;
    let v = unit_axis.cross(point - pivot);
    (d - tangent * d.dot(tangent), v - tangent * v.dot(tangent))
}
