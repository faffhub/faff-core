use crate::python::storage::PyStorage;
use faff_core::managers::TimesheetManager as RustTimesheetManager;
use faff_core::py_models::timesheet::PyTimesheet;
use faff_core::type_mapping::date_py_to_rust;
use faff_core::workspace::Workspace as RustWorkspace;
use pyo3::prelude::*;
use pyo3::types::PyDate;
use std::sync::Arc;

/// Python wrapper for TimesheetManager
#[pyclass(name = "TimesheetManager")]
#[derive(Clone)]
pub struct PyTimesheetManager {
    manager: Arc<RustTimesheetManager>,
    workspace: Option<Arc<RustWorkspace>>,
}

#[pymethods]
impl PyTimesheetManager {
    #[new]
    pub fn new(storage: Py<PyAny>) -> PyResult<Self> {
        let py_storage = PyStorage::new(storage);
        let manager = RustTimesheetManager::new(Arc::new(py_storage));
        Ok(Self {
            manager: Arc::new(manager),
            workspace: None,
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
    #[pyo3(signature = (date=None))]
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

    /// Alias for list_timesheets (for backwards compatibility)
    #[pyo3(signature = (date=None))]
    pub fn list(&self, date: Option<Bound<'_, PyDate>>) -> PyResult<Vec<PyTimesheet>> {
        self.list_timesheets(date)
    }

    /// Get all audience plugin instances
    ///
    /// This delegates to the Rust TimesheetManager's audiences() method.
    pub fn audiences(&self, _py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let workspace = self.workspace.as_ref().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "TimesheetManager has no workspace reference. This should not happen.",
            )
        })?;

        self.manager
            .audiences(workspace.plugins())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Get a specific audience plugin by ID
    pub fn get_audience(&self, _py: Python<'_>, audience_id: &str) -> PyResult<Option<Py<PyAny>>> {
        let workspace = self.workspace.as_ref().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "TimesheetManager has no workspace reference. This should not happen.",
            )
        })?;

        let plugin_manager_arc = workspace.plugins();
        let mut plugin_manager = plugin_manager_arc.lock().unwrap();

        plugin_manager
            .get_audience_by_id(audience_id)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Submit a timesheet via its audience plugin
    pub fn submit(&self, _py: Python<'_>, timesheet: &PyTimesheet) -> PyResult<()> {
        let workspace = self.workspace.as_ref().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "TimesheetManager has no workspace reference. This should not happen.",
            )
        })?;

        let plugin_manager_arc = workspace.plugins();
        let mut plugin_manager = plugin_manager_arc.lock().unwrap();

        self.manager
            .submit(&timesheet.inner, &mut *plugin_manager)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
}

impl PyTimesheetManager {
    pub fn from_rust(manager: Arc<RustTimesheetManager>, workspace: Arc<RustWorkspace>) -> Self {
        Self {
            manager,
            workspace: Some(workspace),
        }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTimesheetManager>()?;
    Ok(())
}
