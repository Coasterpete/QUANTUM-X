//! Deadline waits and timing evidence for the playback worker (M4K.1).
//!
//! The worker schedules fixed 1/240 s steps against absolute monotonic
//! deadlines (`Instant`) and must sleep, never spin, until the next one.
//!
//! * `mpsc::Receiver::recv_timeout` on Windows parks with a millisecond timeout
//!   that the kernel rounds to the system clock interrupt (~15.6 ms by default).
//!   A 4.17 ms wait then wakes ~15.6 ms later and every wake must catch up about
//!   four steps. This was the pre-M4K.1 behavior.
//! * A high-resolution waitable timer (`CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`,
//!   Windows 10 1803+) honours sub-millisecond relative due times without
//!   raising the process- or system-wide timer resolution. `timeBeginPeriod` is
//!   deliberately not used. One timer is created per worker thread, reused for
//!   every wait and closed when the worker exits.
//!
//! While running, requests are polled before and after each timed wait, so a
//! request is handled no later than the next step deadline (<= 4.17 ms). While
//! paused or idle the worker blocks in `recv`, so it uses no CPU. If the timer
//! cannot be created, or a wait fails, the worker falls back to the portable
//! channel timeout and reports that mechanism.
use std::{
    sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError},
    time::{Duration, Instant},
};

/// Launch-only developer A/B contract, never serialized or shown as a setting.
/// `legacy` forces the pre-M4K.1 channel timeout for comparison measurements.
pub(super) const WAIT_ENV: &str = "QX_PLAYBACK_WAIT";

pub(super) fn legacy_requested(value: Option<&str>) -> Result<bool, String> {
    match value {
        None => Ok(false),
        Some("legacy") => Ok(true),
        Some(_) => Err(format!(
            "{WAIT_ENV}: expected legacy; remove the setting for the high-resolution timer"
        )),
    }
}

/// Upper bound on one timed wait. Deadlines are at most one step ahead; this
/// only guards against an unexpected far deadline.
const MAX_WAIT: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mechanism {
    HighResolutionTimer,
    ChannelTimeout,
}
impl Mechanism {
    pub fn label(self) -> &'static str {
        match self {
            Self::HighResolutionTimer => "high-resolution waitable timer",
            Self::ChannelTimeout => "channel timeout",
        }
    }
}

/// Created on, and owned by, the worker thread.
pub(super) struct Waiter {
    #[cfg(windows)]
    timer: Option<windows::Timer>,
}
impl Waiter {
    pub fn new(legacy: bool) -> Self {
        #[cfg(windows)]
        {
            Self {
                timer: if legacy { None } else { windows::Timer::new() },
            }
        }
        #[cfg(not(windows))]
        {
            let _ = legacy;
            Self {}
        }
    }
    pub fn mechanism(&self) -> Mechanism {
        #[cfg(windows)]
        if self.timer.is_some() {
            return Mechanism::HighResolutionTimer;
        }
        Mechanism::ChannelTimeout
    }
    /// Returns a request if one arrives, or `Timeout` once `deadline` is reached.
    pub fn wait<T>(
        &mut self,
        receive: &Receiver<T>,
        deadline: Instant,
    ) -> Result<T, RecvTimeoutError> {
        #[cfg(windows)]
        if let Some(timer) = &self.timer {
            match receive.try_recv() {
                Ok(request) => return Ok(request),
                Err(TryRecvError::Disconnected) => return Err(RecvTimeoutError::Disconnected),
                Err(TryRecvError::Empty) => {}
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if !remaining.is_zero() && !timer.wait(remaining.min(MAX_WAIT)) {
                // Never retry a failing timer every step: fall back for good.
                self.timer = None;
                return receive.recv_timeout(deadline.saturating_duration_since(Instant::now()));
            }
            return match receive.try_recv() {
                Ok(request) => Ok(request),
                Err(TryRecvError::Empty) => Err(RecvTimeoutError::Timeout),
                Err(TryRecvError::Disconnected) => Err(RecvTimeoutError::Disconnected),
            };
        }
        receive.recv_timeout(deadline.saturating_duration_since(Instant::now()))
    }
}

/// CPU time consumed by the calling thread, in milliseconds (Windows only).
pub(super) fn thread_cpu_ms() -> Option<f64> {
    #[cfg(windows)]
    {
        windows::thread_cpu_ms()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Fixed 0.25 ms bins up to 32 ms plus one overflow bin. Small and `Copy`-sized
/// so it can travel in every published snapshot.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Histogram {
    bins: [u32; Self::BINS + 1],
    max_ms: f64,
}
impl Default for Histogram {
    fn default() -> Self {
        Self {
            bins: [0; Self::BINS + 1],
            max_ms: 0.,
        }
    }
}
impl Histogram {
    const BINS: usize = 128;
    pub const BIN_MS: f64 = 0.25;
    pub fn record(&mut self, ms: f64) {
        let ms = ms.max(0.);
        let bin = ((ms / Self::BIN_MS) as usize).min(Self::BINS);
        self.bins[bin] = self.bins[bin].saturating_add(1);
        self.max_ms = self.max_ms.max(ms);
    }
    pub fn count(&self) -> u64 {
        self.bins.iter().map(|&b| u64::from(b)).sum()
    }
    /// Upper edge of the bin holding quantile `q` (resolution 0.25 ms); the
    /// overflow bin reports the observed maximum.
    pub fn quantile_ms(&self, q: f64) -> Option<f64> {
        let total = self.count();
        if total == 0 {
            return None;
        }
        let rank = ((q.clamp(0., 1.) * total as f64).ceil() as u64).max(1);
        let mut seen = 0;
        for (i, &b) in self.bins.iter().enumerate() {
            seen += u64::from(b);
            if seen >= rank {
                return Some(if i == Self::BINS {
                    self.max_ms
                } else {
                    (i + 1) as f64 * Self::BIN_MS
                });
            }
        }
        Some(self.max_ms)
    }
    /// Fraction of samples strictly below `ms` (bin resolution).
    pub fn fraction_below(&self, ms: f64) -> Option<f64> {
        let total = self.count();
        let edge = ((ms / Self::BIN_MS) as usize).min(Self::BINS);
        (total > 0).then(|| {
            self.bins[..edge].iter().map(|&b| u64::from(b)).sum::<u64>() as f64 / total as f64
        })
    }
    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({
            "count": self.count(),
            "p50_ms": self.quantile_ms(0.5),
            "p90_ms": self.quantile_ms(0.9),
            "p99_ms": self.quantile_ms(0.99),
            "max_ms": self.max_ms,
            "bin_ms": Self::BIN_MS,
        })
    }
}

#[cfg(windows)]
mod windows {
    //! Minimal kernel32 bindings. Declared by hand so the app does not enable
    //! extra `windows-sys` features, which would rebuild winit/wgpu dependents.
    use std::{ffi::c_void, time::Duration};

    type Handle = *mut c_void;
    const CREATE_WAITABLE_TIMER_HIGH_RESOLUTION: u32 = 0x0000_0002;
    const TIMER_ALL_ACCESS: u32 = 0x001F_0003;
    const WAIT_OBJECT_0: u32 = 0;

    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    impl FileTime {
        fn hundred_ns(&self) -> u64 {
            (u64::from(self.high) << 32) | u64::from(self.low)
        }
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateWaitableTimerExW(
            attributes: *const c_void,
            name: *const u16,
            flags: u32,
            access: u32,
        ) -> Handle;
        fn SetWaitableTimer(
            timer: Handle,
            due: *const i64,
            period: i32,
            routine: *const c_void,
            argument: *const c_void,
            resume: i32,
        ) -> i32;
        fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
        fn CloseHandle(handle: Handle) -> i32;
        fn GetCurrentThread() -> Handle;
        fn GetThreadTimes(
            thread: Handle,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }

    /// Owned, non-null, auto-reset high-resolution timer handle. Not `Send`:
    /// it lives and dies on the worker thread that created it.
    pub(super) struct Timer(Handle);
    impl Timer {
        /// `None` before Windows 10 1803 (flag unsupported) or on failure.
        pub fn new() -> Option<Self> {
            // SAFETY: null attributes/name are permitted; flags and access are
            // documented constants. The returned handle is checked for null.
            let handle = unsafe {
                CreateWaitableTimerExW(
                    std::ptr::null(),
                    std::ptr::null(),
                    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                    TIMER_ALL_ACCESS,
                )
            };
            (!handle.is_null()).then_some(Self(handle))
        }
        /// Blocks for `duration` (relative, monotonic). False on any failure.
        pub fn wait(&self, duration: Duration) -> bool {
            // Negative due time = relative interval in 100 ns units, rounded up.
            let units = i64::try_from(duration.as_nanos().div_ceil(100)).unwrap_or(i64::MAX);
            let due = -units.max(1);
            // SAFETY: `self.0` is a valid timer handle owned by this value;
            // `due` outlives the call; no completion routine or argument.
            let set =
                unsafe { SetWaitableTimer(self.0, &due, 0, std::ptr::null(), std::ptr::null(), 0) };
            if set == 0 {
                return false;
            }
            // The extra 100 ms bound means a timer that never signals cannot
            // hang the worker; that case is reported as a failure.
            let bound = u32::try_from(duration.as_millis() + 100).unwrap_or(u32::MAX - 1);
            // SAFETY: valid handle owned by this value.
            unsafe { WaitForSingleObject(self.0, bound) == WAIT_OBJECT_0 }
        }
    }
    impl Drop for Timer {
        fn drop(&mut self) {
            // SAFETY: the handle is valid and closed exactly once here.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    pub(super) fn thread_cpu_ms() -> Option<f64> {
        let mut creation = FileTime::default();
        let mut exit = FileTime::default();
        let mut kernel = FileTime::default();
        let mut user = FileTime::default();
        // SAFETY: the pseudo-handle needs no closing; all out-pointers are
        // valid, properly aligned `FILETIME`-layout locals.
        let ok = unsafe {
            GetThreadTimes(
                GetCurrentThread(),
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        };
        (ok != 0).then(|| (kernel.hundred_ns() + user.hundred_ns()) as f64 / 10_000.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn wait_policy_is_explicit() {
        assert_eq!(legacy_requested(None), Ok(false));
        assert_eq!(legacy_requested(Some("legacy")), Ok(true));
        assert!(legacy_requested(Some("fast")).is_err());
    }

    #[test]
    fn histogram_quantiles_use_bin_upper_edges() {
        let mut h = Histogram::default();
        assert_eq!(h.quantile_ms(0.5), None);
        for ms in [4.1, 4.2, 4.3, 15.6] {
            h.record(ms);
        }
        h.record(100.);
        assert_eq!(h.count(), 5);
        // Third of five samples is 4.3 ms, in the [4.25, 4.5) bin.
        assert_eq!(h.quantile_ms(0.5), Some(4.5));
        assert_eq!(h.quantile_ms(0.4), Some(4.25));
        assert_eq!(h.quantile_ms(1.), Some(100.));
        assert_eq!(h.max_ms, 100.);
        assert_eq!(h.fraction_below(5.), Some(0.6));
    }

    /// Both mechanisms honour requests and deadlines; neither waits past a
    /// generous bound. Precision itself is measured natively, not asserted
    /// here, because test machines are shared and noisy.
    #[test]
    fn waits_return_requests_timeouts_and_disconnects() {
        for legacy in [false, true] {
            let mut waiter = Waiter::new(legacy);
            let (send, receive) = mpsc::channel();
            send.send(7).unwrap();
            assert_eq!(
                waiter.wait(&receive, Instant::now() + Duration::from_secs(5)),
                Ok(7)
            );
            let start = Instant::now();
            let deadline = start + Duration::from_millis(4);
            assert_eq!(
                waiter.wait(&receive, deadline),
                Err(RecvTimeoutError::Timeout)
            );
            let elapsed = start.elapsed();
            assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
            drop(send);
            assert_eq!(
                waiter.wait(&receive, Instant::now() + Duration::from_secs(5)),
                Err(RecvTimeoutError::Disconnected)
            );
        }
        #[cfg(windows)]
        assert_eq!(
            Waiter::new(false).mechanism(),
            Mechanism::HighResolutionTimer
        );
        assert_eq!(Waiter::new(true).mechanism(), Mechanism::ChannelTimeout);
        #[cfg(windows)]
        assert!(thread_cpu_ms().is_some());
    }
}
