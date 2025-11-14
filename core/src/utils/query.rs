use crate::models::{Log, Session};
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FilterError {
    #[error("Invalid filter format: {0}")]
    InvalidFormat(String),
    #[error("Invalid operator: {0}")]
    InvalidOperator(String),
    #[error("Invalid field: {0}")]
    InvalidField(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterOperator {
    Equals,    // =
    Contains,  // ~
    NotEquals, // !=
}

impl FilterOperator {
    fn matches(&self, field_value: &str, filter_value: &str) -> bool {
        match self {
            FilterOperator::Equals => field_value == filter_value,
            FilterOperator::Contains => field_value
                .to_lowercase()
                .contains(&filter_value.to_lowercase()),
            FilterOperator::NotEquals => field_value != filter_value,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterField {
    Alias,
    Role,
    Objective,
    Action,
    Subject,
    Note,
}

impl FromStr for FilterField {
    type Err = FilterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "alias" => Ok(FilterField::Alias),
            "role" => Ok(FilterField::Role),
            "objective" => Ok(FilterField::Objective),
            "action" => Ok(FilterField::Action),
            "subject" => Ok(FilterField::Subject),
            "note" => Ok(FilterField::Note),
            _ => Err(FilterError::InvalidField(s.to_string())),
        }
    }
}

impl FilterField {
    fn get_value<'a>(&self, session: &'a Session) -> Option<&'a str> {
        match self {
            FilterField::Alias => session.intent.alias.as_deref(),
            FilterField::Role => session.intent.role.as_deref(),
            FilterField::Objective => session.intent.objective.as_deref(),
            FilterField::Action => session.intent.action.as_deref(),
            FilterField::Subject => session.intent.subject.as_deref(),
            FilterField::Note => session.note.as_deref(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    pub field: FilterField,
    pub operator: FilterOperator,
    pub value: String,
}

impl Filter {
    pub fn new(field: FilterField, operator: FilterOperator, value: String) -> Self {
        Self {
            field,
            operator,
            value,
        }
    }

    /// Check if a session matches this filter
    pub fn matches(&self, session: &Session) -> bool {
        match self.field.get_value(session) {
            Some(field_value) => self.operator.matches(field_value, &self.value),
            None => false,
        }
    }
}

impl FromStr for Filter {
    type Err = FilterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Try != first (two characters) before = (one character)
        if let Some((field_str, value)) = s.split_once("!=") {
            let field = FilterField::from_str(field_str.trim())?;
            return Ok(Filter::new(
                field,
                FilterOperator::NotEquals,
                value.trim().to_string(),
            ));
        }

        if let Some((field_str, value)) = s.split_once('=') {
            let field = FilterField::from_str(field_str.trim())?;
            return Ok(Filter::new(
                field,
                FilterOperator::Equals,
                value.trim().to_string(),
            ));
        }

        if let Some((field_str, value)) = s.split_once('~') {
            let field = FilterField::from_str(field_str.trim())?;
            return Ok(Filter::new(
                field,
                FilterOperator::Contains,
                value.trim().to_string(),
            ));
        }

        Err(FilterError::InvalidFormat(s.to_string()))
    }
}

/// Query result with aggregated session durations
pub type QueryResult = HashMap<Vec<String>, Duration>;

/// Query sessions across multiple logs with filters
///
/// Returns a map where keys are tuples of field values (based on filters)
/// and values are the total duration for sessions matching those field values.
pub fn query_sessions(
    logs: &[Log],
    filters: &[Filter],
    from_date: Option<NaiveDate>,
    to_date: Option<NaiveDate>,
) -> anyhow::Result<QueryResult> {
    let mut results: QueryResult = HashMap::new();

    for log in logs {
        // Apply date range filter
        if let Some(from) = from_date {
            if log.date < from {
                continue;
            }
        }
        if let Some(to) = to_date {
            if log.date > to {
                continue;
            }
        }

        // Process each session
        for session in &log.timeline {
            // Check if all filters match
            if !filters.iter().all(|filter| filter.matches(session)) {
                continue;
            }

            // Build key from filter field values
            let key: Vec<String> = filters
                .iter()
                .map(|filter| filter.field.get_value(session).unwrap_or("").to_string())
                .collect();

            // Calculate session duration
            let duration = match session.duration() {
                Ok(d) => d,
                Err(_) => continue, // Skip sessions with invalid duration
            };

            // Add to results
            results
                .entry(key)
                .and_modify(|d| *d += duration)
                .or_insert(duration);
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Intent;
    use chrono::{TimeZone, Utc};
    use chrono_tz::America::New_York;

    fn create_test_session(
        alias: Option<&str>,
        role: Option<&str>,
        objective: Option<&str>,
        note: Option<&str>,
    ) -> Session {
        let intent = Intent::new(
            alias.map(String::from),
            role.map(String::from),
            objective.map(String::from),
            None,
            None,
            vec![],
        );

        let start = Utc
            .with_ymd_and_hms(2025, 1, 1, 9, 0, 0)
            .unwrap()
            .with_timezone(&New_York);
        let end = Utc
            .with_ymd_and_hms(2025, 1, 1, 10, 0, 0)
            .unwrap()
            .with_timezone(&New_York);

        Session::new(intent, start, Some(end), note.map(String::from))
    }

    #[test]
    fn test_filter_parsing() {
        let filter: Filter = "role=developer".parse().unwrap();
        assert_eq!(filter.field, FilterField::Role);
        assert_eq!(filter.operator, FilterOperator::Equals);
        assert_eq!(filter.value, "developer");

        let filter: Filter = "objective~planning".parse().unwrap();
        assert_eq!(filter.field, FilterField::Objective);
        assert_eq!(filter.operator, FilterOperator::Contains);
        assert_eq!(filter.value, "planning");

        let filter: Filter = "note!=standup".parse().unwrap();
        assert_eq!(filter.field, FilterField::Note);
        assert_eq!(filter.operator, FilterOperator::NotEquals);
        assert_eq!(filter.value, "standup");
    }

    #[test]
    fn test_filter_matching() {
        let session = create_test_session(
            Some("coding"),
            Some("engineer"),
            Some("feature-development"),
            Some("daily standup"),
        );

        // Equals operator
        let filter: Filter = "role=engineer".parse().unwrap();
        assert!(filter.matches(&session));

        let filter: Filter = "role=manager".parse().unwrap();
        assert!(!filter.matches(&session));

        // Contains operator
        let filter: Filter = "objective~development".parse().unwrap();
        assert!(filter.matches(&session));

        let filter: Filter = "objective~planning".parse().unwrap();
        assert!(!filter.matches(&session));

        // Not equals operator
        let filter: Filter = "note!=meeting".parse().unwrap();
        assert!(filter.matches(&session)); // "daily standup" != "meeting"

        let filter: Filter = "note!=daily standup".parse().unwrap();
        assert!(!filter.matches(&session)); // "daily standup" == "daily standup"
    }

    #[test]
    fn test_query_sessions() {
        let sessions = vec![
            create_test_session(Some("coding"), Some("engineer"), Some("feature-a"), None),
            create_test_session(Some("review"), Some("engineer"), Some("feature-b"), None),
        ];

        let log = Log::new(
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            New_York,
            sessions,
        );

        let filters = vec!["role=engineer".parse().unwrap()];
        let results = query_sessions(&[log], &filters, None, None).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results.contains_key(&vec!["engineer".to_string()]));
    }
}
