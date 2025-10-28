use chrono::{NaiveDate, TimeZone};
use chrono_english::{parse_date_string, Dialect};
use chrono_tz::Tz;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DateParseError {
    #[error("Invalid date string: {0}")]
    InvalidFormat(String),
    #[error("chrono-english parse error: {0}")]
    ChronoEnglish(#[from] chrono_english::DateError),
}

/// Parse a natural language date string relative to a reference date
///
/// Supports:
/// - ISO dates: "2025-08-03", "2018-04-01"
/// - Relative dates: "yesterday", "last monday", "next friday", "today"
/// - Month names: "April 1", "1 April 2018"
/// - Time intervals: "2 days ago", "3 hours ago"
/// - Informal dates: "01/04/18" (UK format)
///
/// # Arguments
/// * `date_str` - The date string to parse (None or empty string returns today)
/// * `today` - Reference date for relative parsing
/// * `timezone` - Timezone for datetime calculations
///
/// # Examples
/// ```
/// use chrono::NaiveDate;
/// use chrono_tz::Europe::London;
/// use faff_core::date_parsing::parse_natural_date;
///
/// let today = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
///
/// // ISO date
/// let date = parse_natural_date(Some("2025-08-03"), today, London).unwrap();
/// assert_eq!(date, NaiveDate::from_ymd_opt(2025, 8, 3).unwrap());
///
/// // Today
/// let date = parse_natural_date(None, today, London).unwrap();
/// assert_eq!(date, today);
///
/// // Relative date
/// let date = parse_natural_date(Some("yesterday"), today, London).unwrap();
/// assert_eq!(date, NaiveDate::from_ymd_opt(2025, 1, 14).unwrap());
/// ```
pub fn parse_natural_date(
    date_str: Option<&str>,
    today: NaiveDate,
    timezone: Tz,
) -> Result<NaiveDate, DateParseError> {
    // Handle None or empty string as "today"
    let date_str = match date_str {
        None => return Ok(today),
        Some(s) if s.trim().is_empty() => return Ok(today),
        Some(s) => s,
    };

    // Handle "today" explicitly (chrono-english doesn't handle it)
    if date_str.trim().to_lowercase() == "today" {
        return Ok(today);
    }

    // Convert today to DateTime for reference (using start of day)
    let base = timezone
        .from_local_datetime(&today.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .ok_or_else(|| {
            DateParseError::InvalidFormat(format!(
                "Could not convert date {} to timezone {}",
                today, timezone
            ))
        })?;

    // Parse with chrono-english (using UK dialect for DD/MM/YY format)
    let parsed = parse_date_string(date_str, base, Dialect::Uk)
        .map_err(|e| DateParseError::ChronoEnglish(e))?;

    // Extract just the date part
    Ok(parsed.date_naive())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use chrono_tz::Europe::London;

    fn test_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap() // Wednesday
    }

    #[test]
    fn test_none_returns_today() {
        let result = parse_natural_date(None, test_date(), London).unwrap();
        assert_eq!(result, test_date());
    }

    #[test]
    fn test_empty_string_returns_today() {
        let result = parse_natural_date(Some(""), test_date(), London).unwrap();
        assert_eq!(result, test_date());
    }

    #[test]
    fn test_today_keyword() {
        let result = parse_natural_date(Some("today"), test_date(), London).unwrap();
        assert_eq!(result, test_date());

        let result = parse_natural_date(Some("TODAY"), test_date(), London).unwrap();
        assert_eq!(result, test_date());
    }

    #[test]
    fn test_iso_date() {
        let result = parse_natural_date(Some("2025-08-03"), test_date(), London).unwrap();
        assert_eq!(result, NaiveDate::from_ymd_opt(2025, 8, 3).unwrap());

        let result = parse_natural_date(Some("2024-12-25"), test_date(), London).unwrap();
        assert_eq!(result, NaiveDate::from_ymd_opt(2024, 12, 25).unwrap());
    }

    #[test]
    fn test_yesterday() {
        let result = parse_natural_date(Some("yesterday"), test_date(), London).unwrap();
        assert_eq!(result, NaiveDate::from_ymd_opt(2025, 1, 14).unwrap());
    }

    #[test]
    fn test_relative_days() {
        let result = parse_natural_date(Some("2 days ago"), test_date(), London).unwrap();
        assert_eq!(result, NaiveDate::from_ymd_opt(2025, 1, 13).unwrap());

        let result = parse_natural_date(Some("1 day ago"), test_date(), London).unwrap();
        assert_eq!(result, NaiveDate::from_ymd_opt(2025, 1, 14).unwrap());
    }

    #[test]
    fn test_weekday_names() {
        // test_date() is Wednesday, Jan 15, 2025

        // "monday" without qualifier means next Monday (chrono-english behavior)
        let result = parse_natural_date(Some("monday"), test_date(), London).unwrap();
        assert_eq!(result, NaiveDate::from_ymd_opt(2025, 1, 20).unwrap()); // Next Monday

        // "last monday"
        let result = parse_natural_date(Some("last monday"), test_date(), London).unwrap();
        assert_eq!(result, NaiveDate::from_ymd_opt(2025, 1, 13).unwrap()); // Last Monday

        // "next friday" - chrono-english interprets as Friday of next week (not this coming Friday)
        let result = parse_natural_date(Some("next friday"), test_date(), London).unwrap();
        assert_eq!(result, NaiveDate::from_ymd_opt(2025, 1, 24).unwrap()); // Friday next week

        // Just "friday" gives us this coming Friday
        let result = parse_natural_date(Some("friday"), test_date(), London).unwrap();
        assert_eq!(result, NaiveDate::from_ymd_opt(2025, 1, 17).unwrap()); // This Friday
    }

    #[test]
    fn test_month_names() {
        // "April 1" in current year
        let result = parse_natural_date(Some("April 1"), test_date(), London).unwrap();
        assert_eq!(result, NaiveDate::from_ymd_opt(2025, 4, 1).unwrap());

        // Full date with month name
        let result = parse_natural_date(Some("1 April 2024"), test_date(), London).unwrap();
        assert_eq!(result, NaiveDate::from_ymd_opt(2024, 4, 1).unwrap());
    }

    #[test]
    fn test_invalid_date() {
        let result = parse_natural_date(Some("not a date"), test_date(), London);
        assert!(result.is_err());

        let result = parse_natural_date(Some("2025-13-01"), test_date(), London);
        assert!(result.is_err());
    }
}
