use chrono::{DateTime, NaiveDate, TimeZone};
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
/// use faff_core::utils::date_parsing::parse_natural_date;
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
                "Could not convert date {today} to timezone {timezone}"
            ))
        })?;

    // Parse with chrono-english (using UK dialect for DD/MM/YY format)
    let parsed =
        parse_date_string(date_str, base, Dialect::Uk).map_err(DateParseError::ChronoEnglish)?;

    // Extract just the date part
    Ok(parsed.date_naive())
}

/// Parse a natural language datetime string, restricted to a specific date
///
/// This function is useful for parsing times on a specific day (e.g., "09:30" for today).
/// It ensures the parsed datetime falls on the expected date, preventing accidental
/// backdating or future dating.
///
/// Supports:
/// - Time formats: "09:30", "14:30", "3pm", "midnight"
/// - Relative times: "2 hours ago", "30 minutes ago"
/// - Special keywords: "now"
///
/// # Arguments
/// * `datetime_str` - The datetime string to parse (None returns now)
/// * `expected_date` - The date the parsed datetime must fall on
/// * `now` - Reference datetime for relative parsing (already timezone-aware)
///
/// # Returns
/// * `Ok(DateTime<Tz>)` - The parsed datetime in the same timezone as `now`
/// * `Err(DateParseError)` - If parsing fails or the result is not on the expected date
///
/// # Examples
/// ```
/// use chrono::{NaiveDate, TimeZone, Timelike};
/// use chrono_tz::Europe::London;
/// use faff_core::utils::date_parsing::parse_natural_datetime;
///
/// let today = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
/// let now = London.from_local_datetime(&today.and_hms_opt(11, 30, 0).unwrap()).unwrap();
///
/// // Parse a time on today
/// let dt = parse_natural_datetime(Some("09:30"), today, now).unwrap();
/// assert_eq!(dt.hour(), 9);
/// assert_eq!(dt.minute(), 30);
/// assert_eq!(dt.date_naive(), today);
///
/// // "2 hours ago" from 11:30 = 09:30 (still today)
/// let dt = parse_natural_datetime(Some("2 hours ago"), today, now).unwrap();
/// assert_eq!(dt.hour(), 9);
/// assert_eq!(dt.minute(), 30);
/// ```
pub fn parse_natural_datetime(
    datetime_str: Option<&str>,
    expected_date: NaiveDate,
    now: DateTime<Tz>,
) -> Result<DateTime<Tz>, DateParseError> {
    // Handle None or empty string as "now"
    let datetime_str = match datetime_str {
        None => return Ok(now),
        Some(s) if s.trim().is_empty() => return Ok(now),
        Some(s) => s,
    };

    // Handle "now" explicitly
    if datetime_str.trim().to_lowercase() == "now" {
        return Ok(now);
    }

    // Parse with chrono-english using current time as reference
    let parsed =
        parse_date_string(datetime_str, now, Dialect::Uk).map_err(DateParseError::ChronoEnglish)?;

    // Validate that the parsed datetime is on the expected date
    let parsed_date = parsed.date_naive();
    if parsed_date != expected_date {
        return Err(DateParseError::InvalidFormat(format!(
            "Parsed time '{datetime_str}' resulted in date {parsed_date} but expected {expected_date}. The --since flag only accepts times on today's date."
        )));
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Timelike};
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

    // Tests for parse_natural_datetime
    fn test_datetime() -> DateTime<Tz> {
        // Wednesday, Jan 15, 2025 at 11:30:00
        London
            .from_local_datetime(&test_date().and_hms_opt(11, 30, 0).unwrap())
            .unwrap()
    }

    #[test]
    fn test_datetime_none_returns_now() {
        let now = test_datetime();
        let result = parse_natural_datetime(None, test_date(), now).unwrap();
        assert_eq!(result, now);
    }

    #[test]
    fn test_datetime_empty_string_returns_now() {
        let now = test_datetime();
        let result = parse_natural_datetime(Some(""), test_date(), now).unwrap();
        assert_eq!(result, now);
    }

    #[test]
    fn test_datetime_now_keyword() {
        let now = test_datetime();
        let result = parse_natural_datetime(Some("now"), test_date(), now).unwrap();
        assert_eq!(result, now);

        let result = parse_natural_datetime(Some("NOW"), test_date(), now).unwrap();
        assert_eq!(result, now);
    }

    #[test]
    fn test_datetime_time_format() {
        let now = test_datetime();

        // "09:30" should parse to 9:30am today
        let result = parse_natural_datetime(Some("09:30"), test_date(), now).unwrap();
        assert_eq!(result.hour(), 9);
        assert_eq!(result.minute(), 30);
        assert_eq!(result.date_naive(), test_date());

        // "14:30" should parse to 2:30pm today
        let result = parse_natural_datetime(Some("14:30"), test_date(), now).unwrap();
        assert_eq!(result.hour(), 14);
        assert_eq!(result.minute(), 30);
        assert_eq!(result.date_naive(), test_date());
    }

    #[test]
    fn test_datetime_relative_time_same_day() {
        let now = test_datetime(); // 11:30

        // "2 hours ago" from 11:30 should give 09:30 (still today)
        let result = parse_natural_datetime(Some("2 hours ago"), test_date(), now).unwrap();
        assert_eq!(result.hour(), 9);
        assert_eq!(result.minute(), 30);
        assert_eq!(result.date_naive(), test_date());

        // "30 minutes ago" from 11:30 should give 11:00 (still today)
        let result = parse_natural_datetime(Some("30 minutes ago"), test_date(), now).unwrap();
        assert_eq!(result.hour(), 11);
        assert_eq!(result.minute(), 0);
        assert_eq!(result.date_naive(), test_date());
    }

    #[test]
    fn test_datetime_rejects_different_date() {
        let now = test_datetime();

        // "yesterday 3pm" should be rejected (different date)
        let result = parse_natural_datetime(Some("yesterday 3pm"), test_date(), now);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("only accepts times on today"));

        // "tomorrow" should be rejected (different date)
        let result = parse_natural_datetime(Some("tomorrow"), test_date(), now);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("only accepts times on today"));
    }

    #[test]
    fn test_datetime_rejects_relative_crossing_midnight() {
        let now = test_datetime(); // 11:30am on Jan 15

        // "20 hours ago" would be 3:30pm on Jan 14 - should be rejected
        let result = parse_natural_datetime(Some("20 hours ago"), test_date(), now);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("only accepts times on today's date"));
    }
}
