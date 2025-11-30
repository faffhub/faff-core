use anyhow::{Context, Result};
use chrono::{Datelike, Duration, NaiveDate, TimeZone};
use chrono_tz::Tz;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

use crate::models::Log;
use crate::storage::Storage;
use crate::workspace::Workspace;

/// Manages log file operations.
///
/// Handles reading, writing, listing, and deleting daily logs.
/// Now includes ledger-wide awareness to handle midnight crossing sessions.
#[derive(Clone)]
pub struct LogManager {
    storage: Arc<dyn Storage>,
    timezone: Tz,
    workspace: Weak<Workspace>,
}

impl LogManager {
    pub fn new(storage: Arc<dyn Storage>, timezone: Tz, workspace: Weak<Workspace>) -> Self {
        Self {
            storage,
            timezone,
            workspace,
        }
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
    pub async fn read_log_raw(&self, date: NaiveDate) -> Result<String> {
        let log_path = self.storage.log_file_path(date);
        self.storage
            .read_string(&log_path)
            .await
            .context(format!("Failed to read log file for {date}"))
    }

    /// Write raw log file contents
    pub async fn write_log_raw(&self, date: NaiveDate, contents: &str) -> Result<()> {
        let log_path = self.storage.log_file_path(date);
        self.storage
            .write_string(&log_path, contents)
            .await
            .context(format!("Failed to write log for {date}"))
    }

    /// Get timezone for creating empty logs
    pub fn timezone(&self) -> Tz {
        self.timezone
    }

    /// Get a log for a given date
    ///
    /// Returns the log if it exists, or an empty in-memory log if it doesn't.
    ///
    /// Materialization: Only happens when the requested date is TODAY (in the configured timezone).
    /// If yesterday has an unclosed session and today has no log file, this will automatically
    /// close yesterday's session at 23:59 and create today's log starting at 00:00 with a
    /// continuation of that session. Historical and future dates are never materialized.
    pub async fn get_log(&self, date: NaiveDate) -> Result<Log> {
        let log_path = self.storage.log_file_path(date);

        if self.storage.exists(&log_path) {
            // Log file exists, read and return it
            let toml_str = self
                .storage
                .read_string(&log_path)
                .await
                .with_context(|| format!("Failed to read log file for {date}"))?;

            let log = Log::from_log_file(&toml_str)
                .with_context(|| format!("Failed to parse log file for {date}"))?;

            Ok(log)
        } else {
            // Log file doesn't exist - check if we should materialize a continuation
            // Only materialize if the requested date is today
            let ws = self
                .workspace
                .upgrade()
                .ok_or_else(|| anyhow::anyhow!("Workspace no longer available"))?;
            let is_today = date == ws.today();

            if is_today {
                let yesterday = date - Duration::days(1);
                let yesterday_path = self.storage.log_file_path(yesterday);

                if self.storage.exists(&yesterday_path) {
                    // Yesterday's file exists - check for unclosed session
                    let yesterday_toml = self.storage.read_string(&yesterday_path).await?;
                    let yesterday_log = Log::from_log_file(&yesterday_toml)?;

                    if let Some(unclosed_session) = yesterday_log.active_session() {
                        // Found an unclosed session in yesterday's log - materialize the continuation
                        let unclosed_session_clone = unclosed_session.clone();
                        self.materialize_continuation(
                            date,
                            yesterday,
                            yesterday_log,
                            &unclosed_session_clone,
                        )
                        .await?;

                        // Now read the materialized log for today
                        let toml_str = self.storage.read_string(&log_path).await?;
                        let log = Log::from_log_file(&toml_str)?;
                        return Ok(log);
                    }
                }
            }

            // No file and no continuation needed - return empty in-memory log
            Ok(Log::new(date, self.timezone, vec![]))
        }
    }

    /// Materialize a continuation session from yesterday to today
    ///
    /// This closes yesterday's unclosed session at 23:59 and creates today's log
    /// with a continuation session starting at 00:00.
    ///
    /// Normally called automatically by get_log() when the date is today.
    /// Public for testing purposes.
    pub async fn materialize_continuation(
        &self,
        today: NaiveDate,
        yesterday: NaiveDate,
        yesterday_log: Log,
        unclosed_session: &crate::models::Session,
    ) -> Result<()> {
        // Get workspace and trackers from plan manager for proper TOML formatting
        let ws = self
            .workspace
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Workspace no longer available"))?;
        let yesterday_trackers = ws.plans().get_trackers(yesterday).await?;
        let today_trackers = ws.plans().get_trackers(today).await?;

        // Close yesterday's session at 23:59
        let end_of_day = self
            .timezone
            .with_ymd_and_hms(
                yesterday.year(),
                yesterday.month(),
                yesterday.day(),
                23,
                59,
                0,
            )
            .single()
            .context("Failed to create end of day timestamp")?;

        let closed_yesterday_log = yesterday_log.stop_active_session(end_of_day)?;

        // Create today's log with continuation session at 00:00
        let start_of_day = self
            .timezone
            .with_ymd_and_hms(today.year(), today.month(), today.day(), 0, 0, 0)
            .single()
            .context("Failed to create start of day timestamp")?;

        let continuation_session = crate::models::Session::new(
            unclosed_session.intent.clone(),
            start_of_day,
            None, // Unclosed
            unclosed_session.note.clone(),
        );

        let today_log = Log::new(today, self.timezone, vec![continuation_session]);

        // Write both files
        self.write_log(&closed_yesterday_log, &yesterday_trackers)
            .await?;
        self.write_log(&today_log, &today_trackers).await?;

        Ok(())
    }

    /// Write a log to storage
    ///
    /// trackers: map of tracker IDs to human-readable names for comments
    pub async fn write_log(
        &self,
        log: &Log,
        trackers: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let log_contents = log.to_log_file(trackers);
        let log_path = self.storage.log_file_path(log.date);

        self.storage
            .write_string(&log_path, &log_contents)
            .await
            .context(format!("Failed to write log for {}", log.date))
    }

    /// List all log dates in storage
    pub async fn list_logs(&self) -> Result<Vec<NaiveDate>> {
        let log_dir = self.storage.log_dir();
        let files = self
            .storage
            .list_files(&log_dir, "*.toml")
            .await
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
    pub async fn delete_log(&self, date: NaiveDate) -> Result<()> {
        let log_path = self.storage.log_file_path(date);

        if !self.storage.exists(&log_path) {
            anyhow::bail!("Log for {} does not exist", date);
        }

        self.storage
            .delete(&log_path)
            .await
            .with_context(|| format!("Failed to delete log for {date}"))
    }

    /// Start a new session with the given intent at a specific time
    ///
    /// Validates that:
    /// - start_time is not in the future (relative to `now`)
    /// - start_time doesn't conflict with existing sessions
    ///
    /// If there's an active session, it will be stopped at `start_time` before
    /// starting the new session.
    pub async fn start_intent(
        &self,
        intent: crate::models::Intent,
        start_time: chrono::DateTime<Tz>,
        note: Option<String>,
    ) -> Result<()> {
        // Get workspace context
        let ws = self
            .workspace
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Workspace no longer available"))?;

        let current_date = start_time.date_naive();
        let now = ws.now();
        let trackers = ws.plans().get_trackers(current_date).await?;
        // Get today's log (returns empty log if file doesn't exist)
        let mut log = self.get_log(current_date).await?;

        // Validate start time is not in the future
        if start_time > now {
            anyhow::bail!(
                "Cannot start session in the future: {} is after current time {}",
                start_time.format("%H:%M:%S"),
                now.format("%H:%M:%S")
            );
        }

        // Validate against existing timeline
        if let Some(last_session) = log.timeline.last() {
            if last_session.end.is_none() {
                // Active session - start_time must be after its start
                if start_time < last_session.start {
                    anyhow::bail!(
                        "Cannot start at {}. Active session started at {}. Start time must be after the current session started.",
                        start_time.format("%H:%M:%S"),
                        last_session.start.format("%H:%M:%S")
                    );
                }
                // Stop the active session at start_time
                log = log.stop_active_session(start_time)?;
            } else {
                // No active session - start_time must be after the last session's end
                let last_end = last_session.end.unwrap();
                if start_time < last_end {
                    anyhow::bail!(
                        "Cannot start at {}. Previous session ended at {}.",
                        start_time.format("%H:%M:%S"),
                        last_end.format("%H:%M:%S")
                    );
                }
            }
        }

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
        let session = crate::models::Session::new(intent, start_time, None, note);

        // Append to log and write
        let updated_log = log.append_session(session)?;
        self.write_log(&updated_log, &trackers).await?;

        Ok(())
    }

    /// Stop the currently active session
    ///
    /// Returns Ok(()) if a session was stopped, or an error if no active session exists
    pub async fn stop_current_session(&self) -> Result<()> {
        // Get workspace context
        let ws = self
            .workspace
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Workspace no longer available"))?;

        let current_date = ws.today();
        let current_time = ws.now();
        let trackers = ws.plans().get_trackers(current_date).await?;

        let log = self.get_log(current_date).await?;

        if log.active_session().is_some() {
            let updated_log = log.stop_active_session(current_time)?;
            self.write_log(&updated_log, &trackers).await?;
            Ok(())
        } else {
            anyhow::bail!("No active session to stop")
        }
    }

    /// Find all logs that contain sessions using the given intent
    ///
    /// Returns a list of (date, session_count) tuples
    pub async fn find_logs_with_intent(&self, intent_id: &str) -> Result<Vec<(NaiveDate, usize)>> {
        let all_dates = self.list_logs().await?;
        let mut logs_with_intent = Vec::new();

        for date in all_dates {
            if let Ok(log) = self.get_log(date).await {
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
    pub async fn update_intent_in_logs(
        &self,
        intent_id: &str,
        updated_intent: crate::models::Intent,
        trackers: &std::collections::HashMap<String, String>,
    ) -> Result<usize> {
        let logs_with_intent = self.find_logs_with_intent(intent_id).await?;
        let mut total_updated = 0;

        for (date, _) in logs_with_intent {
            if let Ok(log) = self.get_log(date).await {
                let (updated_log, count) = log.update_intent(intent_id, updated_intent.clone());

                if count > 0 {
                    self.write_log(&updated_log, trackers).await?;
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
    pub async fn replace_field_in_all_logs(
        &self,
        field: &str,
        old_value: &str,
        new_value: &str,
        trackers: &std::collections::HashMap<String, String>,
    ) -> Result<(usize, usize)> {
        let all_dates = self.list_logs().await?;

        let mut logs_updated = 0;
        let mut sessions_updated = 0;

        for date in all_dates {
            // Load log
            let log = self.get_log(date).await?;

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
                        if field == "role" {
                            Some(new_value.to_string())
                        } else {
                            session.intent.role.clone()
                        },
                        if field == "objective" {
                            Some(new_value.to_string())
                        } else {
                            session.intent.objective.clone()
                        },
                        if field == "action" {
                            Some(new_value.to_string())
                        } else {
                            session.intent.action.clone()
                        },
                        if field == "subject" {
                            Some(new_value.to_string())
                        } else {
                            session.intent.subject.clone()
                        },
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
                let updated_log =
                    crate::models::log::Log::new(log.date, log.timezone, updated_timeline);
                self.write_log(&updated_log, trackers).await?;
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
    pub async fn get_field_usage_stats(
        &self,
        field: &str,
    ) -> Result<(
        HashMap<String, usize>,
        HashMap<String, std::collections::HashSet<chrono::NaiveDate>>,
    )> {
        let all_dates = self.list_logs().await?;

        let mut session_count: HashMap<String, usize> = HashMap::new();
        let mut log_dates: HashMap<String, std::collections::HashSet<chrono::NaiveDate>> =
            HashMap::new();

        for date in all_dates {
            // Load log
            let log = self.get_log(date).await?;

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
                            log_dates.entry(tracker.clone()).or_default().insert(date);
                        }
                        continue;
                    }
                    _ => return Err(anyhow::anyhow!("Unsupported field: {}", field)),
                };

                if let Some(value) = session_field_value {
                    *session_count.entry(value.clone()).or_insert(0) += 1;
                    log_dates.entry(value.clone()).or_default().insert(date);
                }
            }
        }

        Ok((session_count, log_dates))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::mock_storage::MockStorage;
    use crate::workspace::Workspace;
    use std::path::PathBuf;

    // Helper function to create a Workspace with LogManager for tests
    async fn create_test_workspace(storage: Arc<MockStorage>) -> Arc<Workspace> {
        // Add a config file to storage
        storage.add_file(
            PathBuf::from("/faff/config.toml"),
            r#"timezone = "UTC""#.to_string(),
        );
        Workspace::with_storage(storage).await.unwrap()
    }

    #[tokio::test]
    async fn test_log_exists() {
        let storage = Arc::new(MockStorage::new());
        let ws = create_test_workspace(storage.clone()).await;

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        assert!(!ws.logs().log_exists(date));

        // Write a log
        ws.logs()
            .write_log_raw(date, "date = \"2025-03-15\"\n")
            .await
            .unwrap();
        assert!(ws.logs().log_exists(date));
    }

    #[tokio::test]
    async fn test_write_and_read_raw() {
        let storage = Arc::new(MockStorage::new());
        let ws = create_test_workspace(storage.clone()).await;

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let content = "date = \"2025-03-15\"\ntimezone = \"UTC\"\n";

        ws.logs().write_log_raw(date, content).await.unwrap();
        let retrieved = ws.logs().read_log_raw(date).await.unwrap();

        assert_eq!(retrieved, content);
    }

    #[tokio::test]
    async fn test_list_logs() {
        let storage = Arc::new(MockStorage::new());
        let ws = create_test_workspace(storage.clone()).await;

        let date1 = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let date2 = NaiveDate::from_ymd_opt(2025, 3, 16).unwrap();

        ws.logs().write_log_raw(date1, "test").await.unwrap();
        ws.logs().write_log_raw(date2, "test").await.unwrap();

        let dates = ws.logs().list_logs().await.unwrap();
        assert_eq!(dates.len(), 2);
        assert_eq!(dates[0], date1);
        assert_eq!(dates[1], date2);
    }

    #[tokio::test]
    async fn test_get_log_parses_toml() {
        let storage = Arc::new(MockStorage::new());
        let ws = create_test_workspace(storage.clone()).await;

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

        ws.logs().write_log_raw(date, toml_content).await.unwrap();
        let log = ws.logs().get_log(date).await.unwrap();

        assert_eq!(log.date, date);
        assert_eq!(log.timezone, chrono_tz::UTC);
        assert_eq!(log.timeline.len(), 1);

        let session = &log.timeline[0];
        assert_eq!(session.intent.alias.as_ref().unwrap(), "work");
        assert_eq!(session.intent.role.as_ref().unwrap(), "dev");
        assert_eq!(session.note.as_ref().unwrap(), "Morning session");
    }

    #[tokio::test]
    async fn test_get_log_returns_empty_when_missing() {
        let storage = Arc::new(MockStorage::new());
        let ws = create_test_workspace(storage.clone()).await;

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let log = ws.logs().get_log(date).await.unwrap();

        // Should return an empty log when file doesn't exist
        assert_eq!(log.date, date);
        assert_eq!(log.timeline.len(), 0);
    }

    #[tokio::test]
    async fn test_midnight_crossing_continuation() {
        use crate::models::{Intent, Session};
        use chrono::{TimeZone, Timelike};

        let storage = Arc::new(MockStorage::new());
        let ws = create_test_workspace(storage.clone()).await;

        let yesterday = NaiveDate::from_ymd_opt(2025, 3, 14).unwrap();
        let today = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();

        // Create an unclosed session yesterday at 23:30
        let intent = Intent::new(
            Some("work".to_string()),
            Some("dev".to_string()),
            Some("project".to_string()),
            Some("coding".to_string()),
            Some("api".to_string()),
            vec![],
        );

        let session_start = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 14, 23, 30, 0)
            .unwrap();
        let unclosed_session = Session::new(
            intent.clone(),
            session_start,
            None,
            Some("late night coding".to_string()),
        );

        let yesterday_log =
            crate::models::Log::new(yesterday, chrono_tz::UTC, vec![unclosed_session]);

        // Write yesterday's log with unclosed session
        ws.logs()
            .write_log(&yesterday_log, &std::collections::HashMap::new())
            .await
            .unwrap();

        // Manually call materialize_continuation for testing
        // (normally this only happens when date == workspace.today())
        let unclosed = yesterday_log.active_session().unwrap().clone();
        ws.logs()
            .materialize_continuation(today, yesterday, yesterday_log, &unclosed)
            .await
            .unwrap();

        // Now get_log should return the materialized log
        let today_log = ws.logs().get_log(today).await.unwrap();

        // Verify today's log has one session starting at 00:00
        assert_eq!(today_log.timeline.len(), 1);
        let today_session = &today_log.timeline[0];
        assert_eq!(today_session.intent.alias, Some("work".to_string()));
        assert_eq!(today_session.start.hour(), 0);
        assert_eq!(today_session.start.minute(), 0);
        assert!(
            today_session.end.is_none(),
            "Today's session should be unclosed"
        );
        assert_eq!(today_session.note, Some("late night coding".to_string()));

        // Verify yesterday's log was closed at 23:59
        let yesterday_log_closed = ws.logs().get_log(yesterday).await.unwrap();
        assert_eq!(yesterday_log_closed.timeline.len(), 1);
        let yesterday_session_closed = &yesterday_log_closed.timeline[0];
        assert!(
            yesterday_session_closed.end.is_some(),
            "Yesterday's session should be closed"
        );
        let end_time = yesterday_session_closed.end.unwrap();
        assert_eq!(end_time.hour(), 23);
        assert_eq!(end_time.minute(), 59);
    }

    #[tokio::test]
    async fn test_start_intent_validation_future_time() {
        use crate::models::Intent;
        use chrono::{Duration, Utc};

        let storage = Arc::new(MockStorage::new());
        let ws = create_test_workspace(storage.clone()).await;

        // Create a time that is definitely in the future (1 hour from now)
        let future = Utc::now().with_timezone(&chrono_tz::UTC) + Duration::hours(1);

        let intent = Intent::new(Some("work".to_string()), None, None, None, None, vec![]);

        let result = ws.logs().start_intent(intent, future, None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("future"));
    }

    #[tokio::test]
    async fn test_start_intent_validation_overlap_active_session() {
        use crate::models::{Intent, Session};
        use chrono::TimeZone;

        let storage = Arc::new(MockStorage::new());
        let ws = create_test_workspace(storage.clone()).await;

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();

        // Create a log with an active session starting at 10:00
        let intent = Intent::new(Some("existing".to_string()), None, None, None, None, vec![]);
        let session_start = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 10, 0, 0)
            .unwrap();
        let session = Session::new(intent, session_start, None, None);
        let log = crate::models::Log::new(date, chrono_tz::UTC, vec![session]);
        ws.logs().write_log(&log, &HashMap::new()).await.unwrap();

        // Try to start a new session at 09:00 (before active session started)
        let new_intent = Intent::new(Some("new".to_string()), None, None, None, None, vec![]);
        let bad_start = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 9, 0, 0)
            .unwrap();

        let result = ws.logs().start_intent(new_intent, bad_start, None).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Active session"));
    }

    #[tokio::test]
    async fn test_start_intent_validation_overlap_completed_session() {
        use crate::models::{Intent, Session};
        use chrono::TimeZone;

        let storage = Arc::new(MockStorage::new());
        let ws = create_test_workspace(storage.clone()).await;

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();

        // Create a log with a completed session from 09:00 to 10:00
        let intent = Intent::new(Some("existing".to_string()), None, None, None, None, vec![]);
        let session_start = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 9, 0, 0)
            .unwrap();
        let session_end = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 10, 0, 0)
            .unwrap();
        let session = Session::new(intent, session_start, Some(session_end), None);
        let log = crate::models::Log::new(date, chrono_tz::UTC, vec![session]);
        ws.logs().write_log(&log, &HashMap::new()).await.unwrap();

        // Try to start at 09:30 (overlapping completed session)
        let new_intent = Intent::new(Some("new".to_string()), None, None, None, None, vec![]);
        let bad_start = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 9, 30, 0)
            .unwrap();

        let result = ws.logs().start_intent(new_intent, bad_start, None).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Previous session ended"));
    }

    #[tokio::test]
    async fn test_start_intent_stops_active_session() {
        use crate::models::{Intent, Session};
        use chrono::{TimeZone, Timelike};

        let storage = Arc::new(MockStorage::new());
        let ws = create_test_workspace(storage.clone()).await;

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();

        // Create a log with an active session starting at 09:00
        let intent = Intent::new(Some("existing".to_string()), None, None, None, None, vec![]);
        let session_start = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 9, 0, 0)
            .unwrap();
        let session = Session::new(intent, session_start, None, None);
        let log = crate::models::Log::new(date, chrono_tz::UTC, vec![session]);
        ws.logs().write_log(&log, &HashMap::new()).await.unwrap();

        // Start a new session at 11:00 (after active session started)
        let new_intent = Intent::new(Some("new".to_string()), None, None, None, None, vec![]);
        let new_start = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 11, 0, 0)
            .unwrap();

        ws.logs()
            .start_intent(new_intent, new_start, None)
            .await
            .unwrap();

        // Verify the first session was stopped and second started
        let log = ws.logs().get_log(date).await.unwrap();
        assert_eq!(log.timeline.len(), 2);

        // First session should be closed at 11:00
        assert_eq!(log.timeline[0].intent.alias, Some("existing".to_string()));
        assert!(log.timeline[0].end.is_some());
        assert_eq!(log.timeline[0].end.unwrap().hour(), 11);

        // Second session should be active
        assert_eq!(log.timeline[1].intent.alias, Some("new".to_string()));
        assert!(log.timeline[1].end.is_none());
    }
}
