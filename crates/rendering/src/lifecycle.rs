//! Native editor lifecycle diagnostics: why the event loop ended, and the bounded
//! hidden-window GUI smoke test. Pure state lives here so it can be unit tested
//! without a window or GPU; `workspace.rs` drives it from winit callbacks.
use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};

/// Why the native editor event loop ended. The first cause recorded wins.
#[derive(Clone, Debug, PartialEq)]
pub enum ExitReason {
    /// The OS close request (title-bar X, Alt+F4) was accepted by the surface
    /// without a confirmation prompt.
    WindowClosed,
    /// The surface itself requested closing through `SurfaceOutput::close`
    /// after its own confirmation (the editor's "Discard edits and close").
    CloseApproved,
    /// A bounded hidden-window diagnostic (profile capture or smoke test)
    /// finished its run.
    DiagnosticComplete,
    /// A bounded hidden-window diagnostic failed (timeout, no presented frame,
    /// or its report could not be written).
    DiagnosticFailed(String),
    /// Window, surface, adapter/device or first scene upload failed while the
    /// event loop resumed.
    InitializationFailed(String),
    /// A window event or frame failed after initialization succeeded.
    RenderingFailed(String),
    /// The winit event loop could not be created or returned an error.
    EventLoopFailed(String),
    /// The event loop returned without any recorded cause (platform shutdown).
    EventLoopEnded,
}
impl ExitReason {
    /// Stable machine-readable identifier used in structured logs.
    pub fn code(&self) -> &'static str {
        match self {
            Self::WindowClosed => "window_closed",
            Self::CloseApproved => "close_approved",
            Self::DiagnosticComplete => "diagnostic_complete",
            Self::DiagnosticFailed(_) => "diagnostic_failed",
            Self::InitializationFailed(_) => "initialization_failed",
            Self::RenderingFailed(_) => "rendering_failed",
            Self::EventLoopFailed(_) => "event_loop_failed",
            Self::EventLoopEnded => "event_loop_ended",
        }
    }
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::DiagnosticFailed(d)
            | Self::InitializationFailed(d)
            | Self::RenderingFailed(d)
            | Self::EventLoopFailed(d) => Some(d),
            _ => None,
        }
    }
    /// Failures that make the process exit with a nonzero status.
    pub fn is_fatal(&self) -> bool {
        self.detail().is_some()
    }
}

/// Summary of one native editor run, returned after all window/GPU resources
/// owned by the event loop have been released.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorExit {
    pub reason: ExitReason,
    pub window_created: bool,
    /// Frames whose swapchain image was actually presented.
    pub frames_presented: u64,
    pub uptime_s: f64,
}
impl EditorExit {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "reason": self.reason.code(),
            "fatal": self.reason.is_fatal(),
            "detail": self.reason.detail(),
            "window_created": self.window_created,
            "frames_presented": self.frames_presented,
            "uptime_s": (self.uptime_s * 1000.).round() / 1000.,
        })
    }
    /// Preserves the historical `Result` contract: only failures are errors.
    pub fn into_result(self) -> Result<(), String> {
        match self.reason {
            ExitReason::DiagnosticFailed(e)
            | ExitReason::InitializationFailed(e)
            | ExitReason::RenderingFailed(e)
            | ExitReason::EventLoopFailed(e) => Err(e),
            _ => Ok(()),
        }
    }
}

/// Progress marker shared with a caller-side watchdog thread, so a hard
/// timeout can report where the main thread stopped responding.
#[derive(Clone, Debug, Default)]
pub struct SmokeStage(Arc<AtomicU8>);
const STAGES: [&str; 7] = [
    "event_loop",
    "window",
    "gpu",
    "scene_upload",
    "frames",
    "cleanup",
    "done",
];
impl SmokeStage {
    pub fn get(&self) -> &'static str {
        STAGES[(self.0.load(Ordering::Acquire) as usize).min(STAGES.len() - 1)]
    }
    pub(crate) fn set(&self, stage: &str) {
        if let Some(i) = STAGES.iter().position(|s| *s == stage) {
            self.0.store(i as u8, Ordering::Release);
        }
    }
}

#[derive(Clone, Debug)]
pub struct SmokeConfig {
    /// Presented frames required for a pass.
    pub frames: u32,
    /// Soft deadline checked by the event loop; on expiry it exits cleanly and
    /// releases resources. A blocked main thread needs the caller's watchdog.
    pub timeout: Duration,
    pub stage: SmokeStage,
}
impl Default for SmokeConfig {
    fn default() -> Self {
        Self {
            frames: 5,
            timeout: Duration::from_secs(20),
            stage: SmokeStage::default(),
        }
    }
}

/// Smoke-test bookkeeping updated by the workspace event loop.
#[derive(Debug)]
pub(crate) struct Smoke {
    pub config: SmokeConfig,
    pub start: Instant,
    pub attempts: u64,
    pub presented: u32,
    pub last_surface_status: String,
    pub window_ms: Option<f64>,
    pub gpu_ready_ms: Option<f64>,
    pub first_frame_ms: Option<f64>,
    pub adapter: Option<String>,
    pub surface_size: Option<[u32; 2]>,
    pub resources_released: Option<bool>,
}
impl Smoke {
    pub fn new(config: SmokeConfig, start: Instant) -> Self {
        config.stage.set("event_loop");
        Self {
            config,
            start,
            attempts: 0,
            presented: 0,
            last_surface_status: "none".into(),
            window_ms: None,
            gpu_ready_ms: None,
            first_frame_ms: None,
            adapter: None,
            surface_size: None,
            resources_released: None,
        }
    }
    fn ms(&self, now: Instant) -> f64 {
        now.duration_since(self.start).as_secs_f64() * 1000.
    }
    pub fn window_created(&mut self, now: Instant) {
        self.window_ms = Some(self.ms(now));
        self.config.stage.set("gpu");
    }
    pub fn gpu_ready(&mut self, now: Instant, adapter: String) {
        self.gpu_ready_ms = Some(self.ms(now));
        self.adapter = Some(adapter);
        self.config.stage.set("scene_upload");
    }
    pub fn initialized(&self) {
        self.config.stage.set("frames");
    }
    /// Records one draw attempt. Returns true once enough frames were presented.
    pub fn record(&mut self, now: Instant, surface_status: &str, presented: bool) -> bool {
        self.attempts += 1;
        self.last_surface_status = surface_status.into();
        if presented {
            self.presented += 1;
            if self.first_frame_ms.is_none() {
                self.first_frame_ms = Some(self.ms(now));
            }
        }
        self.complete()
    }
    pub fn complete(&self) -> bool {
        self.presented >= self.config.frames
    }
    pub fn timed_out(&self, now: Instant) -> bool {
        now.duration_since(self.start) >= self.config.timeout
    }
    pub fn timeout_reason(&self) -> ExitReason {
        ExitReason::DiagnosticFailed(format!(
            "GUI smoke test timed out after {:.1} s at stage '{}' with {}/{} frames presented (last surface status: {})",
            self.config.timeout.as_secs_f64(),
            self.config.stage.get(),
            self.presented,
            self.config.frames,
            self.last_surface_status
        ))
    }
    pub fn report(&self, exit: &EditorExit) -> serde_json::Value {
        let passed = exit.reason == ExitReason::DiagnosticComplete
            && self.complete()
            && self.resources_released == Some(true);
        let round = |v: Option<f64>| v.map(|v| (v * 100.).round() / 100.);
        serde_json::json!({
            "ok": passed,
            "result": if passed { "passed" } else if matches!(&exit.reason, ExitReason::DiagnosticFailed(d) if d.contains("timed out")) { "timeout" } else { "failed" },
            "hidden_window": true,
            "frames_requested": self.config.frames,
            "frames_presented": self.presented,
            "draw_attempts": self.attempts,
            "last_surface_status": self.last_surface_status,
            "surface_size": self.surface_size,
            "adapter": self.adapter,
            "timeout_s": self.config.timeout.as_secs_f64(),
            "timings_ms": {
                "window_created": round(self.window_ms),
                "gpu_ready": round(self.gpu_ready_ms),
                "first_frame_presented": round(self.first_frame_ms),
                "total": round(Some(exit.uptime_s * 1000.)),
            },
            "resources_released": self.resources_released,
            "exit": exit.to_json(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn exit(reason: ExitReason) -> EditorExit {
        EditorExit {
            reason,
            window_created: true,
            frames_presented: 3,
            uptime_s: 1.23456,
        }
    }
    #[test]
    fn exit_reasons_have_stable_codes_and_only_failures_are_fatal() {
        let cases = [
            (ExitReason::WindowClosed, "window_closed", false),
            (ExitReason::CloseApproved, "close_approved", false),
            (ExitReason::DiagnosticComplete, "diagnostic_complete", false),
            (ExitReason::EventLoopEnded, "event_loop_ended", false),
            (
                ExitReason::DiagnosticFailed("t".into()),
                "diagnostic_failed",
                true,
            ),
            (
                ExitReason::InitializationFailed("no adapter".into()),
                "initialization_failed",
                true,
            ),
            (
                ExitReason::RenderingFailed("lost".into()),
                "rendering_failed",
                true,
            ),
            (
                ExitReason::EventLoopFailed("x".into()),
                "event_loop_failed",
                true,
            ),
        ];
        for (reason, code, fatal) in cases {
            assert_eq!(reason.code(), code);
            assert_eq!(reason.is_fatal(), fatal);
            let e = exit(reason.clone());
            let json = e.to_json();
            assert_eq!(json["reason"], code);
            assert_eq!(json["fatal"], fatal);
            assert_eq!(json["uptime_s"], 1.235);
            assert_eq!(e.into_result().is_err(), fatal);
        }
        assert_eq!(
            exit(ExitReason::InitializationFailed("no adapter".into())).into_result(),
            Err("no adapter".into())
        );
    }
    #[test]
    fn smoke_passes_only_after_presented_frames_and_released_resources() {
        let start = Instant::now();
        let mut s = Smoke::new(
            SmokeConfig {
                frames: 2,
                ..Default::default()
            },
            start,
        );
        assert_eq!(s.config.stage.get(), "event_loop");
        s.window_created(start);
        assert_eq!(s.config.stage.get(), "gpu");
        s.gpu_ready(start, "Test adapter".into());
        s.initialized();
        assert_eq!(s.config.stage.get(), "frames");
        // Occluded/timeout attempts do not count as presented frames.
        assert!(!s.record(start, "unavailable", false));
        assert!(!s.record(start, "success", true));
        assert!(s.record(start, "suboptimal", true));
        assert_eq!((s.attempts, s.presented), (3, 2));
        let done = exit(ExitReason::DiagnosticComplete);
        assert_eq!(s.report(&done)["ok"], false, "cleanup not yet verified");
        s.resources_released = Some(true);
        let report = s.report(&done);
        assert_eq!(report["ok"], true);
        assert_eq!(report["result"], "passed");
        assert_eq!(report["frames_presented"], 2);
        assert_eq!(report["adapter"], "Test adapter");
    }
    #[test]
    fn smoke_timeout_is_bounded_and_reports_stage() {
        let start = Instant::now();
        let s = Smoke::new(
            SmokeConfig {
                frames: 4,
                timeout: Duration::from_millis(50),
                stage: SmokeStage::default(),
            },
            start,
        );
        assert!(!s.timed_out(start));
        assert!(s.timed_out(start + Duration::from_millis(50)));
        let reason = s.timeout_reason();
        assert!(reason.is_fatal());
        assert!(reason.detail().unwrap().contains("stage 'event_loop'"));
        assert!(reason.detail().unwrap().contains("0/4 frames"));
        let mut s = s;
        s.resources_released = Some(true);
        let report = s.report(&exit(reason));
        assert_eq!(report["ok"], false);
        assert_eq!(report["result"], "timeout");
    }
    #[test]
    fn stage_handle_is_shared_with_watchdog_clones() {
        let stage = SmokeStage::default();
        let watchdog = stage.clone();
        stage.set("cleanup");
        assert_eq!(watchdog.get(), "cleanup");
        stage.set("not-a-stage");
        assert_eq!(watchdog.get(), "cleanup");
    }
}
