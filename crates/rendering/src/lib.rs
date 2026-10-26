//! Development-only native inspection. GPU and window resources stay here.
mod camera;
mod gpu;
mod inspector;
mod offscreen;
mod scene;
mod window;
mod workspace;
pub use inspector::{HELP, inspect};
pub use offscreen::render_offscreen;
pub use scene::{DisplayOptions, InspectionScene, SectionLabel};
pub use window::{StartupSizing, WindowCommand, WindowPreferences};
pub use workspace::{EditorSurface, SurfaceOutput, ViewCommand, edit};
