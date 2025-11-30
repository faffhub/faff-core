use crate::python::managers::{
    identity_manager::PyIdentityManager, log_manager::PyLogManager, plan_manager::PyPlanManager,
    plugin_manager::PyPluginManager, timesheet_manager::PyTimesheetManager,
};
use crate::python::storage::{PyStorage, PyStorageWrapper};
use faff_core::utils::type_mapping::{date_rust_to_py, datetime_rust_to_py};
use faff_core::workspace::Workspace as RustWorkspace;
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDateTime};
use std::sync::{Arc, Mutex};

#[pyclass(name = "Workspace")]
pub struct PyWorkspace {
    inner: Arc<RustWorkspace>,
    // Cache the Python manager wrappers
    plans: PyPlanManager,
    logs: PyLogManager,
    timesheets: PyTimesheetManager,
    identities: PyIdentityManager,
    plugins: PyPluginManager,
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyWorkspace>()?;
    Ok(())
}

#[pymethods]
impl PyWorkspace {
    #[new]
    #[pyo3(signature = (storage=None))]
    fn py_new(storage: Option<Py<PyAny>>) -> PyResult<Self> {
        // Workspace::new() and with_storage() now return Arc<Workspace> directly
        let inner_arc = match storage {
            Some(storage_obj) => {
                let py_storage = PyStorage::new(storage_obj);
                tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(RustWorkspace::with_storage(Arc::new(py_storage)))
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
            }
            None => tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(RustWorkspace::new())
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?,
        };

        // Create Python manager wrappers from the Rust managers
        // Managers are already Arc<Manager> in the workspace
        let plans = PyPlanManager::from_rust_arc(inner_arc.plans().clone(), inner_arc.clone());
        let logs = PyLogManager::from_rust((**inner_arc.logs()).clone(), inner_arc.clone());
        let timesheets =
            PyTimesheetManager::from_rust(inner_arc.timesheets().clone(), inner_arc.clone());
        let identities = PyIdentityManager::from_rust(inner_arc.identities().clone());
        // Clone the plugin manager from inside the mutex
        let plugin_manager_clone = inner_arc.plugins().blocking_lock().clone();
        let plugins = PyPluginManager::from_rust(Arc::new(Mutex::new(plugin_manager_clone)));

        Ok(Self {
            inner: inner_arc,
            plans,
            logs,
            timesheets,
            identities,
            plugins,
        })
    }

    /// Get the current time in the configured timezone
    fn now<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDateTime>> {
        let now = self.inner.now();
        datetime_rust_to_py(py, &now)
    }

    /// Get today's date
    fn today<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDate>> {
        let today = self.inner.today();
        date_rust_to_py(py, &today)
    }

    /// Get the configured timezone
    fn timezone<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let zoneinfo = py.import("zoneinfo")?;
        let zone_info = zoneinfo.call_method1("ZoneInfo", (self.inner.timezone().name(),))?;
        Ok(zone_info)
    }

    /// Get the config
    fn config(&self) -> faff_core::plugins::models::config::PyConfig {
        faff_core::plugins::models::config::PyConfig {
            inner: self.inner.config().clone(),
        }
    }

    /// Get the storage object
    fn storage(&self) -> PyStorageWrapper {
        PyStorageWrapper::new(self.inner.storage().clone())
    }

    /// Parse a natural language date string relative to today
    ///
    /// Supports:
    /// - ISO dates: "2025-08-03"
    /// - Relative dates: "yesterday", "last monday", "today"
    /// - Month names: "April 1"
    /// - Time intervals: "2 days ago"
    ///
    /// Args:
    ///     date_str: The date string to parse (None returns today)
    ///
    /// Returns:
    ///     A Python date object
    fn parse_natural_date<'py>(
        &self,
        py: Python<'py>,
        date_str: Option<&str>,
    ) -> PyResult<Bound<'py, PyDate>> {
        let today = self.inner.today();
        let timezone = self.inner.timezone();

        let parsed = faff_core::utils::date_parsing::parse_natural_date(date_str, today, timezone)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        faff_core::utils::type_mapping::date_rust_to_py(py, &parsed)
    }

    /// Parse a natural language datetime string, restricted to today's date
    ///
    /// This is useful for parsing times on today (e.g., "09:30" for this morning).
    /// It ensures the parsed datetime falls on today's date, preventing accidental
    /// backdating or future dating.
    ///
    /// Supports:
    /// - Time formats: "09:30", "14:30", "3pm", "midnight"
    /// - Relative times: "2 hours ago", "30 minutes ago"
    /// - Special keywords: "now"
    ///
    /// Args:
    ///     datetime_str: The datetime string to parse (None returns now)
    ///
    /// Returns:
    ///     A Python datetime object
    ///
    /// Raises:
    ///     ValueError: If the parsed datetime is not on today's date
    fn parse_natural_datetime<'py>(
        &self,
        py: Python<'py>,
        datetime_str: Option<&str>,
    ) -> PyResult<Bound<'py, PyDateTime>> {
        let today = self.inner.today();
        let now = self.inner.now();

        let parsed =
            faff_core::utils::date_parsing::parse_natural_datetime(datetime_str, today, now)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;

        faff_core::utils::type_mapping::datetime_rust_to_py(py, &parsed)
    }

    /// Get the PlanManager
    #[getter]
    fn plans(&self) -> PyPlanManager {
        self.plans.clone()
    }

    /// Get the LogManager
    #[getter]
    fn logs(&self) -> PyLogManager {
        self.logs.clone()
    }

    /// Get the TimesheetManager
    #[getter]
    fn timesheets(&self) -> PyTimesheetManager {
        self.timesheets.clone()
    }

    /// Get the IdentityManager
    #[getter]
    fn identities(&self) -> PyIdentityManager {
        self.identities.clone()
    }

    /// Get the PluginManager
    #[getter]
    fn plugins(&self) -> PyPluginManager {
        self.plugins.clone()
    }

    fn __repr__(&self) -> String {
        format!("Workspace(timezone={})", self.inner.timezone().name())
    }
}
