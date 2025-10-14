use chrono_tz::Tz;
use pyo3::exceptions::{PyFileNotFoundError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDateTime};
use std::sync::Arc;

use crate::bindings::python::storage::PyStorage;
use crate::bindings::python::type_mapping::{
    date_py_to_rust, date_rust_to_py, datetime_py_to_rust,
};
use crate::managers::LogManager as RustLogManager;

#[pyclass(name = "LogManager")]
pub struct PyLogManager {
    inner: RustLogManager,
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
        let storage: Arc<dyn crate::storage::Storage> = Arc::new(py_storage);

        Ok(Self {
            inner: RustLogManager::new(storage, tz),
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

    /// List all log dates
    fn list_log_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyDate>>> {
        let dates = self
            .inner
            .list_log_dates()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;

        dates
            .into_iter()
            .map(|date| date_rust_to_py(py, &date))
            .collect()
    }

    /// Remove a log for a given date
    fn rm(&self, date: Bound<'_, PyDate>) -> PyResult<()> {
        let naive_date = date_py_to_rust(date)?;
        self.inner
            .rm(naive_date)
            .map_err(|e| PyFileNotFoundError::new_err(e.to_string()))
    }

    /// Get the timezone
    fn timezone<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let zoneinfo = py.import("zoneinfo")?;
        let zone_info = zoneinfo.call_method1("ZoneInfo", (self.inner.timezone().name(),))?;
        Ok(zone_info)
    }

    /// Get a log for a given date (returns empty log if file doesn't exist)
    fn get_log(&self, date: Bound<'_, PyDate>) -> PyResult<crate::bindings::python::models::log::PyLog> {
        let naive_date = date_py_to_rust(date)?;
        let log = self
            .inner
            .get_log(naive_date)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(crate::bindings::python::models::log::PyLog { inner: log })
    }

    /// Write a log to storage
    fn write_log(
        &self,
        log: &crate::bindings::python::models::log::PyLog,
        trackers: std::collections::HashMap<String, String>,
    ) -> PyResult<()> {
        self.inner
            .write_log(&log.inner, &trackers)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Start a new session with the given intent
    fn start_intent_now(
        &self,
        intent: &crate::bindings::python::models::intent::PyIntent,
        note: Option<String>,
        current_date: Bound<'_, PyDate>,
        current_time: Bound<'_, PyDateTime>,
        trackers: std::collections::HashMap<String, String>,
    ) -> PyResult<String> {
        let naive_date = date_py_to_rust(current_date)?;
        let datetime = datetime_py_to_rust(current_time)?;

        self.inner
            .start_intent_now(intent.inner.clone(), note, naive_date, datetime, &trackers)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Stop the currently active session
    fn stop_current_session(
        &self,
        current_date: Bound<'_, PyDate>,
        current_time: Bound<'_, PyDateTime>,
        trackers: std::collections::HashMap<String, String>,
    ) -> PyResult<String> {
        let naive_date = date_py_to_rust(current_date)?;
        let datetime = datetime_py_to_rust(current_time)?;

        self.inner
            .stop_current_session(naive_date, datetime, &trackers)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }
}
