use crate::models::{Timesheet, TimesheetMeta};
use crate::storage::Storage;
use chrono::NaiveDate;
use std::sync::Arc;

/// Manages timesheet storage and retrieval
pub struct TimesheetManager {
    storage: Arc<dyn Storage>,
}

impl TimesheetManager {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    /// Write a timesheet to storage
    pub fn write_timesheet(&self, timesheet: &Timesheet) -> anyhow::Result<()> {
        let timesheet_dir = self.storage.timesheet_dir();
        self.storage.create_dir_all(&timesheet_dir)?;

        // Write the canonical timesheet
        let timesheet_filename = format!(
            "{}.{}.json",
            timesheet.meta.audience_id,
            timesheet.date.format("%Y-%m-%d")
        );
        let timesheet_path = timesheet_dir.join(&timesheet_filename);
        let canonical = timesheet.submittable_timesheet().canonical_form()?;
        self.storage.write_bytes(&timesheet_path, &canonical)?;

        // Write the metadata separately
        let meta_filename = format!("{}.meta", timesheet_filename);
        let meta_path = timesheet_dir.join(&meta_filename);
        let meta_json = serde_json::to_vec(&timesheet.meta)?;
        self.storage.write_bytes(&meta_path, &meta_json)?;

        Ok(())
    }

    /// Get a timesheet for a specific audience and date
    pub fn get_timesheet(
        &self,
        audience_id: &str,
        date: NaiveDate,
    ) -> anyhow::Result<Option<Timesheet>> {
        let timesheet_dir = self.storage.timesheet_dir();
        let timesheet_filename = format!("{}.{}.json", audience_id, date.format("%Y-%m-%d"));
        let timesheet_path = timesheet_dir.join(&timesheet_filename);

        if !self.storage.exists(&timesheet_path) {
            return Ok(None);
        }

        // Read the timesheet
        let timesheet_data = self.storage.read_string(&timesheet_path)?;
        let mut timesheet: Timesheet = serde_json::from_str(&timesheet_data)?;

        // Try to load metadata if it exists
        let meta_filename = format!("{}.meta", timesheet_filename);
        let meta_path = timesheet_dir.join(&meta_filename);

        if self.storage.exists(&meta_path) {
            let meta_data = self.storage.read_string(&meta_path)?;
            let meta: TimesheetMeta = serde_json::from_str(&meta_data)?;
            timesheet.meta = meta;
        }

        Ok(Some(timesheet))
    }

    /// List all timesheets, optionally filtered by date
    pub fn list_timesheets(&self, date: Option<NaiveDate>) -> anyhow::Result<Vec<Timesheet>> {
        let timesheet_dir = self.storage.timesheet_dir();

        let pattern = if let Some(d) = date {
            format!("*.{}.json", d.format("%Y-%m-%d"))
        } else {
            "*.json".to_string()
        };

        let files = self.storage.list_files(&timesheet_dir, &pattern)?;
        let mut timesheets = Vec::new();

        for file in files {
            let filename = file
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

            // Skip meta files
            if filename.ends_with(".meta") {
                continue;
            }

            // Parse audience_id and date from filename: audience.YYYY-MM-DD
            let parts: Vec<&str> = filename.split('.').collect();
            if parts.len() != 2 {
                eprintln!(
                    "[WARN] Skipping file with unexpected format: {} ({} parts)",
                    filename,
                    parts.len()
                );
                continue;
            }

            let audience_id = parts[0];
            let date_str = parts[1];
            let ts_date = match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                Ok(d) => d,
                Err(e) => {
                    eprintln!(
                        "[WARN] Skipping file with invalid date format '{}': {}",
                        date_str, e
                    );
                    continue;
                }
            };

            // Filter by date if specified
            if let Some(filter_date) = date {
                if ts_date != filter_date {
                    continue;
                }
            }

            match self.get_timesheet(audience_id, ts_date) {
                Ok(Some(timesheet)) => timesheets.push(timesheet),
                Ok(None) => {
                    eprintln!(
                        "[WARN] Timesheet file exists but couldn't be loaded: {}.{}",
                        audience_id, date_str
                    );
                }
                Err(e) => {
                    eprintln!(
                        "[ERROR] Failed to load timesheet {}.{}: {}",
                        audience_id, date_str, e
                    );
                    return Err(e);
                }
            }
        }

        timesheets.sort_by_key(|t| t.date);
        Ok(timesheets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TimesheetMeta;
    use crate::test_utils::mock_storage::MockStorage;
    use std::collections::HashMap;

    #[test]
    fn test_write_and_read_timesheet() {
        let storage = Arc::new(MockStorage::new());
        let manager = TimesheetManager::new(storage.clone());

        let date = NaiveDate::from_ymd_opt(2025, 10, 15).unwrap();
        let compiled = chrono::Utc::now().with_timezone(&chrono_tz::Europe::London);
        let meta = TimesheetMeta::new("test_audience".to_string(), None, None);

        let timesheet = Timesheet::new(
            HashMap::new(),
            date,
            compiled,
            chrono_tz::Europe::London,
            vec![],
            HashMap::new(),
            meta,
        );

        // Write timesheet
        manager.write_timesheet(&timesheet).unwrap();

        // Read it back
        let retrieved = manager
            .get_timesheet("test_audience", date)
            .unwrap()
            .expect("Timesheet should exist");

        assert_eq!(retrieved.date, date);
        assert_eq!(retrieved.meta.audience_id, "test_audience");
    }

    #[test]
    fn test_list_timesheets() {
        let storage = Arc::new(MockStorage::new());
        let manager = TimesheetManager::new(storage.clone());

        let date1 = NaiveDate::from_ymd_opt(2025, 10, 15).unwrap();
        let date2 = NaiveDate::from_ymd_opt(2025, 10, 16).unwrap();

        let compiled = chrono::Utc::now().with_timezone(&chrono_tz::Europe::London);

        // Write two timesheets
        for (audience, date) in [("aud1", date1), ("aud2", date2)] {
            let meta = TimesheetMeta::new(audience.to_string(), None, None);
            let timesheet = Timesheet::new(
                HashMap::new(),
                date,
                compiled,
                chrono_tz::Europe::London,
                vec![],
                HashMap::new(),
                meta,
            );
            manager.write_timesheet(&timesheet).unwrap();
        }

        // List all
        let all = manager.list_timesheets(None).unwrap();
        assert_eq!(all.len(), 2);

        // List filtered by date
        let filtered = manager.list_timesheets(Some(date1)).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].date, date1);
    }
}
