use std::sync::atomic::{AtomicU64, Ordering};

/// Tracks heap memory usage for an isolate and enforces limits.
pub struct MemCheck {
    limit_bytes: usize,
}

impl MemCheck {
    pub fn new(limit_bytes: usize) -> Self {
        Self { limit_bytes }
    }

    /// Check if the isolate has exceeded its memory limit.
    /// Returns `(used_bytes, exceeded)`.
    pub fn check(&self, isolate: &mut deno_core::v8::Isolate) -> (usize, bool) {
        let stats = isolate.get_heap_statistics();
        let used = stats.used_heap_size() + stats.external_memory();
        let exceeded = self.limit_bytes > 0 && used >= self.limit_bytes;
        (used, exceeded)
    }

    pub fn limit_bytes(&self) -> usize {
        self.limit_bytes
    }
}

/// Atomic counter for tracking memory across isolates.
pub struct GlobalMemoryTracker {
    total_used: AtomicU64,
}

impl GlobalMemoryTracker {
    pub fn new() -> Self {
        Self {
            total_used: AtomicU64::new(0),
        }
    }

    pub fn add(&self, bytes: u64) {
        self.total_used.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn sub(&self, bytes: u64) {
        self.total_used.fetch_sub(bytes, Ordering::Relaxed);
    }

    pub fn total(&self) -> u64 {
        self.total_used.load(Ordering::Relaxed)
    }
}

impl Default for GlobalMemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mem_check_new_stores_limit() {
        let mc = MemCheck::new(1024);
        assert_eq!(mc.limit_bytes(), 1024);
    }

    #[test]
    fn mem_check_zero_limit() {
        let mc = MemCheck::new(0);
        assert_eq!(mc.limit_bytes(), 0);
    }

    #[test]
    fn global_memory_tracker_starts_zero() {
        let tracker = GlobalMemoryTracker::new();
        assert_eq!(tracker.total(), 0);
    }

    #[test]
    fn global_memory_tracker_add_sub() {
        let tracker = GlobalMemoryTracker::new();
        tracker.add(100);
        tracker.add(200);
        assert_eq!(tracker.total(), 300);
        tracker.sub(50);
        assert_eq!(tracker.total(), 250);
    }

    #[test]
    fn global_memory_tracker_default() {
        let tracker = GlobalMemoryTracker::default();
        assert_eq!(tracker.total(), 0);
    }
}
