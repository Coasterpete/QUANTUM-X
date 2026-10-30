//! Public identity shared by window chrome, application information and packaging.
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductIdentity {
    pub name: String,
    pub repository: String,
    pub windows_executable: String,
    pub development_executable: String,
    pub project_extension: String,
}
pub fn product() -> &'static ProductIdentity {
    static IDENTITY: LazyLock<ProductIdentity> = LazyLock::new(|| {
        serde_json::from_str(include_str!("../../../product.json"))
            .expect("build-validated product identity")
    });
    &IDENTITY
}

/// Brand both native surfaces from the same resource as the executable icon.
/// No loose artwork or current-working-directory lookup is needed at runtime.
pub(crate) fn window_attributes() -> Result<winit::window::WindowAttributes, String> {
    let attributes = winit::window::Window::default_attributes();
    #[cfg(windows)]
    {
        use winit::{
            dpi::PhysicalSize,
            platform::windows::{IconExtWindows, WindowAttributesExtWindows},
            window::Icon,
        };
        // Keep resource ID 101 in sync with apps/quantum-x/build.rs and the
        // distribution verifier. Windows selects/scales the supplied ICO sizes.
        let window_icon = Icon::from_resource(101, None)
            .map_err(|e| format!("cannot load embedded QUANTUM Dynamics window icon: {e}"))?;
        let taskbar_icon = Icon::from_resource(101, Some(PhysicalSize::new(256, 256)))
            .map_err(|e| format!("cannot load embedded QUANTUM Dynamics taskbar icon: {e}"))?;
        Ok(attributes
            .with_window_icon(Some(window_icon))
            .with_taskbar_icon(Some(taskbar_icon)))
    }
    #[cfg(not(windows))]
    Ok(attributes)
}

#[cfg(test)]
mod tests {
    #[test]
    fn public_name_and_technical_identifiers_remain_distinct() {
        let p = super::product();
        assert_eq!(p.name, "QUANTUM Dynamics");
        assert_eq!(p.repository, "QUANTUM-X");
        assert_eq!(p.windows_executable, format!("{}.exe", p.name));
        assert_eq!(p.development_executable, "quantum-x.exe");
        assert_eq!(p.project_extension, "qx.json");
    }
}
