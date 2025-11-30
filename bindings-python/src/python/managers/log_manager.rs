use chrono::Datelike;
use pyo3::exceptions::{PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDateTime};
use std::sync::Arc;

use faff_core::managers::LogManager as RustLogManager;
use faff_core::utils::type_mapping::{date_py_to_rust, date_rust_to_py, datetime_py_to_rust};
use faff_core::workspace::Workspace as RustWorkspace;

#[pyclass(name = "LogManager")]
#[derive(Clone)]
pub struct PyLogManager {
    inner: RustLogManager,
    workspace: Option<Arc<RustWorkspace>>,
}

impl PyLogManager {
    pub fn from_rust(manager: RustLogManager, workspace: Arc<RustWorkspace>) -> Self {
        Self {
            inner: manager,
            workspace: Some(workspace),
        }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLogManager>()?;
    Ok(())
}

#[pymethods]
impl PyLogManager {
    // NOTE: Standalone construction is no longer supported. LogManager must be
    // created through Workspace using the from_rust() method. The LogManager
    // requires a workspace reference to function properly (for operations like
    // start_intent and stop_current_session that need workspace context).
    //
    // #[new]
    // fn py_new(storage: Py<PyAny>, timezone: &Bound<'_, PyAny>) -> PyResult<Self> {
    //     // Convert timezone
    //     let tz_str: String = timezone.call_method0("__str__")?.extract()?;
    //     let tz: Tz = tz_str
    //         .parse()
    //         .map_err(|e| PyValueError::new_err(format!("Invalid timezone: {e}")))?;
    //
    //     // Wrap the Python storage object
    //     let py_storage = PyStorage::new(storage);
    //     let storage: Arc<dyn faff_core::storage::Storage> = Arc::new(py_storage);
    //
    //     // Create a PlanManager for the LogManager to use
    //     let plan_manager = Arc::new(PlanManager::new(storage.clone()));
    //
    //     Ok(Self {
    //         inner: RustLogManager::new(storage, tz, plan_manager),
    //         workspace: None, // Standalone construction doesn't have workspace reference
    //     })
    // }

    /// Check if a log exists for the given date
    fn log_exists(&self, date: Bound<'_, PyDate>) -> PyResult<bool> {
        let naive_date = date_py_to_rust(date)?;
        Ok(self.inner.log_exists(naive_date))
    }

    /// Read raw log file contents
    fn read_log_raw(&self, date: Bound<'_, PyDate>) -> PyResult<String> {
        let naive_date = date_py_to_rust(date)?;
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.inner.read_log_raw(naive_date))
            .map_err(|e| PyFileNotFoundError::new_err(e.to_string()))
    }

    /// Write raw log file contents
    fn write_log_raw(&self, date: Bound<'_, PyDate>, contents: &str) -> PyResult<()> {
        let naive_date = date_py_to_rust(date)?;
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.inner.write_log_raw(naive_date, contents))
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Get the file path for a log
    fn log_file_path(&self, date: Bound<'_, PyDate>) -> PyResult<String> {
        let naive_date = date_py_to_rust(date)?;
        Ok(self
            .inner
            .log_file_path(naive_date)
            .to_string_lossy()
            .into_owned())
    }

    /// List all log dates
    fn list_log_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyDate>>> {
        let dates = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.inner.list_logs())
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        dates
            .into_iter()
            .map(|date| date_rust_to_py(py, &date))
            .collect()
    }

    /// List all logs (returns Log objects)
    fn list_logs(&self, _py: Python<'_>) -> PyResult<Vec<faff_core::plugins::models::log::PyLog>> {
        let dates = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.inner.list_logs())
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut logs = Vec::new();
        for date in dates {
            let log = rt
                .block_on(self.inner.get_log(date))
                .map_err(|e| PyValueError::new_err(e.to_string()))?;

            logs.push(faff_core::plugins::models::log::PyLog { inner: log });
        }

        Ok(logs)
    }

    /// Delete a log for a given date
    fn delete_log(&self, date: Bound<'_, PyDate>) -> PyResult<()> {
        let naive_date = date_py_to_rust(date)?;
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.inner.delete_log(naive_date))
            .map_err(|e| PyFileNotFoundError::new_err(e.to_string()))
    }

    /// Get the timezone
    fn timezone<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let zoneinfo = py.import("zoneinfo")?;
        let zone_info = zoneinfo.call_method1("ZoneInfo", (self.inner.timezone().name(),))?;
        Ok(zone_info)
    }

    /// Get a log for a given date (returns empty log if file doesn't exist)
    fn get_log(&self, date: Bound<'_, PyDate>) -> PyResult<faff_core::plugins::models::log::PyLog> {
        let naive_date = date_py_to_rust(date)?;
        let log = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.inner.get_log(naive_date))
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        Ok(faff_core::plugins::models::log::PyLog { inner: log })
    }

    /// Write a log to storage
    fn write_log(
        &self,
        log: &faff_core::plugins::models::log::PyLog,
        trackers: std::collections::HashMap<String, String>,
    ) -> PyResult<()> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.inner.write_log(&log.inner, &trackers))
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Start a new session with the given intent
    ///
    /// Args:
    ///     intent: The intent for the session
    ///     start_time: Optional start time (defaults to now)
    ///     note: Optional note for the session
    ///
    /// If there's an active session, it will be stopped at the start time.
    /// Validates that start_time is not in the future and doesn't conflict
    /// with existing sessions.
    ///
    /// FIXME: This method gathers context (now, trackers) from workspace before
    /// calling the Rust core. This orchestration logic should live in Rust, not
    /// in the bindings. See BUSINESS_LOGIC_AUDIT.md for the proposed functional
    /// core pattern that would eliminate this.
    #[pyo3(signature = (intent, start_time=None, note=None))]
    fn start_intent(
        &self,
        _py: Python<'_>,
        intent: &faff_core::plugins::models::intent::PyIntent,
        start_time: Option<Bound<'_, PyDateTime>>,
        note: Option<String>,
    ) -> PyResult<()> {
        let workspace = self.workspace.as_ref().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "LogManager has no workspace reference. This should not happen.",
            )
        })?;

        let start = match start_time {
            Some(dt) => datetime_py_to_rust(dt)?,
            None => workspace.now(),
        };

        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(self.inner.start_intent(intent.inner.clone(), start, note))
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Stop the currently active session
    ///
    /// Gets current date, time, and trackers from workspace internally.
    fn stop_current_session(&self, _py: Python<'_>) -> PyResult<()> {
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(self.inner.stop_current_session())
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Find all logs that contain sessions using the given intent
    ///
    /// Returns: list of tuples (date, session_count)
    fn find_logs_with_intent<'py>(
        &self,
        py: Python<'py>,
        intent_id: &str,
    ) -> PyResult<Vec<(Bound<'py, PyDate>, usize)>> {
        let results = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.inner.find_logs_with_intent(intent_id))
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        results
            .into_iter()
            .map(|(date, count)| {
                let py_date = date_rust_to_py(py, &date)?;
                Ok((py_date, count))
            })
            .collect()
    }

    /// Update an intent across all log files
    ///
    /// Returns: total number of sessions updated
    fn update_intent_in_logs(
        &self,
        intent_id: &str,
        updated_intent: &faff_core::plugins::models::intent::PyIntent,
        trackers: std::collections::HashMap<String, String>,
    ) -> PyResult<usize> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.inner.update_intent_in_logs(
                intent_id,
                updated_intent.inner.clone(),
                &trackers,
            ))
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Replace a field value across all log sessions
    ///
    /// Returns tuple of (logs_updated, sessions_updated)
    fn replace_field_in_all_logs(
        &self,
        field: &str,
        old_value: &str,
        new_value: &str,
        trackers: std::collections::HashMap<String, String>,
    ) -> PyResult<(usize, usize)> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                self.inner
                    .replace_field_in_all_logs(field, old_value, new_value, &trackers),
            )
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Get usage statistics for a field across all logs
    ///
    /// Returns tuple of:
    /// - dict of field value -> session count
    /// - dict of field value -> list of log dates
    fn get_field_usage_stats(
        &self,
        field: &str,
        py: Python<'_>,
    ) -> PyResult<(Py<pyo3::types::PyDict>, Py<pyo3::types::PyDict>)> {
        use pyo3::types::{PyDate, PyDict, PyList};

        let (session_count, log_dates) = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.inner.get_field_usage_stats(field))
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        // Convert session counts to dict
        let session_dict = PyDict::new(py);
        for (key, value) in session_count {
            session_dict.set_item(key, value)?;
        }

        // Convert log dates to dict of lists of dates
        let dates_dict = PyDict::new(py);
        for (key, dates) in log_dates {
            let date_list = PyList::empty(py);
            for date in dates {
                let py_date = PyDate::new(py, date.year(), date.month() as u8, date.day() as u8)?;
                date_list.append(py_date)?;
            }
            dates_dict.set_item(key, date_list)?;
        }

        Ok((session_dict.into(), dates_dict.into()))
    }
}
