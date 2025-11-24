use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone};
use chrono_tz::Tz;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::LazyLock;
use thiserror::Error;

use crate::models::intent::Intent;
use crate::models::session::Session;

// Compiled regex for commentifying derived values - validated at compile time
static DERIVED_VALUE_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?m)^--([a-zA-Z_-][a-zA-Z0-9_-]*\s*=\s*.+)$")
        .expect("DERIVED_VALUE_REGEX pattern is valid")
});

#[derive(Error, Debug)]
pub enum LogError {
    #[error("No timeline entries to stop")]
    NoTimelineEntries,
    #[error("Invalid time value: {0}")]
    InvalidTime(String),
    #[error("Ambiguous datetime during DST transition: {0}")]
    AmbiguousDatetime(String),
}

/// Summary statistics for a log
#[derive(Clone, Debug, PartialEq)]
pub struct LogSummary {
    /// Total recorded time in minutes
    pub total_minutes: i64,
    /// Time by intent alias in minutes
    pub by_intent: HashMap<String, i64>,
    /// Time by tracker in minutes
    pub by_tracker: HashMap<String, i64>,
    /// Time by tracker source (prefix before ':') in minutes
    pub by_tracker_source: HashMap<String, i64>,
    /// Weighted mean reflection score (if any sessions have scores)
    pub mean_reflection_score: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Log {
    pub date: NaiveDate,
    pub timezone: Tz,
    pub timeline: Vec<Session>,
}

impl Log {
    pub fn new(date: NaiveDate, timezone: Tz, timeline: Vec<Session>) -> Self {
        Self {
            date,
            timezone,
            timeline,
        }
    }

    /// Calculate SHA256 hash of raw TOML content
    pub fn calculate_hash(toml_content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(toml_content.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Calculate the hash of this log's TOML representation
    ///
    /// This generates the TOML and hashes it, which can be used to detect
    /// if the log has changed since a timesheet was compiled from it.
    pub fn hash(&self, trackers: &HashMap<String, String>) -> String {
        let toml_content = self.to_log_file(trackers);
        Self::calculate_hash(&toml_content)
    }

    /// Returns the active (open) session if one exists
    pub fn active_session(&self) -> Option<&Session> {
        if self.timeline.is_empty() {
            return None;
        }

        let latest = self.timeline.last()?;
        if latest.end.is_none() {
            Some(latest)
        } else {
            None
        }
    }

    /// Append a session to the timeline, automatically stopping any active session
    pub fn append_session(&self, session: Session) -> Result<Log, LogError> {
        if self.active_session().is_some() {
            let stopped_log = self.stop_active_session(session.start)?;
            stopped_log.append_session(session)
        } else {
            let mut new_timeline = self.timeline.clone();
            new_timeline.push(session);
            Ok(Log::new(self.date, self.timezone, new_timeline))
        }
    }

    /// Stop the active session at the given time
    pub fn stop_active_session(&self, stop_time: DateTime<Tz>) -> Result<Log, LogError> {
        if self.timeline.is_empty() {
            return Err(LogError::NoTimelineEntries);
        }

        let mut new_timeline = self.timeline.clone();
        let last_idx = new_timeline.len() - 1;
        new_timeline[last_idx] = new_timeline[last_idx].with_end(stop_time);

        Ok(Log::new(self.date, self.timezone, new_timeline))
    }

    /// Check if all sessions in the log are closed (have end times)
    pub fn is_closed(&self) -> bool {
        self.timeline.iter().all(|session| session.end.is_some())
    }

    /// Calculate total recorded time across all sessions
    ///
    /// Returns an error if timezone conversion fails (e.g., during DST transitions)
    pub fn total_recorded_time(&self) -> Result<Duration, LogError> {
        let mut total = Duration::zero();

        // Get today's date and current time in the log's timezone
        let today = Local::now().date_naive();
        let now = Local::now().with_timezone(&self.timezone);

        for session in &self.timeline {
            let start = session.start;
            let duration = match session.end {
                Some(end) => end - start,
                None => {
                    if self.date == today {
                        // For open sessions on today, use current time
                        now - start
                    } else {
                        // For open sessions on past dates, use end of day
                        let end_of_day_time = NaiveTime::from_hms_opt(23, 59, 59)
                            .ok_or_else(|| LogError::InvalidTime("23:59:59".to_string()))?;
                        let end_of_day_naive = self.date.and_time(end_of_day_time);
                        let end_of_day = self
                            .timezone
                            .from_local_datetime(&end_of_day_naive)
                            .single()
                            .ok_or_else(|| {
                                LogError::AmbiguousDatetime(format!(
                                    "{} in {}",
                                    end_of_day_naive, self.timezone
                                ))
                            })?;
                        end_of_day - start
                    }
                }
            };

            total += duration;
        }

        Ok(total)
    }

    /// Parse a Log from Faffage log file format (TOML)
    pub fn from_log_file(toml_str: &str) -> anyhow::Result<Self> {
        let toml_value: toml::Value = toml::from_str(toml_str)?;

        // Extract date and timezone
        let date_str = toml_value
            .get("date")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'date' field"))?;
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;

        let tz_str = toml_value
            .get("timezone")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'timezone' field"))?;
        let timezone: Tz = tz_str
            .parse()
            .map_err(|e: String| anyhow::anyhow!("Invalid timezone '{}': {}", tz_str, e))?;

        // Parse timeline sessions using Session's from_toml_table method
        let mut sessions = Vec::new();
        if let Some(timeline) = toml_value.get("timeline").and_then(|v| v.as_array()) {
            for entry in timeline {
                if let Some(table) = entry.as_table() {
                    sessions.push(Session::from_toml_table(table, date, timezone)?);
                }
            }
        }

        Ok(Log::new(date, timezone, sessions))
    }

    /// Serialize the Log to Faffage log file format (TOML with comments and formatting)
    ///
    /// trackers: map of tracker IDs to human-readable names for comments
    pub fn to_log_file(&self, trackers: &HashMap<String, String>) -> String {
        let mut lines = vec![
            "# This is a Faff-format log file - see faffage.com for details.".to_string(),
            "# It has been generated but can be edited manually.".to_string(),
            "# Changes to rows starting with '#' will not be saved.".to_string(),
            "version = \"1.1\"".to_string(),
        ];

        // Date with day of week comment
        let day_of_week = self.date.format("%A").to_string();
        lines.push(format!("date = \"{}\" # {}", self.date, day_of_week));

        lines.push(format!("timezone = \"{}\"", self.timezone));

        // Date format hint (derived value, becomes comment)
        let date_format = Self::get_datetime_format(self.date, self.timezone);
        lines.push(format!("--date_format = \"{date_format}\""));

        // Timeline entries
        if self.timeline.is_empty() {
            lines.push("".to_string());
            lines.push("# Timeline is empty.".to_string());
        } else {
            // Sort by start time
            let mut sorted_timeline = self.timeline.clone();
            sorted_timeline.sort_by_key(|s| s.start);

            for session in &sorted_timeline {
                lines.push("".to_string());
                lines.push("[[timeline]]".to_string());

                Self::format_session_to_toml(&mut lines, session, trackers, &date_format);
            }
        }

        let toml_string = lines.join("\n");

        // Post-process: commentify derived values first, then align equals signs
        let commented = Self::commentify_derived_values(&toml_string);
        Self::align_equals(&commented)
    }

    fn format_session_to_toml(
        lines: &mut Vec<String>,
        session: &Session,
        trackers: &HashMap<String, String>,
        date_format: &str,
    ) {
        // Intent ID
        lines.push(format!("intent_id = \"{}\"", session.intent.intent_id));

        // Alias
        if let Some(alias) = &session.intent.alias {
            lines.push(format!("alias = \"{alias}\""));
        }

        // Optional intent fields
        if let Some(role) = &session.intent.role {
            lines.push(format!("role = \"{role}\""));
        }
        if let Some(objective) = &session.intent.objective {
            lines.push(format!("objective = \"{objective}\""));
        }
        if let Some(action) = &session.intent.action {
            lines.push(format!("action = \"{action}\""));
        }
        if let Some(subject) = &session.intent.subject {
            lines.push(format!("subject = \"{subject}\""));
        }

        // Trackers
        let tracker_list = &session.intent.trackers;
        if !tracker_list.is_empty() {
            if tracker_list.len() == 1 {
                let tracker = &tracker_list[0];
                if let Some(name) = trackers.get(tracker) {
                    lines.push(format!("trackers = \"{tracker}\" # {name}"));
                } else {
                    lines.push(format!("trackers = \"{tracker}\""));
                }
            } else {
                lines.push("trackers = [".to_string());
                for tracker in tracker_list {
                    if let Some(name) = trackers.get(tracker) {
                        lines.push(format!("   \"{tracker}\", # {name}"));
                    } else {
                        lines.push(format!("   \"{tracker}\","));
                    }
                }
                lines.push("]".to_string());
            }
        }

        // Start time
        let start_str = Self::format_datetime_for_log(&session.start, date_format);
        lines.push(format!("start = \"{start_str}\""));

        // End time and duration
        if let Some(end) = session.end {
            let end_str = Self::format_datetime_for_log(&end, date_format);
            lines.push(format!("end = \"{end_str}\""));

            // Duration (derived value, becomes comment)
            let duration = end - session.start;
            let duration_str = Self::format_duration(duration);
            lines.push(format!("--duration = \"{duration_str}\""));
        }

        // Note (only include if non-empty)
        if let Some(note) = &session.note {
            if !note.is_empty() {
                lines.push(format!("note = \"{note}\""));
            }
        }

        // Reflection fields (only include if present)
        if let Some(score) = session.reflection_score {
            lines.push(format!("reflection_score = {score}"));
        }
        if let Some(reflection) = &session.reflection {
            if !reflection.is_empty() {
                lines.push(format!("reflection = \"{reflection}\""));
            }
        }
    }

    fn format_datetime_for_log(dt: &DateTime<Tz>, format: &str) -> String {
        if format == "HH:mmZ" {
            // Include timezone offset
            dt.format("%H:%M%z").to_string()
        } else {
            // Just time, no offset
            dt.format("%H:%M").to_string()
        }
    }

    fn format_duration(duration: Duration) -> String {
        let total_seconds = duration.num_seconds();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        let hour_str = if hours == 1 { "hour" } else { "hours" };
        let minute_str = if minutes == 1 { "minute" } else { "minutes" };
        let second_str = if seconds == 1 { "second" } else { "seconds" };

        if hours > 0 {
            if minutes > 0 {
                if seconds > 0 {
                    format!("{hours} {hour_str}, {minutes} {minute_str} and {seconds} {second_str}")
                } else {
                    format!("{hours} {hour_str} and {minutes} {minute_str}")
                }
            } else if seconds > 0 {
                format!("{hours} {hour_str} and {seconds} {second_str}")
            } else {
                format!("{hours} {hour_str}")
            }
        } else if minutes > 0 {
            if seconds > 0 {
                format!("{minutes} {minute_str} and {seconds} {second_str}")
            } else {
                format!("{minutes} {minute_str}")
            }
        } else {
            format!("{seconds} {second_str}")
        }
    }

    fn get_datetime_format(date: NaiveDate, timezone: Tz) -> String {
        if Self::date_has_dst_event(date, timezone) {
            "HH:mmZ".to_string()
        } else {
            "HH:mm".to_string()
        }
    }

    fn date_has_dst_event(date: NaiveDate, timezone: Tz) -> bool {
        let start = timezone
            .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
            .single();
        let end = timezone
            .with_ymd_and_hms(date.year(), date.month(), date.day(), 23, 59, 0)
            .single();

        match (start, end) {
            (Some(start_dt), Some(end_dt)) => {
                // Compare UTC offsets - if they differ, there was a DST event
                start_dt.offset() != end_dt.offset()
            }
            _ => false, // Ambiguous times during DST transition
        }
    }

    fn align_equals(toml_string: &str) -> String {
        let lines: Vec<&str> = toml_string.lines().collect();

        // Find max key length for alignment
        let mut max_key_length = 0;
        for line in &lines {
            if line.contains('=') && !line.trim_start().starts_with('#') {
                if let Some(key) = line.split('=').next() {
                    max_key_length = max_key_length.max(key.trim().len());
                }
            }
        }

        // Align the equals signs
        let mut aligned_lines = Vec::new();
        for line in lines {
            if line.contains('=') && !line.trim_start().starts_with('#') {
                let parts: Vec<&str> = line.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    let value = parts[1].trim();
                    let padding = " ".repeat(max_key_length - key.len());
                    aligned_lines.push(format!("{key}{padding} = {value}"));
                } else {
                    aligned_lines.push(line.to_string());
                }
            } else {
                aligned_lines.push(line.to_string());
            }
        }

        aligned_lines.join("\n")
    }

    fn commentify_derived_values(toml_string: &str) -> String {
        // Replace lines starting with '--variable_name = ' with '# variable_name = '
        DERIVED_VALUE_REGEX
            .replace_all(toml_string, "# $1")
            .to_string()
    }

    /// Update all sessions in the log that use the given intent
    ///
    /// Returns a tuple of (updated_log, count) where count is the number of sessions updated
    pub fn update_intent(&self, intent_id: &str, updated_intent: Intent) -> (Log, usize) {
        let mut new_timeline = self.timeline.clone();
        let mut count = 0;

        for session in &mut new_timeline {
            if session.intent.intent_id == intent_id {
                session.intent = updated_intent.clone();
                count += 1;
            }
        }

        (Log::new(self.date, self.timezone, new_timeline), count)
    }

    /// Calculate summary statistics for this log
    ///
    /// Uses the provided `now` time for calculating duration of open sessions on today.
    /// For open sessions on past dates, caps at end-of-day (23:59).
    /// All durations are in minutes (faff's base unit).
    pub fn summary(&self, now: DateTime<Tz>) -> LogSummary {
        let mut by_intent: HashMap<String, i64> = HashMap::new();
        let mut by_tracker: HashMap<String, i64> = HashMap::new();
        let mut by_tracker_source: HashMap<String, i64> = HashMap::new();
        let mut total_minutes: i64 = 0;

        let mut weighted_score_minutes: f64 = 0.0;
        let mut total_reflected_minutes: f64 = 0.0;

        let today = now.date_naive();

        for session in &self.timeline {
            let end = match session.end {
                Some(end) => end,
                None => {
                    if self.date == today {
                        // For open sessions on today, use current time
                        now
                    } else {
                        // For open sessions on past dates, cap at end of day
                        let end_of_day_time =
                            NaiveTime::from_hms_opt(23, 59, 0).expect("23:59:00 is valid");
                        let end_of_day_naive = self.date.and_time(end_of_day_time);
                        self.timezone
                            .from_local_datetime(&end_of_day_naive)
                            .single()
                            .unwrap_or(now) // fallback to now if DST ambiguity
                    }
                }
            };
            let duration_minutes = (end - session.start).num_minutes();

            total_minutes += duration_minutes;

            // Aggregate by intent alias
            let alias = session.intent.alias.clone().unwrap_or_default();
            *by_intent.entry(alias).or_insert(0) += duration_minutes;

            // Aggregate by tracker and tracker source
            for tracker in &session.intent.trackers {
                *by_tracker.entry(tracker.clone()).or_insert(0) += duration_minutes;

                let source = tracker.split(':').next().unwrap_or("").to_string();
                if !source.is_empty() {
                    *by_tracker_source.entry(source).or_insert(0) += duration_minutes;
                }
            }

            // Track weighted reflection score
            if let Some(score) = session.reflection_score {
                weighted_score_minutes += score as f64 * duration_minutes as f64;
                total_reflected_minutes += duration_minutes as f64;
            }
        }

        let mean_reflection_score = if total_reflected_minutes > 0.0 {
            Some(weighted_score_minutes / total_reflected_minutes)
        } else {
            None
        };

        LogSummary {
            total_minutes,
            by_intent,
            by_tracker,
            by_tracker_source,
            mean_reflection_score,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::intent::Intent;
    use chrono::TimeZone;

    fn sample_intent() -> Intent {
        Intent::new(
            Some("work".to_string()),
            Some("engineer".to_string()),
            Some("development".to_string()),
            Some("coding".to_string()),
            Some("features".to_string()),
            vec![],
        )
    }

    fn sample_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, 3, 15).unwrap()
    }

    fn london_tz() -> Tz {
        Tz::Europe__London
    }

    #[test]
    fn test_create_empty_log() {
        let log = Log::new(sample_date(), london_tz(), vec![]);
        assert_eq!(log.date, sample_date());
        assert_eq!(log.timezone, london_tz());
        assert_eq!(log.timeline.len(), 0);
    }

    #[test]
    fn test_create_log_with_session() {
        let intent = sample_intent();
        let start = london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = london_tz()
            .with_ymd_and_hms(2025, 3, 15, 10, 30, 0)
            .unwrap();
        let session = Session::new(intent, start, Some(end), None);

        let log = Log::new(sample_date(), london_tz(), vec![session.clone()]);

        assert_eq!(log.timeline.len(), 1);
        assert_eq!(log.timeline[0], session);
    }

    #[test]
    fn test_empty_log_has_no_active_session() {
        let log = Log::new(sample_date(), london_tz(), vec![]);
        assert!(log.active_session().is_none());
    }

    #[test]
    fn test_log_with_completed_session_has_no_active_session() {
        let intent = sample_intent();
        let start = london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = london_tz()
            .with_ymd_and_hms(2025, 3, 15, 10, 30, 0)
            .unwrap();
        let session = Session::new(intent, start, Some(end), None);

        let log = Log::new(sample_date(), london_tz(), vec![session]);

        assert!(log.active_session().is_none());
    }

    #[test]
    fn test_log_with_open_session_returns_it() {
        let intent = sample_intent();
        let start = london_tz().with_ymd_and_hms(2025, 3, 15, 14, 0, 0).unwrap();
        let session = Session::new(intent.clone(), start, None, None);

        let log = Log::new(sample_date(), london_tz(), vec![session.clone()]);

        let active = log.active_session();
        assert!(active.is_some());
        assert_eq!(active.unwrap().end, None);
    }

    #[test]
    fn test_only_last_session_matters_for_active() {
        let intent = sample_intent();
        let start1 = london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end1 = london_tz()
            .with_ymd_and_hms(2025, 3, 15, 10, 30, 0)
            .unwrap();
        let session1 = Session::new(intent.clone(), start1, Some(end1), None);

        let start2 = london_tz().with_ymd_and_hms(2025, 3, 15, 14, 0, 0).unwrap();
        let session2 = Session::new(intent, start2, None, None);

        let log = Log::new(sample_date(), london_tz(), vec![session1, session2.clone()]);

        let active = log.active_session();
        assert_eq!(active.unwrap(), &session2);
    }

    #[test]
    fn test_append_to_empty_log() {
        let intent = sample_intent();
        let start = london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = london_tz()
            .with_ymd_and_hms(2025, 3, 15, 10, 30, 0)
            .unwrap();
        let session = Session::new(intent, start, Some(end), None);

        let log = Log::new(sample_date(), london_tz(), vec![]);
        let new_log = log.append_session(session.clone()).unwrap();

        assert_eq!(new_log.timeline.len(), 1);
        assert_eq!(new_log.timeline[0], session);
        // Original unchanged
        assert_eq!(log.timeline.len(), 0);
    }

    #[test]
    fn test_append_to_log_with_completed_sessions() {
        let intent = sample_intent();
        let start1 = london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end1 = london_tz()
            .with_ymd_and_hms(2025, 3, 15, 10, 30, 0)
            .unwrap();
        let session1 = Session::new(intent.clone(), start1, Some(end1), None);

        let start2 = london_tz().with_ymd_and_hms(2025, 3, 15, 11, 0, 0).unwrap();
        let end2 = london_tz().with_ymd_and_hms(2025, 3, 15, 12, 0, 0).unwrap();
        let session2 = Session::new(intent, start2, Some(end2), None);

        let log = Log::new(sample_date(), london_tz(), vec![session1]);
        let new_log = log.append_session(session2.clone()).unwrap();

        assert_eq!(new_log.timeline.len(), 2);
        assert_eq!(new_log.timeline[1], session2);
    }

    #[test]
    fn test_append_automatically_stops_active_session() {
        let intent = sample_intent();
        let start1 = london_tz().with_ymd_and_hms(2025, 3, 15, 14, 0, 0).unwrap();
        let open_session = Session::new(intent.clone(), start1, None, None);

        let start2 = london_tz().with_ymd_and_hms(2025, 3, 15, 15, 0, 0).unwrap();
        let end2 = london_tz().with_ymd_and_hms(2025, 3, 15, 16, 0, 0).unwrap();
        let new_session = Session::new(intent, start2, Some(end2), None);

        let log = Log::new(sample_date(), london_tz(), vec![open_session]);
        let new_log = log.append_session(new_session.clone()).unwrap();

        assert_eq!(new_log.timeline.len(), 2);
        // First session should be stopped at start2
        assert_eq!(new_log.timeline[0].end, Some(start2));
        assert_eq!(new_log.timeline[1], new_session);
    }

    #[test]
    fn test_stop_active_session() {
        let intent = sample_intent();
        let start = london_tz().with_ymd_and_hms(2025, 3, 15, 14, 0, 0).unwrap();
        let open_session = Session::new(intent, start, None, None);

        let log = Log::new(sample_date(), london_tz(), vec![open_session]);

        let stop_time = london_tz()
            .with_ymd_and_hms(2025, 3, 15, 16, 30, 0)
            .unwrap();
        let stopped_log = log.stop_active_session(stop_time).unwrap();

        assert_eq!(stopped_log.timeline[0].end, Some(stop_time));
        // Original unchanged
        assert_eq!(log.timeline[0].end, None);
    }

    #[test]
    fn test_stop_empty_log_raises_error() {
        let log = Log::new(sample_date(), london_tz(), vec![]);
        let stop_time = london_tz()
            .with_ymd_and_hms(2025, 3, 15, 16, 30, 0)
            .unwrap();

        let result = log.stop_active_session(stop_time);
        assert!(matches!(result, Err(LogError::NoTimelineEntries)));
    }

    #[test]
    fn test_empty_log_is_closed() {
        let log = Log::new(sample_date(), london_tz(), vec![]);
        assert!(log.is_closed());
    }

    #[test]
    fn test_log_with_completed_sessions_is_closed() {
        let intent = sample_intent();
        let start = london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = london_tz()
            .with_ymd_and_hms(2025, 3, 15, 10, 30, 0)
            .unwrap();
        let session = Session::new(intent, start, Some(end), None);

        let log = Log::new(sample_date(), london_tz(), vec![session]);
        assert!(log.is_closed());
    }

    #[test]
    fn test_log_with_open_session_is_not_closed() {
        let intent = sample_intent();
        let start = london_tz().with_ymd_and_hms(2025, 3, 15, 14, 0, 0).unwrap();
        let session = Session::new(intent, start, None, None);

        let log = Log::new(sample_date(), london_tz(), vec![session]);
        assert!(!log.is_closed());
    }

    #[test]
    fn test_empty_log_has_zero_time() {
        let log = Log::new(sample_date(), london_tz(), vec![]);
        assert_eq!(log.total_recorded_time().unwrap(), Duration::zero());
    }

    #[test]
    fn test_single_completed_session() {
        let intent = sample_intent();
        let start = london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = london_tz()
            .with_ymd_and_hms(2025, 3, 15, 10, 30, 0)
            .unwrap();
        let session = Session::new(intent, start, Some(end), None);

        let log = Log::new(sample_date(), london_tz(), vec![session]);
        let expected = Duration::hours(1) + Duration::minutes(30);
        assert_eq!(log.total_recorded_time().unwrap(), expected);
    }

    #[test]
    fn test_multiple_completed_sessions() {
        let intent = sample_intent();

        let start1 = london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end1 = london_tz().with_ymd_and_hms(2025, 3, 15, 10, 0, 0).unwrap();
        let session1 = Session::new(intent.clone(), start1, Some(end1), None);

        let start2 = london_tz().with_ymd_and_hms(2025, 3, 15, 14, 0, 0).unwrap();
        let end2 = london_tz()
            .with_ymd_and_hms(2025, 3, 15, 15, 30, 0)
            .unwrap();
        let session2 = Session::new(intent, start2, Some(end2), None);

        let log = Log::new(sample_date(), london_tz(), vec![session1, session2]);

        let expected = Duration::hours(2) + Duration::minutes(30);
        assert_eq!(log.total_recorded_time().unwrap(), expected);
    }

    #[test]
    fn test_open_session_on_past_date_uses_end_of_day() {
        let intent = sample_intent();
        let past_date = NaiveDate::from_ymd_opt(2025, 3, 10).unwrap();

        let start = london_tz().with_ymd_and_hms(2025, 3, 10, 14, 0, 0).unwrap();
        let open_session = Session::new(intent, start, None, None);

        let log = Log::new(past_date, london_tz(), vec![open_session]);
        let total = log.total_recorded_time().unwrap();

        // From 14:00 to 23:59:59 = 9 hours, 59 minutes, 59 seconds
        let expected = Duration::hours(9) + Duration::minutes(59) + Duration::seconds(59);
        assert_eq!(total, expected);
    }

    #[test]
    fn test_to_log_file_empty() {
        let log = Log::new(sample_date(), chrono_tz::UTC, vec![]);
        let trackers = HashMap::new();
        let output = log.to_log_file(&trackers);

        assert!(output.contains("# This is a Faff-format log file"));
        assert!(output.contains("version  = \"1.1\""));
        assert!(output.contains("date     = \"2025-03-15\""));
        assert!(output.contains("timezone = \"UTC\""));
        assert!(output.contains("# Timeline is empty."));
    }

    #[test]
    fn test_to_log_file_with_session() {
        let intent = sample_intent();
        let start = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 9, 0, 0)
            .unwrap();
        let end = chrono_tz::UTC
            .with_ymd_and_hms(2025, 3, 15, 10, 30, 0)
            .unwrap();
        let session = Session::new(intent, start, Some(end), None);

        let log = Log::new(sample_date(), chrono_tz::UTC, vec![session]);
        let trackers = HashMap::new();
        let output = log.to_log_file(&trackers);

        assert!(output.contains("[[timeline]]"));
        assert!(output.contains("alias     = \"work\""));
        assert!(output.contains("start     = \"09:00\""));
        assert!(output.contains("end       = \"10:30\""));
        assert!(output.contains("# duration = \"1 hour and 30 minutes\""));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(Log::format_duration(Duration::hours(2)), "2 hours");
        assert_eq!(Log::format_duration(Duration::minutes(45)), "45 minutes");
        assert_eq!(Log::format_duration(Duration::seconds(30)), "30 seconds");
        assert_eq!(
            Log::format_duration(Duration::hours(1) + Duration::minutes(30)),
            "1 hour and 30 minutes"
        );
        assert_eq!(
            Log::format_duration(
                Duration::hours(2) + Duration::minutes(15) + Duration::seconds(45)
            ),
            "2 hours, 15 minutes and 45 seconds"
        );
    }

    #[test]
    fn test_summary() {
        let intent1 = Intent::new(
            Some("coding".to_string()),
            None,
            None,
            None,
            None,
            vec!["element:123".to_string()],
        );
        let intent2 = Intent::new(
            Some("meeting".to_string()),
            None,
            None,
            None,
            None,
            vec!["element:456".to_string(), "jira:ABC-1".to_string()],
        );

        let start1 = london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end1 = london_tz()
            .with_ymd_and_hms(2025, 3, 15, 10, 30, 0)
            .unwrap(); // 90 mins
        let session1 = Session::new(intent1, start1, Some(end1), None);

        let start2 = london_tz().with_ymd_and_hms(2025, 3, 15, 11, 0, 0).unwrap();
        let end2 = london_tz().with_ymd_and_hms(2025, 3, 15, 12, 0, 0).unwrap(); // 60 mins
        let session2 = Session::new(intent2, start2, Some(end2), None);

        let log = Log::new(sample_date(), london_tz(), vec![session1, session2]);
        let now = london_tz().with_ymd_and_hms(2025, 3, 15, 15, 0, 0).unwrap();

        let summary = log.summary(now);

        assert_eq!(summary.total_minutes, 150);
        assert_eq!(summary.by_intent.get("coding"), Some(&90));
        assert_eq!(summary.by_intent.get("meeting"), Some(&60));
        assert_eq!(summary.by_tracker.get("element:123"), Some(&90));
        assert_eq!(summary.by_tracker.get("element:456"), Some(&60));
        assert_eq!(summary.by_tracker.get("jira:ABC-1"), Some(&60));
        assert_eq!(summary.by_tracker_source.get("element"), Some(&150));
        assert_eq!(summary.by_tracker_source.get("jira"), Some(&60));
        assert_eq!(summary.mean_reflection_score, None);
    }

    #[test]
    fn test_summary_with_reflection_scores() {
        let mut session1 = Session::new(
            sample_intent(),
            london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap(),
            Some(london_tz().with_ymd_and_hms(2025, 3, 15, 10, 0, 0).unwrap()), // 60 mins
            None,
        );
        session1.reflection_score = Some(4);

        let mut session2 = Session::new(
            sample_intent(),
            london_tz().with_ymd_and_hms(2025, 3, 15, 11, 0, 0).unwrap(),
            Some(london_tz().with_ymd_and_hms(2025, 3, 15, 12, 0, 0).unwrap()), // 60 mins
            None,
        );
        session2.reflection_score = Some(2);

        let log = Log::new(sample_date(), london_tz(), vec![session1, session2]);
        let now = london_tz().with_ymd_and_hms(2025, 3, 15, 15, 0, 0).unwrap();

        let summary = log.summary(now);

        // Weighted mean: (4*60 + 2*60) / 120 = 360/120 = 3.0
        assert_eq!(summary.mean_reflection_score, Some(3.0));
    }

    #[test]
    fn test_summary_open_session_on_past_date_caps_at_end_of_day() {
        let intent = sample_intent();
        let past_date = NaiveDate::from_ymd_opt(2025, 3, 10).unwrap();

        // Open session starting at 17:00 on a past date
        let start = london_tz().with_ymd_and_hms(2025, 3, 10, 17, 0, 0).unwrap();
        let open_session = Session::new(intent, start, None, None);

        let log = Log::new(past_date, london_tz(), vec![open_session]);

        // "now" is days later
        let now = london_tz().with_ymd_and_hms(2025, 3, 15, 12, 0, 0).unwrap();
        let summary = log.summary(now);

        // Should cap at 23:59 on the log's date, not use "now"
        // 17:00 to 23:59 = 6 hours 59 minutes = 419 minutes
        assert_eq!(summary.total_minutes, 419);
    }
}
