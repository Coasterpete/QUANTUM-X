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
    initial_attitudes: Vec<crate::TrackAttitude>,
    attitudes: Vec<crate::TrackAttitude>,
    inspection: crate::consist::ConsistInspection,
    previous: crate::consist::ConsistInspection,
    current: crate::consist::ConsistInspection,
    ticks: u64,
    operations: qx_document::operations::Operations,
    circuit: bool,
    telemetry: crate::devices::TrainTelemetry,
}
impl TrackSimulation {
    pub fn new(
        train: qx_document::train::Train,
        track: std::sync::Arc<qx_geometry::solver::GeneratedTrack>,
        station_m: f64,
        initial_speed_mps: f64,
    ) -> Result<Self> {
        let mut operations = qx_document::operations::Operations::default();
        operations.physical.initial_speed_mps = initial_speed_mps;
        Self::with_operations(train, track, station_m, operations)
    }
    pub fn with_operations(
        train: qx_document::train::Train,
        track: std::sync::Arc<qx_geometry::solver::GeneratedTrack>,
        station_m: f64,
        operations: qx_document::operations::Operations,
    ) -> Result<Self> {
        operations.validate().map_err(|e| e.to_string())?;
        operations
            .validate_track(&track)
            .map_err(|e| e.to_string())?;
        let circuit = operations.circuit(&track).map_err(|e| e.to_string())?;
        if operations.layout == qx_geometry::topology::LayoutMode::Circuit && !circuit {
            return Err(
                "Circuit is incomplete: inspect endpoint position, tangent and frame diagnostics"
                    .into(),
            );
        }
        let initial_speed_mps = operations.physical.initial_speed_mps;
        if !initial_speed_mps.is_finite() || initial_speed_mps.abs() > 100. {
            return Err("initial speed must be finite and within +/-100 m/s".into());
        }
        let (mut world, guides, attitudes, inspection) = crate::consist::initialize(
            &train,
            &track,
            station_m,
            qx_geometry::math::Vec3::new(0., -operations.physical.gravity_mps2, 0.),
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
            initial_attitudes: attitudes.clone(),
            attitudes,
            previous: inspection.clone(),
            current: inspection.clone(),
            inspection,
            ticks: 0,
            operations,
            circuit,
            telemetry: crate::devices::TrainTelemetry {
                speed_mps: initial_speed_mps,
                ..Default::default()
            },
        })
    }
    pub fn world(&self) -> &World {
        &self.world
    }
    pub fn telemetry(&self) -> &crate::devices::TrainTelemetry {
        &self.telemetry
    }
    pub fn guides(&self) -> &[crate::TrackGuide] {
        &self.guides
    }
    pub fn ticks(&self) -> u64 {
        self.ticks
    }
    pub fn snapshot(&self) -> Result<crate::consist::ConsistInspection> {
        Ok(self.current.clone())
    }
    pub fn previous_snapshot(&self) -> &crate::consist::ConsistInspection {
        &self.previous
    }
    /// One transactional 1/240 s step; on error bodies, guide hints and time are retained.
    pub fn step(&mut self) -> Result<Diagnostics> {
        let next = self
            .ticks
            .checked_add(1)
            .ok_or("simulation tick counter overflow")?;
        let mut world = self.world.clone();
        let mut guides = self.guides.clone();
        let mut attitudes = self.attitudes.clone();
        let stations: Vec<_> = self
            .current
            .components
            .iter()
            .map(|p| p.station_m)
            .collect();
        let before = crate::devices::velocity(&world);
        let (devices, resistance_force_n) = crate::devices::forces(
            &mut world,
            &self.track,
            &stations,
            &self.operations,
            self.circuit,
        )?;
        let result =
            world.step_on_track_topology(&self.track, &mut guides, &mut attitudes, self.circuit)?;
        let mut current = crate::consist::snapshot(&self.train, &world, &guides, &self.current)?;
        // Non-guided masses also need current stations for device occupancy.
        for (i, p) in current.components.iter_mut().enumerate() {
            if !guides.iter().any(|g| g.body == i) {
                p.station_m = crate::solver::project_station_topology(
                    &self.track,
                    world.bodies[i].pose.position_m,
                    self.current.components[i].station_m,
                    self.circuit,
                )?;
            }
        }
        let after = crate::devices::velocity(&world);
        let mass: f64 = world.bodies.iter().map(|b| b.mass.mass_kg).sum();
        let mut speed_mps = 0.;
        for (b, p) in world.bodies.iter().zip(&current.components) {
            let tangent = self
                .track
                .sample_at_distance(p.station_m)
                .map_err(|e| e.to_string())?
                .frame
                .tangent();
            speed_mps += b.linear_velocity_mps.dot(tangent) * b.mass.mass_kg / mass;
        }
        let acceleration_mps2 = (after - before) / crate::FIXED_STEP_SECONDS;
        let local_specific_force_mps2 = world.bodies.iter().zip(&self.world.bodies).fold(
            qx_geometry::math::Vec3::ZERO,
            |v, (b, old)| {
                let specific = (b.linear_velocity_mps - old.linear_velocity_mps)
                    / crate::FIXED_STEP_SECONDS
                    - world.gravity_mps2;
                v + b.frame().world_to_local(specific) * (b.mass.mass_kg / mass)
            },
        );
        let telemetry = crate::devices::TrainTelemetry {
            elapsed_s: next as f64 * crate::FIXED_STEP_SECONDS,
            speed_mps,
            distance_traveled_m: self.telemetry.distance_traveled_m
                + (crate::devices::mean_speed(&self.world) + crate::devices::mean_speed(&world))
                    * 0.5
                    * crate::FIXED_STEP_SECONDS,
            acceleration_mps2,
            specific_force_mps2: acceleration_mps2 - world.gravity_mps2,
            local_specific_force_mps2,
            longitudinal_acceleration_mps2: (speed_mps - self.telemetry.speed_mps)
                / crate::FIXED_STEP_SECONDS,
            applied_device_force_n: devices.iter().map(|d| d.applied_force_n).sum(),
            resistance_force_n,
            devices,
        };
        self.previous = std::mem::replace(&mut self.current, current);
        self.world = world;
        self.guides = guides;
        self.attitudes = attitudes;
        self.ticks = next;
        self.telemetry = telemetry;
        Ok(result)
    }
    pub fn reset(&mut self) {
        self.world = self.initial.clone();
        self.guides = self.initial_guides.clone();
        self.attitudes = self.initial_attitudes.clone();
        self.ticks = 0;
        self.previous = self.inspection.clone();
        self.current = self.inspection.clone();
        self.telemetry = crate::devices::TrainTelemetry {
            speed_mps: self.operations.physical.initial_speed_mps,
            ..Default::default()
        };
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
