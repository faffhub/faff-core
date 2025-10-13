use anyhow::{Context, Result};
use chrono::NaiveDate;
use chrono_tz::Tz;
use std::sync::Arc;

use crate::models::Log;
use crate::storage::Storage;

/// Manages log file operations.
///
/// Handles reading, writing, listing, and deleting daily logs.
/// Note: TOML parsing/formatting is currently handled on the Python side.
/// This manager provides the storage abstraction layer.
pub struct LogManager {
    storage: Arc<dyn Storage>,
    timezone: Tz,
}

impl LogManager {
    pub fn new(storage: Arc<dyn Storage>, timezone: Tz) -> Self {
        Self { storage, timezone }
    }

    /// Get the path for a log file
    pub fn log_file_path(&self, date: NaiveDate) -> std::path::PathBuf {
        self.storage.log_file_path(date)
    }

    /// Check if a log file exists
    pub fn log_exists(&self, date: NaiveDate) -> bool {
        let log_path = self.storage.log_file_path(date);
        self.storage.exists(&log_path)
    }

    /// Read the raw log file contents
    pub fn read_log_raw(&self, date: NaiveDate) -> Result<String> {
        let log_path = self.storage.log_file_path(date);
        self.storage.read_string(&log_path)
            .context(format!("Failed to read log file for {}", date))
    }

    /// Write raw log file contents
    pub fn write_log_raw(&self, date: NaiveDate, contents: &str) -> Result<()> {
        let log_path = self.storage.log_file_path(date);
        self.storage.write_string(&log_path, contents)
            .context(format!("Failed to write log for {}", date))
    }

    /// Get timezone for creating empty logs
    pub fn timezone(&self) -> Tz {
        self.timezone
    }

    /// Get a log for a given date. Returns an empty log if the file doesn't exist.
    pub fn get_log(&self, date: NaiveDate) -> Result<Log> {
        let log_path = self.storage.log_file_path(date);

        if self.storage.exists(&log_path) {
            let toml_str = self.storage.read_string(&log_path)
                .context(format!("Failed to read log file for {}", date))?;

            Log::from_log_file(&toml_str)
                .context(format!("Failed to parse log file for {}", date))
        } else {
            // Return empty log
            Ok(Log::new(date, self.timezone, vec![]))
        }
    }

    /// Write a log to storage
    ///
    /// trackers: map of tracker IDs to human-readable names for comments
    pub fn write_log(&self, log: &Log, trackers: &std::collections::HashMap<String, String>) -> Result<()> {
        let log_contents = log.to_log_file(trackers);
        let log_path = self.storage.log_file_path(log.date);

        self.storage.write_string(&log_path, &log_contents)
            .context(format!("Failed to write log for {}", log.date))
    }

    /// List all log dates in storage
    pub fn list_log_dates(&self) -> Result<Vec<NaiveDate>> {
        let log_dir = self.storage.log_dir();
        let files = self.storage.list_files(&log_dir, "*.toml")?;

        let mut dates = Vec::new();
        for file in files {
            // Extract date from filename (YYYY-MM-DD.toml)
            if let Some(stem) = file.file_stem().and_then(|s| s.to_str()) {
                if let Ok(date) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") {
                    dates.push(date);
                }
            }
        }

        dates.sort();
        Ok(dates)
    }

    /// Remove a log for a given date
    pub fn rm(&self, date: NaiveDate) -> Result<()> {
        let log_path = self.storage.log_file_path(date);

        if !self.storage.exists(&log_path) {
            anyhow::bail!("Log file for {} not found", date);
        }

        // Delete the file
        std::fs::remove_file(&log_path)
            .context(format!("Failed to delete log for {}", date))
    }

    /// Start a new session with the given intent at the current time
    ///
    /// Returns an error message if tracker validation fails, otherwise returns success message
    pub fn start_intent_now(
        &self,
        intent: crate::models::Intent,
        note: Option<String>,
        current_date: NaiveDate,
        current_time: chrono::DateTime<Tz>,
        trackers: &std::collections::HashMap<String, String>,
    ) -> Result<String> {
        // Get today's log
        let log = self.get_log(current_date)?;

        // Validate trackers if any are specified
        if !intent.trackers.is_empty() {
            let tracker_ids: std::collections::HashSet<_> = trackers.keys().collect();
            let intent_tracker_set: std::collections::HashSet<_> = intent.trackers.iter().collect();

            if !intent_tracker_set.is_subset(&tracker_ids) {
                let missing: Vec<_> = intent_tracker_set
                    .difference(&tracker_ids)
                    .map(|s| s.as_str())
                    .collect();
                anyhow::bail!("Tracker {} not found in today's plan.", missing.join(","));
            }
        }

        // Create new session
        let session = crate::models::Session::new(intent.clone(), current_time, None, note);

        // Append to log and write
        let updated_log = log.append_session(session);
        self.write_log(&updated_log, trackers)?;

        let alias = intent.alias.unwrap_or_else(|| "session".to_string());
        let time_str = current_time.format("%H:%M:%S");
        Ok(format!("Started logging {} at {}.", alias, time_str))
    }

    /// Stop the currently active session
    ///
    /// Returns success message with session alias and stop time, or message if no active session
    pub fn stop_current_session(
        &self,
        current_date: NaiveDate,
        current_time: chrono::DateTime<Tz>,
        trackers: &std::collections::HashMap<String, String>,
    ) -> Result<String> {
        let log = self.get_log(current_date)?;

        if let Some(active_session) = log.active_session() {
            let alias = active_session.intent.alias.clone().unwrap_or_else(|| "session".to_string());
            let updated_log = log.stop_active_session(current_time)?;
            self.write_log(&updated_log, trackers)?;

            let time_str = current_time.format("%H:%M:%S");
            Ok(format!("Stopped logging for {} at {}.", alias, time_str))
        } else {
            Ok("No ongoing timeline entries found to stop.".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// In-memory storage for testing
    struct MockStorage {
        files: Mutex<HashMap<PathBuf, String>>,
        log_dir: PathBuf,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                files: Mutex::new(HashMap::new()),
                log_dir: PathBuf::from("/logs"),
            }
        }
    }

    impl Storage for MockStorage {
        fn root_dir(&self) -> PathBuf {
            PathBuf::from("/")
        }

        fn log_dir(&self) -> PathBuf {
            self.log_dir.clone()
        }

        fn plan_dir(&self) -> PathBuf {
            PathBuf::from("/plans")
        }

        fn identity_dir(&self) -> PathBuf {
            PathBuf::from("/keys")
        }

        fn timesheet_dir(&self) -> PathBuf {
            PathBuf::from("/timesheets")
        }

        fn config_file(&self) -> PathBuf {
            PathBuf::from("/config.toml")
        }

        fn read_bytes(&self, path: &PathBuf) -> Result<Vec<u8>> {
            let files = self.files.lock().unwrap();
            files.get(path)
                .map(|s| s.as_bytes().to_vec())
                .ok_or_else(|| anyhow::anyhow!("File not found: {:?}", path))
        }

        fn read_string(&self, path: &PathBuf) -> Result<String> {
            let files = self.files.lock().unwrap();
            files.get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("File not found: {:?}", path))
        }

        fn write_bytes(&self, path: &PathBuf, data: &[u8]) -> Result<()> {
            let mut files = self.files.lock().unwrap();
            files.insert(path.clone(), String::from_utf8_lossy(data).to_string());
            Ok(())
        }

        fn write_string(&self, path: &PathBuf, data: &str) -> Result<()> {
            let mut files = self.files.lock().unwrap();
            files.insert(path.clone(), data.to_string());
            Ok(())
        }

        fn exists(&self, path: &PathBuf) -> bool {
            let files = self.files.lock().unwrap();
            files.contains_key(path)
        }

        fn create_dir_all(&self, _path: &PathBuf) -> Result<()> {
            Ok(())
        }

        fn list_files(&self, dir: &PathBuf, pattern: &str) -> Result<Vec<PathBuf>> {
            let files = self.files.lock().unwrap();
            let glob_pattern = pattern.replace("*", ".*");
            let re = regex::Regex::new(&glob_pattern).unwrap();

            Ok(files.keys()
                .filter(|p| p.starts_with(dir))
                .filter(|p| p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| re.is_match(n))
                    .unwrap_or(false))
                .cloned()
                .collect())
        }
    }

    #[test]
    fn test_log_exists() {
        let storage = Arc::new(MockStorage::new());
        let manager = LogManager::new(storage.clone(), chrono_tz::UTC);

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        assert!(!manager.log_exists(date));

        // Write a log
        manager.write_log_raw(date, "date = \"2025-03-15\"\n").unwrap();
        assert!(manager.log_exists(date));
    }

    #[test]
    fn test_write_and_read_raw() {
        let storage = Arc::new(MockStorage::new());
        let manager = LogManager::new(storage, chrono_tz::UTC);

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let content = "date = \"2025-03-15\"\ntimezone = \"UTC\"\n";

        manager.write_log_raw(date, content).unwrap();
        let retrieved = manager.read_log_raw(date).unwrap();

        assert_eq!(retrieved, content);
    }

    #[test]
    fn test_list_log_dates() {
        let storage = Arc::new(MockStorage::new());
        let manager = LogManager::new(storage, chrono_tz::UTC);

        let date1 = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let date2 = NaiveDate::from_ymd_opt(2025, 3, 16).unwrap();

        manager.write_log_raw(date1, "test").unwrap();
        manager.write_log_raw(date2, "test").unwrap();

        let dates = manager.list_log_dates().unwrap();
        assert_eq!(dates.len(), 2);
        assert_eq!(dates[0], date1);
        assert_eq!(dates[1], date2);
    }

    #[test]
    fn test_get_log_parses_toml() {
        let storage = Arc::new(MockStorage::new());
        let manager = LogManager::new(storage, chrono_tz::UTC);

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let toml_content = r#"
date = "2025-03-15"
timezone = "UTC"
version = "0.3.0"

[[timeline]]
alias = "work"
role = "dev"
objective = "feature"
action = "implement"
subject = "api"
trackers = ["PROJECT-123"]
start = "09:00"
end = "10:30"
note = "Morning session"
"#;

        manager.write_log_raw(date, toml_content).unwrap();
        let log = manager.get_log(date).unwrap();

        assert_eq!(log.date, date);
        assert_eq!(log.timezone, chrono_tz::UTC);
        assert_eq!(log.timeline.len(), 1);

        let session = &log.timeline[0];
        assert_eq!(session.intent.alias.as_ref().unwrap(), "work");
        assert_eq!(session.intent.role.as_ref().unwrap(), "dev");
        assert_eq!(session.note.as_ref().unwrap(), "Morning session");
    }

    #[test]
    fn test_get_log_returns_empty_when_missing() {
        let storage = Arc::new(MockStorage::new());
        let manager = LogManager::new(storage, chrono_tz::UTC);

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let log = manager.get_log(date).unwrap();

        assert_eq!(log.date, date);
        assert_eq!(log.timeline.len(), 0);
    }
}
