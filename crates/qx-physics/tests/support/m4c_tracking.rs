//! Explicit local continuation state; a failed outer tick rolls back both the
//! rigid-body state and every station hint. Stations never prescribe motion.
use super::{
    contract::Model,
    dynamics,
    guidance::point,
    original_dynamics::{State, Step, initial},
    surface::{Failure, Rail, Result},
};
use qx_geometry::math::Vec3;

pub struct Tracked {
    pub model: Model,
    pub state: State,
    pub ticks: usize,
}
impl Tracked {
    pub fn new(mut model: Model, rail: &Rail, speed: f64) -> Result<Self> {
        let (state, _) = initial(&model, rail, speed)?;
        for support in &mut model.supports {
            let (s, _) = rail.project(
                support.surface,
                point(state.poses[support.owner], support.local),
                support.station,
                support.half_window,
            )?;
            support.station = s;
            // Explicit research continuation envelope: 8 cm search, 4 cm max
            // station innovation per 1/240 s tick. Reject larger seeks/speeds.
            support.half_window = 0.08;
        }
        Ok(Self {
            model,
            state,
            ticks: 0,
        })
    }
    pub fn hints(&self) -> Vec<f64> {
        self.model.supports.iter().map(|s| s.station).collect()
    }
    pub fn step(&mut self, rail: &Rail, gravity: Vec3) -> Result<Step> {
        let before = self.state.clone();
        let report = dynamics::step(&self.model, rail, &mut self.state, gravity)?;
        let candidate = self
            .model
            .supports
            .iter()
            .map(|support| {
                let (s, _) = rail.project(
                    support.surface,
                    point(self.state.poses[support.owner], support.local),
                    support.station,
                    support.half_window,
                )?;
                if (s - support.station).abs() > 0.04 {
                    return Err(Failure::Inaccessible);
                }
                Ok(s)
            })
            .collect::<Result<Vec<_>>>();
        let hints = match candidate {
            Ok(hints) => hints,
            Err(error) => {
                self.state = before;
                return Err(error);
            }
        };
        for (support, s) in self.model.supports.iter_mut().zip(hints) {
            support.station = s;
        }
        self.ticks += 1;
        Ok(report)
    }
}
