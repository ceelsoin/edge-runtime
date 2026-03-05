use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

/// Global runtime metrics aggregated from all functions.
#[derive(Debug, Default)]
pub struct GlobalMetrics {
    pub total_requests: AtomicU64,
    pub total_errors: AtomicU64,
}

impl GlobalMetrics {
    pub fn snapshot(&self) -> GlobalMetricsSnapshot {
        GlobalMetricsSnapshot {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_errors: self.total_errors.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalMetricsSnapshot {
    pub total_requests: u64,
    pub total_errors: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_metrics_default_zeros() {
        let m = GlobalMetrics::default();
        let snap = m.snapshot();
        assert_eq!(snap.total_requests, 0);
        assert_eq!(snap.total_errors, 0);
    }

    #[test]
    fn global_metrics_snapshot_reflects_updates() {
        let m = GlobalMetrics::default();
        m.total_requests.fetch_add(42, Ordering::Relaxed);
        m.total_errors.fetch_add(7, Ordering::Relaxed);
        let snap = m.snapshot();
        assert_eq!(snap.total_requests, 42);
        assert_eq!(snap.total_errors, 7);
    }

    #[test]
    fn global_metrics_snapshot_serializes() {
        let snap = GlobalMetricsSnapshot {
            total_requests: 100,
            total_errors: 5,
        };
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("\"total_requests\":100"));
        assert!(json.contains("\"total_errors\":5"));
    }
}
