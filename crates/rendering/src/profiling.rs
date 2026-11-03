//! Opt-in, noninteractive native surface measurements. Hidden windows never
//! activate, receive synthetic desktop input, or change user preferences.
use serde::Serialize;
use std::{path::PathBuf, time::Instant};

pub struct NativeProfile {
    pub output: PathBuf,
    pub hz: u32,
    pub seconds: f64,
    pub legacy_wait: bool,
}
#[derive(Clone, Default, Serialize)]
pub(crate) struct DrawTiming {
    pub size: [u32; 2],
    pub viewport_px: [f32; 4],
    pub surface_status: String,
    pub presented: bool,
    pub acquire_ms: f64,
    pub encode_submit_ms: f64,
    pub present_ms: f64,
}
pub(crate) struct Capture {
    pub config: NativeProfile,
    pub start: Instant,
    pub next: Instant,
    previous: Option<Instant>,
    pub rows: Vec<serde_json::Value>,
    pub missed_deadlines: u64,
}
impl Capture {
    pub fn new(config: NativeProfile) -> Result<Self, String> {
        if ![30, 60, 144].contains(&config.hz)
            || !(2. ..=60.).contains(&config.seconds)
            || config.output.exists()
        {
            return Err(
                "profile requires 30/60/144 Hz, 2..60 seconds and a new output directory".into(),
            );
        }
        std::fs::create_dir_all(&config.output).map_err(|e| e.to_string())?;
        let start = Instant::now();
        Ok(Self {
            config,
            start,
            next: start,
            previous: None,
            rows: vec![],
            missed_deadlines: 0,
        })
    }
    pub fn record(
        &mut self,
        start: Instant,
        ui_ms: f64,
        upload_ms: f64,
        draw: &DrawTiming,
        data: serde_json::Value,
    ) {
        let dt = self
            .previous
            .replace(start)
            .map(|p| start.duration_since(p).as_secs_f64() * 1000.);
        self.rows.push(serde_json::json!({"wall_s":start.duration_since(self.start).as_secs_f64(),"frame_interval_ms":dt,"ui_ms":ui_ms,"upload_ms":upload_ms,"frame_cpu_ms":start.elapsed().as_secs_f64()*1000.,"draw":draw,"pose":data}));
    }
    pub fn finish(
        &self,
        adapter: &str,
        monitor_refresh_millihertz: Option<u32>,
    ) -> Result<(), String> {
        let report = serde_json::json!({"native_winit_surface":true,"hidden_window":true,"visible_display_refresh_measured":false,"monitor_refresh_millihertz":monitor_refresh_millihertz,"present_mode":"AutoVsync","width":1440,"height":960,"target_hz":self.config.hz,"legacy_16ms_wait":self.config.legacy_wait,"adapter":adapter,"missed_deadlines":self.missed_deadlines,"rows":self.rows});
        std::fs::write(
            self.config.output.join("native-frames.json"),
            serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
}
