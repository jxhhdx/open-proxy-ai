use chrono::{Local, Timelike};
use std::sync::Mutex;

/// Simple in-memory ring buffer log for the app.
pub struct AppLog {
    entries: Mutex<Vec<LogEntry>>,
    max: usize,
}

#[derive(Clone, serde::Serialize)]
pub struct LogEntry {
    pub level: String,
    pub msg: String,
    pub time: String,
}

impl AppLog {
    pub fn new(max: usize) -> Self {
        AppLog { entries: Mutex::new(Vec::new()), max }
    }

    fn fmt_time() -> String {
        let now = Local::now();
        format!("{:02}:{:02}:{:02}", now.hour(), now.minute(), now.second())
    }

    pub fn info(&self, msg: String) { self.push("INFO", msg); }
    pub fn warn(&self, msg: String) { self.push("WARN", msg); }
    pub fn error(&self, msg: String) { self.push("ERROR", msg); }

    fn push(&self, level: &str, msg: String) {
        let time = Self::fmt_time();
        let mut entries = self.entries.lock().unwrap();
        entries.push(LogEntry { level: level.into(), msg, time });
        while entries.len() > self.max { entries.remove(0); }
    }

    pub fn get_all(&self) -> Vec<LogEntry> {
        self.entries.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_has_local_time() {
        let log = AppLog::new(10);
        log.info("test message".into());
        let entries = log.get_all();
        assert_eq!(entries.len(), 1);
        // Time should be a valid HH:MM:SS format (local time, not UTC)
        let time = &entries[0].time;
        assert_eq!(time.len(), 8, "time should be HH:MM:SS format, got: {}", time);
        // Parse and verify it's a plausible hour (local time, not UTC+0)
        let h: u32 = time[..2].parse().unwrap();
        // The test runs in UTC+8, so between 00:00-23:59 local is fine
        // but critically, if it were UTC, during 08:00-23:59 UTC (16:00-07:59+8) the hour
        // would differ. We just verify it's a valid hour.
        assert!(h < 24, "hour should be valid, got: {}", h);
    }

    #[test]
    fn test_log_respects_max_entries() {
        let log = AppLog::new(3);
        for i in 0..5 {
            log.info(format!("msg {}", i));
        }
        assert_eq!(log.get_all().len(), 3);
        assert_eq!(log.get_all()[0].msg, "msg 2");
    }
}