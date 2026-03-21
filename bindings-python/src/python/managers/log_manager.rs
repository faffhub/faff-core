use chrono::Datelike;
use pyo3::exceptions::{PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDateTime};
use std::sync::Arc;

use crate::python::runtime::runtime;
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
    /// Check if a log exists for the given date
    fn log_exists(&self, date: Bound<'_, PyDate>) -> PyResult<bool> {
        let naive_date = date_py_to_rust(date)?;
        Ok(self.inner.log_exists(naive_date))
    }

    /// Read raw log file contents
    fn read_log_raw(&self, date: Bound<'_, PyDate>) -> PyResult<String> {
        let naive_date = date_py_to_rust(date)?;
        runtime()
            .block_on(self.inner.read_log_raw(naive_date))
            .map_err(|e| PyFileNotFoundError::new_err(e.to_string()))
    }

    /// Write raw log file contents
    fn write_log_raw(&self, date: Bound<'_, PyDate>, contents: &str) -> PyResult<()> {
        let naive_date = date_py_to_rust(date)?;
        runtime()
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
        let dates = runtime()
            .block_on(self.inner.list_logs())
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        dates
            .into_iter()
            .map(|date| date_rust_to_py(py, &date))
            .collect()
    }

    /// List all logs (returns Log objects)
    fn list_logs(&self, _py: Python<'_>) -> PyResult<Vec<faff_core::plugins::models::log::PyLog>> {
        let inner = &self.inner;
        runtime().block_on(async move {
            let dates = inner
                .list_logs()
                .await
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            let mut logs = Vec::with_capacity(dates.len());
            for date in dates {
                let log = inner
                    .get_log(date)
                    .await
                    .map_err(|e| PyValueError::new_err(e.to_string()))?;
                logs.push(faff_core::plugins::models::log::PyLog { inner: log });
            }
            Ok(logs)
        })
    }

    /// List the N most recent logs (returns Log objects, sorted oldest-first)
    fn list_logs_recent(
        &self,
        _py: Python<'_>,
        n: usize,
    ) -> PyResult<Vec<faff_core::plugins::models::log::PyLog>> {
        let inner = &self.inner;
        runtime().block_on(async move {
            let mut dates = inner
                .list_logs()
                .await
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            dates.sort();
            let recent: Vec<_> = dates.into_iter().rev().take(n).collect();
            let mut logs = Vec::with_capacity(recent.len());
            for date in recent.into_iter().rev() {
                let log = inner
                    .get_log(date)
                    .await
                    .map_err(|e| PyValueError::new_err(e.to_string()))?;
                logs.push(faff_core::plugins::models::log::PyLog { inner: log });
            }
            Ok(logs)
        })
    }

    /// Delete a log for a given date
    fn delete_log(&self, date: Bound<'_, PyDate>) -> PyResult<()> {
        let naive_date = date_py_to_rust(date)?;
        runtime()
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
        let log = runtime()
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
        runtime()
            .block_on(self.inner.write_log(&log.inner, &trackers))
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Start a new session
    ///
    /// Args:
    ///     title: Optional session title
    ///     role: Optional role
    ///     impact: Optional impact
    ///     mode: Optional mode
    ///     subject: Optional subject
    ///     trackers: List of tracker IDs
    ///     start_time: Optional start time (defaults to now)
    ///     note: Optional note for the session
    ///
    /// If there's an active session, it will be stopped at the start time.
    /// Validates that start_time is not in the future and doesn't conflict
    /// with existing sessions.
    #[pyo3(signature = (title=None, role=None, impact=None, mode=None, subject=None, trackers=vec![], start_time=None, note=None))]
    #[allow(clippy::too_many_arguments)]
    fn start_session(
        &self,
        _py: Python<'_>,
        title: Option<String>,
        role: Option<String>,
        impact: Option<String>,
        mode: Option<String>,
        subject: Option<String>,
        trackers: Vec<String>,
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

        runtime()
            .block_on(
                self.inner
                    .start_session(title, role, impact, mode, subject, trackers, start, note),
            )
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Stop the currently active session
    ///
    /// Gets current date, time, and trackers from workspace internally.
    fn stop_current_session(&self, _py: Python<'_>) -> PyResult<()> {
        runtime()
            .block_on(self.inner.stop_current_session())
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
        runtime()
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

        let (session_count, log_dates) = runtime()
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
