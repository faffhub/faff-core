use serde::Serialize;
use chrono::{NaiveDate, DateTime, NaiveTime, Datelike, Duration, Local, TimeZone};
use chrono_tz::Tz;
use thiserror::Error;

use crate::models::session::Session;

#[derive(Error, Debug)]
pub enum LogError {
    #[error("No timeline entries to stop")]
    NoTimelineEntries,
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
    pub fn append_session(&self, session: Session) -> Log {
        if self.active_session().is_some() {
            let stopped_log = self.stop_active_session(session.start)
                .expect("active_session exists, so stop should work");
            stopped_log.append_session(session)
        } else {
            let mut new_timeline = self.timeline.clone();
            new_timeline.push(session);
            Log::new(self.date, self.timezone, new_timeline)
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
    pub fn total_recorded_time(&self) -> Duration {
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
                            .expect("valid time");
                        let end_of_day_naive = self.date.and_time(end_of_day_time);
                        let end_of_day = self.timezone
                            .from_local_datetime(&end_of_day_naive)
                            .single()
                            .expect("valid datetime");
                        end_of_day - start
                    }
                }
            };

            total = total + duration;
        }

        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::intent::Intent;
    use chrono::{TimeZone, Timelike};

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
        let end = london_tz().with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();
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
        let end = london_tz().with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();
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
        let end1 = london_tz().with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();
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
        let end = london_tz().with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();
        let session = Session::new(intent, start, Some(end), None);

        let log = Log::new(sample_date(), london_tz(), vec![]);
        let new_log = log.append_session(session.clone());

        assert_eq!(new_log.timeline.len(), 1);
        assert_eq!(new_log.timeline[0], session);
        // Original unchanged
        assert_eq!(log.timeline.len(), 0);
    }

    #[test]
    fn test_append_to_log_with_completed_sessions() {
        let intent = sample_intent();
        let start1 = london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end1 = london_tz().with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();
        let session1 = Session::new(intent.clone(), start1, Some(end1), None);

        let start2 = london_tz().with_ymd_and_hms(2025, 3, 15, 11, 0, 0).unwrap();
        let end2 = london_tz().with_ymd_and_hms(2025, 3, 15, 12, 0, 0).unwrap();
        let session2 = Session::new(intent, start2, Some(end2), None);

        let log = Log::new(sample_date(), london_tz(), vec![session1]);
        let new_log = log.append_session(session2.clone());

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
        let new_log = log.append_session(new_session.clone());

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

        let stop_time = london_tz().with_ymd_and_hms(2025, 3, 15, 16, 30, 0).unwrap();
        let stopped_log = log.stop_active_session(stop_time).unwrap();

        assert_eq!(stopped_log.timeline[0].end, Some(stop_time));
        // Original unchanged
        assert_eq!(log.timeline[0].end, None);
    }

    #[test]
    fn test_stop_empty_log_raises_error() {
        let log = Log::new(sample_date(), london_tz(), vec![]);
        let stop_time = london_tz().with_ymd_and_hms(2025, 3, 15, 16, 30, 0).unwrap();

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
        let end = london_tz().with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();
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
        assert_eq!(log.total_recorded_time(), Duration::zero());
    }

    #[test]
    fn test_single_completed_session() {
        let intent = sample_intent();
        let start = london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = london_tz().with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();
        let session = Session::new(intent, start, Some(end), None);

        let log = Log::new(sample_date(), london_tz(), vec![session]);
        let expected = Duration::hours(1) + Duration::minutes(30);
        assert_eq!(log.total_recorded_time(), expected);
    }

    #[test]
    fn test_multiple_completed_sessions() {
        let intent = sample_intent();

        let start1 = london_tz().with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end1 = london_tz().with_ymd_and_hms(2025, 3, 15, 10, 0, 0).unwrap();
        let session1 = Session::new(intent.clone(), start1, Some(end1), None);

        let start2 = london_tz().with_ymd_and_hms(2025, 3, 15, 14, 0, 0).unwrap();
        let end2 = london_tz().with_ymd_and_hms(2025, 3, 15, 15, 30, 0).unwrap();
        let session2 = Session::new(intent, start2, Some(end2), None);

        let log = Log::new(sample_date(), london_tz(), vec![session1, session2]);

        let expected = Duration::hours(2) + Duration::minutes(30);
        assert_eq!(log.total_recorded_time(), expected);
    }

    #[test]
    fn test_open_session_on_past_date_uses_end_of_day() {
        let intent = sample_intent();
        let past_date = NaiveDate::from_ymd_opt(2025, 3, 10).unwrap();

        let start = london_tz().with_ymd_and_hms(2025, 3, 10, 14, 0, 0).unwrap();
        let open_session = Session::new(intent, start, None, None);

        let log = Log::new(past_date, london_tz(), vec![open_session]);
        let total = log.total_recorded_time();

        // From 14:00 to 23:59:59 = 9 hours, 59 minutes, 59 seconds
        let expected = Duration::hours(9) + Duration::minutes(59) + Duration::seconds(59);
        assert_eq!(total, expected);
    }
}
