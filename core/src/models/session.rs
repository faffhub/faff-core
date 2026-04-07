use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::collections::HashMap;

use crate::models::valuetype::ValueType;

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone};
use chrono_tz::Tz;

use anyhow::{bail, Result};

use thiserror::Error;

// Custom serializer for DateTime<Tz> that preserves semantic timezone info
fn serialize_datetime<S>(dt: &DateTime<Tz>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // Only use Z for actual UTC timezones
    let tz_name = dt.timezone().name();
    if tz_name == "UTC" || tz_name == "Etc/UTC" {
        if dt.timestamp_subsec_micros() > 0 {
            serializer.serialize_str(&dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string())
        } else {
            serializer.serialize_str(&dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        }
    } else {
        // Always use offset for non-UTC timezones (never Z)
        serializer.serialize_str(&dt.to_rfc3339())
    }
}

fn serialize_optional_datetime<S>(
    dt: &Option<DateTime<Tz>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match dt {
        Some(dt) => serialize_datetime(dt, serializer),
        None => serializer.serialize_none(),
    }
}

// Custom deserializer for DateTime<Tz> that parses RFC3339 strings
// Converts to UTC since we can't reconstruct semantic timezone names from offsets
fn deserialize_datetime<'de, D>(deserializer: D) -> Result<DateTime<Tz>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&chrono_tz::UTC))
        .map_err(serde::de::Error::custom)
}

fn deserialize_optional_datetime<'de, D>(deserializer: D) -> Result<Option<DateTime<Tz>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) => {
            let dt = DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&chrono_tz::UTC))
                .map_err(serde::de::Error::custom)?;
            Ok(Some(dt))
        }
        None => Ok(None),
    }
}

/// Custom deserializer for trackers that handles both string and array formats
fn deserialize_trackers<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct TrackersVisitor;

    impl<'de> Visitor<'de> for TrackersVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or array of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Vec<String>, E>
        where
            E: de::Error,
        {
            Ok(vec![value.to_string()])
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Vec<String>, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut trackers = Vec::new();
            while let Some(value) = seq.next_element()? {
                trackers.push(value);
            }
            Ok(trackers)
        }
    }

    deserializer.deserialize_any(TrackersVisitor)
}

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("Cannot compute duration: session has no end time")]
    MissingEnd,
    #[error("Cannot compute duration: end time is before start time")]
    EndBeforeStart,
}

fn combine_date_time(date: NaiveDate, tz: Tz, time_str: &str) -> Result<DateTime<Tz>> {
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
        .ok_or_else(|| anyhow::anyhow!("Ambiguous or nonexistent time for {} in {}", naive, tz))
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Session {
    #[serde(alias = "alias")]
    pub title: Option<String>,
    pub role: Option<String>,
    #[serde(alias = "objective")]
    pub impact: Option<String>,
    #[serde(alias = "action")]
    pub mode: Option<String>,
    pub subject: Option<String>,
    #[serde(default, deserialize_with = "deserialize_trackers")]
    pub trackers: Vec<String>,
    #[serde(
        serialize_with = "serialize_datetime",
        deserialize_with = "deserialize_datetime"
    )]
    pub start: DateTime<Tz>,
    #[serde(
        serialize_with = "serialize_optional_datetime",
        deserialize_with = "deserialize_optional_datetime"
    )]
    pub end: Option<DateTime<Tz>>,
    pub note: Option<String>,
    /// Reflection score (1-5 scale) for filtering and analysis
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflection_score: Option<i32>,
    /// Freeform reflection text about this session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflection: Option<String>,
}

impl Session {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        title: Option<String>,
        role: Option<String>,
        impact: Option<String>,
        mode: Option<String>,
        subject: Option<String>,
        trackers: Vec<String>,
        start: DateTime<Tz>,
        end: Option<DateTime<Tz>>,
        note: Option<String>,
    ) -> Self {
        Self {
            title,
            role,
            impact,
            mode,
            subject,
            trackers,
            start,
            end,
            note,
            reflection_score: None,
            reflection: None,
        }
    }

    // def from_dict_with_tz(cls, data: dict, date: pendulum.Date, timezone: pendulum.Timezone | pendulum.FixedTimezone) -> Session:
    pub fn from_dict_with_tz(
        dict: HashMap<String, ValueType>,
        date: chrono::NaiveDate,
        timezone: chrono_tz::Tz,
    ) -> Result<Self, String> {
        let title = dict
            .get("title")
            .or_else(|| dict.get("alias"))
            .and_then(|v| v.as_string())
            .cloned();

        let role = dict.get("role").and_then(|v| v.as_string()).cloned();

        let impact = dict
            .get("impact")
            .or_else(|| dict.get("objective"))
            .and_then(|v| v.as_string())
            .cloned();

        let mode = dict
            .get("mode")
            .or_else(|| dict.get("action"))
            .and_then(|v| v.as_string())
            .cloned();

        let subject = dict.get("subject").and_then(|v| v.as_string()).cloned();

        // Handle trackers as either a string or a list
        let trackers = dict
            .get("trackers")
            .map(|v| {
                if let Some(s) = v.as_string() {
                    vec![s.clone()]
                } else if let Some(list) = v.as_list() {
                    list.clone()
                } else {
                    vec![]
                }
            })
            .unwrap_or_default();

        let start: String = dict
            .get("start")
            .and_then(|v| v.as_string())
            .cloned()
            .ok_or("Missing 'start' field in session dict")?;

        // Let's create our start time by combining a naive date object (date), a timezone object (timezone),
        // and a string representation of the time (start) which will include a offset if-and-only-if there is any
        // chance of time ambiguity resulting from daylight saving on that day.
        let start: DateTime<Tz> =
            combine_date_time(date, timezone, &start).map_err(|e| e.to_string())?;

        let end = dict.get("end").and_then(|v| v.as_string()).cloned();

        let end = match end {
            Some(s) => Some(combine_date_time(date, timezone, &s).map_err(|e| e.to_string())?),
            None => None,
        };

        let note = dict.get("note").and_then(|v| v.as_string()).cloned();

        let reflection_score = dict.get("reflection_score").and_then(|v| v.as_integer());
        let reflection = dict.get("reflection").and_then(|v| v.as_string()).cloned();

        Ok(Self {
            title,
            role,
            impact,
            mode,
            subject,
            trackers,
            start,
            end,
            note,
            reflection_score,
            reflection,
        })
    }

    pub fn with_end(&self, end: DateTime<Tz>) -> Self {
        Self {
            end: Some(end),
            ..self.clone()
        }
    }

    /// Replace all semantic fields wholesale, preserving start/end and any
    /// reflection. Used by `LogManager::update_active_session` so the
    /// caller can hand in the desired final state of the session without
    /// touching the time-bounded or post-hoc reflection bits.
    #[allow(clippy::too_many_arguments)]
    pub fn with_fields(
        &self,
        title: Option<String>,
        role: Option<String>,
        impact: Option<String>,
        mode: Option<String>,
        subject: Option<String>,
        trackers: Vec<String>,
        note: Option<String>,
    ) -> Self {
        Self {
            title,
            role,
            impact,
            mode,
            subject,
            trackers,
            note,
            ..self.clone()
        }
    }

    pub fn with_reflection(&self, score: Option<i32>, reflection: Option<String>) -> Self {
        Self {
            reflection_score: score,
            reflection,
            ..self.clone()
        }
    }

    /// Get duration of a closed session
    ///
    /// Returns error if session has no end time (use `elapsed()` instead).
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

    /// Get elapsed time for an open session
    ///
    /// For open sessions, returns time elapsed since start.
    /// Panics if session is already closed (use `duration()` instead).
    pub fn elapsed(&self, now: DateTime<Tz>) -> Duration {
        assert!(
            self.end.is_none(),
            "elapsed() called on closed session - use duration() instead"
        );
        now - self.start
    }

    /// Parse a Session from a TOML table (from log file format)
    pub fn from_toml_table(
        table: &toml::map::Map<String, toml::Value>,
        date: NaiveDate,
        timezone: Tz,
    ) -> anyhow::Result<Self> {
        // Extract fields directly from the TOML table (try new names first, fall back to old)
        let title = table
            .get("title")
            .or_else(|| table.get("alias"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let role = table
            .get("role")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let impact = table
            .get("impact")
            .or_else(|| table.get("objective"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mode = table
            .get("mode")
            .or_else(|| table.get("action"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let subject = table
            .get("subject")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Handle trackers as either a string or array
        let trackers = match table.get("trackers") {
            Some(toml::Value::String(s)) => vec![s.clone()],
            Some(toml::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            _ => vec![],
        };

        // Parse start/end times
        let start_str = table
            .get("start")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'start' field in session"))?;
        let start = Self::parse_time_from_toml(start_str, date, timezone)?;

        let end = table
            .get("end")
            .and_then(|v| v.as_str())
            .map(|s| Self::parse_time_from_toml(s, date, timezone))
            .transpose()?;

        let note = table
            .get("note")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let reflection_score = table
            .get("reflection_score")
            .and_then(|v| v.as_integer())
            .and_then(|i| i32::try_from(i).ok());

        let reflection = table
            .get("reflection")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(Session {
            title,
            role,
            impact,
            mode,
            subject,
            trackers,
            start,
            end,
            note,
            reflection_score,
            reflection,
        })
    }

    fn parse_time_from_toml(
        time_str: &str,
        date: NaiveDate,
        timezone: Tz,
    ) -> anyhow::Result<DateTime<Tz>> {
        // Trim whitespace to handle malformed input
        let time_str = time_str.trim();

        // Time can be "HH:MM" or "HH:MM+OFFSET"
        if time_str.contains('+') || (time_str.matches('-').count() > 0 && time_str.len() > 5) {
            // Has timezone offset
            let datetime_str = format!("{date}T{time_str}");
            let dt = chrono::DateTime::parse_from_str(&datetime_str, "%Y-%m-%dT%H:%M%z")?;
            Ok(dt.with_timezone(&timezone))
        } else {
            // Just time, use the log's timezone
            let parts: Vec<&str> = time_str.split(':').collect();
            if parts.len() != 2 {
                anyhow::bail!("Invalid time format: {}", time_str);
            }
            let hour: u32 = parts[0].trim().parse()?;
            let minute: u32 = parts[1].trim().parse()?;

            timezone
                .with_ymd_and_hms(date.year(), date.month(), date.day(), hour, minute, 0)
                .single()
                .ok_or_else(|| anyhow::anyhow!("Invalid datetime: {} {}:{}", date, hour, minute))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Timelike};

    fn sample_session(start: DateTime<Tz>) -> Session {
        Session::new(
            Some("work".to_string()),
            Some("engineer".to_string()),
            Some("development".to_string()),
            Some("coding".to_string()), // mode
            Some("features".to_string()),
            vec![],
            start,
            None,
            None,
        )
    }

    #[test]
    fn test_session_creation() {
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();

        let session = Session::new(
            Some("work".to_string()),
            Some("engineer".to_string()),
            Some("development".to_string()),
            Some("coding".to_string()),
            Some("features".to_string()),
            vec![],
            start,
            Some(end),
            None,
        );

        assert_eq!(session.title, Some("work".to_string()));
        assert_eq!(session.role, Some("engineer".to_string()));
        assert_eq!(session.start, start);
        assert_eq!(session.end, Some(end));
        assert_eq!(session.note, None);
    }

    #[test]
    fn test_session_with_note() {
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();

        let session = Session::new(
            None,
            None,
            None,
            None,
            None,
            vec![],
            start,
            None,
            Some("Working on tests".to_string()),
        );

        assert_eq!(session.note, Some("Working on tests".to_string()));
    }

    #[test]
    fn test_duration_completed_session() {
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();

        let session = Session::new(None, None, None, None, None, vec![], start, Some(end), None);
        let duration = session.duration().unwrap();

        assert_eq!(duration, Duration::minutes(90));
    }

    #[test]
    fn test_duration_open_session_error() {
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();

        let session = Session::new(None, None, None, None, None, vec![], start, None, None);
        let result = session.duration();

        assert!(matches!(result, Err(SessionError::MissingEnd)));
    }

    #[test]
    fn test_elapsed_open_session() {
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let now = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();

        let session = Session::new(None, None, None, None, None, vec![], start, None, None);
        let elapsed = session.elapsed(now);

        assert_eq!(elapsed, Duration::minutes(90));
    }

    #[test]
    #[should_panic(expected = "elapsed() called on closed session")]
    fn test_elapsed_closed_session_panics() {
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 0, 0).unwrap();
        let now = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 11, 0, 0).unwrap();

        let session = Session::new(None, None, None, None, None, vec![], start, Some(end), None);
        session.elapsed(now); // should panic
    }

    #[test]
    fn test_duration_end_before_start_error() {
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();

        let session = Session::new(None, None, None, None, None, vec![], start, Some(end), None);
        let result = session.duration();

        assert!(matches!(result, Err(SessionError::EndBeforeStart)));
    }

    #[test]
    fn test_with_end() {
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();

        let open_session = sample_session(start);
        assert_eq!(open_session.end, None);

        let closed_session = open_session.with_end(end);
        assert_eq!(closed_session.end, Some(end));
        assert_eq!(closed_session.title, Some("work".to_string()));
        assert_eq!(closed_session.start, start);
    }

    #[test]
    fn test_with_end_immutability() {
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 30, 0).unwrap();

        let open_session = sample_session(start);
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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid time format"));
    }

    #[test]
    fn test_from_dict_with_tz_basic() {
        let mut dict = HashMap::new();
        dict.insert(
            "role".to_string(),
            ValueType::String("engineer".to_string()),
        );
        dict.insert("mode".to_string(), ValueType::String("coding".to_string()));
        dict.insert(
            "subject".to_string(),
            ValueType::String("tests".to_string()),
        );
        dict.insert("start".to_string(), ValueType::String("09:00".to_string()));
        dict.insert("end".to_string(), ValueType::String("10:30".to_string()));

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let tz = Tz::UTC;

        let session = Session::from_dict_with_tz(dict, date, tz).unwrap();

        assert_eq!(session.role, Some("engineer".to_string()));
        assert_eq!(session.mode, Some("coding".to_string()));
        assert_eq!(session.subject, Some("tests".to_string()));
        assert_eq!(session.start.hour(), 9);
        assert_eq!(session.end.unwrap().hour(), 10);
        assert_eq!(session.end.unwrap().minute(), 30);
    }

    #[test]
    fn test_from_dict_with_tz_open_session() {
        let mut dict = HashMap::new();
        dict.insert(
            "role".to_string(),
            ValueType::String("engineer".to_string()),
        );
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
        dict.insert(
            "note".to_string(),
            ValueType::String("Test note".to_string()),
        );

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
        dict.insert(
            "trackers".to_string(),
            ValueType::String("work:admin".to_string()),
        );

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let tz = Tz::UTC;

        let session = Session::from_dict_with_tz(dict, date, tz).unwrap();

        assert_eq!(session.trackers, vec!["work:admin".to_string()]);
    }

    #[test]
    fn test_from_dict_with_tz_multiple_trackers_list() {
        let mut dict = HashMap::new();
        dict.insert("start".to_string(), ValueType::String("09:00".to_string()));
        dict.insert(
            "trackers".to_string(),
            ValueType::List(vec!["work:admin".to_string(), "personal:study".to_string()]),
        );

        let date = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
        let tz = Tz::UTC;

        let session = Session::from_dict_with_tz(dict, date, tz).unwrap();

        assert_eq!(session.trackers.len(), 2);
        assert!(session.trackers.contains(&"work:admin".to_string()));
        assert!(session.trackers.contains(&"personal:study".to_string()));
    }

    #[test]
    fn test_session_equality() {
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();
        let end = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 10, 0, 0).unwrap();

        let session1 = Session::new(
            Some("work".to_string()),
            None,
            None,
            None,
            None,
            vec![],
            start,
            Some(end),
            None,
        );
        let session2 = Session::new(
            Some("work".to_string()),
            None,
            None,
            None,
            None,
            vec![],
            start,
            Some(end),
            None,
        );

        assert_eq!(session1, session2);
    }

    #[test]
    fn test_session_clone() {
        let start = Tz::UTC.with_ymd_and_hms(2025, 3, 15, 9, 0, 0).unwrap();

        let session1 = Session::new(
            None,
            None,
            None,
            None,
            None,
            vec![],
            start,
            None,
            Some("note".to_string()),
        );
        let session2 = session1.clone();

        assert_eq!(session1, session2);
        assert_eq!(session2.note, Some("note".to_string()));
    }
}
