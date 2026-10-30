//! Development-only native inspection. GPU and window resources stay here.
pub mod asset;
mod asset_gpu;
mod camera;
mod gpu;
pub mod identity;
mod inspector;
mod mesh_gpu;
mod offscreen;
mod scene;
mod window;
mod workspace;
pub use inspector::{HELP, inspect};
pub use offscreen::render_offscreen;
pub use scene::{DisplayMode, DisplayOptions, InspectionScene, OverlayLine, SectionLabel};
pub use window::{StartupSizing, WindowCommand, WindowPreferences};
pub use workspace::{EditorSurface, SurfaceOutput, ViewCommand, edit};
