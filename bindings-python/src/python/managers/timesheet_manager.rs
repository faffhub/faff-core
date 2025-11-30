use faff_core::managers::TimesheetManager as RustTimesheetManager;
use faff_core::plugins::models::timesheet::PyTimesheet;
use faff_core::utils::type_mapping::date_py_to_rust;
use faff_core::workspace::Workspace as RustWorkspace;
use pyo3::prelude::*;
use pyo3::types::PyDate;
use std::sync::Arc;

/// Python wrapper for TimesheetManager
#[pyclass(name = "TimesheetManager")]
#[derive(Clone)]
pub struct PyTimesheetManager {
    manager: Arc<RustTimesheetManager>,
}

#[pymethods]
impl PyTimesheetManager {
    // NOTE: Standalone construction is no longer supported. TimesheetManager must be
    // created through Workspace using the from_rust() method. The TimesheetManager
    // requires a workspace reference to function properly (for operations that
    // need access to other managers like LogManager, IdentityManager, PluginManager).
    //
    // #[new]
    // pub fn new(storage: Py<PyAny>) -> PyResult<Self> {
    //     let py_storage = PyStorage::new(storage);
    //     let manager = RustTimesheetManager::new(Arc::new(py_storage), Weak::new());
    //     Ok(Self {
    //         manager: Arc::new(manager),
    //         workspace: None,
    //     })
    // }

    /// Write a timesheet to storage
    pub fn write_timesheet(&self, timesheet: &PyTimesheet) -> PyResult<()> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.write_timesheet(&timesheet.inner))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Get a timesheet for a specific audience and date
    pub fn get_timesheet(
        &self,
        audience_id: &str,
        date: Bound<'_, PyDate>,
    ) -> PyResult<Option<PyTimesheet>> {
        let naive_date = date_py_to_rust(date)?;
        let timesheet = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_timesheet(audience_id, naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(timesheet.map(|t| PyTimesheet { inner: t }))
    }

    /// List all timesheets, optionally filtered by date
    #[pyo3(signature = (date=None))]
    pub fn list_timesheets(&self, date: Option<Bound<'_, PyDate>>) -> PyResult<Vec<PyTimesheet>> {
        let naive_date = date.map(date_py_to_rust).transpose()?;

        let timesheets = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.list_timesheets(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(timesheets
            .into_iter()
            .map(|t| PyTimesheet { inner: t })
            .collect())
    }

    /// Delete a timesheet for a specific audience and date
    pub fn delete_timesheet(&self, audience_id: &str, date: Bound<'_, PyDate>) -> PyResult<()> {
        let naive_date = date_py_to_rust(date)?;
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.delete_timesheet(audience_id, naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Get all audience plugin instances
    ///
    /// Gets workspace context internally.
    pub fn audiences(&self, _py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.audiences())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Get a specific audience plugin by ID
    ///
    /// Gets workspace context internally.
    pub fn get_audience(&self, _py: Python<'_>, audience_id: &str) -> PyResult<Option<Py<PyAny>>> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.get_audience(audience_id))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    /// Compile a timesheet from a log using an audience plugin
    ///
    /// This automatically calculates and stores the log hash in the timesheet metadata.
    /// Gets workspace context internally.
    pub fn compile(
        &self,
        _py: Python<'_>,
        log: &faff_core::plugins::models::log::PyLog,
        plugin: Bound<'_, PyAny>,
    ) -> PyResult<PyTimesheet> {
        // Convert Bound to Py for the Rust API
        let plugin_py: Py<PyAny> = plugin.unbind();

        let timesheet = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.compile(&log.inner, &plugin_py))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(PyTimesheet { inner: timesheet })
    }

    /// Find timesheets that are stale (log has changed since compilation)
    ///
    /// Gets workspace context internally.
    #[pyo3(signature = (date=None))]
    pub fn find_stale_timesheets(
        &self,
        _py: Python<'_>,
        date: Option<Bound<'_, PyDate>>,
    ) -> PyResult<Vec<PyTimesheet>> {
        let naive_date = date.map(date_py_to_rust).transpose()?;

        let stale = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.find_stale_timesheets(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(stale
            .into_iter()
            .map(|t| PyTimesheet { inner: t })
            .collect())
    }

    /// Find timesheets with failed submissions
    #[pyo3(signature = (date=None))]
    pub fn find_failed_submissions(
        &self,
        _py: Python<'_>,
        date: Option<Bound<'_, PyDate>>,
    ) -> PyResult<Vec<PyTimesheet>> {
        let naive_date = date.map(date_py_to_rust).transpose()?;

        let failed = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.find_failed_submissions(naive_date))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(failed
            .into_iter()
            .map(|t| PyTimesheet { inner: t })
            .collect())
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
    /// The signed timesheet
    ///
    /// # Errors
    /// Returns an error if no valid signing keys are found or if signing fails
    ///
    /// Gets workspace context internally.
    pub fn sign_timesheet(
        &self,
        _py: Python<'_>,
        timesheet: &PyTimesheet,
        signing_ids: Vec<String>,
    ) -> PyResult<PyTimesheet> {
        let signed = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.sign_timesheet(&timesheet.inner, &signing_ids))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(PyTimesheet { inner: signed })
    }

    /// Submit a timesheet via its audience plugin
    ///
    /// Gets workspace context internally.
    pub fn submit(&self, _py: Python<'_>, timesheet: &PyTimesheet) -> PyResult<()> {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.manager.submit(&timesheet.inner))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }
}

impl PyTimesheetManager {
    pub fn from_rust(manager: Arc<RustTimesheetManager>, _workspace: Arc<RustWorkspace>) -> Self {
        Self { manager }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTimesheetManager>()?;
    Ok(())
}
