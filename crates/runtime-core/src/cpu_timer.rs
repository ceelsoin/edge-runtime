use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Tracks CPU time consumed by an isolate.
///
/// Uses wall-clock time as an approximation. For true CPU time tracking
/// on Linux, you could use `clock_gettime(CLOCK_THREAD_CPUTIME_ID)`.
pub struct CpuTimer {
    started_at: Option<Instant>,
    accumulated_ms: u64,
    limit_ms: u64,
    exceeded: Arc<AtomicBool>,
}

impl CpuTimer {
    pub fn new(limit_ms: u64) -> Self {
        Self {
            started_at: None,
            accumulated_ms: 0,
            limit_ms,
            exceeded: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start timing a request.
    pub fn start(&mut self) {
        self.started_at = Some(Instant::now());
    }

    /// Stop timing and accumulate elapsed time. Returns elapsed ms for this request.
    pub fn stop(&mut self) -> u64 {
        if let Some(started) = self.started_at.take() {
            let elapsed = started.elapsed().as_millis() as u64;
            self.accumulated_ms += elapsed;
            if self.limit_ms > 0 && self.accumulated_ms >= self.limit_ms {
                self.exceeded.store(true, Ordering::Relaxed);
            }
            elapsed
        } else {
            0
        }
    }

    /// Check if the CPU time limit has been exceeded.
    pub fn is_exceeded(&self) -> bool {
        self.exceeded.load(Ordering::Relaxed)
    }

    /// Get the shared exceeded flag (for passing to V8 interrupt).
    pub fn exceeded_flag(&self) -> Arc<AtomicBool> {
        self.exceeded.clone()
    }

    pub fn accumulated_ms(&self) -> u64 {
        self.accumulated_ms
    }

    pub fn limit_ms(&self) -> u64 {
        self.limit_ms
    }
}

/// Wall-clock timeout guard for a single request.
pub struct WallClockGuard {
    deadline: Instant,
}

impl WallClockGuard {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            deadline: Instant::now() + Duration::from_millis(timeout_ms),
        }
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    pub fn as_sleep(&self) -> tokio::time::Sleep {
        tokio::time::sleep(self.remaining())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn cpu_timer_new_not_exceeded() {
        let timer = CpuTimer::new(5000);
        assert!(!timer.is_exceeded());
        assert_eq!(timer.accumulated_ms(), 0);
        assert_eq!(timer.limit_ms(), 5000);
    }

    #[test]
    fn cpu_timer_start_stop_accumulates() {
        let mut timer = CpuTimer::new(10_000);
        timer.start();
        thread::sleep(Duration::from_millis(50));
        let elapsed = timer.stop();
        assert!(elapsed >= 30, "elapsed should be >= 30ms, got {elapsed}");
        assert!(timer.accumulated_ms() >= 30);
        assert!(!timer.is_exceeded());
    }

    #[test]
    fn cpu_timer_stop_without_start_returns_zero() {
        let mut timer = CpuTimer::new(1000);
        let elapsed = timer.stop();
        assert_eq!(elapsed, 0);
    }

    #[test]
    fn cpu_timer_exceeds_limit() {
        let mut timer = CpuTimer::new(10);
        timer.start();
        thread::sleep(Duration::from_millis(30));
        timer.stop();
        assert!(timer.is_exceeded());
    }

    #[test]
    fn cpu_timer_exceeded_flag_shared() {
        let mut timer = CpuTimer::new(10);
        let flag = timer.exceeded_flag();
        assert!(!flag.load(Ordering::Relaxed));
        timer.start();
        thread::sleep(Duration::from_millis(30));
        timer.stop();
        assert!(flag.load(Ordering::Relaxed));
    }

    #[test]
    fn cpu_timer_multiple_start_stop() {
        let mut timer = CpuTimer::new(10_000);
        timer.start();
        thread::sleep(Duration::from_millis(20));
        timer.stop();
        let first = timer.accumulated_ms();

        timer.start();
        thread::sleep(Duration::from_millis(20));
        timer.stop();
        assert!(timer.accumulated_ms() > first);
    }

    #[test]
    fn wall_clock_not_expired_initially() {
        let guard = WallClockGuard::new(5000);
        assert!(!guard.is_expired());
        assert!(guard.remaining() > Duration::from_millis(4000));
    }

    #[test]
    fn wall_clock_expires() {
        let guard = WallClockGuard::new(10);
        thread::sleep(Duration::from_millis(30));
        assert!(guard.is_expired());
        assert_eq!(guard.remaining(), Duration::ZERO);
    }

    #[test]
    fn wall_clock_remaining_decreases() {
        let guard = WallClockGuard::new(1000);
        let r1 = guard.remaining();
        thread::sleep(Duration::from_millis(50));
        let r2 = guard.remaining();
        assert!(r2 < r1);
    }
}
