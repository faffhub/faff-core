use chrono::Datelike;
use chrono_tz::Tz;
use pyo3::exceptions::{PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDate;
use std::sync::Arc;

use crate::python::storage::PyStorage;
use faff_core::managers::LogManager as RustLogManager;
use faff_core::type_mapping::{date_py_to_rust, date_rust_to_py};
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
    #[new]
    fn py_new(storage: Py<PyAny>, timezone: &Bound<'_, PyAny>) -> PyResult<Self> {
        // Convert timezone
        let tz_str: String = timezone.call_method0("__str__")?.extract()?;
        let tz: Tz = tz_str
            .parse()
            .map_err(|e| PyValueError::new_err(format!("Invalid timezone: {}", e)))?;

        // Wrap the Python storage object
        let py_storage = PyStorage::new(storage);
        let storage: Arc<dyn faff_core::storage::Storage> = Arc::new(py_storage);

        Ok(Self {
            inner: RustLogManager::new(storage, tz),
            workspace: None, // Standalone construction doesn't have workspace reference
        })
    }

    /// Check if a log exists for the given date
    fn log_exists(&self, date: Bound<'_, PyDate>) -> PyResult<bool> {
        let naive_date = date_py_to_rust(date)?;
        Ok(self.inner.log_exists(naive_date))
    }

    /// Read raw log file contents
    fn read_log_raw(&self, date: Bound<'_, PyDate>) -> PyResult<String> {
        let naive_date = date_py_to_rust(date)?;
        self.inner
            .read_log_raw(naive_date)
            .map_err(|e| PyFileNotFoundError::new_err(e.to_string()))
    }

    /// Write raw log file contents
    fn write_log_raw(&self, date: Bound<'_, PyDate>, contents: &str) -> PyResult<()> {
        let naive_date = date_py_to_rust(date)?;
        self.inner
            .write_log_raw(naive_date, contents)
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
        let dates = self
            .inner
            .list_logs()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        dates
            .into_iter()
            .map(|date| date_rust_to_py(py, &date))
            .collect()
    }

    /// List all logs (returns Log objects)
    fn list_logs(&self, _py: Python<'_>) -> PyResult<Vec<faff_core::py_models::log::PyLog>> {
        let dates = self
            .inner
            .list_logs()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        let mut logs = Vec::new();
        for date in dates {
            if let Some(log) = self
                .inner
                .get_log(date)
                .map_err(|e| PyValueError::new_err(e.to_string()))?
            {
                logs.push(faff_core::py_models::log::PyLog { inner: log });
            }
        }

        Ok(logs)
    }

    /// Delete a log for a given date
    fn delete_log(&self, date: Bound<'_, PyDate>) -> PyResult<()> {
        let naive_date = date_py_to_rust(date)?;
        self.inner
            .delete_log(naive_date)
            .map_err(|e| PyFileNotFoundError::new_err(e.to_string()))
    }

    /// Get the timezone
    fn timezone<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let zoneinfo = py.import("zoneinfo")?;
        let zone_info = zoneinfo.call_method1("ZoneInfo", (self.inner.timezone().name(),))?;
        Ok(zone_info)
    }

    /// Get a log for a given date (returns None if file doesn't exist)
    fn get_log(
        &self,
        date: Bound<'_, PyDate>,
    ) -> PyResult<Option<faff_core::py_models::log::PyLog>> {
        let naive_date = date_py_to_rust(date)?;
        let log = self
            .inner
            .get_log(naive_date)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(log.map(|inner| faff_core::py_models::log::PyLog { inner }))
    }

    /// Get a log for a given date (creates an empty log if file doesn't exist)
    fn get_log_or_create(
        &self,
        date: Bound<'_, PyDate>,
    ) -> PyResult<faff_core::py_models::log::PyLog> {
        let naive_date = date_py_to_rust(date)?;
        let log = self
            .inner
            .get_log_or_create(naive_date)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(faff_core::py_models::log::PyLog { inner: log })
    }

    /// Write a log to storage
    fn write_log(
        &self,
        log: &faff_core::py_models::log::PyLog,
        trackers: std::collections::HashMap<String, String>,
    ) -> PyResult<()> {
        self.inner
            .write_log(&log.inner, &trackers)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Start a new session with the given intent
    ///
    /// Auto-fills current_date, current_time, and trackers from workspace
    #[pyo3(signature = (intent, note=None))]
    fn start_intent_now(
        &self,
        _py: Python<'_>,
        intent: &faff_core::py_models::intent::PyIntent,
        note: Option<String>,
    ) -> PyResult<()> {
        let workspace = self.workspace.as_ref().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "LogManager has no workspace reference. This should not happen.",
            )
        })?;

        // Get current date and time from workspace
        let current_date = workspace.today();
        let current_time = workspace.now();

        // Get trackers from plan manager
        let trackers = workspace
            .plans()
            .get_trackers(current_date)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        self.inner
            .start_intent_now(
                intent.inner.clone(),
                note,
                current_date,
                current_time,
                &trackers,
            )
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Stop the currently active session
    ///
    /// Auto-fills current_date, current_time, and trackers from workspace
    fn stop_current_session(&self, _py: Python<'_>) -> PyResult<()> {
        let workspace = self.workspace.as_ref().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "LogManager has no workspace reference. This should not happen.",
            )
        })?;

        // Get current date and time from workspace
        let current_date = workspace.today();
        let current_time = workspace.now();

        // Get trackers from plan manager
        let trackers = workspace
            .plans()
            .get_trackers(current_date)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        self.inner
            .stop_current_session(current_date, current_time, &trackers)
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
        let results = self
            .inner
            .find_logs_with_intent(intent_id)
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
        updated_intent: &faff_core::py_models::intent::PyIntent,
        trackers: std::collections::HashMap<String, String>,
    ) -> PyResult<usize> {
        self.inner
            .update_intent_in_logs(intent_id, updated_intent.inner.clone(), &trackers)
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
        self.inner
            .replace_field_in_all_logs(field, old_value, new_value, &trackers)
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

        let (session_count, log_dates) = self.inner
            .get_field_usage_stats(field)
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
