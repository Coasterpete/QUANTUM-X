//! Development-only native inspection. GPU and window resources stay here.
mod camera;
mod gpu;
mod inspector;
mod scene;
pub use inspector::{HELP, inspect};
pub use scene::{InspectionScene, SectionLabel};
