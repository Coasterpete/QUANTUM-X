//! Experimental rigid-body foundation. SI units; +Y is up.
//! This is not yet a complete train or wheel/rail simulation.

mod angular;
mod assembly;
mod body;
pub use angular::AngularJoint;
pub mod inspection;
mod playback;
mod solver;
pub mod train;

pub use body::{Body, MassProperties, Pose};
pub use playback::{Simulation, TrackSimulation};
pub use solver::{
    BallJoint, Diagnostics, FIXED_STEP_SECONDS, Guide, TrackAttitude, TrackGuide, World,
};

pub type Result<T> = std::result::Result<T, String>;

pub mod consist;
pub mod devices;
