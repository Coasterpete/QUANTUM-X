//! Experimental rigid-body foundation. SI units; +Y is up.
//! This is not yet a complete train or wheel/rail simulation.

mod body;
pub mod inspection;
mod playback;
mod solver;
pub mod train;

pub use body::{Body, MassProperties, Pose};
pub use playback::Simulation;
pub use solver::{BallJoint, Diagnostics, FIXED_STEP_SECONDS, Guide, TrackGuide, World};

pub type Result<T> = std::result::Result<T, String>;

pub mod consist;
