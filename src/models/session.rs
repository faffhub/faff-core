use serde::{Serialize}; // Removed Deserialize since it's a PITA at the moment

use std::collections::HashMap;

use crate::models::intent::Intent;
use crate::models::valuetype::ValueType;

use chrono::{NaiveDate, NaiveTime, DateTime, TimeZone};
use chrono_tz::Tz;

use anyhow::{Result, bail};

fn combine_date_time(
    date: NaiveDate,
    tz: Tz,
    time_str: &str,
) -> Result<DateTime<Tz>> {
    // First try to parse time_str as a full time + offset (e.g. 09:30+01:00)
    if let Ok(dt_fixed) = DateTime::parse_from_rfc3339(&format!("{}T{}", date, time_str)) {
        // Convert to Tz
        Ok(dt_fixed.with_timezone(&tz))
    } else {
        // Fall back: parse as naive time and localize
        let time = NaiveTime::parse_from_str(time_str, "%H:%M")
            .map_err(|_| anyhow::anyhow!("Invalid time format: {}", time_str))?;

        let naive = date.and_time(time);

        // Apply tz — will handle DST correctly
        Ok(tz.from_local_datetime(&naive)
            .single()
            .ok_or_else(|| anyhow::anyhow!(
                "Ambiguous or nonexistent time for {} in {}",
                naive,
                tz
            ))?)
    }
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

}