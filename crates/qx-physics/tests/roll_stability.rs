//! Conventional-train roll reference regressions. These check the mechanical
//! distinction between body-to-gear roll (the former free pendulum axis),
//! gear-to-track roll (authored banking/inversion), and body-to-track roll
//! (the resulting car orientation). Pitch/yaw articulation on hills must stay.
use qx_document::train::{DevelopmentArrangement, Train};
use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    math::{Frame, Quaternion, Vec3},
    solver::{GeneratedTrack, GenerationOptions, generate},
    transition::{Channel, Function, Transition, Unit},
};
use qx_physics::{
    Diagnostics, TrackAttitude, TrackGuide, World,
    consist::{ConsistInspection, initialize},
};
use std::sync::Arc;

fn conventional(n: usize) -> Train {
    Train::development_consist(
        "roll-reference".into(),
        n,
        DevelopmentArrangement::Conventional,
    )
    .unwrap()
}

fn straight(length_m: f64) -> Arc<GeneratedTrack> {
    Arc::new(
        generate(
            &InitialConditions::default(),
            &[Section::straight(1, "straight", length_m)],
            GenerationOptions::default(),
        )
        .unwrap(),
    )
}

/// `plane_angle_rad` 0 is a vertical curve (hill/valley/loop); pi/2 is a
/// horizontal turn. `bank` is the authored initial roll about the tangent.
fn arc(radius_m: f64, bank: f64, plane_angle_rad: f64, length_m: f64) -> Arc<GeneratedTrack> {
    let mut section = Section::straight(1, "arc", length_m);
    section.kind = SectionKind::Arc {
        radius_m,
        plane_angle_rad,
    };
    Arc::new(
        generate(
            &InitialConditions {
                orientation: Quaternion::axis_angle(Vec3::X, bank).unwrap(),
                ..Default::default()
            },
            &[section],
            GenerationOptions::default(),
        )
        .unwrap(),
    )
}

/// Smoothly varying pitch/yaw curvature and banking, so separated bogies and
/// cars genuinely see different local rail frames.
fn twisting_banked_turn() -> Arc<GeneratedTrack> {
    let length = 160.;
    let mut section = Section::straight(1, "twisting bank", length);
    let varying = |start, end| Channel {
        unit: Unit::RadiansPerMetre,
        transitions: vec![Transition {
            id: 1,
            length,
            start,
            end,
            amplitude: 0.,
            function: Function::Linear {},
        }],
    };
    section.kind = SectionKind::RiderLocal {
        pitch_rate: varying(-0.01, 0.014),
        yaw_rate: varying(0.008, 0.022),
    };
    section.roll_rate = Channel {
        unit: Unit::RadiansPerMetre,
        transitions: vec![Transition {
            id: 1,
            length,
            start: 0.004,
            end: 0.012,
            amplitude: 0.,
            function: Function::Linear {},
        }],
    };
    Arc::new(
        generate(
            &InitialConditions {
                orientation: Quaternion::axis_angle(Vec3::X, 0.25).unwrap(),
                ..Default::default()
            },
            &[section],
            GenerationOptions::default(),
        )
        .unwrap(),
    )
}

fn frame(pose: qx_physics::Pose) -> Frame {
    Frame::new(pose.orientation).expect("unit pose")
}

/// Independent swing/twist measurement, distinct from total relative rotation.
fn roll_about(reference: Frame, other: Frame) -> f64 {
    let q = reference
        .orientation()
        .conjugate()
        .compose(other.orientation());
    (2. * q.x.atan2(q.w) + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI
}

fn body_to_gear_roll(body: &qx_physics::Body, gear: &qx_physics::Body) -> f64 {
    roll_about(frame(body.pose), frame(gear.pose))
}

/// Remaining relative rotation other than the constrained roll: bogie pitch and
/// yaw articulation that must survive a roll-only joint.
fn body_to_gear_pitch_yaw(body: &qx_physics::Body, gear: &qx_physics::Body) -> f64 {
    let relative = body
        .pose
        .orientation
        .conjugate()
        .compose(gear.pose.orientation)
        .rotation_vector()
        .unwrap();
    relative.y.hypot(relative.z)
}

fn pose_to_track_roll(track: &GeneratedTrack, pose: qx_physics::Pose, station_m: f64) -> f64 {
    let sample = track.sample_at_distance(station_m).unwrap();
    roll_about(sample.frame, frame(pose))
}

fn energy(world: &World) -> f64 {
    let g = -world.gravity_mps2.y;
    world
        .bodies
        .iter()
        .map(|b| {
            let i = b.mass.principal_inertia_kg_m2;
            let w = frame(b.pose).world_to_local(b.angular_velocity_radps);
            let rotational = 0.5 * (i.x * w.x * w.x + i.y * w.y * w.y + i.z * w.z * w.z);
            let kinetic = 0.5 * b.mass.mass_kg * b.linear_velocity_mps.dot(b.linear_velocity_mps);
            kinetic + rotational + b.mass.mass_kg * g * b.pose.position_m.y
        })
        .sum()
}

struct Rig {
    train: Train,
    track: Arc<GeneratedTrack>,
    world: World,
    guides: Vec<TrackGuide>,
    attitudes: Vec<TrackAttitude>,
    inspection: ConsistInspection,
}

impl Rig {
    fn new(train: Train, track: Arc<GeneratedTrack>, station_m: f64, speed_mps: f64) -> Self {
        let (mut world, guides, attitudes, inspection) =
            initialize(&train, &track, station_m, Vec3::new(0., -9.81, 0.), 32, 4).unwrap();
        for (body, component) in world.bodies.iter_mut().zip(&inspection.components) {
            body.linear_velocity_mps = track
                .sample_at_distance(component.station_m)
                .unwrap()
                .frame
                .tangent()
                * speed_mps;
        }
        Self {
            train,
            track,
            world,
            guides,
            attitudes,
            inspection,
        }
    }
    fn step(&mut self) -> Diagnostics {
        self.world
            .step_on_track_topology(&self.track, &mut self.guides, &mut self.attitudes, false)
            .unwrap()
    }
    fn body_index(&self, id: &str) -> usize {
        self.train
            .components
            .iter()
            .position(|c| c.id == id)
            .unwrap()
    }
    fn gear_indices(&self, car: usize) -> Vec<usize> {
        self.train.cars[car]
            .bogie_ids
            .iter()
            .map(|id| self.body_index(id))
            .collect()
    }
    fn body_station(&self, body: usize) -> f64 {
        let car = self
            .train
            .cars
            .iter()
            .position(|c| self.body_index(&c.body_id) == body)
            .unwrap();
        let gears = self.gear_indices(car);
        let mut s = gears.iter().map(|i| self.gear_station(*i)).sum::<f64>() / gears.len() as f64;
        // Project the measured COM independently, starting on this car's branch.
        for _ in 0..24 {
            let sample = self.track.sample_at_distance(s).unwrap();
            let ds = (self.world.bodies[body].pose.position_m - sample.position_m)
                .dot(sample.frame.tangent());
            s = (s + ds).clamp(0., self.track.length_m());
            if ds.abs() < 1e-10 {
                return s;
            }
        }
        panic!("measurement projection did not converge");
    }
    fn gear_station(&self, gear: usize) -> f64 {
        if let Some(a) = self.attitudes.iter().find(|a| a.body == gear) {
            return a.station_m;
        }
        self.guides
            .iter()
            .find(|g| g.body == gear)
            .map_or(self.inspection.components[gear].station_m, |g| g.station_m)
    }
    fn max_body_to_track_roll(&self) -> f64 {
        let mut worst: f64 = 0.;
        for car in &self.train.cars {
            let body = self.body_index(&car.body_id);
            worst = worst.max(
                pose_to_track_roll(
                    &self.track,
                    self.world.bodies[body].pose,
                    self.body_station(body),
                )
                .abs(),
            );
        }
        worst
    }
    fn max_gear_to_track_roll(&self) -> f64 {
        let mut worst: f64 = 0.;
        for car in 0..self.train.cars.len() {
            for gear in self.gear_indices(car) {
                worst = worst.max(
                    pose_to_track_roll(
                        &self.track,
                        self.world.bodies[gear].pose,
                        self.gear_station(gear),
                    )
                    .abs(),
                );
            }
        }
        worst
    }
    fn max_body_to_gear_roll(&self) -> f64 {
        let mut worst: f64 = 0.;
        for car in 0..self.train.cars.len() {
            let body = self.body_index(&self.train.cars[car].body_id);
            for gear in self.gear_indices(car) {
                worst = worst.max(
                    body_to_gear_roll(&self.world.bodies[body], &self.world.bodies[gear]).abs(),
                );
            }
        }
        worst
    }
    fn max_body_to_gear_pitch_yaw(&self) -> f64 {
        let mut worst: f64 = 0.;
        for car in 0..self.train.cars.len() {
            let body = self.body_index(&self.train.cars[car].body_id);
            for gear in self.gear_indices(car) {
                worst = worst.max(body_to_gear_pitch_yaw(
                    &self.world.bodies[body],
                    &self.world.bodies[gear],
                ));
            }
        }
        worst
    }
    fn max_angular_speed(&self) -> f64 {
        self.world
            .bodies
            .iter()
            .map(|b| b.angular_velocity_radps.length())
            .fold(0., f64::max)
    }
}

#[derive(Default)]
struct Peaks {
    body_gear: f64,
    gear_track: f64,
    body_track: f64,
    gear_track_total: f64,
    articulation: f64,
    omega: f64,
    joint: f64,
    guide: f64,
    angular: f64,
    attitude: f64,
}
impl Peaks {
    fn observe(&mut self, r: &Rig, d: &Diagnostics) {
        self.body_gear = self.body_gear.max(r.max_body_to_gear_roll());
        self.gear_track = self.gear_track.max(r.max_gear_to_track_roll());
        self.body_track = self.body_track.max(r.max_body_to_track_roll());
        self.articulation = self.articulation.max(r.max_body_to_gear_pitch_yaw());
        self.omega = self.omega.max(r.max_angular_speed());
        self.joint = self.joint.max(d.max_joint_error_m);
        self.guide = self.guide.max(d.max_guide_error_m);
        self.angular = self.angular.max(d.max_angular_error_rad);
        self.attitude = self.attitude.max(d.max_attitude_error_rad);
        for a in &r.attitudes {
            let target = r.track.sample_at_distance(a.station_m).unwrap().frame;
            let q = target
                .orientation()
                .conjugate()
                .compose(r.world.bodies[a.body].pose.orientation);
            self.gear_track_total = self
                .gear_track_total
                .max(q.rotation_vector().unwrap().length());
        }
        assert!(
            d.angular_torque_on_b_nm
                .iter()
                .chain(&d.attitude_torque_nm)
                .all(|t| t.is_finite())
        );
    }
    fn report(&self, name: &str) {
        println!(
            "{name}: peak roll body/gear={:.6} gear/track={:.6} body/track={:.6} deg; gear/track total={:.6} articulation={:.6} deg; joint={:.3e} guide={:.3e} m; angular={:.3e} rail={:.3e} rad; omega={:.6} rad/s",
            self.body_gear.to_degrees(),
            self.gear_track.to_degrees(),
            self.body_track.to_degrees(),
            self.gear_track_total.to_degrees(),
            self.articulation.to_degrees(),
            self.joint,
            self.guide,
            self.angular,
            self.attitude,
            self.omega
        );
    }
    fn assert_travel(&self) {
        // Body/gear includes the rear spherical bearing's legitimate rail twist.
        // Body/track also includes front-bearing bank versus the car COM station.
        assert!(self.body_gear < 0.02 && self.body_track < 0.02);
        assert!(self.gear_track < 1e-3 && self.gear_track_total < 0.015);
        assert!(self.angular < 1e-3 && self.attitude < 1e-3);
        assert!(self.joint < 0.002 && self.guide < 0.002);
        assert!(self.omega.is_finite() && self.omega < 10.);
    }
}
fn run(rig: &mut Rig, ticks: usize) -> Peaks {
    let mut peaks = Peaks::default();
    for _ in 0..ticks {
        let d = rig.step();
        peaks.observe(rig, &d);
    }
    peaks
}
fn perturb(rig: &mut Rig, angle: f64, speed: f64) {
    let i = rig.body_index("car-1");
    let body = &mut rig.world.bodies[i];
    let axis = frame(body.pose).tangent();
    let y = rig
        .train
        .connections
        .iter()
        .find(|j| j.component_a == "car-1")
        .unwrap()
        .anchor_a_m
        .y;
    let pivot = body.world_point(Vec3::new(0., y, 0.)).unwrap();
    let q = Quaternion::axis_angle(axis, angle).unwrap();
    body.pose.position_m = pivot
        + Frame::new(q)
            .unwrap()
            .local_to_world(body.pose.position_m - pivot);
    body.pose.orientation = q.compose(body.pose.orientation).normalized().unwrap();
    body.angular_velocity_radps = axis * speed;
    body.linear_velocity_mps = body
        .angular_velocity_radps
        .cross(body.pose.position_m - pivot);
}

#[test]
fn stationary_upright_train_holds_its_attitude_without_pendulum_motion() {
    let mut rig = Rig::new(conventional(2), straight(200.), 100., 0.);
    let start = energy(&rig.world);
    let positions: Vec<_> = rig.world.bodies.iter().map(|b| b.pose.position_m).collect();
    let peaks = run(&mut rig, 480);
    peaks.report("stationary 2 s");
    assert!(peaks.body_gear < 1e-8 && peaks.gear_track < 1e-8 && peaks.body_track < 1e-8);
    assert!(peaks.angular < 1e-8 && peaks.attitude < 1e-8 && peaks.omega < 1e-6);
    for (b, p) in rig.world.bodies.iter().zip(positions) {
        assert!((b.pose.position_m - p).length() < 1e-6);
    }
    let drift = (energy(&rig.world) - start).abs() / start.abs().max(1.);
    assert!(drift < 1e-7, "stationary energy drift {drift:e}");
}

#[test]
fn perturbed_car_resists_roll_and_keeps_articulation_free() {
    let mut rig = Rig::new(conventional(1), straight(200.), 100., 0.);
    perturb(&mut rig, 0.28, 1.5);
    assert!(rig.max_body_to_track_roll() > 0.27);
    let initial = run(&mut rig, 120);
    initial.report("16-degree perturbation first 0.5 s");
    let settled = run(&mut rig, 120);
    settled.report("16-degree perturbation settled 0.5-1 s");
    assert!(initial.body_track < 0.28 && initial.omega < 60.);
    assert!(settled.body_track < 1e-6 && settled.body_gear < 1e-6 && settled.omega < 1e-5);
}

#[test]
fn former_ball_suspension_reproduces_toppling_while_roll_bearing_prevents_it() {
    use qx_document::train::mechanism::JointModel;
    let mut old = conventional(1);
    for j in &mut old.connections {
        j.model = JointModel::Ball;
    }
    let track = straight(200.);
    let mut baseline = Rig::new(old, track.clone(), 100., 0.);
    // Reproduce the old missing rail-roll reaction and free longitudinal bearing.
    baseline.attitudes.clear();
    let mut corrected = Rig::new(conventional(1), track, 100., 0.);
    perturb(&mut baseline, 0.01, 0.);
    perturb(&mut corrected, 0.01, 0.);
    let before = run(&mut baseline, 960);
    let after = run(&mut corrected, 960);
    before.report("former ball mechanism control 4 s");
    after.report("corrected same perturbation 4 s");
    assert!(before.body_track.to_degrees() > 170.);
    assert!(after.body_track.to_degrees() < 0.6);
}

#[test]
fn banked_turn_keeps_body_and_gear_on_the_authored_track_frame() {
    let track = arc(45., 0.55, std::f64::consts::FRAC_PI_2, 300.);
    let mut rig = Rig::new(conventional(2), track, 60., 14.);
    let peaks = run(&mut rig, 480);
    peaks.report("banked curve 2 s");
    peaks.assert_travel();
    assert!(peaks.articulation > 0.01);
}

#[test]
fn hills_and_valleys_keep_roll_pinned_while_bogies_articulate() {
    for (name, bank, plane) in [
        ("valley", 0., 0.),
        ("crest", 0., std::f64::consts::PI),
        ("inverted hill", std::f64::consts::PI, 0.),
    ] {
        let mut rig = Rig::new(conventional(2), arc(40., bank, plane, 90.), 15., 12.);
        let peaks = run(&mut rig, 240);
        peaks.report(name);
        peaks.assert_travel();
        assert!(peaks.articulation > 0.005, "bogie pitch/yaw was locked out");
    }
}

#[test]
fn full_inversion_keeps_gear_and_body_with_the_inverted_track() {
    let radius = 20.;
    let track = arc(radius, 0., 0., std::f64::consts::TAU * radius);
    let mut rig = Rig::new(conventional(1), track, std::f64::consts::PI * radius, 5.);
    let peaks = run(&mut rig, 240);
    peaks.report("inverted apex 1 s");
    peaks.assert_travel();
    assert!(
        frame(rig.world.bodies[rig.body_index("car-1")].pose)
            .normal()
            .y
            < -0.8
    );
}

#[test]
fn two_car_train_traverses_an_entire_vertical_loop_without_a_pose_reset() {
    let radius = 20.;
    let end = 20. + std::f64::consts::TAU * radius;
    let mut loop_section = Section::straight(2, "numerical loop fixture", end - 20.);
    loop_section.kind = SectionKind::Arc {
        radius_m: radius,
        plane_angle_rad: 0.,
    };
    let track = Arc::new(
        generate(
            &InitialConditions::default(),
            &[
                Section::straight(1, "entry", 20.),
                loop_section,
                Section::straight(3, "exit", 80.),
            ],
            GenerationOptions::default(),
        )
        .unwrap(),
    );
    let mut rig = Rig::new(conventional(2), track, 15., 38.);
    let mut peaks = Peaks::default();
    let mut inverted = false;
    let mut completed = false;
    let start_energy = energy(&rig.world);
    for tick in 0..1500 {
        let d = rig.step();
        peaks.observe(&rig, &d);
        inverted |= frame(rig.world.bodies[rig.body_index("car-1")].pose)
            .normal()
            .y
            < -0.99;
        if rig.guides.iter().all(|g| g.station_m > end + 3.) {
            println!(
                "loop completed in {} ticks ({:.6} s), relative energy drift={:.6}%",
                tick + 1,
                (tick + 1) as f64 / 240.,
                100. * (energy(&rig.world) - start_energy) / start_energy
            );
            completed = true;
            break;
        }
    }
    peaks.report("full two-car loop traversal");
    assert!(
        inverted && completed,
        "train did not traverse the entire inversion"
    );
    peaks.assert_travel();
    assert!((energy(&rig.world) - start_energy).abs() / start_energy < 0.05);
}

#[test]
fn multi_car_consist_on_changing_curvature_and_bank_keeps_each_local_frame() {
    let track = twisting_banked_turn();
    let mut rig = Rig::new(conventional(3), track.clone(), 40., 12.);
    let peaks = run(&mut rig, 480);
    peaks.report("three cars changing curvature and bank 2 s");
    peaks.assert_travel();
    let first = track
        .sample_at_distance(rig.body_station(rig.body_index("car-1")))
        .unwrap()
        .frame;
    let last = track
        .sample_at_distance(rig.body_station(rig.body_index("car-3")))
        .unwrap()
        .frame;
    assert!(first.tangent().dot(last.tangent()) < 0.999);
    assert!(first.normal().dot(last.normal()) < 0.999);
    assert!(peaks.articulation > 0.005);
}

#[test]
fn banked_arc_energy_drift_stays_within_declared_stabilization_envelope() {
    let mut rig = Rig::new(
        conventional(2),
        arc(15., 0.3, std::f64::consts::FRAC_PI_2, 90.),
        10.,
        10.,
    );
    let start = energy(&rig.world);
    let peaks = run(&mut rig, 240);
    let drift = (energy(&rig.world) - start) / start.abs().max(1.);
    println!("banked arc energy drift={:.6}%", drift * 100.);
    peaks.assert_travel();
    assert!(drift.abs() < 0.05);
}

#[test]
fn below_track_body_retains_its_intended_pendulum_freedom() {
    let train =
        Train::development_consist("suspended".into(), 1, DevelopmentArrangement::BelowTrack)
            .unwrap();
    let mut rig = Rig::new(train, straight(200.), 100., 0.);
    perturb(&mut rig, 0.25, 0.);
    let initial = rig.max_body_to_gear_roll();
    let peaks = run(&mut rig, 240);
    peaks.report("suspended body remains free");
    assert!(rig.world.angular_joints.is_empty());
    let final_roll = roll_about(
        Frame::new(Quaternion::IDENTITY).unwrap(),
        frame(rig.world.bodies[rig.body_index("car-1")].pose),
    );
    println!(
        "suspended signed roll after 1 s={:.6} deg",
        final_roll.to_degrees()
    );
    assert!(
        initial > 0.24 && final_roll < -0.1,
        "pendulum must cross equilibrium"
    );
    assert!(peaks.gear_track < 1e-8);
}

#[test]
fn spatial_rail_twist_reactions_preserve_unforced_energy_after_velocity_seating() {
    let mut rig = Rig::new(conventional(2), twisting_banked_turn(), 40., 12.);
    rig.world.gravity_mps2 = Vec3::ZERO;
    // Project inconsistent seed velocities before measuring free evolution.
    run(&mut rig, 24);
    let start = energy(&rig.world);
    let peaks = run(&mut rig, 480);
    let drift = (energy(&rig.world) - start) / start;
    println!("changing-bank unforced energy drift={:.6}%", drift * 100.);
    peaks.assert_travel();
    assert!(drift.abs() < 0.01);
}
