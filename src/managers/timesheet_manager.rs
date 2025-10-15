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
            timesheet.date.format("%Y%m%d")
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
        let timesheet_filename = format!("{}.{}.json", audience_id, date.format("%Y%m%d"));
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
            format!("*.{}.json", d.format("%Y%m%d"))
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

            // Parse audience_id and date from filename: audience.YYYYMMDD
            let parts: Vec<&str> = filename.split('.').collect();
            if parts.len() != 2 {
                continue;
            }

            let audience_id = parts[0];
            let date_str = parts[1];
            let ts_date = NaiveDate::parse_from_str(date_str, "%Y%m%d")?;

            // Filter by date if specified
            if let Some(filter_date) = date {
                if ts_date != filter_date {
                    continue;
                }
            }

            if let Some(timesheet) = self.get_timesheet(audience_id, ts_date)? {
                timesheets.push(timesheet);
            }
        }

        timesheets.sort_by_key(|t| t.date);
        Ok(timesheets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{SubmittableTimesheet, TimesheetMeta};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Mutex;

    struct MockStorage {
        files: Mutex<HashMap<PathBuf, Vec<u8>>>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                files: Mutex::new(HashMap::new()),
            }
        }
    }

    impl Storage for MockStorage {
        fn root_dir(&self) -> PathBuf {
            PathBuf::from("/")
        }
        fn log_dir(&self) -> PathBuf {
            PathBuf::from("/logs")
        }
        fn plan_dir(&self) -> PathBuf {
            PathBuf::from("/plans")
        }
        fn identity_dir(&self) -> PathBuf {
            PathBuf::from("/identities")
        }
        fn timesheet_dir(&self) -> PathBuf {
            PathBuf::from("/timesheets")
        }
        fn config_file(&self) -> PathBuf {
            PathBuf::from("/config.toml")
        }
        fn read_string(&self, path: &PathBuf) -> anyhow::Result<String> {
            let bytes = self.read_bytes(path)?;
            Ok(String::from_utf8(bytes)?)
        }
        fn read_bytes(&self, path: &PathBuf) -> anyhow::Result<Vec<u8>> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("File not found"))
        }
        fn write_string(&self, path: &PathBuf, data: &str) -> anyhow::Result<()> {
            self.write_bytes(path, data.as_bytes())
        }
        fn write_bytes(&self, path: &PathBuf, data: &[u8]) -> anyhow::Result<()> {
            self.files.lock().unwrap().insert(path.clone(), data.to_vec());
            Ok(())
        }
        fn exists(&self, path: &PathBuf) -> bool {
            self.files.lock().unwrap().contains_key(path)
        }
        fn create_dir_all(&self, _path: &PathBuf) -> anyhow::Result<()> {
            Ok(())
        }
        fn list_files(&self, dir: &PathBuf, pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
            let files = self.files.lock().unwrap();
            let mut result = Vec::new();

            for path in files.keys() {
                if path.parent() == Some(dir) {
                    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

                    // Simple glob matching for patterns like "*.json" or "*.20251015.json"
                    let matches = if pattern.starts_with("*.") {
                        let suffix = &pattern[1..]; // Remove leading *
                        filename.ends_with(suffix)
                    } else if pattern == "*" {
                        true
                    } else {
                        filename == pattern
                    };

                    if matches {
                        result.push(path.clone());
                    }
                }
            }

            Ok(result)
        }
    }

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
