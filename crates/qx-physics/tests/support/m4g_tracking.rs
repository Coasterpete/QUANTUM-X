//! Speed-aware bounded continuation, driven by actual point velocities.
use super::{
    contract::Model,
    guidance::point,
    original_dynamics::{State, Step, initial},
    profiled_dynamics as refined_dynamics,
    surface::{Failure, Rail, Result},
};
use qx_geometry::math::Vec3;
use qx_physics::FIXED_STEP_SECONDS as DT;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Envelope {
    pub rate: f64,
    pub innovation: f64,
    pub half: f64,
}
pub struct Tracked {
    pub model: Model,
    pub state: State,
    pub ticks: usize,
    pub envelopes: Vec<Envelope>,
    pub substeps: usize,
    pub profile: refined_dynamics::Profile,
    pub tracking_time: std::time::Duration,
}
impl Tracked {
    pub fn new(mut model: Model, rail: &Rail, speed: f64) -> Result<Self> {
        if !speed.is_finite() {
            return Err(Failure::Invalid);
        }
        let (state, _) = initial(&model, rail, speed)?;
        for s in &mut model.supports {
            s.station = rail
                .project(
                    s.surface,
                    point(state.poses[s.owner], s.local),
                    s.station,
                    s.half_window,
                )?
                .0;
        }
        Ok(Self {
            model,
            state,
            ticks: 0,
            envelopes: vec![],
            substeps: 4,
            profile: Default::default(),
            tracking_time: std::time::Duration::ZERO,
        })
    }
    pub fn hints(&self) -> Vec<f64> {
        self.model.supports.iter().map(|s| s.station).collect()
    }
    pub fn step(&mut self, rail: &Rail, gravity: Vec3) -> Result<Step> {
        if !gravity.is_finite()
            || self.state.velocity.len() != 6 * self.state.poses.len()
            || self.state.velocity.iter().any(|v| !v.is_finite())
        {
            return Err(Failure::Invalid);
        }
        let tracking_clock = std::time::Instant::now();
        let mut envelopes = vec![];
        for s in &self.model.supports {
            let p = self.state.poses[s.owner];
            let x = point(p, s.local);
            let j = rail.jet(s.surface, s.station)?;
            let denom = j.d.dot(j.d) - (x - j.p).dot(j.dd);
            let v = &self.state.velocity[s.owner * 6..s.owner * 6 + 6];
            let u =
                Vec3::new(v[0], v[1], v[2]) + Vec3::new(v[3], v[4], v[5]).cross(x - p.position_m);
            if denom <= 1e-8 {
                return Err(Failure::Singular);
            }
            let rate = j.d.dot(u) / denom;
            let motion = rate.abs() * DT;
            // 25% speed variation margin + 2 mm numerical/force uncertainty.
            // |P''|/|P'| and |k| estimate chart turning locally. They are not
            // certified supremum bounds on an arbitrary unseen interval.
            let turn = j.dd.length() / j.d.length() + j.k.length();
            let innovation = motion * 1.25 + 0.002;
            let half = innovation + 0.01;
            if !half.is_finite() || half > 0.20 || turn * half > 0.15 {
                return Err(Failure::Inaccessible);
            }
            envelopes.push(Envelope {
                rate,
                innovation,
                half,
            });
        }
        // Candidate windows and state are private until ALL post-step gates pass.
        // Restoring windows on every exit also preserves late-failure rollback.
        let old_windows: Vec<_> = self.model.supports.iter().map(|s| s.half_window).collect();
        for (s, e) in self.model.supports.iter_mut().zip(&envelopes) {
            s.half_window = e.half;
        }
        let mut next = self.state.clone();
        let candidate = (|| {
            assert_eq!(self.substeps, 4);
            self.tracking_time = tracking_clock.elapsed();
            let (report, profile) =
                refined_dynamics::profiled(&self.model, rail, &mut next, gravity, true)?;
            self.profile = profile;
            let tracking_clock = std::time::Instant::now();
            let hints = self
                .model
                .supports
                .iter()
                .zip(&envelopes)
                .map(|(s, e)| {
                    let station = rail
                        .project(
                            s.surface,
                            point(next.poses[s.owner], s.local),
                            s.station,
                            e.half,
                        )?
                        .0;
                    if (station - s.station).abs() > e.innovation
                        || (station - s.station - e.rate * DT).abs()
                            > 0.25 * e.rate.abs() * DT + 0.002
                    {
                        return Err(Failure::Inaccessible);
                    }
                    Ok(station)
                })
                .collect::<Result<Vec<_>>>()?;
            self.tracking_time += tracking_clock.elapsed();
            Ok((report, hints))
        })();
        for (s, h) in self.model.supports.iter_mut().zip(old_windows) {
            s.half_window = h;
        }
        let (report, hints) = candidate?;
        for (s, h) in self.model.supports.iter_mut().zip(hints) {
            s.station = h;
        }
        self.state = next;
        self.ticks += 1;
        self.envelopes = envelopes;
        Ok(report)
    }
}
