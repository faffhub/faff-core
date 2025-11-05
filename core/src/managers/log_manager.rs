use anyhow::{Context, Result};
use chrono::NaiveDate;
use chrono_tz::Tz;
use std::collections::HashMap;
use std::sync::Arc;

use crate::models::Log;
use crate::storage::Storage;

/// Manages log file operations.
///
/// Handles reading, writing, listing, and deleting daily logs.
/// Note: TOML parsing/formatting is currently handled on the Python side.
/// This manager provides the storage abstraction layer.
#[derive(Clone)]
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
        self.storage
            .read_string(&log_path)
            .context(format!("Failed to read log file for {}", date))
    }

    /// Write raw log file contents
    pub fn write_log_raw(&self, date: NaiveDate, contents: &str) -> Result<()> {
        let log_path = self.storage.log_file_path(date);
        self.storage
            .write_string(&log_path, contents)
            .context(format!("Failed to write log for {}", date))
    }

    /// Get timezone for creating empty logs
    pub fn timezone(&self) -> Tz {
        self.timezone
    }

    /// Get a log for a given date
    ///
    /// Returns None if the log file doesn't exist
    pub fn get_log(&self, date: NaiveDate) -> Result<Option<Log>> {
        let log_path = self.storage.log_file_path(date);

        if self.storage.exists(&log_path) {
            let toml_str = self
                .storage
                .read_string(&log_path)
                .with_context(|| format!("Failed to read log file for {}", date))?;

            let log = Log::from_log_file(&toml_str)
                .with_context(|| format!("Failed to parse log file for {}", date))?;

            Ok(Some(log))
        } else {
            Ok(None)
        }
    }

    /// Get a log for a given date, creating an empty one if it doesn't exist
    ///
    /// This is a convenience method for callers who always want a log to work with
    pub fn get_log_or_create(&self, date: NaiveDate) -> Result<Log> {
        if let Some(log) = self.get_log(date)? {
            Ok(log)
        } else {
            Ok(Log::new(date, self.timezone, vec![]))
        }
    }

    /// Write a log to storage
    ///
    /// trackers: map of tracker IDs to human-readable names for comments
    pub fn write_log(
        &self,
        log: &Log,
        trackers: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let log_contents = log.to_log_file(trackers);
        let log_path = self.storage.log_file_path(log.date);

        self.storage
            .write_string(&log_path, &log_contents)
            .context(format!("Failed to write log for {}", log.date))
    }

    /// List all log dates in storage
    pub fn list_logs(&self) -> Result<Vec<NaiveDate>> {
        let log_dir = self.storage.log_dir();
        let files = self
            .storage
            .list_files(&log_dir, "*.toml")
            .context("Failed to list log files")?;

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

    /// Delete a log for a given date
    pub fn delete_log(&self, date: NaiveDate) -> Result<()> {
        let log_path = self.storage.log_file_path(date);

        if !self.storage.exists(&log_path) {
            anyhow::bail!("Log for {} does not exist", date);
        }

        self.storage
            .delete(&log_path)
            .with_context(|| format!("Failed to delete log for {}", date))
    }

    /// Start a new session with the given intent at the current time
    pub fn start_intent_now(
        &self,
        intent: crate::models::Intent,
        note: Option<String>,
        current_date: NaiveDate,
        current_time: chrono::DateTime<Tz>,
        trackers: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        // Get today's log or create empty one
        let log = self.get_log_or_create(current_date)?;

        // Validate trackers if any are specified
        if !intent.trackers.is_empty() {
            let tracker_ids: std::collections::HashSet<_> = trackers.keys().collect();
            let intent_tracker_set: std::collections::HashSet<_> = intent.trackers.iter().collect();

            if !intent_tracker_set.is_subset(&tracker_ids) {
                let missing: Vec<_> = intent_tracker_set
                    .difference(&tracker_ids)
                    .map(|s| s.as_str())
                    .collect();
                anyhow::bail!("Tracker {} not found in today's plan", missing.join(", "));
            }
        }

        // Create new session
        let session = crate::models::Session::new(intent, current_time, None, note);

        // Append to log and write
        let updated_log = log.append_session(session)?;
        self.write_log(&updated_log, trackers)?;

        Ok(())
    }

    /// Stop the currently active session
    ///
    /// Returns Ok(()) if a session was stopped, or an error if no active session exists
    pub fn stop_current_session(
        &self,
        current_date: NaiveDate,
        current_time: chrono::DateTime<Tz>,
        trackers: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let log = self.get_log_or_create(current_date)?;

        if log.active_session().is_some() {
            let updated_log = log.stop_active_session(current_time)?;
            self.write_log(&updated_log, trackers)?;
            Ok(())
        } else {
            anyhow::bail!("No active session to stop")
        }
    }

    /// Find all logs that contain sessions using the given intent
    ///
    /// Returns a list of (date, session_count) tuples
    pub fn find_logs_with_intent(&self, intent_id: &str) -> Result<Vec<(NaiveDate, usize)>> {
        let all_dates = self.list_logs()?;
        let mut logs_with_intent = Vec::new();

        for date in all_dates {
            if let Ok(Some(log)) = self.get_log(date) {
                let count = log
                    .timeline
                    .iter()
                    .filter(|s| s.intent.intent_id == intent_id)
                    .count();

                if count > 0 {
                    logs_with_intent.push((date, count));
                }
            }
        }

        Ok(logs_with_intent)
    }

    /// Update an intent across all log files
    ///
    /// Returns the total number of sessions updated
    pub fn update_intent_in_logs(
        &self,
        intent_id: &str,
        updated_intent: crate::models::Intent,
        trackers: &std::collections::HashMap<String, String>,
    ) -> Result<usize> {
        let logs_with_intent = self.find_logs_with_intent(intent_id)?;
        let mut total_updated = 0;

        for (date, _) in logs_with_intent {
            if let Ok(Some(log)) = self.get_log(date) {
                let (updated_log, count) = log.update_intent(intent_id, updated_intent.clone());

                if count > 0 {
                    self.write_log(&updated_log, trackers)?;
                    total_updated += count;
                }
            }
        }

        Ok(total_updated)
    }

    /// Replace a field value across all log sessions
    ///
    /// Updates all sessions' embedded intent fields
    ///
    /// # Arguments
    /// * `field` - The field to update (role, objective, action, subject)
    /// * `old_value` - The value to replace
    /// * `new_value` - The new value
    /// * `trackers` - Tracker mappings for reformatting
    ///
    /// # Returns
    /// Tuple of (logs_updated, sessions_updated)
    pub fn replace_field_in_all_logs(
        &self,
        field: &str,
        old_value: &str,
        new_value: &str,
        trackers: &std::collections::HashMap<String, String>,
    ) -> Result<(usize, usize)> {
        let log_dir = self.storage.log_dir();
        let entries = std::fs::read_dir(&log_dir)
            .with_context(|| format!("Failed to read log directory: {}", log_dir.display()))?;

        let mut logs_updated = 0;
        let mut sessions_updated = 0;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Skip non-TOML files
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }

            // Extract date from filename (YYYY-MM-DD.toml)
            let date_str = path.file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid log filename"))?;
            let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .with_context(|| format!("Failed to parse date from filename: {}", date_str))?;

            // Load log
            let log = match self.get_log(date)? {
                Some(log) => log,
                None => continue,
            };

            let mut log_modified = false;
            let mut updated_timeline = Vec::new();

            // Update sessions
            for session in &log.timeline {
                let intent_field_value = match field {
                    "role" => &session.intent.role,
                    "objective" => &session.intent.objective,
                    "action" => &session.intent.action,
                    "subject" => &session.intent.subject,
                    _ => return Err(anyhow::anyhow!("Unsupported field: {}", field)),
                };

                if intent_field_value.as_ref().map(|s| s.as_str()) == Some(old_value) {
                    // Create updated intent
                    let updated_intent = crate::models::intent::Intent::new(
                        session.intent.alias.clone(),
                        if field == "role" { Some(new_value.to_string()) } else { session.intent.role.clone() },
                        if field == "objective" { Some(new_value.to_string()) } else { session.intent.objective.clone() },
                        if field == "action" { Some(new_value.to_string()) } else { session.intent.action.clone() },
                        if field == "subject" { Some(new_value.to_string()) } else { session.intent.subject.clone() },
                        session.intent.trackers.clone(),
                    );

                    // Create updated session
                    let updated_session = crate::models::session::Session {
                        intent: updated_intent,
                        start: session.start,
                        end: session.end,
                        note: session.note.clone(),
                        reflection_score: session.reflection_score,
                        reflection: session.reflection.clone(),
                    };

                    updated_timeline.push(updated_session);
                    sessions_updated += 1;
                    log_modified = true;
                } else {
                    updated_timeline.push(session.clone());
                }
            }

            if log_modified {
                // Create updated log
                let updated_log = crate::models::log::Log::new(
                    log.date,
                    log.timezone,
                    updated_timeline,
                );
                self.write_log(&updated_log, trackers)?;
                logs_updated += 1;
            }
        }

        Ok((logs_updated, sessions_updated))
    }

    /// Get usage statistics for a field across all logs
    ///
    /// Returns tuple of:
    /// - HashMap of field value -> session count
    /// - HashMap of field value -> set of log dates
    pub fn get_field_usage_stats(
        &self,
        field: &str,
    ) -> Result<(HashMap<String, usize>, HashMap<String, std::collections::HashSet<chrono::NaiveDate>>)> {
        let log_dir = self.storage.log_dir();
        let entries = std::fs::read_dir(&log_dir)
            .with_context(|| format!("Failed to read log directory: {}", log_dir.display()))?;

        let mut session_count: HashMap<String, usize> = HashMap::new();
        let mut log_dates: HashMap<String, std::collections::HashSet<chrono::NaiveDate>> = HashMap::new();

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Skip non-TOML files
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }

            // Extract date from filename
            let date_str = path.file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid log filename"))?;
            let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .with_context(|| format!("Failed to parse date from filename: {}", date_str))?;

            // Load log
            let log = match self.get_log(date)? {
                Some(log) => log,
                None => continue,
            };

            // Count sessions
            for session in &log.timeline {
                let session_field_value = match field {
                    "role" => &session.intent.role,
                    "objective" => &session.intent.objective,
                    "action" => &session.intent.action,
                    "subject" => &session.intent.subject,
                    "tracker" => {
                        // Trackers are a list, count each one
                        for tracker in &session.intent.trackers {
                            *session_count.entry(tracker.clone()).or_insert(0) += 1;
                            log_dates.entry(tracker.clone()).or_insert_with(std::collections::HashSet::new).insert(date);
                        }
                        continue;
                    }
                    _ => return Err(anyhow::anyhow!("Unsupported field: {}", field)),
                };

                if let Some(value) = session_field_value {
                    *session_count.entry(value.clone()).or_insert(0) += 1;
                    log_dates.entry(value.clone()).or_insert_with(std::collections::HashSet::new).insert(date);
                }
            }
        }

        Ok((session_count, log_dates))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::mock_storage::MockStorage;

    #[test]
    fn test_log_exists() {
        let storage = Arc::new(MockStorage::new());
        let manager = LogManager::new(storage.clone(), chrono_tz::UTC);

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        assert!(!manager.log_exists(date));

        // Write a log
        manager
            .write_log_raw(date, "date = \"2025-03-15\"\n")
            .unwrap();
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
    fn test_list_logs() {
        let storage = Arc::new(MockStorage::new());
        let manager = LogManager::new(storage, chrono_tz::UTC);

        let date1 = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let date2 = NaiveDate::from_ymd_opt(2025, 3, 16).unwrap();

        manager.write_log_raw(date1, "test").unwrap();
        manager.write_log_raw(date2, "test").unwrap();

        let dates = manager.list_logs().unwrap();
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
        let log = manager.get_log(date).unwrap().unwrap();

        assert_eq!(log.date, date);
        assert_eq!(log.timezone, chrono_tz::UTC);
        assert_eq!(log.timeline.len(), 1);

        let session = &log.timeline[0];
        assert_eq!(session.intent.alias.as_ref().unwrap(), "work");
        assert_eq!(session.intent.role.as_ref().unwrap(), "dev");
        assert_eq!(session.note.as_ref().unwrap(), "Morning session");
    }

    #[test]
    fn test_get_log_returns_none_when_missing() {
        let storage = Arc::new(MockStorage::new());
        let manager = LogManager::new(storage, chrono_tz::UTC);

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let log = manager.get_log(date).unwrap();

        assert!(log.is_none());
    }

    #[test]
    fn test_get_log_or_create() {
        let storage = Arc::new(MockStorage::new());
        let manager = LogManager::new(storage, chrono_tz::UTC);

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let log = manager.get_log_or_create(date).unwrap();

        assert_eq!(log.date, date);
        assert_eq!(log.timeline.len(), 0);
    }
}
