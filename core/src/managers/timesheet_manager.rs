use crate::models::{Timesheet, TimesheetMeta};
use crate::storage::Storage;
use anyhow::Context;
use chrono::NaiveDate;
use std::sync::{Arc, Weak};

/// Manages timesheet storage and retrieval
#[derive(Clone)]
pub struct TimesheetManager {
    storage: Arc<dyn Storage>,
    workspace: Weak<crate::workspace::Workspace>,
}

impl TimesheetManager {
    pub fn new(storage: Arc<dyn Storage>, workspace: Weak<crate::workspace::Workspace>) -> Self {
        Self { storage, workspace }
    }

    /// Write a timesheet to storage
    pub async fn write_timesheet(&self, timesheet: &Timesheet) -> anyhow::Result<()> {
        // Create audience subdirectory
        let audience_dir = self
            .storage
            .timesheet_dir()
            .join(&timesheet.meta.audience_id);
        self.storage
            .create_dir_all(&audience_dir)
            .await
            .context("Failed to create audience timesheet directory")?;

        // Write the canonical timesheet (date only in filename now)
        let timesheet_filename = format!("{}.json", timesheet.date.format("%Y-%m-%d"));
        let timesheet_path = audience_dir.join(&timesheet_filename);
        let canonical = timesheet
            .submittable_timesheet()
            .canonical_form()
            .context("Failed to create canonical form")?;
        self.storage
            .write_bytes(&timesheet_path, &canonical)
            .await
            .with_context(|| {
                format!(
                    "Failed to write timesheet for {} on {}",
                    timesheet.meta.audience_id, timesheet.date
                )
            })?;

        // Write the metadata separately
        let meta_filename = format!("{timesheet_filename}.meta");
        let meta_path = audience_dir.join(&meta_filename);
        let meta_json = serde_json::to_vec(&timesheet.meta)
            .context("Failed to serialize timesheet metadata")?;
        self.storage
            .write_bytes(&meta_path, &meta_json)
            .await
            .context("Failed to write timesheet metadata")?;

        Ok(())
    }

    /// Get a timesheet for a specific audience and date
    ///
    /// Returns None if the timesheet doesn't exist
    pub async fn get_timesheet(
        &self,
        audience_id: &str,
        date: NaiveDate,
    ) -> anyhow::Result<Option<Timesheet>> {
        let audience_dir = self.storage.timesheet_dir().join(audience_id);
        let timesheet_filename = format!("{}.json", date.format("%Y-%m-%d"));
        let timesheet_path = audience_dir.join(&timesheet_filename);

        if !self.storage.exists(&timesheet_path) {
            return Ok(None);
        }

        // Read the timesheet
        let timesheet_data = self
            .storage
            .read_string(&timesheet_path)
            .await
            .with_context(|| format!("Failed to read timesheet for {audience_id} on {date}"))?;
        let mut timesheet: Timesheet = serde_json::from_str(&timesheet_data)
            .with_context(|| format!("Failed to parse timesheet for {audience_id} on {date}"))?;

        // Try to load metadata if it exists
        let meta_filename = format!("{timesheet_filename}.meta");
        let meta_path = audience_dir.join(&meta_filename);

        if self.storage.exists(&meta_path) {
            let meta_data = self
                .storage
                .read_string(&meta_path)
                .await
                .context("Failed to read timesheet metadata")?;
            let meta: TimesheetMeta =
                serde_json::from_str(&meta_data).context("Failed to parse timesheet metadata")?;
            timesheet.meta = meta;
        }

        Ok(Some(timesheet))
    }

    /// List all timesheets, optionally filtered by date
    pub async fn list_timesheets(&self, date: Option<NaiveDate>) -> anyhow::Result<Vec<Timesheet>> {
        let timesheet_dir = self.storage.timesheet_dir();

        // First, get all audience subdirectories using pattern "*"
        let audience_dirs = self
            .storage
            .list_files(&timesheet_dir, "*")
            .await
            .context("Failed to list audience directories")?;

        let mut timesheets = Vec::new();

        // For each audience directory, list timesheet files
        for audience_path in audience_dirs {
            let audience_id = match audience_path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name,
                None => {
                    eprintln!(
                        "[WARN] Skipping directory with invalid name: {}",
                        audience_path.display()
                    );
                    continue;
                }
            };

            // List timesheet files in this audience directory
            let pattern = if let Some(d) = date {
                format!("{}.json", d.format("%Y-%m-%d"))
            } else {
                "*.json".to_string()
            };

            let files = self
                .storage
                .list_files(&audience_path, &pattern)
                .await
                .context("Failed to list timesheet files")?;

            for file in files {
                let filename = file
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

                // Skip meta files
                if filename.ends_with(".meta") {
                    continue;
                }

                // Parse date from filename: YYYY-MM-DD
                let ts_date = match NaiveDate::parse_from_str(filename, "%Y-%m-%d") {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!(
                            "[WARN] Skipping file with invalid date format '{filename}': {e}"
                        );
                        continue;
                    }
                };

                match self.get_timesheet(audience_id, ts_date).await {
                    Ok(Some(timesheet)) => timesheets.push(timesheet),
                    Ok(None) => {
                        eprintln!(
                            "[WARN] Timesheet file exists but couldn't be loaded: {audience_id}/{filename}"
                        );
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Failed to load timesheet {audience_id}/{filename}: {e}");
                        return Err(e);
                    }
                }
            }
        }

        timesheets.sort_by_key(|t| t.date);
        Ok(timesheets)
    }

    /// Check if a timesheet exists for a specific audience and date
    pub fn timesheet_exists(&self, audience_id: &str, date: NaiveDate) -> bool {
        let audience_dir = self.storage.timesheet_dir().join(audience_id);
        let timesheet_filename = format!("{}.json", date.format("%Y-%m-%d"));
        let timesheet_path = audience_dir.join(timesheet_filename);
        self.storage.exists(&timesheet_path)
    }

    /// Delete a timesheet
    pub async fn delete_timesheet(&self, audience_id: &str, date: NaiveDate) -> anyhow::Result<()> {
        let audience_dir = self.storage.timesheet_dir().join(audience_id);
        let timesheet_filename = format!("{}.json", date.format("%Y-%m-%d"));
        let timesheet_path = audience_dir.join(&timesheet_filename);

        if !self.storage.exists(&timesheet_path) {
            anyhow::bail!(
                "Timesheet for audience '{}' on {} does not exist",
                audience_id,
                date
            );
        }

        // Delete the timesheet file
        self.storage
            .delete(&timesheet_path)
            .await
            .with_context(|| {
                format!("Failed to delete timesheet for audience '{audience_id}' on {date}")
            })?;

        // Delete the metadata file if it exists
        let meta_filename = format!("{timesheet_filename}.meta");
        let meta_path = audience_dir.join(&meta_filename);

        if self.storage.exists(&meta_path) {
            self.storage
                .delete(&meta_path)
                .await
                .context("Failed to delete timesheet metadata")?;
        }

        Ok(())
    }

    /// Compile a timesheet from a log using an audience plugin
    ///
    /// This method:
    /// 1. Calculates the log hash from the raw log file
    /// 2. Calls the plugin's compile_time_sheet method
    /// 3. Updates the timesheet metadata with the log hash
    /// 4. Returns the compiled timesheet (does not write to storage)
    ///
    /// # Arguments
    /// * `log` - The log to compile
    /// * `plugin` - The audience plugin to use for compilation
    ///
    /// # Errors
    /// Returns an error if:
    /// - The log file cannot be read
    /// - The plugin compilation fails
    /// - The workspace is no longer available
    #[cfg(feature = "python")]
    pub async fn compile(
        &self,
        log: &crate::models::Log,
        plugin: &pyo3::Py<pyo3::PyAny>,
    ) -> anyhow::Result<Timesheet> {
        use pyo3::prelude::*;

        let ws = self
            .workspace
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Workspace no longer available"))?;
        let log_manager = ws.logs();

        // Calculate hash of the raw log file
        let log_hash = log_manager
            .read_log_raw(log.date)
            .await
            .map(|raw| crate::models::Log::calculate_hash(&raw))?;

        // Call the plugin's compile_time_sheet method
        let timesheet = Python::attach(|py| -> PyResult<Timesheet> {
            use crate::plugins::models::log::PyLog;
            use crate::plugins::models::timesheet::PyTimesheet;

            let pylog = Py::new(py, PyLog { inner: log.clone() })?;

            // Call compile_time_sheet on the plugin
            let result = plugin.call_method1(py, "compile_time_sheet", (pylog,))?;
            let py_timesheet: PyTimesheet = result.extract(py)?;

            Ok(py_timesheet.inner)
        })
        .map_err(|e: PyErr| anyhow::anyhow!("Failed to compile timesheet: {}", e))?;

        // Update the timesheet metadata with the log hash
        let mut updated_timesheet = timesheet;
        updated_timesheet.meta.log_hash = Some(log_hash);

        Ok(updated_timesheet)
    }

    /// Submit a timesheet via its audience plugin
    ///
    /// This method:
    /// 1. Looks up the audience plugin by the timesheet's audience_id
    /// 2. Calls the plugin's submit_timesheet method
    /// 3. Updates the timesheet metadata with submission status (success/failed)
    /// 4. Writes the timesheet back to storage
    ///
    /// # Arguments
    /// * `timesheet` - The timesheet to submit
    ///
    /// # Errors
    /// Returns an error if:
    /// - The workspace is no longer available
    /// - The audience plugin is not found
    /// - Writing the timesheet back fails
    ///
    /// Note: Plugin submission failures are captured in metadata, not returned as errors
    #[cfg(feature = "python")]
    pub async fn submit(&self, timesheet: &Timesheet) -> anyhow::Result<()> {
        use pyo3::prelude::*;

        let ws = self
            .workspace
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Workspace no longer available"))?;
        let plugin_manager_arc = ws.plugins();
        let mut plugin_manager = plugin_manager_arc.lock().await;

        let audience_id = &timesheet.meta.audience_id;
        let submitted_at = chrono::Utc::now().with_timezone(&chrono_tz::UTC);

        // Get the audience plugin
        let audience = plugin_manager
            .get_audience_by_id(audience_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No audience found for {}", audience_id))?;

        // Try to call the plugin's submit_timesheet method and capture the result
        let submission_result = Python::attach(|py| -> PyResult<()> {
            // Create a PyTimesheet wrapper
            use crate::plugins::models::timesheet::PyTimesheet;
            let pytimesheet = Py::new(
                py,
                PyTimesheet {
                    inner: timesheet.clone(),
                },
            )?;

            // Call submit_timesheet on the audience plugin
            audience.call_method1(py, "submit_timesheet", (pytimesheet,))?;

            Ok(())
        });

        // Update timesheet metadata based on submission result
        match submission_result {
            Ok(()) => {
                // Submission succeeded
                let updated_timesheet = timesheet.with_submission_result(
                    crate::models::SubmissionStatus::Success,
                    None,
                    submitted_at,
                );
                self.write_timesheet(&updated_timesheet).await?;
                Ok(())
            }
            Err(e) => {
                // Submission failed - capture the error in metadata and propagate it
                let updated_timesheet = timesheet.with_submission_result(
                    crate::models::SubmissionStatus::Failed,
                    Some(e.to_string()),
                    submitted_at,
                );
                self.write_timesheet(&updated_timesheet).await?;

                // Propagate the error so the caller knows submission failed
                Err(anyhow::anyhow!("Failed to submit timesheet: {}", e))
            }
        }
    }

    /// Find timesheets that are stale (log has changed since compilation)
    ///
    /// A timesheet is considered stale if the hash of its source log file
    /// no longer matches the log_hash stored in its metadata.
    ///
    /// # Arguments
    /// * `date` - Optional date filter (None = check all timesheets)
    ///
    /// # Returns
    /// Vector of stale timesheets
    ///
    /// # Errors
    /// Returns an error if the workspace is no longer available
    pub async fn find_stale_timesheets(
        &self,
        date: Option<chrono::NaiveDate>,
    ) -> anyhow::Result<Vec<Timesheet>> {
        use crate::models::Log;

        let ws = self
            .workspace
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Workspace no longer available"))?;
        let log_manager = ws.logs();

        let all_timesheets = self.list_timesheets(date).await?;
        let mut stale = Vec::new();

        for timesheet in all_timesheets {
            // Skip if no log_hash in metadata (can't determine staleness)
            if timesheet.meta.log_hash.is_none() {
                continue;
            }

            // Try to read the raw log and calculate its current hash
            match log_manager.read_log_raw(timesheet.date).await {
                Ok(raw_log) => {
                    let current_hash = Log::calculate_hash(&raw_log);

                    // Compare with stored hash
                    if let Some(stored_hash) = &timesheet.meta.log_hash {
                        if stored_hash != &current_hash {
                            stale.push(timesheet);
                        }
                    }
                }
                Err(_) => {
                    // Log no longer exists - skip this timesheet
                    continue;
                }
            }
        }

        Ok(stale)
    }

    /// Find timesheets with failed submissions
    ///
    /// # Arguments
    /// * `date` - Optional date filter (None = check all timesheets)
    ///
    /// # Returns
    /// Vector of timesheets that have failed submissions
    pub async fn find_failed_submissions(
        &self,
        date: Option<chrono::NaiveDate>,
    ) -> anyhow::Result<Vec<Timesheet>> {
        use crate::models::SubmissionStatus;

        let all_timesheets = self.list_timesheets(date).await?;
        let failed: Vec<Timesheet> = all_timesheets
            .into_iter()
            .filter(|ts| matches!(ts.meta.submission_status, Some(SubmissionStatus::Failed)))
            .collect();

        Ok(failed)
    }

    /// Sign a timesheet with the given signing identities
    ///
    /// This method signs a timesheet using the specified signing IDs.
    /// For each signing ID, it retrieves the signing key from the identity manager
    /// and adds a signature to the timesheet.
    ///
    /// # Arguments
    /// * `timesheet` - The timesheet to sign
    /// * `signing_ids` - List of identity IDs to use for signing
    ///
    /// # Returns
    /// The signed timesheet, or an error if any signing operation fails
    ///
    /// # Errors
    /// Returns an error if:
    /// - The workspace is no longer available
    /// - No valid signing keys are found for any of the signing IDs
    /// - The signing operation fails for any key
    pub async fn sign_timesheet(
        &self,
        timesheet: &Timesheet,
        signing_ids: &[String],
    ) -> anyhow::Result<Timesheet> {
        let ws = self
            .workspace
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Workspace no longer available"))?;
        let identity_manager = ws.identities();

        let mut signed_timesheet = timesheet.clone();
        let mut signed_at_least_once = false;

        for signing_id in signing_ids {
            match identity_manager.get_identity(signing_id).await {
                Ok(Some(signing_key)) => {
                    let key_bytes = signing_key.to_bytes();
                    signed_timesheet = signed_timesheet
                        .sign(signing_id, &key_bytes)
                        .with_context(|| format!("Failed to sign with identity '{signing_id}'"))?;
                    signed_at_least_once = true;
                }
                Ok(None) => {
                    // Key doesn't exist - this is a warning but not an error
                    eprintln!("Warning: No identity key found for '{signing_id}'");
                }
                Err(e) => {
                    // Error reading the key - this is more serious
                    return Err(e)
                        .with_context(|| format!("Failed to get identity '{signing_id}'"));
                }
            }
        }

        if !signed_at_least_once && !signing_ids.is_empty() {
            anyhow::bail!(
                "No valid signing keys found for any of the {} signing IDs",
                signing_ids.len()
            );
        }

        Ok(signed_timesheet)
    }

    /// Get audience plugin instances
    ///
    /// This is a convenience method that delegates to the plugin manager.
    /// While audiences are implemented as plugins, they are conceptually
    /// associated with timesheets, so this provides a domain-focused access pattern.
    ///
    /// # Returns
    /// Vector of audience plugin instances
    ///
    /// # Errors
    /// Returns an error if the workspace is no longer available
    #[cfg(feature = "python")]
    pub async fn audiences(&self) -> anyhow::Result<Vec<pyo3::Py<pyo3::PyAny>>> {
        let ws = self
            .workspace
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Workspace no longer available"))?;
        let plugin_manager_arc = ws.plugins();
        let mut pm = plugin_manager_arc.lock().await;
        pm.audiences().await
    }

    /// Get a specific audience plugin by ID
    ///
    /// This is a convenience method that delegates to the plugin manager.
    ///
    /// # Arguments
    /// * `audience_id` - The ID of the audience to get
    ///
    /// # Returns
    /// The audience plugin instance, or None if not found
    ///
    /// # Errors
    /// Returns an error if the workspace is no longer available
    #[cfg(feature = "python")]
    pub async fn get_audience(
        &self,
        audience_id: &str,
    ) -> anyhow::Result<Option<pyo3::Py<pyo3::PyAny>>> {
        let ws = self
            .workspace
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Workspace no longer available"))?;
        let plugin_manager_arc = ws.plugins();
        let mut pm = plugin_manager_arc.lock().await;
        pm.get_audience_by_id(audience_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TimesheetMeta;
    use crate::utils::test_utils::mock_storage::MockStorage;
    use crate::workspace::Workspace;
    use std::collections::HashMap;
    use std::path::PathBuf;

    async fn create_test_workspace() -> Arc<Workspace> {
        let storage = Arc::new(MockStorage::new());
        storage.add_file(
            PathBuf::from("/faff/config.toml"),
            r#"timezone = "Europe/London""#.to_string(),
        );
        Workspace::with_storage(storage).await.unwrap()
    }

    #[tokio::test]
    async fn test_write_and_read_timesheet() {
        let ws = create_test_workspace().await;
        let manager = ws.timesheets();

        let date = NaiveDate::from_ymd_opt(2025, 10, 15).unwrap();
        let compiled = chrono::Utc::now().with_timezone(&chrono_tz::Europe::London);
        let meta = TimesheetMeta::new("test_audience".to_string(), None, "test-hash".to_string());

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
        manager.write_timesheet(&timesheet).await.unwrap();

        // Read it back
        let retrieved = manager
            .get_timesheet("test_audience", date)
            .await
            .unwrap()
            .expect("Timesheet should exist");

        assert_eq!(retrieved.date, date);
        assert_eq!(retrieved.meta.audience_id, "test_audience");
    }

    #[tokio::test]
    async fn test_list_timesheets() {
        let ws = create_test_workspace().await;
        let manager = ws.timesheets();

        let date1 = NaiveDate::from_ymd_opt(2025, 10, 15).unwrap();
        let date2 = NaiveDate::from_ymd_opt(2025, 10, 16).unwrap();

        let compiled = chrono::Utc::now().with_timezone(&chrono_tz::Europe::London);

        // Write two timesheets
        for (audience, date) in [("aud1", date1), ("aud2", date2)] {
            let meta = TimesheetMeta::new(audience.to_string(), None, "test-hash".to_string());
            let timesheet = Timesheet::new(
                HashMap::new(),
                date,
                compiled,
                chrono_tz::Europe::London,
                vec![],
                HashMap::new(),
                meta,
            );
            manager.write_timesheet(&timesheet).await.unwrap();
        }

        // List all
        let all = manager.list_timesheets(None).await.unwrap();
        assert_eq!(all.len(), 2);

        // List filtered by date
        let filtered = manager.list_timesheets(Some(date1)).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].date, date1);
    }

    #[tokio::test]
    async fn test_timesheet_exists() {
        let ws = create_test_workspace().await;
        let manager = ws.timesheets();

        let date = NaiveDate::from_ymd_opt(2025, 10, 15).unwrap();
        let compiled = chrono::Utc::now().with_timezone(&chrono_tz::Europe::London);

        assert!(!manager.timesheet_exists("test_audience", date));

        let meta = TimesheetMeta::new("test_audience".to_string(), None, "test-hash".to_string());
        let timesheet = Timesheet::new(
            HashMap::new(),
            date,
            compiled,
            chrono_tz::Europe::London,
            vec![],
            HashMap::new(),
            meta,
        );
        manager.write_timesheet(&timesheet).await.unwrap();

        assert!(manager.timesheet_exists("test_audience", date));
    }

    #[tokio::test]
    async fn test_delete_timesheet() {
        let ws = create_test_workspace().await;
        let manager = ws.timesheets();

        let date = NaiveDate::from_ymd_opt(2025, 10, 15).unwrap();
        let compiled = chrono::Utc::now().with_timezone(&chrono_tz::Europe::London);

        let meta = TimesheetMeta::new("test_audience".to_string(), None, "test-hash".to_string());
        let timesheet = Timesheet::new(
            HashMap::new(),
            date,
            compiled,
            chrono_tz::Europe::London,
            vec![],
            HashMap::new(),
            meta,
        );
        manager.write_timesheet(&timesheet).await.unwrap();

        assert!(manager.timesheet_exists("test_audience", date));

        manager
            .delete_timesheet("test_audience", date)
            .await
            .unwrap();

        assert!(!manager.timesheet_exists("test_audience", date));
    }

    #[tokio::test]
    async fn test_delete_nonexistent_timesheet() {
        let ws = create_test_workspace().await;
        let manager = ws.timesheets();

        let date = NaiveDate::from_ymd_opt(2025, 10, 15).unwrap();

        let result = manager.delete_timesheet("nonexistent", date).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }
}
