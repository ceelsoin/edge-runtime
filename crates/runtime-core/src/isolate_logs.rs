use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct IsolateConsoleLog {
    pub timestamp: DateTime<Utc>,
    pub function_name: String,
    pub request_id: String,
    pub level: u8,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct IsolateLogConfig {
    pub function_name: String,
    pub emit_to_stdout: bool,
}

impl Default for IsolateLogConfig {
    fn default() -> Self {
        Self {
            function_name: "unknown".to_string(),
            emit_to_stdout: true,
        }
    }
}

const MAX_COLLECTED_LOGS: usize = 10_000;

static LOG_COLLECTOR: OnceLock<Mutex<VecDeque<IsolateConsoleLog>>> = OnceLock::new();

fn collector() -> &'static Mutex<VecDeque<IsolateConsoleLog>> {
    LOG_COLLECTOR.get_or_init(|| Mutex::new(VecDeque::with_capacity(1024)))
}

pub fn push_collected_log(entry: IsolateConsoleLog) {
    let Ok(mut guard) = collector().lock() else {
        return;
    };

    if guard.len() >= MAX_COLLECTED_LOGS {
        let _ = guard.pop_front();
    }

    guard.push_back(entry);
}

pub fn collected_log_count() -> usize {
    collector().lock().map(|g| g.len()).unwrap_or(0)
}

pub fn drain_collected_logs(max_items: usize) -> Vec<IsolateConsoleLog> {
    if max_items == 0 {
        return Vec::new();
    }

    let Ok(mut guard) = collector().lock() else {
        return Vec::new();
    };

    let to_take = guard.len().min(max_items);
    let mut out = Vec::with_capacity(to_take);
    for _ in 0..to_take {
        if let Some(item) = guard.pop_front() {
            out.push(item);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collector_accepts_entries() {
        let before = collected_log_count();
        push_collected_log(IsolateConsoleLog {
            timestamp: Utc::now(),
            function_name: "test-fn".to_string(),
            request_id: "isolate-console".to_string(),
            level: 1,
            message: "hello".to_string(),
        });
        let after = collected_log_count();
        assert!(after >= before + 1);
    }

    #[test]
    fn collector_drains_up_to_limit() {
        push_collected_log(IsolateConsoleLog {
            timestamp: Utc::now(),
            function_name: "test-fn".to_string(),
            request_id: "isolate-console".to_string(),
            level: 1,
            message: "one".to_string(),
        });
        push_collected_log(IsolateConsoleLog {
            timestamp: Utc::now(),
            function_name: "test-fn".to_string(),
            request_id: "isolate-console".to_string(),
            level: 1,
            message: "two".to_string(),
        });

        let drained = drain_collected_logs(1);
        assert_eq!(drained.len(), 1);
    }
}
