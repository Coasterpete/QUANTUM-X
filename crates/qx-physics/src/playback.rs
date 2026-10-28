use crate::{Diagnostics, Result, World};

/// Independent body dynamics on a fixed generated track. Neither the definition
/// nor the track is changed by stepping, pausing or resetting this transient state.
#[derive(Clone, Debug)]
pub struct TrackSimulation {
    train: qx_document::train::Train,
    track: std::sync::Arc<qx_geometry::solver::GeneratedTrack>,
    initial: World,
    world: World,
    initial_guides: Vec<crate::TrackGuide>,
    guides: Vec<crate::TrackGuide>,
    inspection: crate::consist::ConsistInspection,
    ticks: u64,
}
impl TrackSimulation {
    pub fn new(
        train: qx_document::train::Train,
        track: std::sync::Arc<qx_geometry::solver::GeneratedTrack>,
        station_m: f64,
        initial_speed_mps: f64,
    ) -> Result<Self> {
        if !initial_speed_mps.is_finite() || initial_speed_mps.abs() > 100. {
            return Err("initial speed must be finite and within +/-100 m/s".into());
        }
        let (mut world, guides, inspection) = crate::consist::initialize(
            &train,
            &track,
            station_m,
            qx_geometry::math::Vec3::new(0., -9.81, 0.),
            32,
            4,
        )?;
        if guides.is_empty() {
            return Err("playback requires at least one explicit track guide".into());
        }
        for (b, p) in world.bodies.iter_mut().zip(&inspection.components) {
            b.linear_velocity_mps = track
                .sample_at_distance(p.station_m)
                .map_err(|e| e.to_string())?
                .frame
                .tangent()
                * initial_speed_mps;
        }
        Ok(Self {
            train,
            track,
            initial: world.clone(),
            world,
            initial_guides: guides.clone(),
            guides,
            inspection,
            ticks: 0,
        })
    }
    pub fn world(&self) -> &World {
        &self.world
    }
    pub fn guides(&self) -> &[crate::TrackGuide] {
        &self.guides
    }
    pub fn ticks(&self) -> u64 {
        self.ticks
    }
    pub fn snapshot(&self) -> Result<crate::consist::ConsistInspection> {
        crate::consist::snapshot(&self.train, &self.world, &self.guides, &self.inspection)
    }
    /// One transactional 1/240 s step; on error bodies, guide hints and time are retained.
    pub fn step(&mut self) -> Result<Diagnostics> {
        let next = self
            .ticks
            .checked_add(1)
            .ok_or("simulation tick counter overflow")?;
        let result = self.world.step_on_track(&self.track, &mut self.guides)?;
        self.ticks = next;
        Ok(result)
    }
    pub fn reset(&mut self) {
        self.world = self.initial.clone();
        self.guides = self.initial_guides.clone();
        self.ticks = 0;
    }
}

/// Fixed-step playback state. No render-frame clock or implicit catch-up policy.
#[derive(Clone, Debug)]
pub struct Simulation {
    initial: World,
    world: World,
    ticks: u64,
    paused: bool,
}
impl Simulation {
    pub fn new(world: World) -> Result<Self> {
        world.validate()?;
        Ok(Self {
            initial: world.clone(),
            world,
            ticks: 0,
            paused: true,
        })
    }
    pub fn world(&self) -> &World {
        &self.world
    }
    pub fn ticks(&self) -> u64 {
        self.ticks
    }
    pub fn paused(&self) -> bool {
        self.paused
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
    /// Single-step works while paused; unsuccessful steps do not advance time.
    pub fn step(&mut self) -> Result<Diagnostics> {
        let ticks = self
            .ticks
            .checked_add(1)
            .ok_or("simulation tick counter overflow")?;
        let result = self.world.step()?;
        self.ticks = ticks;
        Ok(result)
    }
    /// Advance at most one fixed step when running; caller controls wall-clock scheduling.
    pub fn tick(&mut self) -> Result<Option<Diagnostics>> {
        if self.paused {
            Ok(None)
        } else {
            self.step().map(Some)
        }
    }
    pub fn reset(&mut self) {
        self.world = self.initial.clone();
        self.ticks = 0;
        self.paused = true;
    }
}
