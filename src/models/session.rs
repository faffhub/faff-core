use serde::{Serialize}; // Removed Deserialize since it's a PITA at the moment

use std::collections::HashMap;

use crate::models::intent::Intent;
use crate::models::valuetype::ValueType;

use chrono::{NaiveDate, NaiveTime, DateTime, TimeZone, Duration};
use chrono_tz::Tz;

use anyhow::{Result, bail};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("Cannot compute duration: session has no end time")]
    MissingEnd,
    #[error("Cannot compute duration: end time is before start time")]
    EndBeforeStart,
}

fn combine_date_time(
    date: NaiveDate,
    tz: Tz,
    time_str: &str,
) -> Result<DateTime<Tz>> {
    // Don't accept any offset here — only plain time strings
    if time_str.contains('+') || time_str.contains('-') {
        bail!(
            "Fixed-offset time strings like '{}' are not allowed. Use HH:MM format.",
            time_str
        );
    }

    let time = NaiveTime::parse_from_str(time_str, "%H:%M")
        .map_err(|_| anyhow::anyhow!("Invalid time format: {}", time_str))?;

    let naive = date.and_time(time);

    tz.from_local_datetime(&naive)
        .single()
        .ok_or_else(|| anyhow::anyhow!(
            "Ambiguous or nonexistent time for {} in {}",
            naive,
            tz
        ))
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Session {
    pub intent: Intent,
    pub start: DateTime<Tz>,
    pub end: Option<DateTime<Tz>>,
    pub note: Option<String>,
}

impl Session {
    pub fn new(intent: Intent, start: DateTime<Tz>, end: Option<DateTime<Tz>>, note: Option<String>) -> Self {
        Self { intent, start, end, note }
    }

    // def from_dict_with_tz(cls, data: dict, date: pendulum.Date, timezone: pendulum.Timezone | pendulum.FixedTimezone) -> Session:
    pub fn from_dict_with_tz(dict: HashMap<String, ValueType>, date: chrono::NaiveDate, timezone: chrono_tz::Tz) -> Result<Self, String> {
        let alias = dict.get("alias")
                        .and_then(|v| v.as_string())
                        .cloned();

        let role = dict.get("role")
                        .and_then(|v| v.as_string())
                        .cloned();

        let objective = dict.get("objective")
                        .and_then(|v| v.as_string())
                        .cloned();

        let action = dict.get("action")
                        .and_then(|v| v.as_string())
                        .cloned();

        let subject = dict.get("subject")
                        .and_then(|v| v.as_string())
                        .cloned();

        // FIXME: This should work with a list or a single tracker.
        let trackers = dict
            .get("trackers")
            .and_then(|v| v.as_string())
            .cloned()
            .map(|s| vec![s])
            .unwrap_or_default();

        let intent: Intent = Intent::new(alias, role, objective, action, subject, trackers);

        let start: String = dict.get("start")
                        .and_then(|v| v.as_string())
                        .cloned()
                        .ok_or("Missing 'start' field in session dict")?;

        // Let's create our start time by combining a naive date object (date), a timezone object (timezone), 
        // and a string representation of the time (start) which will include a offset if-and-only-if there is any
        // chance of time ambiguity resulting from daylight saving on that day.
        let start: DateTime<Tz> = combine_date_time(date, timezone, &start)
            .map_err(|e| e.to_string())?;

        let end = dict.get("end")
                        .and_then(|v| v.as_string())
                        .cloned();

        let end = match end {
            Some(s) => Some(combine_date_time(date, timezone, &s).map_err(|e| e.to_string())?),
            None => None,
        };

        let note = dict.get("note")
                        .and_then(|v| v.as_string())
                        .cloned(); 

        Ok(Self {
            intent,
            start,
            end,
            note,
        })
    }

    pub fn with_end(&self, end: DateTime<Tz>) -> Self {
        Self {
            end: Some(end),
            ..self.clone()
        }
    }

    pub fn duration(&self) -> Result<Duration, SessionError> {
        match self.end {
            Some(end) => {
                if end < self.start {
                    Err(SessionError::EndBeforeStart)
                } else {
                    Ok(end - self.start)
                }
            }
            None => Err(SessionError::MissingEnd),
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Timelike};

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

    #[test]
    fn test_session_creation() {
        let intent = sample_intent();
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();

        let session = Session::new(intent.clone(), start, Some(end), None);

        assert_eq!(session.intent, intent);
        assert_eq!(session.start, start);
        assert_eq!(session.end, Some(end));
        assert_eq!(session.note, None);
    }

    #[test]
    fn test_session_with_note() {
        let intent = sample_intent();
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();

        let session = Session::new(
            intent,
            start,
            None,
            Some("Working on tests".to_string()),
        );

        assert_eq!(session.note, Some("Working on tests".to_string()));
    }

    #[test]
    fn test_duration_completed_session() {
        let intent = sample_intent();
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();

        let session = Session::new(intent, start, Some(end), None);
        let duration = session.duration().unwrap();

        assert_eq!(duration, Duration::minutes(90));
    }

    #[test]
    fn test_duration_open_session_error() {
        let intent = sample_intent();
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();

        let session = Session::new(intent, start, None, None);
        let result = session.duration();

        assert!(matches!(result, Err(SessionError::MissingEnd)));
    }

    #[test]
    fn test_duration_end_before_start_error() {
        let intent = sample_intent();
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();

        let session = Session::new(intent, start, Some(end), None);
        let result = session.duration();

        assert!(matches!(result, Err(SessionError::EndBeforeStart)));
    }

    #[test]
    fn test_with_end() {
        let intent = sample_intent();
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();

        let open_session = Session::new(intent.clone(), start, None, None);
        assert_eq!(open_session.end, None);

        let closed_session = open_session.with_end(end);
        assert_eq!(closed_session.end, Some(end));
        assert_eq!(closed_session.intent, intent);
        assert_eq!(closed_session.start, start);
    }

    #[test]
    fn test_with_end_immutability() {
        let intent = sample_intent();
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();

        let open_session = Session::new(intent, start, None, None);
        let _closed_session = open_session.with_end(end);

        // Original should be unchanged
        assert_eq!(open_session.end, None);
    }

    #[test]
    fn test_combine_date_time_valid() {
        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let tz = Tz::UTC;

        let result = combine_date_time(date, tz, "14:30").unwrap();

        assert_eq!(result.hour(), 14);
        assert_eq!(result.minute(), 30);
        assert_eq!(result.second(), 0);
    }

    #[test]
    fn test_combine_date_time_with_timezone() {
        let date = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let tz = Tz::Europe__London;

        let result = combine_date_time(date, tz, "12:00").unwrap();

        assert_eq!(result.hour(), 12);
        assert_eq!(result.minute(), 0);
    }

    #[test]
    fn test_combine_date_time_rejects_offset() {
        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let tz = Tz::UTC;

        let result = combine_date_time(date, tz, "14:30+01:00");

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not allowed"));
    }

    #[test]
    fn test_combine_date_time_invalid_format() {
        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let tz = Tz::UTC;

        let result = combine_date_time(date, tz, "25:99");

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid time format"));
    }

    #[test]
    fn test_from_dict_with_tz_basic() {
        let mut dict = HashMap::new();
        dict.insert("role".to_string(), ValueType::String("engineer".to_string()));
        dict.insert("action".to_string(), ValueType::String("coding".to_string()));
        dict.insert("subject".to_string(), ValueType::String("tests".to_string()));
        dict.insert("start".to_string(), ValueType::String("09:00".to_string()));
        dict.insert("end".to_string(), ValueType::String("10:30".to_string()));

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let tz = Tz::UTC;

        let session = Session::from_dict_with_tz(dict, date, tz).unwrap();

        assert_eq!(session.intent.role, Some("engineer".to_string()));
        assert_eq!(session.intent.action, Some("coding".to_string()));
        assert_eq!(session.intent.subject, Some("tests".to_string()));
        assert_eq!(session.start.hour(), 9);
        assert_eq!(session.end.unwrap().hour(), 10);
        assert_eq!(session.end.unwrap().minute(), 30);
    }

    #[test]
    fn test_from_dict_with_tz_open_session() {
        let mut dict = HashMap::new();
        dict.insert("role".to_string(), ValueType::String("engineer".to_string()));
        dict.insert("start".to_string(), ValueType::String("09:00".to_string()));

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let tz = Tz::UTC;

        let session = Session::from_dict_with_tz(dict, date, tz).unwrap();

        assert_eq!(session.end, None);
    }

    #[test]
    fn test_from_dict_with_tz_with_note() {
        let mut dict = HashMap::new();
        dict.insert("start".to_string(), ValueType::String("09:00".to_string()));
        dict.insert("note".to_string(), ValueType::String("Test note".to_string()));

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let tz = Tz::UTC;

        let session = Session::from_dict_with_tz(dict, date, tz).unwrap();

        assert_eq!(session.note, Some("Test note".to_string()));
    }

    #[test]
    fn test_from_dict_with_tz_missing_start() {
        let dict = HashMap::new();

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let tz = Tz::UTC;

        let result = Session::from_dict_with_tz(dict, date, tz);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'start' field"));
    }

    #[test]
    fn test_from_dict_with_tz_single_tracker_string() {
        let mut dict = HashMap::new();
        dict.insert("start".to_string(), ValueType::String("09:00".to_string()));
        dict.insert("trackers".to_string(), ValueType::String("work:admin".to_string()));

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let tz = Tz::UTC;

        let session = Session::from_dict_with_tz(dict, date, tz).unwrap();

        assert_eq!(session.intent.trackers, vec!["work:admin".to_string()]);
    }

    #[test]
    fn test_session_equality() {
        let intent1 = sample_intent();
        let intent2 = sample_intent();
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 0, 0).unwrap();

        let session1 = Session::new(intent1, start, Some(end), None);
        let session2 = Session::new(intent2, start, Some(end), None);

        assert_eq!(session1, session2);
    }

    #[test]
    fn test_session_clone() {
        let intent = sample_intent();
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();

        let session1 = Session::new(intent, start, None, Some("note".to_string()));
        let session2 = session1.clone();

        assert_eq!(session1, session2);
        assert_eq!(session2.note, Some("note".to_string()));
    }
}