//! Logical client sizes and physical desktop positions are deliberately distinct.
//! Preferences are supplied/persisted by the app, never by a coaster document.
use serde::{Deserialize, Serialize};
use winit::{
    dpi::{LogicalSize, PhysicalPosition},
    monitor::MonitorHandle,
    window::Window,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartupSizing {
    Automatic,
    Custom,
    #[default]
    Remember,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindowPreferences {
    pub startup: StartupSizing,
    pub custom_size: [f64; 2],
    pub last_size: Option<[f64; 2]>,
    /// Outer top-left in desktop physical pixels (can be negative).
    pub position: Option<[i32; 2]>,
    pub maximized: bool,
    /// egui zoom, independent of monitor DPI and logical client dimensions.
    pub ui_scale: f32,
}
impl Default for WindowPreferences {
    fn default() -> Self {
        Self {
            startup: StartupSizing::Remember,
            custom_size: [1280., 800.],
            last_size: None,
            position: None,
            maximized: false,
            ui_scale: 1.,
        }
    }
}
impl WindowPreferences {
    pub fn sanitize(&mut self) {
        self.custom_size = valid_size(self.custom_size).unwrap_or([1280., 800.]);
        self.last_size = self.last_size.and_then(valid_size);
        if !self.ui_scale.is_finite() {
            self.ui_scale = 1.;
        }
        self.ui_scale = self.ui_scale.clamp(0.75, 2.);
    }
}
fn valid_size(size: [f64; 2]) -> Option<[f64; 2]> {
    size.iter()
        .all(|v| v.is_finite() && *v > 0.)
        .then(|| [size[0].clamp(960., 16384.), size[1].clamp(640., 16384.)])
}
#[derive(Clone)]
pub enum WindowCommand {
    Apply(WindowPreferences),
    Reset,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkArea {
    pub position: [i32; 2],
    pub size: [u32; 2],
    pub scale: f64,
}
#[derive(Debug)]
pub(crate) struct Placement {
    pub size: [f64; 2],
    pub position: [i32; 2],
    pub minimum: [f64; 2],
}
pub(crate) fn resolve(prefs: &WindowPreferences, areas: &[WorkArea]) -> Placement {
    let fallback = WorkArea {
        position: [0, 0],
        size: [1280, 800],
        scale: 1.,
    };
    let area = prefs
        .position
        .and_then(|p| {
            areas.iter().find(|a| {
                // Restore on the monitor containing the title bar; stale/off-screen positions recover.
                p[0] as i64 >= a.position[0] as i64
                    && p[1] as i64 >= a.position[1] as i64
                    && (p[0] as i64) < a.position[0] as i64 + a.size[0] as i64
                    && (p[1] as i64) < a.position[1] as i64 + a.size[1] as i64
            })
        })
        .or_else(|| areas.first())
        .unwrap_or(&fallback);
    let scale = if area.scale.is_finite() && area.scale > 0. {
        area.scale
    } else {
        1.
    };
    // Reserve space for nonclient borders/title bar, in logical pixels. Windows supplies rcWork.
    let maximum = [
        (area.size[0] as f64 / scale - 16.).max(1.),
        (area.size[1] as f64 / scale - 48.).max(1.),
    ];
    let minimum = [960_f64.min(maximum[0]), 640_f64.min(maximum[1])];
    let automatic = [maximum[0] * 0.90, maximum[1] * 0.88];
    let desired = match prefs.startup {
        StartupSizing::Automatic => automatic,
        StartupSizing::Custom => valid_size(prefs.custom_size).unwrap_or(automatic),
        StartupSizing::Remember => prefs.last_size.and_then(valid_size).unwrap_or(automatic),
    };
    let size = [
        desired[0].clamp(minimum[0], maximum[0]),
        desired[1].clamp(minimum[1], maximum[1]),
    ];
    let outer = [(size[0] + 16.) * scale, (size[1] + 48.) * scale];
    let centered = [
        area.position[0] as f64 + (area.size[0] as f64 - outer[0]) * 0.5,
        area.position[1] as f64 + (area.size[1] as f64 - outer[1]) * 0.5,
    ];
    let requested = prefs
        .position
        .map(|p| [p[0] as f64, p[1] as f64])
        .unwrap_or(centered);
    let position = std::array::from_fn(|i| {
        requested[i]
            .clamp(
                area.position[i] as f64,
                (area.position[i] as f64 + area.size[i] as f64 - outer[i])
                    .max(area.position[i] as f64),
            )
            .round() as i32
    });
    Placement {
        size,
        position,
        minimum,
    }
}
pub(crate) fn work_area(monitor: &MonitorHandle) -> WorkArea {
    let p = monitor.position();
    let s = monitor.size();
    let mut area = WorkArea {
        position: [p.x, p.y],
        size: [s.width, s.height],
        scale: monitor.scale_factor(),
    };
    #[cfg(windows)]
    {
        use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO};
        use winit::platform::windows::MonitorHandleExtWindows;
        // SAFETY: MONITORINFO contains only integer fields and RECTs; all-zero is valid.
        let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        // SAFETY: winit supplies a live monitor handle; info is correctly sized, writable
        // storage. Read rcWork only after Win32 reports success. No display modes change.
        if unsafe { GetMonitorInfoW(monitor.hmonitor() as _, &mut info) } != 0 {
            let r = info.rcWork;
            area.position = [r.left, r.top];
            area.size = [
                (r.right - r.left).max(1) as u32,
                (r.bottom - r.top).max(1) as u32,
            ];
        }
    }
    area
}
pub(crate) fn apply(window: &Window, prefs: &WindowPreferences) {
    let mut monitors: Vec<_> = window.available_monitors().collect();
    if let Some(primary) = window.primary_monitor() {
        monitors.sort_by_key(|m| *m != primary);
    }
    let areas: Vec<_> = monitors.iter().map(work_area).collect();
    let p = resolve(prefs, &areas);
    window.set_maximized(false);
    window.set_min_inner_size(Some(LogicalSize::new(p.minimum[0], p.minimum[1])));
    let _ = window.request_inner_size(LogicalSize::new(p.size[0], p.size[1]));
    window.set_outer_position(PhysicalPosition::new(p.position[0], p.position[1]));
    window.set_maximized(prefs.maximized);
}
pub(crate) fn capture(window: &Window, prefs: &mut WindowPreferences) {
    prefs.maximized = window.is_maximized();
    if !prefs.maximized && window.is_minimized() != Some(true) {
        let size = window.inner_size().to_logical::<f64>(window.scale_factor());
        if size.width > 0. && size.height > 0. {
            prefs.last_size = Some([size.width, size.height]);
        }
        if let Ok(p) = window.outer_position() {
            prefs.position = Some([p.x, p.y]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_and_offscreen_preferences_recover_inside_usable_monitor() {
        let areas = [WorkArea {
            position: [-1920, 20],
            size: [1920, 1040],
            scale: 1.5,
        }];
        let mut prefs = WindowPreferences {
            position: Some([50000, -50000]),
            last_size: Some([f64::INFINITY, -1.]),
            ui_scale: f32::NAN,
            ..Default::default()
        };
        prefs.sanitize();
        let p = resolve(&prefs, &areas);
        assert_eq!(prefs.ui_scale, 1.);
        assert!(p.position[0] >= -1920 && p.position[1] >= 20);
        assert!((p.position[0] as f64 + (p.size[0] + 16.) * 1.5) <= 0.5);
        assert!((p.position[1] as f64 + (p.size[1] + 48.) * 1.5) <= 1060.5);
    }
    #[test]
    fn tiny_monitors_custom_and_maximized_restore_have_bounded_normal_size() {
        for size in [[640, 480], [3840, 2160]] {
            let areas = [WorkArea {
                position: [0, 0],
                size,
                scale: 1.,
            }];
            let p = resolve(
                &WindowPreferences {
                    startup: StartupSizing::Custom,
                    custom_size: [1e8, 1e8],
                    maximized: true,
                    ..Default::default()
                },
                &areas,
            );
            assert!(p.size[0] <= size[0] as f64 && p.size[1] <= size[1] as f64);
            assert!(p.minimum[0] <= p.size[0] && p.minimum[1] <= p.size[1]);
        }
    }
}
