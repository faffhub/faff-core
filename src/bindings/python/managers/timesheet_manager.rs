use crate::bindings::python::models::timesheet::PyTimesheet;
use crate::bindings::python::storage::PyStorage;
use crate::bindings::python::type_mapping::date_py_to_rust;
use crate::managers::TimesheetManager as RustTimesheetManager;
use pyo3::prelude::*;
use pyo3::types::PyDate;
use std::sync::Arc;

/// Python wrapper for TimesheetManager
#[pyclass(name = "TimesheetManager")]
pub struct PyTimesheetManager {
    manager: Arc<RustTimesheetManager>,
}

#[pymethods]
impl PyTimesheetManager {
    #[new]
    pub fn new(storage: Py<PyAny>) -> PyResult<Self> {
        let py_storage = PyStorage::new(storage);
        let manager = RustTimesheetManager::new(Arc::new(py_storage));
        Ok(Self {
            manager: Arc::new(manager),
        })
    }

    /// Write a timesheet to storage
    pub fn write_timesheet(&self, timesheet: &PyTimesheet) -> PyResult<()> {
        self.manager
            .write_timesheet(&timesheet.inner)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Get a timesheet for a specific audience and date
    pub fn get_timesheet(
        &self,
        audience_id: &str,
        date: Bound<'_, PyDate>,
    ) -> PyResult<Option<PyTimesheet>> {
        let naive_date = date_py_to_rust(date)?;
        let timesheet = self
            .manager
            .get_timesheet(audience_id, naive_date)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(timesheet.map(|t| PyTimesheet { inner: t }))
    }

    /// List all timesheets, optionally filtered by date
    pub fn list_timesheets(&self, date: Option<Bound<'_, PyDate>>) -> PyResult<Vec<PyTimesheet>> {
        let naive_date = date.map(date_py_to_rust).transpose()?;

        let timesheets = self
            .manager
            .list_timesheets(naive_date)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(timesheets
            .into_iter()
            .map(|t| PyTimesheet { inner: t })
            .collect())
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTimesheetManager>()?;
    Ok(())
}
