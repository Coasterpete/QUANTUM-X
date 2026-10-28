use crate::{Diagnostics, Result, World};

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
